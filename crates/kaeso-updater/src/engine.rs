use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use kaeso_update::Version;
use kaeso_update::manifest::{self, Image, Manifest, Service};
use kaeso_update::policy;
use kaeso_update::protocol::{self, Job, JobState, Reason, Status, Step};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::clone;
use crate::compose::{self, ComposeFile};
use crate::docker::{Docker, Listed, PROJECT_LABEL};
use crate::files::{MaintenanceFlag, Phase, Saved, StateFile, Switched};
use crate::source::Source;
use crate::verify::{Subject, Verify, hex};

const ORDER: [Service; 2] = [Service::Console, Service::Gateway];
const PULLED: [Service; 3] = [Service::Gateway, Service::Console, Service::Updater];
const PREVIOUS_SUFFIX: &str = "-previous";
const STEADY_POLLS: u32 = 3;
const MAINTENANCE_TRIES: u32 = 10;

#[derive(Clone, Debug)]
pub struct Settings {
    pub compose: ComposeFile,
    pub maintenance: MaintenanceFlag,
    pub state: StateFile,
    pub hostname: String,
    pub poll: Duration,
    pub gateway_timeout: Duration,
    pub console_timeout: Duration,
    pub rollback_timeout: Duration,
    pub docker_wait: Duration,
    pub stop_seconds: u32,
}

#[derive(Debug)]
struct Failure {
    reason: Reason,
    detail: String,
}

fn fail(reason: Reason, detail: impl std::fmt::Display) -> Failure {
    Failure {
        reason,
        detail: detail.to_string(),
    }
}

struct Target {
    service: Service,
    image: Image,
    id: String,
    name: String,
    container: Value,
    old_image: Value,
    new_image: Value,
}

struct Prepared {
    from: Version,
    targets: Vec<Target>,
}

#[derive(Debug, thiserror::Error)]
pub enum BeginError {
    #[error("an update is already running")]
    Busy,
    #[error("the updater cannot save the update's state: {0}")]
    State(std::io::Error),
}

pub struct Engine {
    settings: Settings,
    docker: Docker,
    source: Source,
    verify: Arc<dyn Verify>,
    current: Mutex<Option<Saved>>,
}

impl Engine {
    pub fn new(
        settings: Settings,
        docker: Docker,
        source: Source,
        verify: Arc<dyn Verify>,
    ) -> Arc<Engine> {
        let saved = settings.state.load();
        Arc::new(Engine {
            settings,
            docker,
            source,
            verify,
            current: Mutex::new(saved),
        })
    }

    fn snapshot(&self) -> Option<Saved> {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn update(&self, f: impl FnOnce(&mut Saved)) {
        if let Err(e) = self.record(f) {
            tracing::warn!(detail = %e.detail, "cannot save the update's state");
        }
    }

    fn record(&self, f: impl FnOnce(&mut Saved)) -> Result<(), Failure> {
        let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(saved) = guard.as_mut() else {
            return Ok(());
        };
        f(saved);
        self.settings.state.save(saved).map_err(|e| {
            fail(
                Reason::StateUnwritable,
                format!("cannot save the update's progress: {e}"),
            )
        })
    }

    pub fn last(&self) -> Option<Job> {
        self.snapshot().map(|s| s.job)
    }

    pub fn job(&self, id: &str) -> Option<Job> {
        self.last().filter(|j| j.id == id)
    }

    pub async fn status(&self) -> Status {
        Status {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol: protocol::VERSION,
            docker: self.docker.version().await.is_ok(),
            job: self.last(),
        }
    }

    pub fn begin(self: &Arc<Self>, id: String, to: Version) -> Result<Job, BeginError> {
        let job = {
            let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
            if guard
                .as_ref()
                .is_some_and(|s| s.job.state == JobState::Running)
            {
                return Err(BeginError::Busy);
            }
            let saved = Saved::new(Job {
                id,
                from: String::new(),
                to: to.to_string(),
                state: JobState::Running,
                step: Step::Verifying,
                reason: None,
                detail: None,
                started_at: OffsetDateTime::now_utc(),
                finished_at: None,
            });
            self.settings
                .state
                .save(&saved)
                .map_err(BeginError::State)?;
            let job = saved.job.clone();
            *guard = Some(saved);
            job
        };
        let engine = Arc::clone(self);
        tokio::spawn(async move { engine.run(to).await });
        Ok(job)
    }

    fn step(&self, step: Step) {
        tracing::info!(step = step.as_str(), "update step");
        self.update(|s| s.job.step = step);
        if matches!(step, Step::Switching | Step::Migrating | Step::Online)
            && let Err(e) = self.settings.maintenance.set(step)
        {
            tracing::warn!(error = %e, "cannot write the maintenance flag");
        }
    }

    async fn clear_maintenance(&self) -> Result<(), std::io::Error> {
        let mut tries = 0;
        loop {
            match self.settings.maintenance.clear() {
                Ok(()) => return Ok(()),
                Err(e) if tries >= MAINTENANCE_TRIES => return Err(e),
                Err(e) => {
                    tracing::warn!(error = %e, "cannot remove the maintenance flag yet");
                    tries += 1;
                    tokio::time::sleep(self.settings.poll).await;
                }
            }
        }
    }

    async fn end(&self, state: JobState, failure: Option<Failure>) {
        let (state, failure) = match self.clear_maintenance().await {
            Ok(()) => (state, failure),
            Err(e) => {
                let left = format!(
                    "the maintenance flag could not be removed ({e}), so Caddy still answers that Kaeso is being updated; delete `active` from the kaeso_maintenance volume"
                );
                let failure = match failure {
                    Some(f) => Failure {
                        reason: f.reason,
                        detail: format!("{}; {left}", f.detail),
                    },
                    None => fail(Reason::StateUnwritable, left),
                };
                (JobState::NeedsAttention, Some(failure))
            }
        };
        self.update(|s| {
            s.job.state = state;
            s.job.finished_at = Some(OffsetDateTime::now_utc());
            if let Some(f) = &failure {
                s.job.reason = Some(f.reason);
                s.job.detail = Some(f.detail.clone());
            }
        });
        match failure {
            Some(f) => {
                tracing::warn!(state = state.as_str(), reason = f.reason.as_str(), detail = %f.detail, "update ended")
            }
            None => tracing::info!(state = state.as_str(), "update ended"),
        }
    }

    async fn run(self: Arc<Self>, to: Version) {
        let prepared = match self.prepare(&to).await {
            Ok(p) => p,
            Err(f) => return self.end(JobState::Failed, Some(f)).await,
        };
        self.step(Step::Switching);
        for target in &prepared.targets {
            if let Err(f) = self.switch(target).await {
                return self.roll_back(f).await;
            }
            let new_id = self.new_id(target.service);
            let (timeout, patient) = match target.service {
                Service::Gateway => {
                    self.step(Step::Migrating);
                    (self.settings.gateway_timeout, true)
                }
                _ => (self.settings.console_timeout, false),
            };
            if let Err(why) = self.healthy(&new_id, timeout, patient).await {
                let f = fail(
                    Reason::Unhealthy,
                    format!("the new {} {why}", target.service.as_str()),
                );
                return self.roll_back(f).await;
            }
        }
        self.step(Step::Online);
        self.finish(&prepared.from, &to, false).await;
    }

    fn new_id(&self, service: Service) -> String {
        self.snapshot()
            .and_then(|s| s.switched.into_iter().find(|w| w.service == service))
            .and_then(|w| w.new_id)
            .unwrap_or_default()
    }

    fn write_compose(&self, from: &Version, to: &Version, recovering: bool) -> Option<Failure> {
        let by_hand = format!("set the Kaeso image tags in compose.yaml to {to} by hand");
        let Some(kept) = self.settings.state.kept_compose() else {
            return Some(fail(
                Reason::ComposeUnwritable,
                format!("{to} runs, but the updater lost its copy of compose.yaml; {by_hand}"),
            ));
        };
        let prepared = compose::retag(&kept, from, to);
        let now = match self.settings.compose.read() {
            Ok(now) => now,
            Err(e) => {
                return Some(fail(
                    Reason::ComposeUnwritable,
                    format!("{to} runs, but {e}; {by_hand}"),
                ));
            }
        };
        if now == prepared {
            return None;
        }
        let torn =
            recovering && (now.is_empty() || prepared.starts_with(&now) || kept.starts_with(&now));
        if compose::retag(&now, from, to) != prepared && !torn {
            return Some(fail(
                Reason::ComposeChanged,
                format!("{to} runs, but compose.yaml changed during the update; {by_hand}"),
            ));
        }
        self.settings.compose.write(&prepared).err().map(|e| {
            fail(
                Reason::ComposeUnwritable,
                format!("{to} runs, but {e}; {by_hand}"),
            )
        })
    }

    async fn finish(&self, from: &Version, to: &Version, recovering: bool) {
        let warning = self.write_compose(from, to, recovering);
        let saved = self.snapshot();
        let images = saved.as_ref().map(|s| s.images.clone()).unwrap_or_default();
        for image in &images {
            if let Err(e) = self
                .docker
                .tag(&image.reference(), &image.name, &to.to_string())
                .await
            {
                tracing::warn!(error = %e, image = %image.reference(), "cannot tag the new image");
            }
        }
        let old: Vec<String> = saved
            .map(|s| s.switched.into_iter().map(|w| w.old_id).collect())
            .unwrap_or_default();
        for id in old {
            if let Err(e) = self.docker.remove(&id).await {
                tracing::warn!(error = %e, "cannot remove a previous container");
            }
        }
        self.end(JobState::Succeeded, warning).await;
    }

    async fn verified(&self, name: &str, content: &[u8], commit: &str) -> Result<(), Failure> {
        self.verify
            .verify(Subject {
                name,
                content,
                commit,
            })
            .await
            .map_err(|e| fail(Reason::Unverified, e))
    }

    async fn release(&self, version: &Version) -> Result<(Vec<u8>, Manifest), Failure> {
        let bytes = self
            .source
            .asset(version, manifest::ASSET, manifest::MAX_BYTES)
            .await
            .map_err(|e| fail(Reason::ManifestUnavailable, e))?;
        let manifest =
            Manifest::parse(&bytes, version).map_err(|e| fail(Reason::ManifestInvalid, e))?;
        self.verified(manifest::ASSET, &bytes, &manifest.commit)
            .await?;
        Ok((bytes, manifest))
    }

    async fn prepare(&self, to: &Version) -> Result<Prepared, Failure> {
        let (_, manifest) = self.release(to).await?;
        if manifest.updater_protocol > protocol::VERSION {
            return Err(fail(
                Reason::UpdaterOutdated,
                format!(
                    "the release needs updater protocol {}",
                    manifest.updater_protocol
                ),
            ));
        }
        let project = self.project().await?;
        self.update(|s| s.project = Some(project.clone()));
        let mut targets = Vec::new();
        for service in ORDER {
            targets.push(self.target(&project, service, &manifest).await?);
        }
        let from = targets
            .iter()
            .find(|t| t.service == Service::Gateway)
            .and_then(|t| clone::version_of(&t.old_image))
            .and_then(|v| Version::parse(v).ok())
            .ok_or_else(|| {
                fail(
                    Reason::ServicesNotFound,
                    "the running gateway's image names no version",
                )
            })?;
        self.update(|s| s.job.from = from.to_string());
        if *to <= from {
            return Err(fail(Reason::NotNewer, format!("{from} runs already")));
        }
        if policy::is_major(&from, to) {
            return Err(fail(Reason::Major, format!("{to} is a new major version")));
        }
        if manifest.asks_for_steps_after(&from) {
            return Err(fail(
                Reason::ManualSteps,
                format!(
                    "a release from {from} to {to} asks for steps by hand ({})",
                    manifest
                        .manual_since
                        .as_deref()
                        .unwrap_or(&manifest.version)
                ),
            ));
        }
        self.compose_change(&manifest, &from, to).await?;
        let images: Vec<Image> = PULLED.iter().map(|s| manifest.image(*s).clone()).collect();
        for image in &images {
            let index = self
                .source
                .index(image)
                .await
                .map_err(|e| fail(Reason::Unverified, e))?;
            self.verified(&image.name, &index, &manifest.commit).await?;
        }
        self.update(|s| s.images = images.clone());
        self.step(Step::Downloading);
        for image in &images {
            self.docker
                .pull(image)
                .await
                .map_err(|e| fail(Reason::PullFailed, e))?;
            let local = self
                .docker
                .image(&image.reference())
                .await
                .map_err(|e| fail(Reason::DockerUnavailable, e))?
                .ok_or_else(|| {
                    fail(
                        Reason::PullFailed,
                        format!("{} is not in Docker after the pull", image.reference()),
                    )
                })?;
            let says = clone::version_of(&local).unwrap_or("no version");
            if says != to.to_string() {
                return Err(fail(
                    Reason::Unverified,
                    format!("{} says it is {says}, not {to}", image.reference()),
                ));
            }
            if let Some(t) = targets.iter_mut().find(|t| t.image == *image) {
                t.new_image = local;
            }
        }
        let migrates = self.migrates(&from, &manifest).await;
        self.update(|s| s.migrates = migrates);
        Ok(Prepared { from, targets })
    }

    async fn migrates(&self, from: &Version, to: &Manifest) -> bool {
        match self.release(from).await {
            Ok((_, running)) => running.migrations != to.migrations,
            Err(f) => {
                tracing::info!(detail = %f.detail, "the running release's migrations are unknown; the update counts as migrating the database");
                true
            }
        }
    }

    async fn project(&self) -> Result<String, Failure> {
        let me = self
            .docker
            .inspect(&self.settings.hostname)
            .await
            .map_err(|e| fail(Reason::DockerUnavailable, e))?;
        me.pointer(&format!("/Config/Labels/{PROJECT_LABEL}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                fail(
                    Reason::ServicesNotFound,
                    "the updater does not run in a Compose project",
                )
            })
    }

    async fn target(
        &self,
        project: &str,
        service: Service,
        manifest: &Manifest,
    ) -> Result<Target, Failure> {
        let listed = self
            .docker
            .containers(project, service.as_str())
            .await
            .map_err(|e| fail(Reason::DockerUnavailable, e))?;
        let (left, current): (Vec<Listed>, Vec<Listed>) = listed
            .into_iter()
            .partition(|c| c.name.ends_with(PREVIOUS_SUFFIX));
        if let Some(stale) = left.first() {
            return Err(fail(
                Reason::ServicesNotFound,
                format!(
                    "{} is left from an earlier update; remove it with `docker rm {}` first",
                    stale.name, stale.name
                ),
            ));
        }
        let [Listed { id, .. }] = current.as_slice() else {
            let names: Vec<&str> = current.iter().map(|c| c.name.as_str()).collect();
            return Err(fail(
                Reason::ServicesNotFound,
                if names.is_empty() {
                    format!("{project} has no {} container", service.as_str())
                } else {
                    format!(
                        "{project} has {} {} containers ({}); remove all but the one Compose started",
                        names.len(),
                        service.as_str(),
                        names.join(", ")
                    )
                },
            ));
        };
        let container = self
            .docker
            .inspect(id)
            .await
            .map_err(|e| fail(Reason::DockerUnavailable, e))?;
        if container.pointer("/State/Running").and_then(Value::as_bool) != Some(true) {
            return Err(fail(
                Reason::ServicesNotFound,
                format!("the {} is not running", service.as_str()),
            ));
        }
        let image_id = container
            .get("Image")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(Reason::ServicesNotFound, "a container names no image"))?;
        let old_image = self
            .docker
            .image(image_id)
            .await
            .map_err(|e| fail(Reason::DockerUnavailable, e))?
            .ok_or_else(|| fail(Reason::ServicesNotFound, "the running image is missing"))?;
        let name = clone::name(&container).map_err(|e| fail(Reason::ServicesNotFound, e))?;
        Ok(Target {
            service,
            image: manifest.image(service).clone(),
            id: id.clone(),
            name,
            container,
            old_image,
            new_image: Value::Null,
        })
    }

    async fn compose_change(
        &self,
        manifest: &Manifest,
        from: &Version,
        to: &Version,
    ) -> Result<(), Failure> {
        let asset = self
            .source
            .asset(to, manifest::COMPOSE_ASSET, compose::MAX_BYTES)
            .await
            .map_err(|e| fail(Reason::ManifestUnavailable, e))?;
        if hex(&Sha256::digest(&asset)) != manifest.compose.sha256 {
            return Err(fail(
                Reason::ManifestInvalid,
                "compose.yaml of the release does not match the manifest",
            ));
        }
        let local = self
            .settings
            .compose
            .read()
            .map_err(|e| fail(Reason::ComposeUnwritable, e))?;
        if compose::retag(&local, from, to) != asset {
            return Err(fail(
                Reason::ComposeChanged,
                "compose.yaml differs from the release's beyond the image versions",
            ));
        }
        self.settings
            .compose
            .writable()
            .map_err(|e| fail(Reason::ComposeUnwritable, e))?;
        self.settings.state.keep_compose(&local).map_err(|e| {
            fail(
                Reason::StateUnwritable,
                format!("cannot keep a copy of compose.yaml: {e}"),
            )
        })
    }

    fn set_phase(
        &self,
        service: Service,
        phase: Phase,
        new_id: Option<String>,
    ) -> Result<(), Failure> {
        self.record(|s| {
            if let Some(w) = s.switched.iter_mut().find(|w| w.service == service) {
                w.phase = phase;
                if new_id.is_some() {
                    w.new_id = new_id;
                }
            }
        })
    }

    async fn switch(&self, t: &Target) -> Result<(), Failure> {
        let docker = |e: crate::docker::DockerError| fail(Reason::DockerUnavailable, e);
        self.record(|s| {
            s.switched.push(Switched {
                service: t.service,
                name: t.name.clone(),
                old_id: t.id.clone(),
                new_id: None,
                phase: Phase::Stopping,
            })
        })?;
        self.docker
            .stop(&t.id, self.settings.stop_seconds)
            .await
            .map_err(docker)?;
        self.docker
            .rename(&t.id, &format!("{}{PREVIOUS_SUFFIX}", t.name))
            .await
            .map_err(docker)?;
        let mut spec = clone::spec(
            &t.container,
            &t.old_image,
            &t.new_image,
            &t.image.reference(),
        )
        .map_err(|e| fail(Reason::ServicesNotFound, e))?;
        if t.service == Service::Gateway {
            clone::start_period(&mut spec, self.settings.gateway_timeout);
        }
        self.set_phase(t.service, Phase::Creating, None)?;
        let new_id = self.docker.create(&t.name, &spec).await.map_err(docker)?;
        self.set_phase(t.service, Phase::Created, Some(new_id.clone()))?;
        self.docker.start(&new_id).await.map_err(docker)?;
        self.set_phase(t.service, Phase::Started, None)
    }

    async fn healthy(&self, id: &str, timeout: Duration, patient: bool) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut steady = 0;
        loop {
            match self.docker.inspect(id).await {
                Ok(c) => {
                    let state = c.get("State").cloned().unwrap_or(Value::Null);
                    let restarts = c.get("RestartCount").and_then(Value::as_u64).unwrap_or(0);
                    let health = state.pointer("/Health/Status").and_then(Value::as_str);
                    let running = state.get("Running").and_then(Value::as_bool) == Some(true);
                    if restarts > 0
                        || state.get("Restarting").and_then(Value::as_bool) == Some(true)
                    {
                        return Err("restarted after a crash".into());
                    }
                    if !running {
                        let code = state.get("ExitCode").and_then(Value::as_i64).unwrap_or(-1);
                        return Err(format!("stopped with exit code {code}"));
                    }
                    match health {
                        Some("healthy") => return Ok(()),
                        Some("unhealthy") if !patient => {
                            return Err("reported itself unhealthy".into());
                        }
                        Some(_) => {}
                        None => {
                            steady += 1;
                            if steady >= STEADY_POLLS {
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => tracing::debug!(error = %e, "cannot inspect the new container yet"),
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(format!(
                    "was not healthy within {} seconds",
                    timeout.as_secs()
                ));
            }
            tokio::time::sleep(self.settings.poll).await;
        }
    }

    async fn remove_strays(&self, project: &str, w: &Switched) -> Result<(), String> {
        let listed = self
            .docker
            .containers(project, w.service.as_str())
            .await
            .map_err(|e| e.to_string())?;
        for stray in listed
            .iter()
            .filter(|c| c.name == w.name && c.id != w.old_id)
        {
            self.docker
                .remove(&stray.id)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn restore(&self, w: &Switched, project: Option<&str>) -> Result<(), String> {
        match (&w.new_id, project) {
            (Some(new_id), _) => {
                if let Err(e) = self.docker.stop(new_id, self.settings.stop_seconds).await {
                    tracing::warn!(error = %e, "cannot stop the new container");
                }
                self.docker
                    .remove(new_id)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            (None, Some(project)) => self.remove_strays(project, w).await?,
            (None, None) => {}
        }
        if let Err(e) = self.docker.rename(&w.old_id, &w.name).await {
            tracing::debug!(error = %e, "the previous container keeps its name");
        }
        self.docker
            .start(&w.old_id)
            .await
            .map_err(|e| e.to_string())?;
        self.healthy(&w.old_id, self.settings.rollback_timeout, false)
            .await
            .map_err(|why| format!("the previous {} {why}", w.service.as_str()))
    }

    async fn started_gateway(&self, saved: &Saved) -> Option<Switched> {
        let w = saved
            .switched
            .iter()
            .find(|w| w.service == Service::Gateway)?;
        match (w.phase, w.new_id.as_deref()) {
            (Phase::Started, _) => Some(w.clone()),
            (Phase::Created, Some(id)) => {
                let started_at = match self.docker.inspect(id).await {
                    Ok(c) => c
                        .pointer("/State/StartedAt")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    Err(e) => {
                        tracing::warn!(error = %e, "cannot tell whether the new gateway started; it counts as started");
                        return Some(w.clone());
                    }
                };
                started_at
                    .filter(|t| !t.starts_with("0001-01-01"))
                    .map(|_| w.clone())
            }
            _ => None,
        }
    }

    async fn roll_back(&self, failure: Failure) {
        let Some(saved) = self.snapshot() else {
            return self.end(JobState::Failed, Some(failure)).await;
        };
        let gateway = if saved.migrates {
            self.started_gateway(&saved).await
        } else {
            None
        };
        if let Some(w) = gateway {
            let kept: Vec<String> = saved
                .switched
                .iter()
                .map(|s| format!("{}{PREVIOUS_SUFFIX}", s.name))
                .collect();
            let detail = format!(
                "{}; the new gateway may have migrated the database, so the new {} left in place and the previous ones stay stopped as {}",
                failure.detail,
                if saved.switched.len() > 1 {
                    "console and gateway were"
                } else {
                    "gateway was"
                },
                kept.join(" and ")
            );
            tracing::warn!(gateway = %w.name, "the update needs a person");
            return self
                .end(
                    JobState::NeedsAttention,
                    Some(Failure {
                        reason: failure.reason,
                        detail,
                    }),
                )
                .await;
        }
        let mut problems = Vec::new();
        for w in saved.switched.iter().rev() {
            if let Err(e) = self.restore(w, saved.project.as_deref()).await {
                problems.push(e);
            }
        }
        let problems = if problems.is_empty() {
            String::new()
        } else {
            format!("; rolling back failed: {}", problems.join("; "))
        };
        if problems.is_empty() {
            self.end(JobState::RolledBack, Some(failure)).await;
        } else {
            self.end(
                JobState::RollbackFailed,
                Some(Failure {
                    reason: failure.reason,
                    detail: format!("{}{problems}", failure.detail),
                }),
            )
            .await;
        }
    }

    async fn docker_answers(&self) -> bool {
        let deadline = tokio::time::Instant::now() + self.settings.docker_wait;
        loop {
            match self.docker.version().await {
                Ok(_) => return true,
                Err(e) if tokio::time::Instant::now() >= deadline => {
                    tracing::warn!(error = %e, "Docker does not answer");
                    return false;
                }
                Err(_) => tokio::time::sleep(self.settings.poll).await,
            }
        }
    }

    pub async fn recover(&self) {
        let Some(saved) = self.snapshot() else {
            return;
        };
        if saved.job.state != JobState::Running {
            return;
        }
        tracing::warn!(
            step = saved.job.step.as_str(),
            "the updater restarted during an update"
        );
        if !self.docker_answers().await {
            let detail = format!(
                "the updater restarted during the update and Docker did not answer within {} seconds; check the gateway and console containers",
                self.settings.docker_wait.as_secs()
            );
            let state = if saved.switched.is_empty() {
                JobState::Failed
            } else {
                JobState::NeedsAttention
            };
            return self
                .end(state, Some(fail(Reason::DockerUnavailable, detail)))
                .await;
        }
        let interrupted = fail(
            Reason::Interrupted,
            "the updater restarted during the update",
        );
        let versions = (
            Version::parse(&saved.job.from),
            Version::parse(&saved.job.to),
        );
        if let (Ok(from), Ok(to)) = versions {
            if saved.job.step == Step::Online {
                return self.finish(&from, &to, true).await;
            }
            let started = self.started_gateway(&saved).await;
            if let Some(new_id) = started.as_ref().and_then(|w| w.new_id.as_deref())
                && self
                    .healthy(new_id, self.settings.gateway_timeout, true)
                    .await
                    .is_ok()
            {
                self.step(Step::Online);
                return self.finish(&from, &to, true).await;
            }
        }
        if saved.switched.is_empty() {
            return self.end(JobState::Failed, Some(interrupted)).await;
        }
        self.roll_back(interrupted).await;
    }
}
