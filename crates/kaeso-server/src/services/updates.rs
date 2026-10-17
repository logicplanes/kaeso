use std::sync::LazyLock;
use std::time::Duration;

use futures::StreamExt;
use kaeso_core::id::UpdateId;
use kaeso_store::StoreError;
use kaeso_store::pg::updates::{self, Choices, Finish, JobRow, ReleaseRow, Settings};
use kaeso_store::pg::{self, locks};
use kaeso_update::Version;
use kaeso_update::manifest::{self, Manifest};
use kaeso_update::policy::{self, Channel, Plan, Release};
use kaeso_update::protocol::{JobState, Reason};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, ETAG, IF_NONE_MATCH, RETRY_AFTER};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

use crate::services::updater::{self, UpdaterError};
use crate::state::AppState;

pub const CHECK_INTERVAL: time::Duration = time::Duration::hours(12);
pub const RETRY_INTERVAL: time::Duration = time::Duration::hours(1);
pub const LOST_AFTER: time::Duration = time::Duration::hours(1);
pub const START_GRACE: time::Duration = time::Duration::minutes(2);
const JITTER_SECONDS: u32 = 3600;
const MANIFESTS_AT_ONCE: usize = 4;
const MANIFESTS_PER_CHECK: usize = 12;
const PER_PAGE: &str = "50";
const MAX_LIST: usize = 4 << 20;
const MAX_NOTES: usize = 64 << 10;

static HTTP: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let downgrade = attempt.url().scheme() != "https"
                && attempt.previous().iter().any(|u| u.scheme() == "https");
            if downgrade || attempt.previous().len() > 5 {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .inspect_err(|e| tracing::error!(error = %e, "cannot build the release client"))
        .ok()
});

pub fn running() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

pub fn defaults(config: &crate::config::Config) -> Choices {
    let u = &config.updates;
    Choices {
        check_enabled: u.check,
        channel: u.channel.as_str().to_owned(),
        apply_enabled: u.apply,
    }
}

#[derive(Clone, Debug)]
pub struct Known {
    pub version: Version,
    pub row: ReleaseRow,
    pub manifest: Option<Manifest>,
}

impl Release for Known {
    fn version(&self) -> &Version {
        &self.version
    }

    fn needs_manual_steps(&self) -> bool {
        self.manifest.as_ref().is_some_and(|m| m.manual)
    }

    fn prepared(&self) -> bool {
        self.manifest.is_some()
    }

    fn asks_for_steps_after(&self, running: &Version) -> bool {
        self.manifest
            .as_ref()
            .is_some_and(|m| m.asks_for_steps_after(running))
    }
}

pub fn known(rows: Vec<ReleaseRow>) -> Vec<Known> {
    rows.into_iter()
        .filter_map(|row| {
            let version = Version::parse(&row.version).ok()?;
            let manifest = row
                .manifest
                .as_ref()
                .and_then(|m| serde_json::from_value::<Manifest>(m.clone()).ok());
            Some(Known {
                version,
                row,
                manifest,
            })
        })
        .collect()
}

pub fn channel(settings: &Settings) -> Channel {
    Channel::parse(&settings.channel).unwrap_or(Channel::Stable)
}

pub fn plan<'a>(settings: &Settings, releases: &'a [Known]) -> Option<Plan<'a, Known>> {
    policy::plan(&running(), channel(settings), releases)
}

#[derive(Debug, thiserror::Error)]
enum CheckError {
    #[error("{0} cannot be reached ({1})")]
    Unreachable(String, &'static str),
    #[error("{0} refused more requests for now")]
    RateLimited(String, Option<time::Duration>),
    #[error("{0} answered {1}")]
    Status(String, u16),
    #[error("the release list from {0} does not parse: {1}")]
    Parse(String, String),
    #[error("the release list from {0} is larger than 4 MiB")]
    TooLarge(String),
    #[error("no HTTP client")]
    Client,
}

impl CheckError {
    fn retry_after(&self) -> Option<time::Duration> {
        match self {
            CheckError::RateLimited(_, after) => *after,
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default, with = "time::serde::rfc3339::option")]
    published_at: Option<OffsetDateTime>,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

enum Fetched {
    NotModified,
    Releases {
        etag: Option<String>,
        releases: Vec<GitHubRelease>,
    },
}

fn host_of(url: &url::Url) -> String {
    url.host_str().unwrap_or("the release server").to_owned()
}

fn kind_of(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timed out"
    } else if e.is_connect() {
        "no connection"
    } else {
        "request failed"
    }
}

async fn fetch_list(state: &AppState, etag: Option<&str>) -> Result<Fetched, CheckError> {
    let url = &state.config.updates.releases_url;
    let host = host_of(url);
    let client = HTTP.as_ref().ok_or(CheckError::Client)?;
    let mut paged = url.clone();
    paged.query_pairs_mut().append_pair("per_page", PER_PAGE);
    let mut req = client
        .get(paged)
        .header(ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(tag) = etag {
        req = req.header(IF_NONE_MATCH, tag);
    }
    let mut resp = req
        .send()
        .await
        .map_err(|e| CheckError::Unreachable(host.clone(), kind_of(&e)))?;
    let status = resp.status();
    if status == StatusCode::NOT_MODIFIED {
        return Ok(Fetched::NotModified);
    }
    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::FORBIDDEN {
        return Err(CheckError::RateLimited(
            host,
            rate_limit_wait(resp.headers(), state.clock.now()),
        ));
    }
    if !status.is_success() {
        return Err(CheckError::Status(host, status.as_u16()));
    }
    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| CheckError::Unreachable(host.clone(), kind_of(&e)))?
    {
        if body.len() + chunk.len() > MAX_LIST {
            return Err(CheckError::TooLarge(host));
        }
        body.extend_from_slice(&chunk);
    }
    let releases =
        serde_json::from_slice(&body).map_err(|e| CheckError::Parse(host, e.to_string()))?;
    Ok(Fetched::Releases { etag, releases })
}

fn rate_limit_wait(
    headers: &reqwest::header::HeaderMap,
    now: OffsetDateTime,
) -> Option<time::Duration> {
    let number = |name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
    };
    if let Some(seconds) = number(RETRY_AFTER.as_str()) {
        return Some(time::Duration::seconds(seconds.max(0)));
    }
    number("x-ratelimit-reset")
        .map(|reset| time::Duration::seconds((reset - now.unix_timestamp()).max(0)))
}

pub fn downloads_of(releases_url: &url::Url) -> String {
    if releases_url.host_str() == Some("api.github.com") {
        kaeso_update::DOWNLOADS.to_owned()
    } else {
        format!("{}/", releases_url.origin().ascii_serialization())
    }
}

async fn fetch_manifest(url: &str, version: &Version, downloads: &str) -> Result<Manifest, String> {
    if !url.starts_with(downloads) {
        return Err(format!(
            "the release manifest is not under {downloads}, so it is ignored"
        ));
    }
    let client = HTTP.as_ref().ok_or_else(|| "no HTTP client".to_owned())?;
    let mut resp = client
        .get(url)
        .header(ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("the release manifest cannot be fetched ({})", kind_of(&e)))?;
    if !resp.status().is_success() {
        return Err(format!(
            "the release manifest answered {}",
            resp.status().as_u16()
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("the release manifest cannot be fetched ({})", kind_of(&e)))?
    {
        if body.len() + chunk.len() > manifest::MAX_BYTES {
            return Err(manifest::ManifestError::TooLarge.to_string());
        }
        body.extend_from_slice(&chunk);
    }
    Manifest::parse(&body, version).map_err(|e| e.to_string())
}

fn notes(body: Option<String>) -> String {
    let mut text = body.unwrap_or_default();
    if text.len() > MAX_NOTES {
        let mut end = MAX_NOTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

enum Wanted {
    Ready(Option<serde_json::Value>, Option<String>),
    Fetch(String),
}

async fn rows_of(
    releases: Vec<GitHubRelease>,
    previous: &[Known],
    downloads: &str,
) -> Vec<ReleaseRow> {
    let current = running();
    let mut listed: Vec<(Version, GitHubRelease, Wanted)> = Vec::new();
    for r in releases.into_iter().filter(|r| !r.draft) {
        let Some(version) = policy::parse_tag(&r.tag_name) else {
            continue;
        };
        if listed.iter().any(|(v, _, _)| *v == version) {
            continue;
        }
        let asset = r
            .assets
            .iter()
            .find(|a| a.name == manifest::ASSET)
            .map(|a| a.browser_download_url.clone());
        let kept = previous
            .iter()
            .find(|k| k.version == version && k.manifest.is_some());
        let wanted = match (kept, asset) {
            (Some(k), _) => Wanted::Ready(k.row.manifest.clone(), None),
            (None, _) if version <= current => Wanted::Ready(None, None),
            (None, None) => {
                Wanted::Ready(None, Some("the release has no release manifest".to_owned()))
            }
            (None, Some(url)) => Wanted::Fetch(url),
        };
        listed.push((version, r, wanted));
    }
    let mut fetches: Vec<(Version, String)> = listed
        .iter()
        .filter_map(|(v, _, w)| match w {
            Wanted::Fetch(url) => Some((v.clone(), url.clone())),
            Wanted::Ready(..) => None,
        })
        .collect();
    fetches.sort_by(|a, b| b.0.cmp(&a.0));
    fetches.truncate(MANIFESTS_PER_CHECK);
    let fetched: Vec<(Version, Result<Manifest, String>)> = futures::stream::iter(fetches)
        .map(|(version, url)| async move {
            let result = fetch_manifest(&url, &version, downloads).await;
            (version, result)
        })
        .buffer_unordered(MANIFESTS_AT_ONCE)
        .collect()
        .await;
    listed
        .into_iter()
        .map(|(version, r, wanted)| {
            let (manifest, manifest_error) = match wanted {
                Wanted::Ready(m, e) => (m, e),
                Wanted::Fetch(_) => match fetched.iter().find(|(v, _)| *v == version) {
                    Some((_, Ok(m))) => (serde_json::to_value(m).ok(), None),
                    Some((_, Err(e))) => (None, Some(e.clone())),
                    None => (
                        None,
                        Some("the release manifest is fetched at a later check".to_owned()),
                    ),
                },
            };
            ReleaseRow {
                version: version.to_string(),
                published_at: r.published_at.unwrap_or(OffsetDateTime::UNIX_EPOCH),
                prerelease: r.prerelease || !version.pre.is_empty(),
                notes: notes(r.body),
                url: r.html_url,
                manifest,
                manifest_error,
            }
        })
        .collect()
}

fn jitter() -> time::Duration {
    let random = u32::from_le_bytes(kaeso_crypto::random_bytes::<4>());
    time::Duration::seconds(i64::from(random % JITTER_SECONDS))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checked {
    Unchanged,
    Changed,
    Failed(String),
}

pub async fn check(state: &AppState) -> Result<Checked, StoreError> {
    let st = state.clone();
    pg::with_advisory_lock(&state.pg, locks::UPDATE_CHECK, || async move {
        check_locked(&st).await
    })
    .await
}

async fn check_locked(state: &AppState) -> Result<Checked, StoreError> {
    let settings = updates::settings(&state.pg).await?;
    if let Some(until) = settings.blocked_until.filter(|u| state.clock.now() < *u) {
        tracing::debug!(%until, "GitHub asked to wait; the check waits too");
        return Ok(Checked::Failed(settings.last_error.unwrap_or_else(|| {
            "GitHub refused more requests for now".to_owned()
        })));
    }
    let previous = known(updates::releases(&state.pg).await?);
    let current = running();
    let incomplete = previous
        .iter()
        .any(|k| k.version > current && k.manifest.is_none());
    let etag = settings
        .etag
        .as_deref()
        .filter(|_| !previous.is_empty() && !incomplete);
    let fetched = fetch_list(state, etag).await;
    let now = state.clock.now();
    match fetched {
        Ok(Fetched::NotModified) => {
            updates::record_check(
                &state.pg,
                now,
                None,
                None,
                now + CHECK_INTERVAL + jitter(),
                None,
            )
            .await?;
            Ok(Checked::Unchanged)
        }
        Ok(Fetched::Releases { etag, releases }) => {
            let downloads = downloads_of(&state.config.updates.releases_url);
            let rows = rows_of(releases, &previous, &downloads).await;
            let mut tx = state.pg.begin().await?;
            updates::replace_releases(&mut tx, &rows, etag.as_deref()).await?;
            updates::record_check(
                &mut *tx,
                now,
                None,
                None,
                now + CHECK_INTERVAL + jitter(),
                None,
            )
            .await?;
            tx.commit().await?;
            tracing::info!(releases = rows.len(), "fetched the Kaeso release list");
            Ok(Checked::Changed)
        }
        Err(e) => {
            let message = e.to_string();
            if settings.last_error.as_deref() == Some(message.as_str()) {
                tracing::debug!(error = %message, "the update check failed again");
            } else {
                tracing::warn!(error = %message, "the update check failed; it tries again later");
            }
            let wait = e
                .retry_after()
                .map_or(RETRY_INTERVAL, |w| w.max(RETRY_INTERVAL));
            let blocked = e.retry_after().map(|w| now + w);
            updates::record_check(
                &state.pg,
                now,
                None,
                Some(&message),
                now + wait + jitter(),
                blocked,
            )
            .await?;
            Ok(Checked::Failed(message))
        }
    }
}

pub async fn due(state: &AppState) -> Result<bool, StoreError> {
    let settings = updates::settings(&state.pg).await?;
    Ok(settings.check_enabled
        && settings
            .next_check_at
            .is_none_or(|at| state.clock.now() >= at))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdaterState {
    NotInstalled,
    Unreachable,
    Ready,
    Outdated,
}

impl UpdaterState {
    pub fn as_str(self) -> &'static str {
        match self {
            UpdaterState::NotInstalled => "not_installed",
            UpdaterState::Unreachable => "unreachable",
            UpdaterState::Ready => "ready",
            UpdaterState::Outdated => "outdated",
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdaterProbe {
    pub state: UpdaterState,
    pub version: Option<String>,
}

pub async fn probe(state: &AppState, required: Option<u32>) -> UpdaterProbe {
    let Some(client) = updater::of(state) else {
        return UpdaterProbe {
            state: UpdaterState::NotInstalled,
            version: None,
        };
    };
    match client.status().await {
        Ok(status) => {
            let speaks = updater::speaks(&status, required.unwrap_or(0));
            UpdaterProbe {
                state: if speaks && status.docker {
                    UpdaterState::Ready
                } else if speaks {
                    UpdaterState::Unreachable
                } else {
                    UpdaterState::Outdated
                },
                version: Some(status.version),
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "the updater does not answer");
            UpdaterProbe {
                state: UpdaterState::Unreachable,
                version: None,
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Outcome<'a> {
    pub state: JobState,
    pub step: &'a str,
    pub reason: Option<Reason>,
    pub detail: Option<&'a str>,
}

pub async fn finish(
    state: &AppState,
    job: &JobRow,
    outcome: &Outcome<'_>,
) -> Result<bool, StoreError> {
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let finished = updates::finish_job(
        &mut *tx,
        job.id,
        &Finish {
            state: outcome.state.as_str(),
            step: outcome.step,
            reason: outcome.reason.map(Reason::as_str),
            detail: outcome.detail,
            at: now,
        },
    )
    .await?;
    if finished {
        crate::platform::audit::system_event(
            &mut tx,
            None,
            "instance.update_finish",
            Some(("update", UpdateId::from_uuid(job.id).to_string())),
            Some(json!({ "version": job.from_version })),
            Some(json!({
                "version": job.to_version,
                "result": outcome.state.as_str(),
                "reason": outcome.reason.map(Reason::as_str),
            })),
            now,
        )
        .await?;
    }
    tx.commit().await?;
    if finished {
        tracing::info!(
            from = %job.from_version,
            to = %job.to_version,
            result = outcome.state.as_str(),
            reason = outcome.reason.map(Reason::as_str),
            "update finished"
        );
    }
    Ok(finished)
}

pub async fn reconcile(state: &AppState, job: JobRow) -> Result<JobRow, StoreError> {
    if job.state != JobState::Running.as_str() {
        return Ok(job);
    }
    let id = UpdateId::from_uuid(job.id).to_string();
    let seen = match updater::of(state) {
        None => Err(UpdaterError::NotSent("not configured".into())),
        Some(client) => client.job(&id).await,
    };
    match seen {
        Ok(Some(remote)) if remote.state.is_finished() => {
            finish(
                state,
                &job,
                &Outcome {
                    state: remote.state,
                    step: remote.step.as_str(),
                    reason: remote.reason,
                    detail: remote.detail.as_deref(),
                },
            )
            .await?;
        }
        Ok(Some(remote)) => {
            if remote.step.as_str() != job.step {
                updates::set_step(&state.pg, job.id, remote.step.as_str()).await?;
            }
        }
        Ok(None) if state.clock.now() - job.started_at < START_GRACE => {
            tracing::debug!("the updater has not taken up the update yet");
        }
        Ok(None) => {
            finish(
                state,
                &job,
                &Outcome {
                    state: JobState::Failed,
                    step: &job.step,
                    reason: Some(Reason::Interrupted),
                    detail: Some("the updater has no record of this update"),
                },
            )
            .await?;
        }
        Err(e) if state.clock.now() - job.started_at > LOST_AFTER => {
            tracing::warn!(error = %e, "the updater lost touch with a running update");
            finish(
                state,
                &job,
                &Outcome {
                    state: JobState::Failed,
                    step: &job.step,
                    reason: Some(Reason::UpdaterUnreachable),
                    detail: Some("the updater did not answer for an hour"),
                },
            )
            .await?;
        }
        Err(e) => tracing::debug!(error = %e, "the updater does not answer about a running update"),
    }
    Ok(updates::job(&state.pg, job.id).await?.unwrap_or(job))
}

pub async fn reconcile_running(state: &AppState) -> Result<Option<JobRow>, StoreError> {
    match updates::running_job(&state.pg).await? {
        Some(job) => reconcile(state, job).await.map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn notes_are_cut_at_64_kib_on_a_character_boundary() {
        let long = "ä".repeat(MAX_NOTES);
        let cut = notes(Some(long));
        assert!(cut.len() <= MAX_NOTES);
        assert!(cut.chars().all(|c| c == 'ä'));
        assert_eq!(notes(None), "");
    }

    #[test]
    fn rate_limits_wait_for_retry_after_or_the_reset_time() {
        let now = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        let mut h = HeaderMap::new();
        assert_eq!(rate_limit_wait(&h, now), None);
        h.insert("x-ratelimit-reset", HeaderValue::from_static("4600"));
        assert_eq!(
            rate_limit_wait(&h, now),
            Some(time::Duration::seconds(3600))
        );
        h.insert(RETRY_AFTER, HeaderValue::from_static("120"));
        assert_eq!(rate_limit_wait(&h, now), Some(time::Duration::seconds(120)));
    }

    #[test]
    fn manifests_come_only_from_kaesos_releases_or_the_configured_server() {
        let github =
            url::Url::parse("https://api.github.com/repos/logicplanes/kaeso/releases").unwrap();
        assert_eq!(
            downloads_of(&github),
            "https://github.com/logicplanes/kaeso/releases/download/"
        );
        let mirror =
            url::Url::parse("http://releases:8080/repos/logicplanes/kaeso/releases").unwrap();
        assert_eq!(downloads_of(&mirror), "http://releases:8080/");
    }

    #[test]
    fn the_jitter_stays_within_an_hour() {
        for _ in 0..100 {
            let j = jitter();
            assert!(j >= time::Duration::ZERO && j < time::Duration::hours(1));
        }
    }

    #[test]
    fn updater_states_have_stable_names() {
        let names: Vec<&str> = [
            UpdaterState::NotInstalled,
            UpdaterState::Unreachable,
            UpdaterState::Ready,
            UpdaterState::Outdated,
        ]
        .iter()
        .map(|s| s.as_str())
        .collect();
        assert_eq!(names, ["not_installed", "unreachable", "ready", "outdated"]);
    }
}
