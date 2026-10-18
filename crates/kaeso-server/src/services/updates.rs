use std::sync::LazyLock;
use std::time::Duration;

use futures::StreamExt;
use kaeso_core::id::UpdateId;
use kaeso_store::StoreError;
use kaeso_store::pg::updates::{self, Choices, Finish, JobRow, NewJob, ReleaseRow, Settings};
use kaeso_store::pg::{self, locks};
use kaeso_update::Version;
use kaeso_update::manifest::{self, Manifest};
use kaeso_update::policy::{self, Channel, Plan, Release};
use kaeso_update::protocol::{JobState, Reason, StartRequest};
use kaeso_update::schedule::{self, Frequency, Mode, Schedule, Weekday};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, ETAG, IF_NONE_MATCH, LINK, RETRY_AFTER};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

use crate::services::updater::{self, Updater, UpdaterError};
use crate::state::AppState;

pub const CHECK_INTERVAL: time::Duration = time::Duration::hours(12);
pub const RETRY_INTERVAL: time::Duration = time::Duration::hours(1);
pub const LOST_AFTER: time::Duration = time::Duration::hours(1);
pub const START_GRACE: time::Duration = time::Duration::minutes(2);
pub const FRESH_FOR_AUTO: time::Duration = time::Duration::hours(1);
const JITTER_SECONDS: u32 = 3600;
const MANIFESTS_AT_ONCE: usize = 4;
const MANIFESTS_PER_CHECK: usize = 12;
const PER_PAGE: &str = "50";
const MAX_LIST: usize = 4 << 20;
const MAX_PAGES: usize = 10;
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
        channel: u.channel.as_str().to_owned(),
        mode: u.mode.as_str().to_owned(),
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

    fn prerelease(&self) -> bool {
        self.row.prerelease
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

pub fn mode(settings: &Settings) -> Mode {
    Mode::parse(&settings.mode).unwrap_or(Mode::Manual)
}

pub fn schedule(settings: &Settings, timezone: &str) -> Schedule {
    Schedule {
        frequency: Frequency::parse(&settings.auto_frequency).unwrap_or(Frequency::Daily),
        weekday: Weekday::from_number(settings.auto_weekday).unwrap_or(Weekday::Monday),
        hour: u8::try_from(settings.auto_hour).unwrap_or(schedule::DEFAULT_HOUR),
        zone: crate::time_zone::or_utc(timezone),
    }
}

pub fn next_auto_update(
    settings: &Settings,
    timezone: &str,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    if mode(settings) != Mode::Automatic {
        return None;
    }
    let plan = schedule(settings, timezone);
    let after = settings
        .auto_window_at
        .map_or(now, |handled| handled.max(now));
    match plan.window(now) {
        Some(start)
            if settings
                .auto_window_at
                .is_none_or(|handled| handled < start) =>
        {
            Some(start)
        }
        _ => plan.next(after),
    }
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

enum Page {
    NotModified,
    Releases {
        etag: Option<String>,
        next: Option<url::Url>,
        releases: Vec<GitHubRelease>,
    },
}

fn next_page(headers: &reqwest::header::HeaderMap) -> Option<url::Url> {
    let link = headers.get(LINK)?.to_str().ok()?;
    link.split(',').find_map(|part| {
        let (target, params) = part.split_once(';')?;
        params
            .split(';')
            .any(|p| p.trim() == r#"rel="next""#)
            .then(|| target.trim().trim_start_matches('<').trim_end_matches('>'))
            .and_then(|t| url::Url::parse(t).ok())
    })
}

async fn fetch_page(
    state: &AppState,
    url: url::Url,
    etag: Option<&str>,
) -> Result<Page, CheckError> {
    let host = host_of(&url);
    let client = HTTP.as_ref().ok_or(CheckError::Client)?;
    let mut req = client
        .get(url)
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
        return Ok(Page::NotModified);
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
    let next = next_page(resp.headers());
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
    Ok(Page::Releases {
        etag,
        next,
        releases,
    })
}

async fn fetch_list(state: &AppState, etag: Option<&str>) -> Result<Fetched, CheckError> {
    let mut url = state.config.updates.releases_url.clone();
    url.query_pairs_mut().append_pair("per_page", PER_PAGE);
    let origin = url.origin();
    let (first_etag, mut next, mut all) = match fetch_page(state, url, etag).await? {
        Page::NotModified => return Ok(Fetched::NotModified),
        Page::Releases {
            etag,
            next,
            releases,
        } => (etag, next, releases),
    };
    let mut pages = 1;
    while let Some(url) = next.take() {
        if pages >= MAX_PAGES || url.origin() != origin {
            break;
        }
        match fetch_page(state, url, None).await? {
            Page::NotModified => break,
            Page::Releases {
                next: more,
                releases,
                ..
            } => {
                all.extend(releases);
                next = more;
                pages += 1;
            }
        }
    }
    Ok(Fetched::Releases {
        etag: first_etag,
        releases: all,
    })
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
    Ok(state.config.updates.check
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

pub async fn dispatch(
    state: &AppState,
    client: &Updater<'_>,
    job: &JobRow,
) -> Result<Result<(), UpdaterError>, StoreError> {
    let request = StartRequest {
        id: UpdateId::from_uuid(job.id).to_string(),
        version: job.to_version.clone(),
    };
    match client.start(&request).await {
        Ok(_) => Ok(Ok(())),
        Err(e) if e.certain() => {
            let detail = e.to_string();
            finish(
                state,
                job,
                &Outcome {
                    state: JobState::Failed,
                    step: &job.step,
                    reason: Some(if e.refused() {
                        Reason::UpdaterRefused
                    } else {
                        Reason::UpdaterUnreachable
                    }),
                    detail: Some(&detail),
                },
            )
            .await?;
            Ok(Err(e))
        }
        Err(e) => {
            tracing::warn!(error = %e, "the updater did not confirm the update; its progress tells");
            Ok(Ok(()))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auto {
    Off,
    NotDue,
    UpToDate,
    Manual,
    CheckFailed,
    UpdaterNotReady,
    Busy,
    Started(String),
    Failed(String),
}

impl Auto {
    pub fn summary(&self) -> String {
        match self {
            Auto::Off => "automatic updates off".to_owned(),
            Auto::NotDue => "no update window".to_owned(),
            Auto::UpToDate => "update window: up to date".to_owned(),
            Auto::Manual => "update window: the release needs steps by hand".to_owned(),
            Auto::CheckFailed => "update window: releases couldn't be fetched".to_owned(),
            Auto::UpdaterNotReady => "update window: the updater isn't ready".to_owned(),
            Auto::Busy => "update window: an update already runs".to_owned(),
            Auto::Started(version) => format!("update window: updating to {version}"),
            Auto::Failed(version) => format!("update window: the update to {version} didn't start"),
        }
    }
}

pub async fn auto_update(state: &AppState) -> Result<Auto, StoreError> {
    let settings = updates::settings(&state.pg).await?;
    if mode(&settings) != Mode::Automatic {
        return Ok(Auto::Off);
    }
    let now = state.clock.now();
    let timezone = kaeso_store::pg::instance::time_zone(&state.pg).await?;
    let Some(start) = schedule(&settings, &timezone).window(now) else {
        return Ok(Auto::NotDue);
    };
    if !updates::claim_window(&state.pg, start).await? {
        return Ok(Auto::NotDue);
    }
    if settings
        .last_success_at
        .is_none_or(|at| now - at > FRESH_FOR_AUTO)
        && let Checked::Failed(_) = check(state).await?
    {
        return Ok(Auto::CheckFailed);
    }
    let settings = updates::settings(&state.pg).await?;
    let releases = known(updates::releases(&state.pg).await?);
    let Some(plan) = plan(&settings, &releases) else {
        return Ok(Auto::UpToDate);
    };
    if plan.manual.is_some() {
        return Ok(Auto::Manual);
    }
    let Some(client) = updater::of(state) else {
        return Ok(Auto::UpdaterNotReady);
    };
    let required = plan
        .target
        .manifest
        .as_ref()
        .map_or(0, |m| m.updater_protocol);
    match client.status().await {
        Ok(status) if updater::speaks(&status, required) && status.docker => {}
        _ => return Ok(Auto::UpdaterNotReady),
    }
    let to = plan.target.version.to_string();
    let from = running().to_string();
    let id = UpdateId::new();
    let mut tx = state.pg.begin().await?;
    let started = updates::start_job(
        &mut *tx,
        &NewJob {
            id: id.uuid(),
            from_version: &from,
            to_version: &to,
            started_by: None,
            automatic: true,
            started_at: now,
        },
    )
    .await?;
    if !started {
        return Ok(Auto::Busy);
    }
    crate::platform::audit::system_event(
        &mut tx,
        None,
        "instance.update_start",
        Some(("update", id.to_string())),
        Some(json!({ "version": from })),
        Some(json!({ "version": to, "automatic": true })),
        now,
    )
    .await?;
    tx.commit().await?;
    let job = updates::job(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| StoreError::Corrupt("the update row vanished".into()))?;
    tracing::info!(%from, %to, "automatic update started");
    Ok(match dispatch(state, &client, &job).await? {
        Ok(()) => Auto::Started(to),
        Err(_) => Auto::Failed(to),
    })
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
            updates::seen(&state.pg, job.id, remote.step.as_str(), state.clock.now()).await?;
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
        Err(e)
            if state.clock.now() - job.last_contact_at.unwrap_or(job.started_at) > LOST_AFTER =>
        {
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
        let feed = url::Url::parse("https://update.kaeso.app/v1/releases.json").unwrap();
        assert_eq!(downloads_of(&feed), "https://update.kaeso.app/");
    }

    #[test]
    fn the_next_page_comes_from_githubs_link_header() {
        let mut h = HeaderMap::new();
        assert_eq!(next_page(&h), None);
        h.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/repositories/1/releases?per_page=50&page=2>; rel="next", <https://api.github.com/repositories/1/releases?per_page=50&page=4>; rel="last""#,
            ),
        );
        assert_eq!(
            next_page(&h).unwrap().as_str(),
            "https://api.github.com/repositories/1/releases?per_page=50&page=2"
        );
        h.insert(
            LINK,
            HeaderValue::from_static(r#"<https://api.github.com/x?page=1>; rel="prev""#),
        );
        assert_eq!(next_page(&h), None);
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
