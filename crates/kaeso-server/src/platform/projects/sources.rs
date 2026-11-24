use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, ProjectId, ServiceAccountId, SourceId, UserId};
use kaeso_core::sources::{KEY_PREFIX, SourceKind};
use kaeso_store::pg::sources::{self as store, Activity, NewSource, Source};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::{Date, Duration, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query, id_param};
use crate::platform::validate::validate_name;
use crate::state::AppState;

/// Days of activity a source's view carries, today included.
pub const ACTIVITY_DAYS: i64 = 14;
/// A source that reported within this long counts as connected.
const CONNECTED_WITHIN: Duration = Duration::hours(24);

pub const SOURCE_ARCHIVED: &str = "The source is archived.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(
    description = "The client a source reports from, which decides how Kaeso reads what it sends."
)]
pub enum SourceKindView {
    ClaudeCode,
    Codex,
    GeminiCli,
    Antigravity,
    #[serde(rename = "opentelemetry")]
    OpenTelemetry,
    Http,
    Litellm,
    Openrouter,
}

impl From<SourceKind> for SourceKindView {
    fn from(k: SourceKind) -> Self {
        match k {
            SourceKind::ClaudeCode => SourceKindView::ClaudeCode,
            SourceKind::Codex => SourceKindView::Codex,
            SourceKind::GeminiCli => SourceKindView::GeminiCli,
            SourceKind::Antigravity => SourceKindView::Antigravity,
            SourceKind::OpenTelemetry => SourceKindView::OpenTelemetry,
            SourceKind::Http => SourceKindView::Http,
            SourceKind::Litellm => SourceKindView::Litellm,
            SourceKind::Openrouter => SourceKindView::Openrouter,
        }
    }
}

impl From<SourceKindView> for SourceKind {
    fn from(k: SourceKindView) -> Self {
        match k {
            SourceKindView::ClaudeCode => SourceKind::ClaudeCode,
            SourceKindView::Codex => SourceKind::Codex,
            SourceKindView::GeminiCli => SourceKind::GeminiCli,
            SourceKindView::Antigravity => SourceKind::Antigravity,
            SourceKindView::OpenTelemetry => SourceKind::OpenTelemetry,
            SourceKindView::Http => SourceKind::Http,
            SourceKindView::Litellm => SourceKind::Litellm,
            SourceKindView::Openrouter => SourceKind::Openrouter,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(
    description = "Whether a source reports: `waiting` for its first event, `connected` \
    when it reported within the last day, `idle` when it reported before that, `disabled` or \
    `archived`."
)]
pub enum SourceStatus {
    Waiting,
    Connected,
    Idle,
    Disabled,
    Archived,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What a source reported on one day, UTC.")]
pub struct SourceDay {
    #[doc = "The day."]
    #[schema(value_type = String, format = Date, example = "2026-11-22")]
    pub day: String,
    #[doc = "Requests it reported that Kaeso logged."]
    pub requests: i64,
    #[doc = "Tool calls it reported that Kaeso logged."]
    pub tool_calls: i64,
    #[doc = "Records it sent again, which Kaeso had already logged and left out."]
    pub duplicates: i64,
    #[doc = "Records or batches Kaeso refused, such as malformed ones."]
    pub refused: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "A source: a client that reports the requests and tool calls it makes \
    without the gateway. Kaeso logs and counts them as observed; it never sits in their path and \
    can't refuse them."
)]
pub struct ExternalSourceView {
    #[doc = "Id."]
    pub id: SourceId,
    #[doc = "The project its requests count in."]
    pub project_id: ProjectId,
    #[doc = "The client it reports from."]
    pub kind: SourceKindView,
    #[doc = "Name."]
    #[schema(example = "Claude Code, Ada's laptop")]
    pub name: String,
    #[doc = "The key, with only its start and last four characters."]
    #[schema(example = "sk-kso-src-AbC123…wXyZ")]
    pub key: String,
    #[doc = "The member its requests count for when an event names nobody Kaeso knows."]
    pub user_id: Option<UserId>,
    #[doc = "The service account its requests count for instead of a member."]
    pub service_account_id: Option<ServiceAccountId>,
    #[doc = "Whether an event's email address counts the request for the organization member \
        with that address."]
    pub match_members: bool,
    #[doc = "Whether prompts, replies and tool arguments the source sends are kept, as far as \
        the organization and the project keep content."]
    pub content: bool,
    #[doc = "Whether it reports."]
    pub status: SourceStatus,
    #[doc = "Its first event."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub first_event_at: Option<OffsetDateTime>,
    #[doc = "Its latest event."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_event_at: Option<OffsetDateTime>,
    #[doc = "When Kaeso last refused something it sent."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_error_at: Option<OffsetDateTime>,
    #[doc = "Why, as a code: `no_model`, `no_tool` or `no_id` for a record without what its \
        format needs, `too_many_tokens`, `cost_too_high` or `bad_status` for numbers no request \
        has, `too_old` (more than 7 days) or `in_future` for its date, `nobody` when the source \
        counts for no one, `unreadable` for a body Kaeso can't read, `unrecognized` for a batch \
        with no record Kaeso reads, `inactive` for a disabled source."]
    pub last_error: Option<String>,
    #[doc = "The last 14 days, oldest first; days without events are left out."]
    pub activity: Vec<SourceDay>,
    #[doc = "Where it sends its events: the OpenTelemetry endpoint is this URL with `/otlp` \
        added, the JSON endpoints this URL with `/v1/…`."]
    #[schema(example = "https://kaeso.example.com/ingest")]
    pub ingest_url: String,
    #[doc = "Who added it."]
    pub created_by: Option<UserId>,
    #[doc = "When disabled."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub disabled_at: Option<OffsetDateTime>,
    #[doc = "When archived."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A source with its key, shown this once.")]
pub struct SourceCreated {
    #[doc = "The source."]
    pub source: ExternalSourceView,
    #[doc = "The key the client sends its events with. Kaeso keeps only its hash."]
    #[schema(example = "sk-kso-src-AbC123dEf456gHi789jKl012mNo345pQr678sTu901vWxY")]
    pub key: String,
}

pub fn ingest_url(state: &AppState) -> String {
    format!(
        "{}/ingest",
        state.config.public_url.as_str().trim_end_matches('/')
    )
}

pub fn status(s: &Source, now: OffsetDateTime) -> SourceStatus {
    if s.archived_at.is_some() {
        SourceStatus::Archived
    } else if s.disabled_at.is_some() {
        SourceStatus::Disabled
    } else {
        match s.last_event_at {
            None => SourceStatus::Waiting,
            Some(at) if now - at <= CONNECTED_WITHIN => SourceStatus::Connected,
            Some(_) => SourceStatus::Idle,
        }
    }
}

fn day(d: Date) -> String {
    let (y, m, d) = (d.year(), u8::from(d.month()), d.day());
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn view(
    state: &AppState,
    s: &Source,
    days: &[store::Day],
    now: OffsetDateTime,
) -> ExternalSourceView {
    ExternalSourceView {
        id: SourceId::from_uuid(s.id),
        project_id: ProjectId::from_uuid(s.project_id),
        kind: s
            .kind
            .parse::<SourceKind>()
            .map_or(SourceKindView::Http, SourceKindView::from),
        name: s.name.clone(),
        key: format!("{KEY_PREFIX}{}…{}", s.secret_prefix, s.secret_last4),
        user_id: s.user_id.map(UserId::from_uuid),
        service_account_id: s.service_account_id.map(ServiceAccountId::from_uuid),
        match_members: s.match_members,
        content: s.content,
        status: status(s, now),
        first_event_at: s.first_event_at,
        last_event_at: s.last_event_at,
        last_error_at: s.last_error_at,
        last_error: s.last_error.clone(),
        activity: days
            .iter()
            .filter(|d| d.source_id == s.id)
            .map(|d| SourceDay {
                day: day(d.day),
                requests: d.activity.requests,
                tool_calls: d.activity.tool_calls,
                duplicates: d.activity.duplicates,
                refused: d.activity.refused,
            })
            .collect(),
        ingest_url: ingest_url(state),
        created_by: s.created_by.map(UserId::from_uuid),
        disabled_at: s.disabled_at,
        archived_at: s.archived_at,
        created_at: s.created_at,
    }
}

pub async fn views(
    state: &AppState,
    rows: &[Source],
    now: OffsetDateTime,
) -> ApiResult<Vec<ExternalSourceView>> {
    let ids: Vec<Uuid> = rows.iter().map(|s| s.id).collect();
    let from = now.date() - Duration::days(ACTIVITY_DAYS - 1);
    let days = store::activity(&state.pg, &ids, from).await?;
    Ok(rows.iter().map(|s| view(state, s, &days, now)).collect())
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SourceFilter {
    #[doc = "Include archived sources (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/sources",
    summary = "The project's sources, newest first.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams, SourceFilter),
    responses((status = 200, body = List<ExternalSourceView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
    Query(f): Query<SourceFilter>,
) -> ApiResult<Json<List<ExternalSourceView>>> {
    let page = page.page()?;
    let rows = store::list(
        &state.pg,
        ctx.project()?.project.id,
        f.include_archived.unwrap_or(false),
        page.after,
        page.fetch(),
    )
    .await?;
    let list = List::from_rows(&page, rows, |s| s.id, |s| s);
    Ok(Json(List {
        data: views(&state, &list.data, ctx.now).await?,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new source.")]
pub struct SourceCreate {
    #[doc = "The client it reports from."]
    pub kind: SourceKindView,
    #[doc = "A name to recognise it by, 1 to 100 characters."]
    #[schema(example = "Claude Code, Ada's laptop")]
    pub name: String,
    #[doc = "The member its requests count for when an event names nobody Kaeso knows; the \
        caller when neither this nor `service_account_id` is given."]
    pub user_id: Option<UserId>,
    #[doc = "A service account of the project to count them for instead."]
    pub service_account_id: Option<ServiceAccountId>,
    #[doc = "Whether an event's email address counts the request for the organization member \
        with that address; default `false`."]
    pub match_members: Option<bool>,
    #[doc = "Whether to keep prompts, replies and tool arguments the source sends; default \
        `false`."]
    pub content: Option<bool>,
    #[serde(flatten)]
    #[schema(ignore)]
    pub other: crate::platform::json::NoOtherFields,
}

/// The member or service account a source's requests count for, checked against the project.
async fn attribution(
    state: &AppState,
    db: &mut PgConnection,
    ctx: &Ctx,
    user: Option<UserId>,
    service_account: Option<ServiceAccountId>,
) -> ApiResult<(Option<Uuid>, Option<Uuid>)> {
    match (user, service_account) {
        (Some(_), Some(_)) => Err(ApiError::invalid(
            "user_id",
            "Choose a member or a service account, not both.",
        )),
        (None, Some(sa)) => {
            let found = kaeso_store::pg::service_accounts::get(
                &mut *db,
                ctx.project()?.project.id,
                sa.uuid(),
            )
            .await?;
            match found {
                Some(s) if s.archived_at.is_none() => Ok((None, Some(s.id))),
                _ => Err(ApiError::invalid(
                    "service_account_id",
                    "No active service account of this project has that id.",
                )),
            }
        }
        (Some(u), None) => {
            let sees = crate::services::observed::sees_project(
                state,
                ctx.org()?.org.id,
                ctx.project()?.project.id,
                u.uuid(),
            )
            .await?;
            if !sees {
                return Err(ApiError::invalid(
                    "user_id",
                    "No member who can see this project has that id.",
                ));
            }
            Ok((Some(u.uuid()), None))
        }
        (None, None) => match ctx.acting_user() {
            Some(u) => Ok((Some(u.uuid()), None)),
            None => Err(ApiError::invalid(
                "user_id",
                "Choose the member or service account the source counts for.",
            )),
        },
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/sources",
    summary = "Adds a source and returns its key, shown this once.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = SourceCreate,
    responses((status = 201, body = SourceCreated))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<SourceCreate>,
) -> ApiResult<(StatusCode, Json<SourceCreated>)> {
    let project = ensure_active(&ctx, ARCHIVED)?;
    let name = validate_name("name", &req.name)?;
    let org = ctx.org()?.org.id;
    let kind = SourceKind::from(req.kind);
    let made = kaeso_core::sources::generate(kaeso_crypto::random_bytes::<64>);
    let id = SourceId::new();
    let mut tx = state.pg.begin().await?;
    let (user_id, service_account_id) =
        attribution(&state, &mut tx, &ctx, req.user_id, req.service_account_id).await?;
    store::insert(
        &mut *tx,
        &NewSource {
            id: id.uuid(),
            org_id: org,
            project_id: project,
            kind: kind.as_str(),
            name: &name,
            secret_hash: &made.hash,
            secret_prefix: &made.prefix,
            secret_last4: &made.last4,
            user_id,
            service_account_id,
            match_members: req.match_members.unwrap_or(false),
            content: req.content.unwrap_or(false),
            created_by: ctx.acting_user().map(|u| u.uuid()),
            at: ctx.now,
        },
    )
    .await?;
    let row = store::get(&mut *tx, project, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("source"))?;
    let source = view(&state, &row, &[], ctx.now);
    ctx.audit(Some(ctx.org()?.id()), Action::SourceCreate)
        .target("source", id)
        .after(&source)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::observed::sources_changed(project).await;
    Ok((
        StatusCode::CREATED,
        Json(SourceCreated {
            source,
            key: made.secret.expose_secret().to_owned(),
        }),
    ))
}

async fn find(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<Source> {
    store::get(
        &state.pg,
        ctx.project()?.project.id,
        id_param::<SourceId>(id, "source")?.uuid(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("source"))
}

async fn lock_active(tx: &mut PgConnection, ctx: &Ctx, id: &str) -> ApiResult<Source> {
    let row = store::get_for_update(
        &mut *tx,
        ctx.project()?.project.id,
        id_param::<SourceId>(id, "source")?.uuid(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("source"))?;
    if row.archived_at.is_some() {
        return Err(ApiError::Conflict(SOURCE_ARCHIVED.into()));
    }
    Ok(row)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/sources/{source}",
    summary = "One source.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("source" = SourceId, Path)),
    responses((status = 200, body = ExternalSourceView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ExternalSourceView>> {
    let row = find(&state, &ctx, &id).await?;
    let mut list = views(&state, std::slice::from_ref(&row), ctx.now).await?;
    list.pop()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("source"))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Changes to a source. A member and a service account replace each other.")]
pub struct SourceUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "The member its requests count for."]
    pub user_id: Option<UserId>,
    #[doc = "The service account its requests count for."]
    pub service_account_id: Option<ServiceAccountId>,
    #[doc = "Whether an event's email address counts the request for that member."]
    pub match_members: Option<bool>,
    #[doc = "Whether to keep the content the source sends."]
    pub content: Option<bool>,
    #[doc = "`true` refuses everything it sends until set back to `false`."]
    pub disabled: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/sources/{source}",
    summary = "Renames a source, changes whom its requests count for and whether it keeps \
    content, or disables it.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("source" = SourceId, Path)),
    request_body = SourceUpdate,
    responses((status = 200, body = ExternalSourceView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    Body(req): Body<SourceUpdate>,
) -> ApiResult<Json<ExternalSourceView>> {
    ensure_active(&ctx, ARCHIVED)?;
    let mut tx = state.pg.begin().await?;
    let before = lock_active(&mut tx, &ctx, &id).await?;
    let mut after = before.clone();
    if let Some(name) = req.name {
        after.name = validate_name("name", &name)?;
    }
    if req.user_id.is_some() || req.service_account_id.is_some() {
        let (user, sa) =
            attribution(&state, &mut tx, &ctx, req.user_id, req.service_account_id).await?;
        after.user_id = user;
        after.service_account_id = sa;
    }
    if let Some(m) = req.match_members {
        after.match_members = m;
    }
    if let Some(c) = req.content {
        after.content = c;
    }
    if let Some(d) = req.disabled {
        after.disabled_at = match (d, before.disabled_at) {
            (true, Some(at)) => Some(at),
            (true, None) => Some(ctx.now),
            (false, _) => None,
        };
    }
    after.updated_at = ctx.now;
    store::update(&mut *tx, &after).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::SourceUpdate)
        .target("source", SourceId::from_uuid(after.id))
        .before(view(&state, &before, &[], ctx.now))
        .after(view(&state, &after, &[], ctx.now))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::observed::sources_changed(after.project_id).await;
    let mut list = views(&state, std::slice::from_ref(&after), ctx.now).await?;
    list.pop()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("source"))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/sources/{source}/rotate",
    summary = "Replaces a source's key; the old key stops working at once. The new key is shown \
    this once.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("source" = SourceId, Path)),
    responses((status = 200, body = SourceCreated))
)]
pub async fn rotate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<SourceCreated>> {
    let project = ensure_active(&ctx, ARCHIVED)?;
    let made = kaeso_core::sources::generate(kaeso_crypto::random_bytes::<64>);
    let mut tx = state.pg.begin().await?;
    let mut row = lock_active(&mut tx, &ctx, &id).await?;
    store::set_secret(
        &mut *tx,
        project,
        row.id,
        &made.hash,
        &made.prefix,
        &made.last4,
        ctx.now,
    )
    .await?;
    row.secret_prefix = made.prefix.clone();
    row.secret_last4 = made.last4.clone();
    row.updated_at = ctx.now;
    ctx.audit(Some(ctx.org()?.id()), Action::SourceRotate)
        .target("source", SourceId::from_uuid(row.id))
        .after(serde_json::json!({ "last4": made.last4 }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let mut list = views(&state, std::slice::from_ref(&row), ctx.now).await?;
    let source = list.pop().ok_or_else(|| ApiError::not_found("source"))?;
    Ok(Json(SourceCreated {
        source,
        key: made.secret.expose_secret().to_owned(),
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/sources/{source}/archive",
    summary = "Archives a source: its key stops working and it can't be changed again. What it \
    reported stays in the logs and the usage. Permanent.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("source" = SourceId, Path)),
    responses((status = 200, body = ExternalSourceView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ExternalSourceView>> {
    ensure_active(&ctx, ARCHIVED)?;
    let mut tx = state.pg.begin().await?;
    let mut row = lock_active(&mut tx, &ctx, &id).await?;
    row.archived_at = Some(ctx.now);
    row.updated_at = ctx.now;
    store::update(&mut *tx, &row).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::SourceArchive)
        .target("source", SourceId::from_uuid(row.id))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let mut list = views(&state, std::slice::from_ref(&row), ctx.now).await?;
    list.pop()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("source"))
}

/// Records activity of a day for the source, refused records included.
pub async fn add_activity(state: &AppState, source: Uuid, at: OffsetDateTime, a: Activity) {
    if a == Activity::default() {
        return;
    }
    if let Err(e) = store::add_activity(&state.pg, source, at.date(), a).await {
        tracing::warn!(error = %e, "cannot count a source's activity");
    }
}
