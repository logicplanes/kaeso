use axum::Json;
use axum::extract::State;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_core::id::{
    ApiKeyId, McpServerId, OrgId, PortalGrantId, ProjectId, ServiceAccountId, ToolCallId, UserId,
};
use kaeso_core::rights::ProjectRight;
use kaeso_store::clickhouse::tool_calls::{self as store, Filter, ToolCallRow};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::keys::Party;
use crate::platform::logs::{decode_cursor, log_store, parse_time, unavailable};
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

const STATUSES: [&str; 3] = ["ok", "error", "refused"];

#[derive(Clone, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ToolCallFilter {
    #[doc = "From (inclusive), RFC 3339."]
    pub from: Option<String>,
    #[doc = "To (exclusive), RFC 3339."]
    pub to: Option<String>,
    #[doc = "Only calls with this key."]
    pub key: Option<ApiKeyId>,
    #[doc = "Only this user's calls."]
    pub user: Option<UserId>,
    #[doc = "Only this service account's calls."]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "Only this MCP server's tools."]
    pub server: Option<McpServerId>,
    #[doc = "Only this tool, by its name for clients, such as `x__create_post`."]
    pub tool: Option<String>,
    #[doc = "Only calls that went through (`ok`), failed (`error`) or were refused before\n\
        reaching the server (`refused`)."]
    pub status: Option<String>,
    #[doc = "Only this error type: `denied`, `tool_off`, `tool_changed`, `limited`,\n\
        `upstream_auth`, `upstream_unreachable`, `upstream_error`, `timeout` or\n\
        `too_large`."]
    pub error_type: Option<String>,
}

impl ToolCallFilter {
    fn filter(&self) -> ApiResult<Filter> {
        if let Some(s) = &self.status
            && !STATUSES.contains(&s.as_str())
        {
            return Err(ApiError::invalid(
                "status",
                "`status` must be `ok`, `error` or `refused`.",
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
            caller: self
                .user
                .map(|u| u.uuid())
                .or(self.service_account.map(|s| s.uuid())),
            server: self.server.map(|s| s.uuid()),
            tool: self.tool.clone(),
            status: self.status.clone(),
            error_type: self.error_type.clone(),
        })
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool call through the project's MCP portal.")]
pub struct ToolCallView {
    #[doc = "Id."]
    pub id: ToolCallId,
    #[doc = "Arrival."]
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[doc = "The tool, by its name for clients."]
    #[schema(example = "x__create_post")]
    pub tool: String,
    #[doc = "Its name on the server."]
    #[schema(example = "create_post")]
    pub upstream_tool: String,
    #[doc = "The MCP server."]
    pub server: McpServerId,
    #[doc = "The person or service account the call ran as."]
    pub caller: Party,
    #[doc = "The key it came with, for a key."]
    pub key_id: Option<ApiKeyId>,
    #[doc = "The client's sign-in to the portal, for a signed-in client."]
    pub grant_id: Option<PortalGrantId>,
    #[doc = "The client's name, as it calls itself."]
    #[schema(example = "claude-ai")]
    pub client: String,
    #[doc = "The MCP version the client spoke."]
    #[schema(example = "2026-07-28")]
    pub protocol_version: String,
    #[doc = "`portal` or `playground`."]
    pub source: String,
    #[doc = "`ok`, `error` or `refused`."]
    pub status: String,
    #[doc = "Why it failed or was refused; empty when it went through."]
    pub error_type: String,
    #[doc = "What the client was told; empty when it went through."]
    pub error_message: String,
    #[doc = "Total time in milliseconds."]
    pub duration_ms: u32,
    #[doc = "The MCP server's part, in milliseconds; `null` when the call never reached it."]
    pub upstream_ms: Option<u32>,
    #[doc = "Bytes of the client's request."]
    pub request_bytes: u64,
    #[doc = "Bytes of the answer."]
    pub response_bytes: u64,
    #[doc = "Whether arguments and result were logged (content logging was on)."]
    pub has_content: bool,
}

fn view(r: &ToolCallRow) -> ToolCallView {
    let caller_id = if r.caller_kind == "user" {
        UserId::from_uuid(r.caller_id).to_string()
    } else {
        ServiceAccountId::from_uuid(r.caller_id).to_string()
    };
    ToolCallView {
        id: ToolCallId::from_uuid(r.id),
        started_at: r.started_at,
        tool: r.tool.clone(),
        upstream_tool: r.upstream_tool.clone(),
        server: McpServerId::from_uuid(r.server_id),
        caller: Party {
            kind: r.caller_kind.clone(),
            id: Some(caller_id),
        },
        key_id: r.key_id.map(ApiKeyId::from_uuid),
        grant_id: r.grant_id.map(PortalGrantId::from_uuid),
        client: r.client_name.clone(),
        protocol_version: r.protocol_version.clone(),
        source: r.source.clone(),
        status: r.status.clone(),
        error_type: r.error_type.clone(),
        error_message: r.error_message.clone(),
        duration_ms: r.duration_ms,
        upstream_ms: r.upstream_ms,
        request_bytes: r.request_bytes,
        response_bytes: r.response_bytes,
        has_content: r.has_content,
    }
}

fn encode_cursor(r: &ToolCallRow) -> String {
    let millis = r.started_at.unix_timestamp_nanos() / 1_000_000;
    URL_SAFE_NO_PAD.encode(format!("{millis}.{}", r.id))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs/tool-calls",
    summary = "The project's tool calls, newest first (metadata only).",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams, ToolCallFilter),
    responses((status = 200, body = List<ToolCallView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
    Query(f): Query<ToolCallFilter>,
) -> ApiResult<Json<List<ToolCallView>>> {
    let page = page.page_by(decode_cursor)?;
    let p = &ctx.project()?.project;
    let rows = store::search(
        log_store(&state)?,
        p.org_id,
        p.id,
        &f.filter()?,
        page.after,
        page.fetch(),
    )
    .await
    .map_err(unavailable)?;
    Ok(Json(List::from_page(&page, rows, encode_cursor, |r| {
        view(&r)
    })))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool call with its arguments and result.")]
pub struct ToolCallEntryView {
    #[doc = "The metadata."]
    #[serde(flatten)]
    pub call: ToolCallView,
    #[doc = "Arguments and result, for callers with `logs.content.view` when they were logged."]
    pub content: Option<ToolCallContentView>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool call's arguments and result.")]
pub struct ToolCallContentView {
    #[doc = "The arguments as the client sent them."]
    #[schema(value_type = Object)]
    pub arguments: serde_json::Value,
    #[doc = "The JSON-RPC answer the client got."]
    #[schema(value_type = Object)]
    pub result: serde_json::Value,
}

fn json_or_text(s: &str) -> serde_json::Value {
    if s.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_owned()))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/logs/tool-calls/{call}",
    summary = "One tool call; arguments and result only with `logs.content.view`.",
    tag = "logs",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("call" = ToolCallId, Path)),
    responses((status = 200, body = ToolCallEntryView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, call)): Path<(String, String, String)>,
) -> ApiResult<Json<ToolCallEntryView>> {
    let id: ToolCallId = call.parse().map_err(|_| ApiError::not_found("tool call"))?;
    let scope = ctx.project()?;
    let p = &scope.project;
    let clickhouse = log_store(&state)?;
    let row = store::get(clickhouse, p.org_id, p.id, id.uuid())
        .await
        .map_err(unavailable)?
        .ok_or_else(|| ApiError::not_found("tool call"))?;
    let content = if row.has_content && scope.rights.contains(ProjectRight::LogsContentView) {
        store::content(clickhouse, p.org_id, p.id, row.id)
            .await
            .map_err(unavailable)?
            .map(|c| ToolCallContentView {
                arguments: json_or_text(&c.arguments),
                result: json_or_text(&c.result),
            })
    } else {
        None
    };
    Ok(Json(ToolCallEntryView {
        call: view(&row),
        content,
    }))
}
