use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::Bytes;
use kaeso_core::id::{ApiKeyId, OrgId, ProjectId, RequestId, ServiceAccountId, UserId};
use kaeso_core::rights::ProjectRight;
use kaeso_store::clickhouse::logs::{self as ch, Filter, LogRow};
use kaeso_store::pg::{instance, log_jobs, orgs, projects};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::access::Ctx;
use super::error::{ApiError, ApiResult};
use super::json::Body as JsonBody;
use super::keys::creator_of;
use super::pagination::List;
use crate::platform::params::{Path, Query};
use crate::services::logs::{cost_from_raw, forget};
use crate::state::AppState;

fn unavailable(e: impl std::fmt::Display) -> ApiError {
    tracing::warn!(error = %e, "ClickHouse request failed");
    ApiError::Unavailable("Request logs are unavailable right now; try again shortly.".into())
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "An org's log settings.")]
pub struct OrgLogSettings {
    #[doc = "Whether request and reply bodies are logged."]
    pub content_logging: bool,
    #[doc = "Days metadata is kept."]
    pub metadata_retention_days: i32,
    #[doc = "Days content is kept (never longer than metadata)."]
    pub content_retention_days: i32,
    #[doc = "The install's maximum for metadata."]
    pub max_metadata_retention_days: i32,
    #[doc = "The install's maximum for content."]
    pub max_content_retention_days: i32,
}

async fn org_settings(state: &AppState, org: &orgs::Org) -> ApiResult<OrgLogSettings> {
    let i = instance::get(&state.pg).await?;
    Ok(OrgLogSettings {
        content_logging: org.content_logging,
        metadata_retention_days: org.metadata_retention_days,
        content_retention_days: org.content_retention_days,
        max_metadata_retention_days: i.max_metadata_retention_days,
        max_content_retention_days: i.max_content_retention_days,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/logs/settings",
    summary = "The org's log retention and content logging.",
    tag = "logs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgLogSettings))
)]
pub async fn get_org_settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<OrgLogSettings>> {
    Ok(Json(org_settings(&state, &ctx.org()?.org).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Log settings changes.")]
pub struct OrgLogSettingsUpdate {
    #[doc = "Log request and reply bodies."]
    pub content_logging: Option<bool>,
    #[doc = "Days to keep metadata."]
    pub metadata_retention_days: Option<i32>,
    #[doc = "Days to keep content."]
    pub content_retention_days: Option<i32>,
}

async fn forget_org(state: &AppState, org: Uuid) -> ApiResult<()> {
    let mut after = None;
    loop {
        let page = projects::list(&state.pg, org, None, true, after, 500).await?;
        for p in &page {
            forget(state, p.id).await;
        }
        match page.last() {
            Some(p) if page.len() == 500 => after = Some(p.id),
            _ => return Ok(()),
        }
    }
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/logs/settings",
    summary = "Changes retention or content logging. A shorter retention removes older\n\
    rows in the background; a longer one applies to new rows.",
    tag = "logs",
    params(("org" = OrgId, Path)),
    request_body = OrgLogSettingsUpdate,
    responses((status = 200, body = OrgLogSettings))
)]
pub async fn update_org_settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    JsonBody(req): JsonBody<OrgLogSettingsUpdate>,
) -> ApiResult<Json<OrgLogSettings>> {
    let limits = instance::get(&state.pg).await?;
    let mut tx = state.pg.begin().await?;
    let before = crate::platform::orgs::org::lock_changeable(&mut tx, &ctx).await?;
    let mut org = before.clone();
    if let Some(c) = req.content_logging {
        org.content_logging = c;
    }
    if let Some(d) = req.metadata_retention_days {
        org.metadata_retention_days = d;
    }
    if let Some(d) = req.content_retention_days {
        org.content_retention_days = d;
    }
    if !(1..=limits.max_metadata_retention_days).contains(&org.metadata_retention_days) {
        return Err(ApiError::invalid(
            "metadata_retention_days",
            format!(
                "Must be 1 to {} days on this install.",
                limits.max_metadata_retention_days
            ),
        ));
    }
    if !(1..=limits.max_content_retention_days).contains(&org.content_retention_days) {
        return Err(ApiError::invalid(
            "content_retention_days",
            format!(
                "Must be 1 to {} days on this install.",
                limits.max_content_retention_days
            ),
        ));
    }
    if org.content_retention_days > org.metadata_retention_days {
        return Err(ApiError::invalid(
            "content_retention_days",
            "Content is never kept longer than metadata.",
        ));
    }
    let now = ctx.now;
    let (kind, by) = creator_of(&ctx);
    orgs::update(&mut *tx, &org).await?;
    let epoch = OffsetDateTime::UNIX_EPOCH;
    if org.metadata_retention_days < before.metadata_retention_days {
        let cut = now - time::Duration::days(i64::from(org.metadata_retention_days));
        log_jobs::enqueue(
            &mut *tx,
            &log_jobs::NewLogJob {
                id: Uuid::now_v7(),
                org_id: org.id,
                project_id: None,
                kind: "shrink_metadata",
                from_at: epoch,
                to_at: cut,
                requested_by_kind: kind,
                requested_by_id: by,
            },
        )
        .await?;
    } else if org.content_retention_days < before.content_retention_days {
        let cut = now - time::Duration::days(i64::from(org.content_retention_days));
        log_jobs::enqueue(
            &mut *tx,
            &log_jobs::NewLogJob {
                id: Uuid::now_v7(),
                org_id: org.id,
                project_id: None,
                kind: "shrink_content",
                from_at: epoch,
                to_at: cut,
                requested_by_kind: kind,
                requested_by_id: by,
            },
        )
        .await?;
    }
    ctx.audit(Some(ctx.org()?.id()), "logs.settings_update")
        .before(serde_json::json!({ "content_logging": before.content_logging, "metadata_retention_days": before.metadata_retention_days, "content_retention_days": before.content_retention_days }))
        .after(serde_json::json!({ "content_logging": org.content_logging, "metadata_retention_days": org.metadata_retention_days, "content_retention_days": org.content_retention_days }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    forget_org(&state, org.id).await?;
    Ok(Json(org_settings(&state, &org).await?))
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "A project's content logging.")]
pub struct ProjectLogSettings {
    #[doc = "The project's own switch."]
    pub content_logging: bool,
    #[doc = "Whether content is logged: the org's and the project's switch are both on."]
    pub effective_content_logging: bool,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs/settings",
    summary = "The project's content logging.",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectLogSettings))
)]
pub async fn get_project_settings(
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectLogSettings>> {
    let p = &ctx.project()?.project;
    Ok(Json(ProjectLogSettings {
        content_logging: p.content_logging,
        effective_content_logging: p.content_logging && ctx.org()?.org.content_logging,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A project content switch.")]
pub struct ProjectLogSettingsUpdate {
    #[doc = "Log request and reply bodies in this project (only when the org does)."]
    pub content_logging: bool,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/logs/settings",
    summary = "Switches content logging for the project. It cannot be switched on while\n\
    the org has it off.",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectLogSettingsUpdate,
    responses((status = 200, body = ProjectLogSettings))
)]
pub async fn update_project_settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    JsonBody(req): JsonBody<ProjectLogSettingsUpdate>,
) -> ApiResult<Json<ProjectLogSettings>> {
    let mut tx = state.pg.begin().await?;
    let org_on = if req.content_logging {
        crate::platform::orgs::org::lock_changeable(&mut tx, &ctx)
            .await?
            .content_logging
    } else {
        ctx.org()?.org.content_logging
    };
    if req.content_logging && !org_on {
        return Err(ApiError::Conflict(
            "The org has content logging off; a project cannot switch it on.".into(),
        ));
    }
    let scope = &ctx.project()?.project;
    let mut p = projects::get_for_update(&mut *tx, scope.org_id, scope.id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    let before = p.content_logging;
    p.content_logging = req.content_logging;
    projects::update(&mut *tx, &p).await?;
    ctx.audit(Some(ctx.org()?.id()), "logs.project_content_logging")
        .target("project", ProjectId::from_uuid(p.id))
        .before(serde_json::json!({ "content_logging": before }))
        .after(serde_json::json!({ "content_logging": p.content_logging }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    forget(&state, p.id).await;
    Ok(Json(ProjectLogSettings {
        content_logging: p.content_logging,
        effective_content_logging: p.content_logging && org_on,
    }))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Log storage of one project.")]
pub struct ProjectStorageView {
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "Log entries."]
    pub entries: u64,
    #[doc = "Entries with content."]
    pub entries_with_content: u64,
    #[doc = "Content as received, in bytes."]
    pub logical_bytes: u64,
    #[doc = "Estimated size on disk, in bytes."]
    pub estimated_disk_bytes: u64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An org's log storage.")]
pub struct StorageView {
    #[doc = "Log entries."]
    pub entries: u64,
    #[doc = "Content as received, in bytes."]
    pub logical_bytes: u64,
    #[doc = "Estimated size on disk: metadata rows at the table's average row size,\n\
        plus content at its current compression ratio."]
    pub estimated_disk_bytes: u64,
    #[doc = "Per project."]
    pub projects: Vec<ProjectStorageView>,
}

pub(crate) async fn disk_factors(state: &AppState) -> ApiResult<(f64, f64)> {
    let log = ch::table_size(&state.clickhouse, "request_log")
        .await
        .map_err(unavailable)?;
    let content = ch::table_size(&state.clickhouse, "request_content")
        .await
        .map_err(unavailable)?;
    let per_row = if log.rows == 0 {
        0.0
    } else {
        log.bytes_on_disk as f64 / log.rows as f64
    };
    let ratio = if content.uncompressed == 0 {
        1.0
    } else {
        content.compressed as f64 / content.uncompressed as f64
    };
    Ok((per_row, ratio))
}

fn estimate(entries: u64, logical: u64, (per_row, ratio): (f64, f64)) -> u64 {
    (entries as f64 * per_row + logical as f64 * ratio).round() as u64
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/logs/storage",
    summary = "How many log entries the org has and how much space they take.",
    tag = "logs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = StorageView))
)]
pub async fn storage(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<StorageView>> {
    let org = ctx.org()?.org.id;
    let rows = ch::storage(&state.clickhouse, org)
        .await
        .map_err(unavailable)?;
    let factors = disk_factors(&state).await?;
    let projects: Vec<ProjectStorageView> = rows
        .iter()
        .map(|r| ProjectStorageView {
            project_id: ProjectId::from_uuid(r.project_id),
            entries: r.entries,
            entries_with_content: r.with_content,
            logical_bytes: r.logical_bytes,
            estimated_disk_bytes: estimate(r.entries, r.logical_bytes, factors),
        })
        .collect();
    let entries = projects.iter().map(|p| p.entries).sum();
    let logical_bytes = projects.iter().map(|p| p.logical_bytes).sum();
    Ok(Json(StorageView {
        entries,
        logical_bytes,
        estimated_disk_bytes: projects.iter().map(|p| p.estimated_disk_bytes).sum(),
        projects,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A deletion of logs in a time range.")]
pub struct DeletionRequest {
    #[doc = "From (inclusive), RFC 3339."]
    #[schema(example = "2026-10-01T00:00:00Z")]
    pub from: String,
    #[doc = "To (exclusive), RFC 3339."]
    #[schema(example = "2026-10-08T00:00:00Z")]
    pub to: String,
    #[doc = "Only this project."]
    pub project: Option<ProjectId>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A queued deletion or retention shrink.")]
pub struct LogJobView {
    #[doc = "Id."]
    pub id: String,
    #[doc = "`deletion`, `shrink_metadata` or `shrink_content`."]
    pub kind: String,
    #[doc = "One project, or the whole org."]
    pub project_id: Option<ProjectId>,
    #[doc = "From."]
    #[serde(with = "time::serde::rfc3339")]
    pub from: OffsetDateTime,
    #[doc = "To."]
    #[serde(with = "time::serde::rfc3339")]
    pub to: OffsetDateTime,
    #[doc = "`pending` or `done`."]
    pub state: &'static str,
    #[doc = "The last failure, while pending."]
    pub last_error: Option<String>,
    #[doc = "Queued."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Done."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub done_at: Option<OffsetDateTime>,
}

fn job_view(j: &log_jobs::LogJob) -> LogJobView {
    LogJobView {
        id: j.id.to_string(),
        kind: j.kind.clone(),
        project_id: j.project_id.map(ProjectId::from_uuid),
        from: j.from_at,
        to: j.to_at,
        state: if j.done_at.is_some() {
            "done"
        } else {
            "pending"
        },
        last_error: if j.done_at.is_none() {
            j.last_error.clone()
        } else {
            None
        },
        created_at: j.created_at,
        done_at: j.done_at,
    }
}

fn parse_time(field: &str, s: &str) -> ApiResult<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).map_err(|_| {
        ApiError::invalid(
            field,
            format!("`{field}` must be a time such as 2026-10-01T00:00:00Z."),
        )
    })
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/logs/deletions",
    summary = "Deletes the org's logs in a time range, optionally of one project. Runs\n\
    in the background; audited.",
    tag = "logs",
    params(("org" = OrgId, Path)),
    request_body = DeletionRequest,
    responses((status = 202, body = LogJobView))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    JsonBody(req): JsonBody<DeletionRequest>,
) -> ApiResult<(StatusCode, Json<LogJobView>)> {
    let org = ctx.org()?.org.id;
    let (from, to) = (parse_time("from", &req.from)?, parse_time("to", &req.to)?);
    if from >= to {
        return Err(ApiError::invalid("to", "`to` must be after `from`."));
    }
    if let Some(p) = req.project {
        projects::get(&state.pg, org, p.uuid())
            .await?
            .ok_or_else(|| ApiError::invalid("project", "No such project in this org."))?;
    }
    let id = Uuid::now_v7();
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    log_jobs::enqueue(
        &mut *tx,
        &log_jobs::NewLogJob {
            id,
            org_id: org,
            project_id: req.project.map(|p| p.uuid()),
            kind: "deletion",
            from_at: from,
            to_at: to,
            requested_by_kind: kind,
            requested_by_id: by,
        },
    )
    .await?;
    ctx.audit(Some(ctx.org()?.id()), "logs.delete")
        .after(serde_json::json!({ "from": req.from, "to": req.to, "project": req.project }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let st = state.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::services::logs::run_jobs(&st).await {
            tracing::warn!(error = %e, "log deletion will be retried");
        }
    });
    let job = log_jobs::list(&state.pg, org, 50)
        .await?
        .into_iter()
        .find(|j| j.id == id)
        .ok_or_else(|| ApiError::internal("queued deletion vanished"))?;
    Ok((StatusCode::ACCEPTED, Json(job_view(&job))))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/logs/deletions",
    summary = "The org's recent log deletions and retention shrinks.",
    tag = "logs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<LogJobView>))
)]
pub async fn deletions(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<List<LogJobView>>> {
    let jobs = log_jobs::list(&state.pg, ctx.org()?.org.id, 100).await?;
    Ok(Json(List::complete(jobs.iter().map(job_view).collect())))
}

#[derive(Clone, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LogFilter {
    #[doc = "From (inclusive), RFC 3339."]
    pub from: Option<String>,
    #[doc = "To (exclusive), RFC 3339."]
    pub to: Option<String>,
    #[doc = "Only this key."]
    pub key: Option<ApiKeyId>,
    #[doc = "Only this user's requests."]
    pub user: Option<UserId>,
    #[doc = "Only this service account's requests."]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "Only this provider model id."]
    pub model: Option<String>,
    #[doc = "Only this provider."]
    pub provider: Option<String>,
    #[doc = "Only this HTTP status."]
    pub status: Option<u16>,
    #[doc = "Only failed (`true`) or successful (`false`) requests."]
    pub failed: Option<bool>,
    #[doc = "Only this error kind, e.g. `rate_limited`, `stream_interrupted` or\n\
        `client_disconnected`."]
    pub error_kind: Option<String>,
    #[doc = "Only streamed (`true`) or complete (`false`) requests."]
    pub stream: Option<bool>,
    #[doc = "Only requests from the model API (`api`) or the console's\n\
        Playground (`playground`)."]
    pub source: Option<String>,
}

impl LogFilter {
    fn filter(&self) -> ApiResult<Filter> {
        if let Some(s) = &self.source
            && !SOURCES.contains(&s.as_str())
        {
            return Err(ApiError::invalid(
                "source",
                "`source` must be `api` or `playground`.",
            ));
        }
        Ok(Filter {
            from: self
                .from
                .as_deref()
                .map(|s| parse_time("from", s))
                .transpose()?,
            to: self
                .to
                .as_deref()
                .map(|s| parse_time("to", s))
                .transpose()?,
            key: self.key.map(|k| k.uuid()),
            principal: self
                .user
                .map(|u| u.uuid())
                .or(self.service_account.map(|s| s.uuid())),
            model: self.model.clone(),
            provider: self.provider.clone(),
            status: self.status,
            failed: self.failed,
            error_kind: self.error_kind.clone(),
            stream: self.stream,
            source: self.source.clone(),
        })
    }
}

const SOURCES: [&str; 2] = ["api", "playground"];

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Token counts of a request.")]
pub struct UsageView {
    #[doc = "Uncached input tokens."]
    pub input_tokens: u64,
    #[doc = "Output tokens (reasoning included)."]
    pub output_tokens: u64,
    #[doc = "Cache reads."]
    pub cache_read_tokens: u64,
    #[doc = "Cache writes."]
    pub cache_write_tokens: u64,
    #[doc = "Reasoning tokens."]
    pub reasoning_tokens: u64,
    #[doc = "Whether these counts are partly estimated: the stream ended (the app\n\
        went away, or the provider's stream broke off) before the provider\n\
        reported its final usage. Input then comes from the provider's\n\
        report at the start of the stream or its token-counting endpoint\n\
        (else about 4 bytes of the request per token), output from the\n\
        content received (at least one token per delta and per 4 bytes).\n\
        Costs and limits use the same numbers."]
    pub estimated: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A request log entry.")]
pub struct LogView {
    #[doc = "The request id (also in `kaeso-request-id`)."]
    pub id: String,
    #[doc = "Arrival."]
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[doc = "Total time in milliseconds."]
    pub duration_ms: u32,
    #[doc = "Time to the provider's first byte, in milliseconds."]
    pub ttfb_ms: Option<u32>,
    #[doc = "The key; `null` for requests from the Playground, which have none."]
    pub key_id: Option<ApiKeyId>,
    #[doc = "Where the request came from: `api` (the model API, with a key) or\n\
        `playground` (the console's Playground, as the member)."]
    #[schema(example = "api")]
    pub source: String,
    #[doc = "`user` or `service_account`."]
    pub principal_kind: String,
    #[doc = "The user or service account."]
    pub principal_id: String,
    #[doc = "The endpoint, e.g. `chat/completions`."]
    pub endpoint: String,
    #[doc = "The app's dialect."]
    pub ingress_dialect: String,
    #[doc = "The provider's dialect."]
    pub egress_dialect: String,
    #[doc = "The provider."]
    pub provider: String,
    #[doc = "The model the app named."]
    pub requested_model: String,
    #[doc = "The provider model called."]
    pub model: String,
    #[doc = "Streamed."]
    pub stream: bool,
    #[doc = "The request's status: the HTTP status sent to the app; for a stream\n\
        that failed after its headers were sent, the status of the failure\n\
        (the provider's error status, else 502; 504 for a timeout); 499 when\n\
        the app went away before the end."]
    pub status: u16,
    #[doc = "The error kind, for failures: a kpp error kind (`invalid_request`,\n\
        `authentication`, `permission_denied`, `not_found`,\n\
        `request_too_large`, `rate_limited`, `overloaded`,\n\
        `context_too_long`, `content_filtered`, `unsupported`, `timeout`,\n\
        `billing`, `provider_error`), `stream_interrupted` when the\n\
        provider's stream broke off after the reply had started, or\n\
        `client_disconnected` when the app went away before the end."]
    pub error_kind: Option<String>,
    #[doc = "The error message, for failures."]
    pub error_message: Option<String>,
    #[doc = "Tokens."]
    pub usage: UsageView,
    #[doc = "Cost in USD at the prices of request time."]
    #[schema(value_type = String)]
    pub cost: String,
    #[doc = "User agent."]
    pub user_agent: String,
    #[doc = "Client address."]
    pub client_ip: String,
    #[doc = "The provider's request id."]
    pub upstream_request_id: String,
    #[doc = "Whether content was logged."]
    pub has_content: bool,
    #[doc = "For streamed requests: how far the stream got."]
    pub stream_progress: Option<StreamProgressView>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "How far a streamed reply got.")]
pub struct StreamProgressView {
    #[doc = "Stream events decoded from the provider."]
    pub events: u32,
    #[doc = "Bytes written to the app, keepalives included."]
    pub bytes: u64,
    #[doc = "Time from the request's arrival to the first reply bytes written to\n\
        the app, in milliseconds; `null` when none were."]
    pub first_byte_ms: Option<u32>,
    #[doc = "Keepalives the gateway wrote because the app had seen nothing for the\n\
        keepalive interval."]
    pub keepalives: u32,
}

fn view(r: &LogRow) -> LogView {
    let principal_id = if r.principal_kind == "user" {
        UserId::from_uuid(r.principal_id).to_string()
    } else {
        ServiceAccountId::from_uuid(r.principal_id).to_string()
    };
    let none = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    LogView {
        id: RequestId::from_uuid(r.request_id).to_string(),
        started_at: r.started_at,
        duration_ms: r.duration_ms,
        ttfb_ms: r.ttfb_ms,
        key_id: (!r.key_id.is_nil()).then(|| ApiKeyId::from_uuid(r.key_id)),
        source: r.source.clone(),
        principal_kind: r.principal_kind.clone(),
        principal_id,
        endpoint: r.endpoint.clone(),
        ingress_dialect: r.ingress_dialect.clone(),
        egress_dialect: r.egress_dialect.clone(),
        provider: r.provider.clone(),
        requested_model: r.requested_model.clone(),
        model: r.resolved_model.clone(),
        stream: r.stream,
        status: r.status,
        error_kind: none(&r.error_kind),
        error_message: none(&r.error_message),
        usage: UsageView {
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_write_tokens: r.cache_write_tokens,
            reasoning_tokens: r.reasoning_tokens,
            estimated: r.usage_estimated,
        },
        cost: cost_from_raw(r.cost).to_string(),
        user_agent: r.user_agent.clone(),
        client_ip: r.client_ip.clone(),
        upstream_request_id: r.upstream_request_id.clone(),
        has_content: r.has_content,
        stream_progress: r.stream.then_some(StreamProgressView {
            events: r.stream_events,
            bytes: r.stream_bytes,
            first_byte_ms: r.stream_first_byte_ms,
            keepalives: r.stream_keepalives,
        }),
    }
}

fn encode_cursor(r: &LogRow) -> String {
    let millis = r.started_at.unix_timestamp_nanos() / 1_000_000;
    URL_SAFE_NO_PAD.encode(format!("{millis}.{}", r.request_id))
}

fn decode_cursor(c: &str) -> Option<(OffsetDateTime, Uuid)> {
    let text = String::from_utf8(URL_SAFE_NO_PAD.decode(c).ok()?).ok()?;
    let (millis, id) = text.split_once('.')?;
    let at =
        OffsetDateTime::from_unix_timestamp_nanos(millis.parse::<i128>().ok()? * 1_000_000).ok()?;
    Some((at, id.parse().ok()?))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LogPage {
    #[doc = "How many entries, 1 to 100 (default 20)."]
    pub limit: Option<u32>,
    #[doc = "The `next_cursor` of the previous page."]
    pub after: Option<String>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs",
    summary = "The project's request log, newest first (metadata only).",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), LogPage, LogFilter),
    responses((status = 200, body = List<LogView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<LogPage>,
    Query(f): Query<LogFilter>,
) -> ApiResult<Json<List<LogView>>> {
    let limit = page.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err(ApiError::invalid(
            "limit",
            "`limit` must be between 1 and 100.",
        ));
    }
    let after = match &page.after {
        Some(c) => Some(decode_cursor(c).ok_or_else(|| {
            ApiError::invalid("after", "`after` is not a cursor from a previous page.")
        })?),
        None => None,
    };
    let p = &ctx.project()?.project;
    let mut rows = ch::search(
        &state.clickhouse,
        p.org_id,
        p.id,
        &f.filter()?,
        after,
        limit + 1,
    )
    .await
    .map_err(unavailable)?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if has_more {
        rows.last().map(encode_cursor)
    } else {
        None
    };
    Ok(Json(List {
        data: rows.iter().map(view).collect(),
        has_more,
        next_cursor,
    }))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A log entry with content.")]
pub struct LogEntryView {
    #[doc = "The metadata."]
    #[serde(flatten)]
    pub log: LogView,
    #[doc = "Content, for callers with `logs.content.view` when it was logged."]
    pub content: Option<ContentView>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Request and reply bodies.")]
pub struct ContentView {
    #[doc = "The app's request body (JSON when it was JSON)."]
    #[schema(value_type = Object)]
    pub request: serde_json::Value,
    #[doc = "The reply in KPP form, including what the app's format could not show."]
    #[schema(value_type = Object)]
    pub reply: serde_json::Value,
}

fn content_view(c: &ch::ContentRow) -> ContentView {
    let parse = |s: &str| {
        if s.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_owned()))
        }
    };
    ContentView {
        request: parse(&c.request_body),
        reply: parse(&c.reply),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs/{log}",
    summary = "One log entry; content only with `logs.content.view`.",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("log" = RequestId, Path)),
    responses((status = 200, body = LogEntryView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<LogEntryView>> {
    let id: RequestId = id.parse().map_err(|_| ApiError::not_found("log entry"))?;
    let scope = ctx.project()?;
    let p = &scope.project;
    let row = ch::get(&state.clickhouse, p.org_id, p.id, id.uuid())
        .await
        .map_err(unavailable)?
        .ok_or_else(|| ApiError::not_found("log entry"))?;
    let content = if row.has_content && scope.rights.contains(ProjectRight::LogsContentView) {
        ch::contents(&state.clickhouse, p.org_id, p.id, &[row.request_id])
            .await
            .map_err(unavailable)?
            .first()
            .map(content_view)
    } else {
        None
    };
    Ok(Json(LogEntryView {
        log: view(&row),
        content,
    }))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportParams {
    #[doc = "`jsonl` (default) or `csv` (metadata only)."]
    pub format: Option<String>,
    #[doc = "JSON Lines: include content (needs `logs.content.view`)."]
    pub include_content: Option<bool>,
}

const CSV_HEADER: &str = "id,started_at,duration_ms,ttfb_ms,key_id,principal_kind,principal_id,endpoint,ingress_dialect,egress_dialect,provider,requested_model,model,stream,status,error_kind,input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,reasoning_tokens,cost,user_agent,client_ip,upstream_request_id,stream_events,stream_bytes,stream_first_byte_ms,stream_keepalives,usage_estimated,source\n";

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn csv_line(v: &LogView) -> String {
    let progress = v.stream_progress.as_ref();
    let fields = [
        v.id.clone(),
        v.started_at.format(&Rfc3339).unwrap_or_default(),
        v.duration_ms.to_string(),
        v.ttfb_ms.map(|t| t.to_string()).unwrap_or_default(),
        v.key_id.map(|k| k.to_string()).unwrap_or_default(),
        v.principal_kind.clone(),
        v.principal_id.clone(),
        v.endpoint.clone(),
        v.ingress_dialect.clone(),
        v.egress_dialect.clone(),
        v.provider.clone(),
        v.requested_model.clone(),
        v.model.clone(),
        v.stream.to_string(),
        v.status.to_string(),
        v.error_kind.clone().unwrap_or_default(),
        v.usage.input_tokens.to_string(),
        v.usage.output_tokens.to_string(),
        v.usage.cache_read_tokens.to_string(),
        v.usage.cache_write_tokens.to_string(),
        v.usage.reasoning_tokens.to_string(),
        v.cost.clone(),
        v.user_agent.clone(),
        v.client_ip.clone(),
        v.upstream_request_id.clone(),
        progress.map(|p| p.events.to_string()).unwrap_or_default(),
        progress.map(|p| p.bytes.to_string()).unwrap_or_default(),
        progress
            .and_then(|p| p.first_byte_ms)
            .map(|t| t.to_string())
            .unwrap_or_default(),
        progress
            .map(|p| p.keepalives.to_string())
            .unwrap_or_default(),
        v.usage.estimated.to_string(),
        v.source.clone(),
    ];
    let mut line = fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",");
    line.push('\n');
    line
}

struct ExportState {
    state: AppState,
    org: Uuid,
    project: Uuid,
    filter: Filter,
    after: Option<(OffsetDateTime, Uuid)>,
    content: bool,
    csv: bool,
    done: bool,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs/export",
    summary = "Exports the log entries matching a filter, newest first, streamed in\n\
    batches: JSON Lines (metadata, plus content with `include_content=true`\n\
    and `logs.content.view`) or CSV (metadata only).",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ExportParams, LogFilter),
    responses((status = 200, description = "JSON Lines or CSV.", content_type = "application/x-ndjson"))
)]
pub async fn export(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(params): Query<ExportParams>,
    Query(f): Query<LogFilter>,
) -> ApiResult<Response> {
    let csv = match params.format.as_deref() {
        None | Some("jsonl") => false,
        Some("csv") => true,
        Some(_) => return Err(ApiError::invalid("format", "Use `jsonl` or `csv`.")),
    };
    let scope = ctx.project()?;
    let content = params.include_content.unwrap_or(false);
    if content && (csv || !scope.rights.contains(ProjectRight::LogsContentView)) {
        return Err(if csv {
            ApiError::invalid("include_content", "CSV exports carry metadata only.")
        } else {
            ApiError::PermissionDenied(
                "Exporting content needs the right `logs.content.view`.".into(),
            )
        });
    }
    let filter = f.filter()?;
    ch::search(
        &state.clickhouse,
        scope.project.org_id,
        scope.project.id,
        &filter,
        None,
        1,
    )
    .await
    .map_err(unavailable)?;
    ctx.audit(Some(ctx.org()?.id()), "logs.export")
        .target("project", ProjectId::from_uuid(scope.project.id))
        .after(
            serde_json::json!({ "format": if csv { "csv" } else { "jsonl" }, "content": content }),
        )
        .write(&mut *state.pg.acquire().await?)
        .await?;
    let start = ExportState {
        state: state.clone(),
        org: scope.project.org_id,
        project: scope.project.id,
        filter,
        after: None,
        content,
        csv,
        done: false,
    };
    let header_sent = csv;
    let stream = futures::stream::unfold((start, header_sent), |(mut s, mut header)| async move {
        if s.done {
            return None;
        }
        let mut out = String::new();
        if header {
            out.push_str(CSV_HEADER);
            header = false;
        }
        match ch::search(
            &s.state.clickhouse,
            s.org,
            s.project,
            &s.filter,
            s.after,
            1000,
        )
        .await
        {
            Ok(rows) => {
                s.done = rows.len() < 1000;
                s.after = rows.last().map(|r| (r.started_at, r.request_id));
                let contents = if s.content {
                    let ids: Vec<Uuid> = rows
                        .iter()
                        .filter(|r| r.has_content)
                        .map(|r| r.request_id)
                        .collect();
                    ch::contents(&s.state.clickhouse, s.org, s.project, &ids)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                for r in &rows {
                    let v = view(r);
                    if s.csv {
                        out.push_str(&csv_line(&v));
                    } else {
                        let mut obj = serde_json::to_value(&v).unwrap_or_default();
                        if s.content
                            && let Some(map) = obj.as_object_mut()
                        {
                            let content = contents
                                .iter()
                                .find(|c| c.request_id == r.request_id)
                                .map(|c| serde_json::to_value(content_view(c)).unwrap_or_default())
                                .unwrap_or(serde_json::Value::Null);
                            map.insert("content".into(), content);
                        }
                        out.push_str(&obj.to_string());
                        out.push('\n');
                    }
                }
                Some((Ok::<_, std::io::Error>(Bytes::from(out)), (s, header)))
            }
            Err(e) => {
                tracing::warn!(error = %e, "log export stopped: ClickHouse failed");
                s.done = true;
                Some((
                    Err(std::io::Error::other("ClickHouse failed during the export")),
                    (s, header),
                ))
            }
        }
    });
    let mut resp = Response::new(Body::from_stream(stream));
    let (ct, ext) = if csv {
        ("text/csv; charset=utf-8", "csv")
    } else {
        ("application/x-ndjson", "jsonl")
    };
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"kaeso-logs-{}.{ext}\"",
        ProjectId::from_uuid(scope.project.id)
    )) {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}
