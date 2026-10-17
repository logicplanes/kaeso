use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{UpdateId, UserId};
use kaeso_store::pg::updates::{self, Choices, JobRow, NewJob};
use kaeso_update::UPGRADE_GUIDE;
use kaeso_update::policy::{self, Channel};
use kaeso_update::protocol::{JobState, Reason, StartRequest};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::views::display_name;
use crate::services::updater::{self, UpdaterError};
use crate::services::updates::{self as service, Known, Outcome, UpdaterState};
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

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A Kaeso release newer than the running version.")]
pub struct ReleaseView {
    #[doc = "The version."]
    #[schema(example = "0.2.0")]
    pub version: String,
    #[doc = "When it was published."]
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
    #[doc = "Whether it is a pre-release."]
    pub prerelease: bool,
    #[doc = "Whether it is a new major version, which is never applied in one click."]
    pub major: bool,
    #[doc = "Whether its release notes ask for steps by hand."]
    pub manual_steps: bool,
    #[doc = "The release notes, in Markdown."]
    pub notes: String,
    #[doc = "The release's page."]
    #[schema(example = "https://github.com/logicplanes/kaeso/releases/tag/v0.2.0")]
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The update settings.")]
pub struct UpdateSettingsView {
    #[doc = "Whether the gateway looks for new releases twice a day."]
    pub check: bool,
    #[doc = "Which releases are offered."]
    pub channel: UpdateChannel,
    #[doc = "Whether instance admins may apply updates in one click through the updater."]
    pub apply: bool,
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
    #[schema(example = "0.2.0")]
    pub from: String,
    #[doc = "The version it updates to."]
    #[schema(example = "0.2.1")]
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
    #[doc = "Who started it; `null` once that user is deleted."]
    pub started_by: Option<UpdateActor>,
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
    #[doc = "Why the newest release can't be applied in one click: `major` (a new major version), `manual_steps` (a release on the way asks for steps by hand) or `not_prepared` (it has no release manifest); `null` when it can."]
    pub manual: Option<&'static str>,
    #[doc = "The newest offered release, when it is newer than the running version."]
    pub latest: Option<ReleaseView>,
    #[doc = "Every offered release newer than the running version, newest first."]
    pub releases: Vec<ReleaseView>,
    #[doc = "Whether the newest release can be applied now: one-click updates are on, nothing asks for steps by hand, the updater is ready and no update runs."]
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
    let busy = job
        .as_ref()
        .is_some_and(|j| j.state == JobState::Running.as_str());
    let manual = plan.as_ref().and_then(|p| p.manual);
    let can_apply = plan.is_some()
        && manual.is_none()
        && settings.apply_enabled
        && probe.state == UpdaterState::Ready
        && !busy;
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
            check: settings.check_enabled,
            channel: service::channel(&settings).into(),
            apply: settings.apply_enabled,
        },
        check: UpdateCheckView {
            last_checked_at: settings.last_checked_at,
            last_success_at: settings.last_success_at,
            next_check_at: settings.next_check_at.filter(|_| settings.check_enabled),
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
    let checked = service::check(&state).await?;
    let view = overview(&state).await?;
    ctx.audit(None, "instance.update_check")
        .after(json!({
            "found": view.latest.as_ref().map(|l| l.version.clone()),
            "failed": matches!(checked, service::Checked::Failed(_)),
        }))
        .write(&mut *state.pg.acquire().await?)
        .await?;
    Ok(Json(view))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to the update settings.")]
pub struct UpdateSettingsChange {
    #[doc = "Whether the gateway looks for new releases twice a day."]
    pub check: Option<bool>,
    #[doc = "Which releases are offered."]
    pub channel: Option<UpdateChannel>,
    #[doc = "Whether instance admins may apply updates in one click; turning it on needs a recent sign-in."]
    pub apply: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/instance/updates/settings",
    summary = "Changes whether the gateway looks for releases, which ones it offers, and whether updates apply in one click (turning that on needs a recent sign-in).",
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
    if req.apply == Some(true) && !before.apply_enabled {
        ctx.require_recent_sign_in()?;
    }
    let after = Choices {
        check_enabled: req.check.unwrap_or(before.check_enabled),
        channel: req.channel.map_or(before.channel.clone(), |c| {
            Channel::from(c).as_str().to_owned()
        }),
        apply_enabled: req.apply.unwrap_or(before.apply_enabled),
    };
    updates::choose(&mut *tx, &after).await?;
    ctx.audit(None, "instance.update_settings")
        .before(json!({
            "check": before.check_enabled,
            "channel": before.channel,
            "apply": before.apply_enabled,
        }))
        .after(json!({
            "check": after.check_enabled,
            "channel": after.channel,
            "apply": after.apply_enabled,
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
    #[schema(example = "0.2.1")]
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
    if !settings.apply_enabled {
        return Err(ApiError::Conflict(
            "One-click updates are off; turn them on in the update settings first.".into(),
        ));
    }
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
            started_by: user.uuid(),
            started_at: ctx.now,
        },
    )
    .await?;
    if !started {
        return Err(ApiError::Conflict(
            "An update is already running; wait for it to finish.".into(),
        ));
    }
    ctx.audit(None, "instance.update_start")
        .target("update", id)
        .before(json!({ "version": from }))
        .after(json!({ "version": to }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let request = StartRequest {
        id: id.to_string(),
        version: to.clone(),
    };
    match client.start(&request).await {
        Ok(_) => {}
        Err(e) if e.certain() => {
            let job = updates::job(&state.pg, id.uuid())
                .await?
                .ok_or_else(|| ApiError::internal("the update row vanished"))?;
            let detail = e.to_string();
            service::finish(
                &state,
                &job,
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
        Err(e) => {
            tracing::warn!(error = %e, "the updater did not confirm the update; its progress tells")
        }
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
