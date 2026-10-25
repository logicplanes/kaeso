use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{UpdateId, UserId};
use kaeso_store::pg::updates::{self, Choices, JobRow, NewJob, Window};
use kaeso_update::UPGRADE_GUIDE;
use kaeso_update::policy::{self, Channel};
use kaeso_update::protocol::JobState;
use kaeso_update::schedule::{Frequency, Mode, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::views::display_name;
use crate::services::updater::{self, UpdaterError};
use crate::services::updates::{self as service, Known, UpdaterState};
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Which releases are offered.")]
pub enum UpdateChannel {
    #[doc = "Stable releases only."]
    Stable,
    #[doc = "Pre-releases too."]
    Prerelease,
}

impl From<Channel> for UpdateChannel {
    fn from(c: Channel) -> Self {
        match c {
            Channel::Stable => UpdateChannel::Stable,
            Channel::Prerelease => UpdateChannel::Prerelease,
        }
    }
}

impl From<UpdateChannel> for Channel {
    fn from(c: UpdateChannel) -> Self {
        match c {
            UpdateChannel::Stable => Channel::Stable,
            UpdateChannel::Prerelease => Channel::Prerelease,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How the install takes updates through the updater.")]
pub enum UpdateMode {
    #[doc = "An instance admin starts each update in the console."]
    Manual,
    #[doc = "The gateway installs new releases by itself in the update window."]
    Automatic,
}

impl From<Mode> for UpdateMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Manual => UpdateMode::Manual,
            Mode::Automatic => UpdateMode::Automatic,
        }
    }
}

impl From<UpdateMode> for Mode {
    fn from(m: UpdateMode) -> Self {
        match m {
            UpdateMode::Manual => Mode::Manual,
            UpdateMode::Automatic => Mode::Automatic,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How often automatic updates run.")]
pub enum UpdateFrequency {
    #[doc = "Every day."]
    Daily,
    #[doc = "Once a week, on the chosen day."]
    Weekly,
}

impl From<Frequency> for UpdateFrequency {
    fn from(f: Frequency) -> Self {
        match f {
            Frequency::Daily => UpdateFrequency::Daily,
            Frequency::Weekly => UpdateFrequency::Weekly,
        }
    }
}

impl From<UpdateFrequency> for Frequency {
    fn from(f: UpdateFrequency) -> Self {
        match f {
            UpdateFrequency::Daily => Frequency::Daily,
            UpdateFrequency::Weekly => Frequency::Weekly,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "A day of the week.")]
pub enum UpdateWeekday {
    #[doc = "Monday."]
    Monday,
    #[doc = "Tuesday."]
    Tuesday,
    #[doc = "Wednesday."]
    Wednesday,
    #[doc = "Thursday."]
    Thursday,
    #[doc = "Friday."]
    Friday,
    #[doc = "Saturday."]
    Saturday,
    #[doc = "Sunday."]
    Sunday,
}

impl From<Weekday> for UpdateWeekday {
    fn from(d: Weekday) -> Self {
        match d {
            Weekday::Monday => UpdateWeekday::Monday,
            Weekday::Tuesday => UpdateWeekday::Tuesday,
            Weekday::Wednesday => UpdateWeekday::Wednesday,
            Weekday::Thursday => UpdateWeekday::Thursday,
            Weekday::Friday => UpdateWeekday::Friday,
            Weekday::Saturday => UpdateWeekday::Saturday,
            Weekday::Sunday => UpdateWeekday::Sunday,
        }
    }
}

impl From<UpdateWeekday> for Weekday {
    fn from(d: UpdateWeekday) -> Self {
        match d {
            UpdateWeekday::Monday => Weekday::Monday,
            UpdateWeekday::Tuesday => Weekday::Tuesday,
            UpdateWeekday::Wednesday => Weekday::Wednesday,
            UpdateWeekday::Thursday => Weekday::Thursday,
            UpdateWeekday::Friday => Weekday::Friday,
            UpdateWeekday::Saturday => Weekday::Saturday,
            UpdateWeekday::Sunday => Weekday::Sunday,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "When automatic updates run.")]
pub struct UpdateScheduleView {
    #[doc = "Every day or once a week."]
    pub frequency: UpdateFrequency,
    #[doc = "The day of a weekly update."]
    pub weekday: UpdateWeekday,
    #[doc = "The hour the update window opens, 0 to 23, in the instance's time zone; it stays open for an hour."]
    #[schema(example = 3)]
    pub hour: u8,
    #[doc = "The instance's IANA time zone, which the hour follows, daylight saving time included; set in the instance settings."]
    #[schema(example = "Europe/Berlin")]
    pub timezone: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A Kaeso release newer than the running version.")]
pub struct ReleaseView {
    #[doc = "The version."]
    #[schema(example = "40.2.0")]
    pub version: String,
    #[doc = "When it was published."]
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
    #[doc = "Whether it is a pre-release."]
    pub prerelease: bool,
    #[doc = "Whether it is a new major version, which the updater never installs."]
    pub major: bool,
    #[doc = "Whether its release notes ask for steps by hand."]
    pub manual_steps: bool,
    #[doc = "The release notes, in Markdown."]
    pub notes: String,
    #[doc = "The release's page, an `https` URL; empty when the release server gave no `https` link."]
    #[schema(example = "https://github.com/logicplanes/kaeso/releases/tag/v40.2.0")]
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The update settings.")]
pub struct UpdateSettingsView {
    #[doc = "Whether the gateway looks for new releases every 5 minutes; `updates.check` in the configuration turns it off for installs without internet."]
    pub check: bool,
    #[doc = "Which releases are offered."]
    pub channel: UpdateChannel,
    #[doc = "`manual` (an instance admin starts each update) or `automatic` (the gateway installs new releases in the update window)."]
    pub mode: UpdateMode,
    #[doc = "When automatic updates run."]
    pub schedule: UpdateScheduleView,
    #[doc = "When the next update window opens, while automatic updates are on."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_auto_update_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The last look for new releases.")]
pub struct UpdateCheckView {
    #[doc = "When the gateway last looked."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_checked_at: Option<OffsetDateTime>,
    #[doc = "When the gateway last got the list of releases."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_success_at: Option<OffsetDateTime>,
    #[doc = "When it looks next, while checking is on."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_check_at: Option<OffsetDateTime>,
    #[doc = "Why the last look failed, if it did."]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The updater, the optional service that applies updates.")]
pub struct UpdaterView {
    #[doc = "`not_installed` (no updater token is set), `unreachable` (it does not answer, or cannot reach Docker), `outdated` (too old for the newest release) or `ready`."]
    #[schema(example = "ready")]
    pub state: &'static str,
    #[doc = "Its version, when it answers."]
    pub version: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Who started an update.")]
pub struct UpdateActor {
    #[doc = "The user's id."]
    pub id: UserId,
    #[doc = "The user's name."]
    #[schema(example = "Ada Lovelace")]
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An update of the install from one version to another.")]
pub struct UpdateJobView {
    #[doc = "The update's id."]
    pub id: UpdateId,
    #[doc = "The version it updates from."]
    #[schema(example = "40.2.0")]
    pub from: String,
    #[doc = "The version it updates to."]
    #[schema(example = "40.2.1")]
    pub to: String,
    #[doc = "`running`, `succeeded` (with a `reason` when compose.yaml still needs the new version by hand), `failed` (nothing changed), `rolled_back` (the previous version runs again), `rollback_failed` (the previous version did not start again) or `needs_attention` (the new gateway may have migrated the database and did not start; someone has to look)."]
    #[schema(example = "running")]
    pub state: String,
    #[doc = "The step it is on or ended on: `verifying`, `downloading`, `switching`, `migrating` or `online`."]
    #[schema(example = "verifying")]
    pub step: String,
    #[doc = "Why it did not succeed, or what is left to do after it did, as a code: `manifest_unavailable`, `manifest_invalid`, `manual_steps`, `major`, `not_newer`, `updater_outdated`, `compose_changed`, `compose_unwritable`, `services_not_found`, `pull_failed`, `unverified`, `docker_unavailable`, `unhealthy`, `interrupted`, `updater_unreachable`, `updater_refused`, `state_unwritable` or `migrated` (the new gateway didn't start and may have migrated the database)."]
    pub reason: Option<String>,
    #[doc = "The updater's account of what went wrong, in English."]
    pub detail: Option<String>,
    #[doc = "Who started it; `null` when the gateway started it in the update window or once that user is deleted."]
    pub started_by: Option<UpdateActor>,
    #[doc = "Whether the gateway started it by itself in the update window."]
    pub automatic: bool,
    #[doc = "When it started."]
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[doc = "When it ended; `null` while it runs."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

impl UpdateJobView {
    fn of(row: JobRow) -> UpdateJobView {
        UpdateJobView {
            id: UpdateId::from_uuid(row.id),
            from: row.from_version,
            to: row.to_version,
            state: row.state,
            step: row.step,
            reason: row.reason,
            detail: row.detail,
            started_by: row.started_by.map(|id| UpdateActor {
                id: UserId::from_uuid(id),
                name: display_name(row.started_by_name.as_deref()),
            }),
            automatic: row.automatic,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "Updates of the install: the running version, newer releases, the settings, the updater and the last update."
)]
pub struct UpdatesView {
    #[doc = "The version the gateway runs."]
    #[schema(example = json!(env!("CARGO_PKG_VERSION")))]
    pub current: String,
    #[doc = "`unknown` until the gateway first gets the list of releases, `up_to_date`, or `available` when a newer release is offered."]
    #[schema(example = "available")]
    pub state: &'static str,
    #[doc = "Why the updater can't install the newest release: `major` (a new major version), `manual_steps` (a release on the way asks for steps by hand) or `not_prepared` (it has no release manifest); `null` when it can."]
    pub manual: Option<&'static str>,
    #[doc = "The newest offered release, when it is newer than the running version."]
    pub latest: Option<ReleaseView>,
    #[doc = "Every offered release newer than the running version, newest first."]
    pub releases: Vec<ReleaseView>,
    #[doc = "Whether the newest release can be applied now: nothing asks for steps by hand, the updater is ready and no update runs."]
    pub can_apply: bool,
    #[doc = "The settings."]
    pub settings: UpdateSettingsView,
    #[doc = "The last look for releases."]
    pub check: UpdateCheckView,
    #[doc = "The updater."]
    pub updater: UpdaterView,
    #[doc = "The running update, or else the last one."]
    pub job: Option<UpdateJobView>,
    #[doc = "The guide to updating by hand."]
    #[schema(example = "https://docs.kaeso.app/self-hosting/upgrades")]
    pub upgrade_guide: &'static str,
}

fn release_view(running: &kaeso_update::Version, k: &Known) -> ReleaseView {
    ReleaseView {
        version: k.row.version.clone(),
        published_at: k.row.published_at,
        prerelease: k.row.prerelease,
        major: policy::is_major(running, &k.version),
        manual_steps: k.manifest.as_ref().is_some_and(|m| m.manual),
        notes: k.row.notes.clone(),
        url: k.row.url.clone(),
    }
}

async fn overview(state: &AppState) -> ApiResult<UpdatesView> {
    let running = service::running();
    let settings = updates::settings(&state.pg).await?;
    let releases = service::known(updates::releases(&state.pg).await?);
    let plan = service::plan(&settings, &releases);
    let required = plan
        .as_ref()
        .and_then(|p| p.target.manifest.as_ref())
        .map(|m| m.updater_protocol);
    let probe = service::probe(state, required).await;
    let job = match updates::running_job(&state.pg).await? {
        Some(running_job) => Some(service::reconcile(state, running_job).await?),
        None => updates::latest_job(&state.pg).await?,
    };
    let timezone = kaeso_store::pg::instance::time_zone(&state.pg).await?;
    let plan_of_auto = service::schedule(&settings, &timezone);
    let busy = job
        .as_ref()
        .is_some_and(|j| j.state == JobState::Running.as_str());
    let manual = plan.as_ref().and_then(|p| p.manual);
    let can_apply =
        plan.is_some() && manual.is_none() && probe.state == UpdaterState::Ready && !busy;
    Ok(UpdatesView {
        current: running.to_string(),
        state: match (&plan, settings.last_success_at) {
            (Some(_), _) => "available",
            (None, Some(_)) => "up_to_date",
            (None, None) => "unknown",
        },
        manual: manual.map(policy::Manual::as_str),
        latest: plan.as_ref().map(|p| release_view(&running, p.target)),
        releases: plan
            .as_ref()
            .map(|p| p.newer.iter().map(|k| release_view(&running, k)).collect())
            .unwrap_or_default(),
        can_apply,
        settings: UpdateSettingsView {
            check: state.config.updates.check,
            channel: service::channel(&settings).into(),
            mode: service::mode(&settings).into(),
            schedule: UpdateScheduleView {
                frequency: plan_of_auto.frequency.into(),
                weekday: plan_of_auto.weekday.into(),
                hour: plan_of_auto.hour,
                timezone: timezone.clone(),
            },
            next_auto_update_at: service::next_auto_update(&settings, &timezone, state.clock.now()),
        },
        check: UpdateCheckView {
            last_checked_at: settings.last_checked_at,
            last_success_at: settings.last_success_at,
            next_check_at: service::next_check(&settings).filter(|_| state.config.updates.check),
            error: settings.last_error,
        },
        updater: UpdaterView {
            state: probe.state.as_str(),
            version: probe.version,
        },
        job: job.map(UpdateJobView::of),
        upgrade_guide: UPGRADE_GUIDE,
    })
}

#[utoipa::path(
    get,
    path = "/instance/updates",
    summary = "The running version, newer releases with their notes, the update settings, the updater and the last update.",
    tag = "instance",
    responses((status = 200, body = UpdatesView))
)]
pub async fn get(State(state): State<AppState>, _ctx: Ctx) -> ApiResult<Json<UpdatesView>> {
    Ok(Json(overview(&state).await?))
}

#[utoipa::path(
    post,
    path = "/instance/updates/check",
    summary = "Looks for new releases now, even when checking is off.",
    tag = "instance",
    responses((status = 200, body = UpdatesView))
)]
pub async fn check(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<UpdatesView>> {
    let checked = service::check(&state, service::Trigger::Admin).await?;
    let view = overview(&state).await?;
    ctx.audit(None, Action::InstanceUpdateCheck)
        .after(json!({
            "found": view.latest.as_ref().map(|l| l.version.clone()),
            "failed": matches!(checked, service::Checked::Failed(_)),
        }))
        .write(&mut *state.pg.acquire().await?)
        .await?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "When automatic updates run.")]
pub struct UpdateSchedule {
    #[doc = "Every day or once a week."]
    pub frequency: UpdateFrequency,
    #[doc = "The day of a weekly update; ignored for a daily one."]
    pub weekday: UpdateWeekday,
    #[doc = "The hour the update window opens, 0 to 23, in the instance's time zone."]
    #[schema(example = 3, maximum = 23)]
    pub hour: u8,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to the update settings.")]
pub struct UpdateSettingsChange {
    #[doc = "Which releases are offered."]
    pub channel: Option<UpdateChannel>,
    #[doc = "`manual` or `automatic`; choosing `automatic` needs a recent sign-in."]
    pub mode: Option<UpdateMode>,
    #[doc = "When automatic updates run."]
    pub schedule: Option<UpdateSchedule>,
}

fn window(
    before: &kaeso_store::pg::updates::Settings,
    req: &UpdateSettingsChange,
) -> ApiResult<Window> {
    let Some(s) = &req.schedule else {
        return Ok(Window {
            frequency: before.auto_frequency.clone(),
            weekday: before.auto_weekday,
            hour: before.auto_hour,
        });
    };
    if s.hour > 23 {
        return Err(ApiError::invalid(
            "schedule.hour",
            "Choose an hour from 0 to 23.",
        ));
    }
    Ok(Window {
        frequency: Frequency::from(s.frequency).as_str().to_owned(),
        weekday: Weekday::from(s.weekday).number(),
        hour: i16::from(s.hour),
    })
}

fn window_audit(window: &Window) -> serde_json::Value {
    json!({
        "frequency": window.frequency,
        "weekday": window.weekday,
        "hour": window.hour,
    })
}

#[utoipa::path(
    patch,
    path = "/instance/updates/settings",
    summary = "Changes which releases the gateway offers, and whether updates install by hand or in the update window (choosing that needs a recent sign-in).",
    tag = "instance",
    request_body = UpdateSettingsChange,
    responses((status = 200, body = UpdatesView))
)]
pub async fn settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<UpdateSettingsChange>,
) -> ApiResult<Json<UpdatesView>> {
    let mut tx = state.pg.begin().await?;
    let before = updates::settings_for_update(&mut tx).await?;
    if req.mode == Some(UpdateMode::Automatic) && before.mode != Mode::Automatic.as_str() {
        ctx.require_recent_sign_in()?;
    }
    let after = Choices {
        channel: req.channel.map_or(before.channel.clone(), |c| {
            Channel::from(c).as_str().to_owned()
        }),
        mode: req
            .mode
            .map_or(before.mode.clone(), |m| Mode::from(m).as_str().to_owned()),
    };
    let window_before = Window {
        frequency: before.auto_frequency.clone(),
        weekday: before.auto_weekday,
        hour: before.auto_hour,
    };
    let window_after = window(&before, &req)?;
    updates::choose(&mut *tx, &after, &window_after, ctx.now).await?;
    ctx.audit(None, Action::InstanceUpdateSettings)
        .before(json!({
            "channel": before.channel,
            "mode": before.mode,
            "window": window_audit(&window_before),
        }))
        .after(json!({
            "channel": after.channel,
            "mode": after.mode,
            "window": window_audit(&window_after),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(overview(&state).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The release to update to.")]
pub struct ApplyUpdate {
    #[doc = "The newest offered release's version, as the person confirmed it."]
    #[schema(example = "40.2.1")]
    pub version: String,
}

#[utoipa::path(
    post,
    path = "/instance/updates/apply",
    summary = "Updates the install to the newest offered release through the updater, with a recent sign-in; the console and the APIs are unavailable for about a minute.",
    description = "The updater checks the release's signed provenance, downloads the images, switches the console and then the gateway, and waits for the gateway's migrations and health. When the new version fails it goes back to the previous one, unless the new gateway may already have migrated the database: then it leaves the new gateway in place and ends `needs_attention`. Poll `instance/updates/jobs/{update}` for the progress; when the updater doesn't confirm the start, the update stays `running` until the updater reports on it.",
    tag = "instance",
    request_body = ApplyUpdate,
    responses((status = 202, body = UpdateJobView))
)]
pub async fn apply(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ApplyUpdate>,
) -> ApiResult<(StatusCode, Json<UpdateJobView>)> {
    ctx.require_recent_sign_in()?;
    let user = ctx.user_id()?;
    let settings = updates::settings(&state.pg).await?;
    let Some(client) = updater::of(&state) else {
        return Err(ApiError::Conflict(
            "The updater isn't installed; update this install by hand.".into(),
        ));
    };
    let releases = service::known(updates::releases(&state.pg).await?);
    let plan = service::plan(&settings, &releases);
    let Some(plan) = plan.filter(|p| p.target.version.to_string() == req.version.trim()) else {
        return Err(ApiError::invalid(
            "version",
            "That isn't the newest offered release; check for updates again.",
        ));
    };
    if plan.manual.is_some() {
        return Err(ApiError::Conflict(
            "This release needs steps by hand; follow the upgrade guide.".into(),
        ));
    }
    let required = plan
        .target
        .manifest
        .as_ref()
        .map_or(0, |m| m.updater_protocol);
    let status = client.status().await.map_err(updater_unavailable)?;
    if !updater::speaks(&status, required) {
        return Err(ApiError::Conflict(
            "The updater is too old for this release; update it by hand first.".into(),
        ));
    }
    let id = UpdateId::new();
    let from = service::running().to_string();
    let to = plan.target.version.to_string();
    let mut tx = state.pg.begin().await?;
    let started = updates::start_job(
        &mut *tx,
        &NewJob {
            id: id.uuid(),
            from_version: &from,
            to_version: &to,
            started_by: Some(user.uuid()),
            automatic: false,
            started_at: ctx.now,
        },
    )
    .await?;
    if !started {
        return Err(ApiError::Conflict(
            "An update is already running; wait for it to finish.".into(),
        ));
    }
    ctx.audit(None, Action::InstanceUpdateStart)
        .target("update", id)
        .before(json!({ "version": from }))
        .after(json!({ "version": to }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let job = updates::job(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("the update row vanished"))?;
    if let Err(e) = service::dispatch(&state, &client, &job).await? {
        return Err(match e {
            UpdaterError::Busy => ApiError::Conflict(
                "The updater is busy with another update; try again later.".into(),
            ),
            other if other.refused() => ApiError::unavailable(
                "The updater refused the update; nothing was changed.",
                other,
            ),
            other => updater_unavailable(other),
        });
    }
    let job = updates::job(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("the update row vanished"))?;
    Ok((StatusCode::ACCEPTED, Json(UpdateJobView::of(job))))
}

fn updater_unavailable(e: UpdaterError) -> ApiError {
    ApiError::unavailable("The updater couldn't be reached; nothing was changed.", e)
}

#[utoipa::path(
    get,
    path = "/instance/updates/jobs",
    summary = "Past and running updates, newest first.",
    tag = "instance",
    params(PageParams),
    responses((status = 200, body = List<UpdateJobView>))
)]
pub async fn jobs(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<List<UpdateJobView>>> {
    let page = page.page()?;
    service::reconcile_running(&state).await?;
    let rows = updates::jobs(&state.pg, page.after, page.fetch()).await?;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |r| r.id,
        UpdateJobView::of,
    )))
}

#[utoipa::path(
    get,
    path = "/instance/updates/jobs/{update}",
    summary = "One update and its progress.",
    tag = "instance",
    params(("update" = UpdateId, Path)),
    responses((status = 200, body = UpdateJobView))
)]
pub async fn job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<UpdateJobView>> {
    let id: UpdateId = id.parse().map_err(|_| ApiError::not_found("update"))?;
    let row = updates::job(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("update"))?;
    Ok(Json(UpdateJobView::of(
        service::reconcile(&state, row).await?,
    )))
}
