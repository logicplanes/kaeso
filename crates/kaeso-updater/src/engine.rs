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
use crate::docker::{Docker, PROJECT_LABEL};
use crate::files::{MaintenanceFlag, Saved, StateFile, Switched};
use crate::source::Source;
use crate::verify::{Subject, Verify, hex};

const ORDER: [Service; 2] = [Service::Console, Service::Gateway];
const PULLED: [Service; 3] = [Service::Gateway, Service::Console, Service::Updater];
const PREVIOUS_SUFFIX: &str = "-previous";
const STEADY_POLLS: u32 = 3;

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
    compose: Vec<u8>,
    images: Vec<Image>,
    targets: Vec<Target>,
}

#[derive(Debug, thiserror::Error)]
#[error("an update is already running")]
pub struct Busy;

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

    fn with<T>(&self, f: impl FnOnce(&mut Option<Saved>) -> T) -> T {
        let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let out = f(&mut guard);
        if let Some(saved) = guard.as_ref()
            && let Err(e) = self.settings.state.save(saved)
        {
            tracing::warn!(error = %e, "cannot save the update's state");
        }
        out
    }

    fn update(&self, f: impl FnOnce(&mut Saved)) {
        self.with(|current| {
            if let Some(saved) = current.as_mut() {
                f(saved);
            }
        });
    }

    pub fn last(&self) -> Option<Job> {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|s| s.job.clone())
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

    pub fn begin(self: &Arc<Self>, id: String, to: Version) -> Result<Job, Busy> {
        let job = self.with(|current| {
            if current
                .as_ref()
                .is_some_and(|s| s.job.state == JobState::Running)
            {
                return Err(Busy);
            }
            let job = Job {
                id,
                from: String::new(),
                to: to.to_string(),
                state: JobState::Running,
                step: Step::Downloading,
                reason: None,
                detail: None,
                started_at: OffsetDateTime::now_utc(),
                finished_at: None,
            };
            *current = Some(Saved {
                job: job.clone(),
                switched: Vec::new(),
            });
            Ok(job)
        })?;
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

    fn end(&self, state: JobState, failure: Option<Failure>) {
        if let Err(e) = self.settings.maintenance.clear() {
            tracing::warn!(error = %e, "cannot remove the maintenance flag");
        }
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
            Err(f) => return self.end(JobState::Failed, Some(f)),
        };
        self.step(Step::Switching);
        for target in &prepared.targets {
            if let Err(f) = self.switch(target).await {
                return self.roll_back(f).await;
            }
            let new_id = self.new_id(target.service);
            let timeout = match target.service {
                Service::Gateway => {
                    self.step(Step::Migrating);
                    self.settings.gateway_timeout
                }
                _ => self.settings.console_timeout,
            };
            if let Err(why) = self.healthy(&new_id, timeout).await {
                let f = fail(
                    Reason::Unhealthy,
                    format!("the new {} {why}", target.service.as_str()),
                );
                return self.roll_back(f).await;
            }
        }
        self.step(Step::Online);
        self.finish(
            &prepared.from,
            &to,
            Some(&prepared.compose),
            &prepared.images,
        )
        .await;
    }

    fn new_id(&self, service: Service) -> String {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(|s| s.switched.iter().find(|w| w.service == service))
            .and_then(|w| w.new_id.clone())
            .unwrap_or_default()
    }

    async fn finish(&self, from: &Version, to: &Version, compose: Option<&[u8]>, images: &[Image]) {
        let written = match compose {
            Some(bytes) => self.settings.compose.write(bytes),
            None => self
                .settings
                .compose
                .read()
                .and_then(|now| self.settings.compose.write(&compose::retag(&now, from, to))),
        };
        for image in images {
            if let Err(e) = self
                .docker
                .tag(&image.reference(), &image.name, &to.to_string())
                .await
            {
                tracing::warn!(error = %e, image = %image.reference(), "cannot tag the new image");
            }
        }
        let old: Vec<String> = self
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|s| s.switched.iter().map(|w| w.old_id.clone()).collect())
            .unwrap_or_default();
        for id in old {
            if let Err(e) = self.docker.remove(&id).await {
                tracing::warn!(error = %e, "cannot remove a previous container");
            }
        }
        match written {
            Ok(()) => self.end(JobState::Succeeded, None),
            Err(e) => self.end(
                JobState::Failed,
                Some(fail(
                    Reason::ComposeUnwritable,
                    format!("{to} runs, but {e}; set the Kaeso image tags in compose.yaml to {to} by hand"),
                )),
            ),
        }
    }

    async fn prepare(&self, to: &Version) -> Result<Prepared, Failure> {
        let manifest_bytes = self
            .source
            .asset(to, manifest::ASSET, manifest::MAX_BYTES)
            .await
            .map_err(|e| fail(Reason::ManifestUnavailable, e))?;
        let manifest =
            Manifest::parse(&manifest_bytes, to).map_err(|e| fail(Reason::ManifestInvalid, e))?;
        if manifest.manual {
            return Err(fail(
                Reason::ManualSteps,
                "the release asks for steps by hand",
            ));
        }
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
        let compose = self.compose_change(&manifest, &from, to).await?;
        let images: Vec<Image> = PULLED.iter().map(|s| manifest.image(*s).clone()).collect();
        for image in &images {
            self.docker
                .pull(image)
                .await
                .map_err(|e| fail(Reason::PullFailed, e))?;
        }
        self.step(Step::Verifying);
        let unverified = |e: &dyn std::fmt::Display| fail(Reason::Unverified, e);
        self.verify
            .verify(Subject {
                name: manifest::ASSET,
                content: &manifest_bytes,
                commit: &manifest.commit,
            })
            .await
            .map_err(|e| unverified(&e))?;
        for image in &images {
            let index = self.source.index(image).await.map_err(|e| unverified(&e))?;
            self.verify
                .verify(Subject {
                    name: &image.name,
                    content: &index,
                    commit: &manifest.commit,
                })
                .await
                .map_err(|e| unverified(&e))?;
            let local = self
                .docker
                .image(&image.reference())
                .await
                .map_err(|e| fail(Reason::DockerUnavailable, e))?
                .ok_or_else(|| {
                    unverified(&format!(
                        "{} is not in Docker after the pull",
                        image.reference()
                    ))
                })?;
            let says = clone::version_of(&local).unwrap_or("no version");
            if says != to.to_string() {
                return Err(unverified(&format!(
                    "{} says it is {says}, not {to}",
                    image.reference()
                )));
            }
            if let Some(t) = targets.iter_mut().find(|t| t.image == *image) {
                t.new_image = local;
            }
        }
        Ok(Prepared {
            from,
            compose,
            images,
            targets,
        })
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
        let ids = self
            .docker
            .containers(project, service.as_str())
            .await
            .map_err(|e| fail(Reason::DockerUnavailable, e))?;
        let [id] = ids.as_slice() else {
            return Err(fail(
                Reason::ServicesNotFound,
                format!(
                    "{project} has {} {} containers, not one",
                    ids.len(),
                    service.as_str()
                ),
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
    ) -> Result<Vec<u8>, Failure> {
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
        let changed = compose::retag(&local, from, to);
        if changed != asset {
            return Err(fail(
                Reason::ComposeChanged,
                "compose.yaml differs from the release's beyond the image versions",
            ));
        }
        self.settings
            .compose
            .writable()
            .map_err(|e| fail(Reason::ComposeUnwritable, e))?;
        Ok(changed)
    }

    async fn switch(&self, t: &Target) -> Result<(), Failure> {
        let docker = |e: crate::docker::DockerError| fail(Reason::DockerUnavailable, e);
        self.docker
            .stop(&t.id, self.settings.stop_seconds)
            .await
            .map_err(docker)?;
        self.update(|s| {
            s.switched.push(Switched {
                service: t.service,
                name: t.name.clone(),
                old_id: t.id.clone(),
                new_id: None,
            })
        });
        self.docker
            .rename(&t.id, &format!("{}{PREVIOUS_SUFFIX}", t.name))
            .await
            .map_err(docker)?;
        let spec = clone::spec(
            &t.container,
            &t.old_image,
            &t.new_image,
            &t.image.reference(),
        )
        .map_err(|e| fail(Reason::ServicesNotFound, e))?;
        let new_id = self.docker.create(&t.name, &spec).await.map_err(docker)?;
        self.update(|s| {
            if let Some(w) = s.switched.iter_mut().find(|w| w.service == t.service) {
                w.new_id = Some(new_id.clone());
            }
        });
        self.docker.start(&new_id).await.map_err(docker)
    }

    async fn healthy(&self, id: &str, timeout: Duration) -> Result<(), String> {
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
                        Some("unhealthy") => return Err("reported itself unhealthy".into()),
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

    async fn restore(&self, w: &Switched) -> Result<(), String> {
        if let Some(new_id) = &w.new_id {
            if let Err(e) = self.docker.stop(new_id, self.settings.stop_seconds).await {
                tracing::warn!(error = %e, "cannot stop the new container");
            }
            self.docker
                .remove(new_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Err(e) = self.docker.rename(&w.old_id, &w.name).await {
            tracing::debug!(error = %e, "the previous container keeps its name");
        }
        self.docker
            .start(&w.old_id)
            .await
            .map_err(|e| e.to_string())?;
        self.healthy(&w.old_id, self.settings.rollback_timeout)
            .await
            .map_err(|why| format!("the previous {} {why}", w.service.as_str()))
    }

    async fn roll_back(&self, failure: Failure) {
        let switched: Vec<Switched> = self
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|s| s.switched.clone())
            .unwrap_or_default();
        let mut problems = Vec::new();
        for w in switched.iter().rev() {
            if let Err(e) = self.restore(w).await {
                problems.push(e);
            }
        }
        if problems.is_empty() {
            self.end(JobState::RolledBack, Some(failure));
        } else {
            let detail = format!(
                "{}; rolling back failed: {}",
                failure.detail,
                problems.join("; ")
            );
            self.end(
                JobState::RollbackFailed,
                Some(Failure {
                    reason: failure.reason,
                    detail,
                }),
            );
        }
    }

    pub async fn recover(&self) {
        let Some(saved) = self
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        else {
            return;
        };
        if saved.job.state != JobState::Running {
            return;
        }
        tracing::warn!(
            step = saved.job.step.as_str(),
            "the updater restarted during an update"
        );
        let interrupted = fail(
            Reason::Interrupted,
            "the updater restarted during the update",
        );
        if saved.job.step == Step::Online
            && let (Ok(from), Ok(to)) = (
                Version::parse(&saved.job.from),
                Version::parse(&saved.job.to),
            )
        {
            return self.finish(&from, &to, None, &[]).await;
        }
        if saved.switched.is_empty() {
            return self.end(JobState::Failed, Some(interrupted));
        }
        self.roll_back(interrupted).await;
    }
}
