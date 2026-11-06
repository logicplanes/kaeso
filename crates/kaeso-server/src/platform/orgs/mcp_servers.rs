use std::collections::{BTreeMap, BTreeSet};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{McpConnectionId, McpServerId, OrgId, ProjectId, ToolId};
use kaeso_mcp::{names, params::ParamError};
use kaeso_store::pg::mcp_connections::{self as connections, ConnectionRow};
use kaeso_store::pg::mcp_servers::{self as store, Changes, NewServer, ServerRow, Updated};
use kaeso_store::pg::mcp_tools::{self as tools, ServerTools, ToolRow};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::cache::hex;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::{Party, creator, creator_of};
use crate::platform::pagination::List;
use crate::platform::params::{Path, id_param};
use crate::platform::precondition::{IfMatch, etag};
use crate::services::custom_providers::Reachability;
use crate::services::mcp_connections::{self as signin, HeaderProblem, SignInError};
use crate::services::mcp_servers::{self as service, ProbeProblem, SignIn, UrlProblem};
use crate::services::mcp_tools::{self as sync, Skip, SyncError};
use crate::state::AppState;

type Tagged = [(axum::http::HeaderName, axum::http::HeaderValue); 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How Kaeso signs in to an MCP server.")]
pub enum McpAuthMethod {
    #[doc = "No sign-in: the server answers anyone who reaches it."]
    None,
    #[doc = "A secret header, such as `Authorization: Bearer …`, sent with every request."]
    Header,
    #[doc = "OAuth: someone signs in to an account of the server in the browser, and Kaeso keeps\n\
        and renews the tokens."]
    Oauth,
}

impl McpAuthMethod {
    fn as_str(self) -> &'static str {
        match self {
            McpAuthMethod::None => "none",
            McpAuthMethod::Header => "header",
            McpAuthMethod::Oauth => "oauth",
        }
    }

    fn of(s: &str) -> Self {
        match s {
            "header" => McpAuthMethod::Header,
            "oauth" => McpAuthMethod::Oauth,
            _ => McpAuthMethod::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Whose account a call to an MCP server uses.")]
pub enum McpCredentialMode {
    #[doc = "One connection for the whole organization, set up by an admin."]
    Shared,
    #[doc = "Each person connects their own account; a call uses the caller's."]
    PerUser,
}

impl McpCredentialMode {
    fn as_str(self) -> &'static str {
        match self {
            McpCredentialMode::Shared => "shared",
            McpCredentialMode::PerUser => "per_user",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How Kaeso introduced itself to an MCP server's authorization server.")]
pub enum McpRegistration {
    #[doc = "Its client metadata document at `/oauth/client.json`."]
    Cimd,
    #[doc = "Dynamic client registration."]
    Dcr,
    #[doc = "A client an admin registered with the server and entered by hand."]
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "What happens when an MCP server changes a tool's definition.")]
pub enum McpChangePolicy {
    #[doc = "The tool is held: clients don't see it until someone approves the change."]
    Hold,
    #[doc = "The change is approved as it comes."]
    Accept,
}

impl McpChangePolicy {
    fn as_str(self) -> &'static str {
        match self {
            McpChangePolicy::Hold => "hold",
            McpChangePolicy::Accept => "accept",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How the last check of an MCP server went.")]
pub enum McpServerStatus {
    #[doc = "Not checked yet."]
    Unchecked,
    #[doc = "Kaeso reached the server and listed its tools."]
    Ok,
    #[doc = "Kaeso couldn't reach the server."]
    Unreachable,
    #[doc = "Kaeso has no working sign-in for the server; connect it again."]
    SignedOut,
    #[doc = "The server answered with an error."]
    Error,
}

impl McpServerStatus {
    fn of(s: &str) -> Self {
        match s {
            "ok" => McpServerStatus::Ok,
            "unreachable" => McpServerStatus::Unreachable,
            "signed_out" => McpServerStatus::SignedOut,
            "error" => McpServerStatus::Error,
            _ => McpServerStatus::Unchecked,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "How Kaeso signs in to a server with OAuth.")]
pub struct McpOAuthView {
    #[doc = "How Kaeso introduced itself to the authorization server."]
    pub registration: McpRegistration,
    #[doc = "The authorization server Kaeso's client belongs to; `null` before the first sign-in."]
    #[schema(example = "https://auth.example.com")]
    pub issuer: Option<String>,
    #[doc = "Kaeso's client id there; `null` before the first sign-in."]
    pub client_id: Option<String>,
    #[doc = "Whether Kaeso holds a client secret for it. The secret is never shown."]
    pub client_secret: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "What a connection to an MCP server holds.")]
pub enum McpConnectionKind {
    #[doc = "A secret header."]
    Header,
    #[doc = "OAuth tokens, renewed before they run out."]
    Oauth,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A sign-in Kaeso keeps for an MCP server, without its secret.")]
pub struct McpConnectionView {
    #[doc = "Id."]
    pub id: McpConnectionId,
    #[doc = "What it holds."]
    pub kind: McpConnectionKind,
    #[doc = "The header's name, for a header connection."]
    #[schema(example = "Authorization")]
    pub header_name: Option<String>,
    #[doc = "The last four characters of the secret."]
    #[schema(example = "x9Qa")]
    pub last4: String,
    #[doc = "The account it signed in as, when the server says."]
    #[schema(example = "@kaesoapp")]
    pub account: Option<String>,
    #[doc = "When the current access runs out; Kaeso renews it before then."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[doc = "The server stopped accepting it; someone has to sign in again."]
    pub signed_out: bool,
    #[doc = "Last used for a call."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "Last signed in."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

pub fn connection_view(c: &ConnectionRow) -> McpConnectionView {
    McpConnectionView {
        id: McpConnectionId::from_uuid(c.id),
        kind: if c.kind == "oauth" {
            McpConnectionKind::Oauth
        } else {
            McpConnectionKind::Header
        },
        header_name: c.header_name.clone(),
        last4: c.last4.clone(),
        account: c.account_label.clone(),
        expires_at: c.expires_at,
        signed_out: c.auth_error.is_some(),
        last_used_at: c.last_used_at,
        updated_at: c.updated_at,
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, ToSchema)]
#[schema(description = "How many tools a server lists.")]
pub struct McpToolCounts {
    #[doc = "Tools the server lists now."]
    pub total: i64,
    #[doc = "Of those, tools held because their definition changed."]
    pub changed: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "A remote MCP server of the organization. Projects turn it on and choose which\n\
    of its tools their portal offers."
)]
pub struct McpServerView {
    #[doc = "Id."]
    pub id: McpServerId,
    #[doc = "The short name that prefixes its tools outside Kaeso, as in `x__create_post`."]
    #[schema(example = "x")]
    pub handle: String,
    #[doc = "The name people see."]
    #[schema(example = "X")]
    pub name: String,
    #[doc = "Its Streamable HTTP address."]
    #[schema(example = "https://mcp.example.com/mcp")]
    pub url: String,
    #[doc = "How Kaeso signs in to it."]
    pub auth_method: McpAuthMethod,
    #[doc = "Whose account a call uses."]
    pub credential_mode: McpCredentialMode,
    #[doc = "The OAuth client, for `oauth` servers."]
    pub oauth: Option<McpOAuthView>,
    #[doc = "The OAuth scopes Kaeso asks for; empty asks for what the server offers."]
    pub scopes: Vec<String>,
    #[doc = "What happens when the server changes a tool."]
    pub change_policy: McpChangePolicy,
    #[doc = "How the last check went."]
    pub status: McpServerStatus,
    #[doc = "Why the last check failed."]
    pub last_error: Option<String>,
    #[doc = "Last checked."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_checked_at: Option<OffsetDateTime>,
    #[doc = "The MCP version it speaks, such as `2026-07-28`."]
    #[schema(example = "2026-07-28")]
    pub protocol_version: Option<String>,
    #[doc = "What the server says about itself (`name`, `version` and more), as it sent it."]
    #[schema(value_type = Option<Object>)]
    pub server_info: Option<Value>,
    #[doc = "Its tools."]
    pub tools: McpToolCounts,
    #[doc = "The shared sign-in, for `shared` servers that need one; `null` until someone connects it."]
    pub connection: Option<McpConnectionView>,
    #[doc = "Projects that use it."]
    pub projects: Vec<ProjectId>,
    #[doc = "Who added it."]
    pub created_by: Party,
    #[doc = "Added."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Goes up with every change. Send it as `If-Match: \"<revision>\"` to change or delete\n\
        the server only if nothing changed it since."]
    #[schema(example = 1)]
    pub revision: i64,
}

fn registration(s: &str) -> McpRegistration {
    match s {
        "dcr" => McpRegistration::Dcr,
        "manual" => McpRegistration::Manual,
        _ => McpRegistration::Cimd,
    }
}

fn view(
    row: &ServerRow,
    counts: ServerTools,
    connection: Option<&ConnectionRow>,
    projects: &[Uuid],
) -> McpServerView {
    McpServerView {
        id: McpServerId::from_uuid(row.id),
        handle: row.handle.clone(),
        name: row.name.clone(),
        url: row.url.clone(),
        auth_method: McpAuthMethod::of(&row.auth_method),
        credential_mode: if row.credential_mode == "per_user" {
            McpCredentialMode::PerUser
        } else {
            McpCredentialMode::Shared
        },
        oauth: row.oauth_registration.as_deref().map(|r| McpOAuthView {
            registration: registration(r),
            issuer: row.oauth_issuer.clone(),
            client_id: row.oauth_client_id.clone(),
            client_secret: row.has_client_secret,
        }),
        scopes: row.scopes.clone(),
        change_policy: if row.change_policy == "accept" {
            McpChangePolicy::Accept
        } else {
            McpChangePolicy::Hold
        },
        status: McpServerStatus::of(&row.status),
        last_error: row.last_error.clone(),
        last_checked_at: row.last_checked_at,
        protocol_version: row.protocol_version.clone(),
        server_info: row.server_info.clone(),
        tools: McpToolCounts {
            total: counts.total,
            changed: counts.changed,
        },
        connection: connection.map(connection_view),
        projects: projects.iter().copied().map(ProjectId::from_uuid).collect(),
        created_by: creator(&row.created_by_kind, row.created_by_id),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    }
}

async fn one(state: &AppState, row: &ServerRow) -> ApiResult<McpServerView> {
    let counts = tools::counts(&state.pg, row.org_id)
        .await?
        .into_iter()
        .find(|(id, _)| *id == row.id)
        .map(|(_, c)| c)
        .unwrap_or_default();
    let connection = connections::of_server(&state.pg, row.id)
        .await?
        .into_iter()
        .find(|c| c.user_id.is_none());
    let projects = store::projects(&state.pg, row.id).await?;
    Ok(view(row, counts, connection.as_ref(), &projects))
}

fn server_param(raw: &str) -> ApiResult<Uuid> {
    id_param::<McpServerId>(raw, "MCP server").map(|id| id.uuid())
}

async fn server_row(state: &AppState, ctx: &Ctx, raw: &str) -> ApiResult<ServerRow> {
    store::get(&state.pg, ctx.org()?.org.id, server_param(raw)?)
        .await?
        .ok_or_else(|| ApiError::not_found("MCP server"))
}

fn url_problem(p: UrlProblem) -> ApiError {
    ApiError::invalid(
        "url",
        match p {
            UrlProblem::Length => "`url` must be a URL of at most 2048 characters.",
            UrlProblem::Scheme => {
                "`url` must start with `https://`, or `http://` for an allowed network."
            }
            UrlProblem::Host => {
                "`url` must be a full URL with a host, such as `https://mcp.example.com/mcp`."
            }
            UrlProblem::Credentials => {
                "`url` can't hold a user name or password; use a secret header instead."
            }
            UrlProblem::Fragment => "`url` can't have a fragment.",
        },
    )
}

async fn checked_url(state: &AppState, raw: &str) -> ApiResult<url::Url> {
    let url = service::parse_url(raw).map_err(url_problem)?;
    match service::reachability(state, &url).await {
        Reachability::Allowed | Reachability::Unknown => Ok(url),
        Reachability::Refused => {
            let mut https = url.clone();
            let public = url.scheme() == "http"
                && https.set_scheme("https").is_ok()
                && service::reachability(state, &https).await == Reachability::Allowed;
            Err(ApiError::invalid(
                "url",
                if public {
                    "`url` uses `http` for a public address; use `https`."
                } else {
                    "`url` points to an address this install does not allow."
                },
            ))
        }
    }
}

fn checked_name(raw: &str) -> ApiResult<String> {
    service::clean_name(raw).ok_or_else(|| {
        ApiError::invalid(
            "name",
            "The name must be 1 to 100 characters, without control characters.",
        )
    })
}

fn checked_handle(raw: &str) -> ApiResult<&str> {
    if names::valid_handle(raw) {
        Ok(raw)
    } else {
        Err(ApiError::invalid(
            "handle",
            "`handle` must be 1 to 24 lowercase letters, digits and hyphens.",
        ))
    }
}

fn handle_taken(handle: &str) -> ApiError {
    ApiError::Conflict(format!(
        "This organization already has an MCP server `{handle}`."
    ))
}

fn header_problem(p: HeaderProblem) -> ApiError {
    match p {
        HeaderProblem::Name => ApiError::invalid(
            "header.name",
            "`header.name` must be a header name, such as `Authorization`.",
        ),
        HeaderProblem::Reserved => ApiError::invalid(
            "header.name",
            "Kaeso sets this header itself; choose another one.",
        ),
        HeaderProblem::Value => ApiError::invalid(
            "header.value",
            "`header.value` must be printable ASCII without line breaks.",
        ),
    }
}

async fn org_projects(state: &AppState, org: Uuid, wanted: &[ProjectId]) -> ApiResult<Vec<Uuid>> {
    let known: BTreeSet<Uuid> = kaeso_store::pg::projects::ids_of_org(&state.pg, org)
        .await?
        .into_iter()
        .collect();
    let mut out = Vec::new();
    for p in wanted {
        if !known.contains(&p.uuid()) {
            return Err(ApiError::invalid(
                "projects",
                format!("`{p}` is not a project of this organization."),
            ));
        }
        out.push(p.uuid());
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/mcp-servers",
    summary = "The organization's MCP servers, by name.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<McpServerView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<McpServerView>>> {
    let org = ctx.org()?.org.id;
    let rows = store::list(&state.pg, org).await?;
    let counts: BTreeMap<Uuid, ServerTools> =
        tools::counts(&state.pg, org).await?.into_iter().collect();
    let shared: BTreeMap<Uuid, ConnectionRow> = connections::shared_of_org(&state.pg, org)
        .await?
        .into_iter()
        .map(|c| (c.server_id, c))
        .collect();
    let mut links: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for (server, project) in store::project_links(&state.pg, org).await? {
        links.entry(server).or_default().push(project);
    }
    Ok(Json(List::complete(
        rows.iter()
            .map(|r| {
                view(
                    r,
                    counts.get(&r.id).copied().unwrap_or_default(),
                    shared.get(&r.id),
                    links.get(&r.id).map(Vec::as_slice).unwrap_or_default(),
                )
            })
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/mcp-servers/{server}",
    summary = "One MCP server.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path)),
    responses((status = 200, body = McpServerView, headers(("ETag" = String, description = "The server's revision as an entity tag."))))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
) -> ApiResult<(Tagged, Json<McpServerView>)> {
    let row = server_row(&state, &ctx, &server).await?;
    Ok((etag(row.revision), Json(one(&state, &row).await?)))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A secret header Kaeso sends to an MCP server.")]
pub struct McpHeader {
    #[doc = "The header's name."]
    #[schema(example = "Authorization")]
    pub name: String,
    #[doc = "Its value, such as `Bearer <token>`. Kaeso stores it sealed and shows only its last\n\
        four characters."]
    #[schema(value_type = String, example = "Bearer xyz")]
    pub value: SecretString,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "An address to check before adding an MCP server.")]
pub struct McpServerProbe {
    #[doc = "The server's Streamable HTTP address."]
    #[schema(example = "https://mcp.example.com/mcp")]
    pub url: String,
    #[doc = "A secret header to try, for a server that needs one. Kaeso stores it nowhere."]
    #[serde(default)]
    pub header: Option<McpHeader>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Why a check of an MCP server's address failed.")]
pub enum McpProbeProblem {
    #[doc = "The address is one this install does not allow."]
    Refused,
    #[doc = "Nothing answered at the address."]
    Unreachable,
    #[doc = "The server didn't answer in time."]
    Timeout,
    #[doc = "The server answered with an unexpected HTTP status, in `http_status`."]
    Status,
    #[doc = "The server's answer is not MCP."]
    NotMcp,
    #[doc = "The server refused the header."]
    HeaderRefused,
    #[doc = "The server asks for an OAuth sign-in Kaeso can't do, such as one without PKCE."]
    SignIn,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "How an MCP server wants Kaeso to sign in.")]
pub struct McpSignInOffer {
    #[doc = "The way to sign in."]
    pub method: McpAuthMethod,
    #[doc = "The authorization server, for `oauth`."]
    pub issuer: Option<String>,
    #[doc = "Whether it takes Kaeso's client metadata document, so no registration is needed."]
    pub client_metadata_documents: bool,
    #[doc = "Whether it registers clients itself; without this or `client_metadata_documents`,\n\
        an admin registers a client by hand."]
    pub registration: bool,
    #[doc = "The scopes it offers."]
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool an MCP server lists, as it describes it.")]
pub struct McpProbedTool {
    #[doc = "Its name on the server."]
    #[schema(example = "create_post")]
    pub name: String,
    #[doc = "Its title."]
    pub title: Option<String>,
    #[doc = "Its description."]
    pub description: Option<String>,
    #[doc = "Hints such as `readOnlyHint` and `destructiveHint`; claims of the server, not guarantees."]
    #[schema(value_type = Option<Object>)]
    pub annotations: Option<Value>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What Kaeso found at an MCP server's address.")]
pub struct McpServerProbeView {
    #[doc = "The address as Kaeso will use it."]
    pub url: String,
    #[doc = "Why the check failed; `null` when it worked."]
    pub problem: Option<McpProbeProblem>,
    #[doc = "The HTTP status, for `problem: status`."]
    pub http_status: Option<u16>,
    #[doc = "The MCP version it speaks."]
    pub protocol_version: Option<String>,
    #[doc = "What the server says about itself."]
    #[schema(value_type = Option<Object>)]
    pub server_info: Option<Value>,
    #[doc = "How to sign in; `null` when Kaeso couldn't tell."]
    pub sign_in: Option<McpSignInOffer>,
    #[doc = "Its tools, when Kaeso could list them without a sign-in or with the header sent."]
    pub tools: Option<Vec<McpProbedTool>>,
    #[doc = "A free handle that fits the server's name."]
    #[schema(example = "x")]
    pub handle: String,
}

fn probed_tool(v: &Value) -> Option<McpProbedTool> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    Some(McpProbedTool {
        name: text("name")?,
        title: text("title"),
        description: text("description"),
        annotations: v.get("annotations").filter(|a| a.is_object()).cloned(),
    })
}

async fn free_handle(state: &AppState, org: Uuid, from: &str) -> ApiResult<String> {
    let taken: BTreeSet<String> = store::list(&state.pg, org)
        .await?
        .into_iter()
        .map(|s| s.handle)
        .collect();
    let base = service::suggest_handle(from);
    if !taken.contains(&base) {
        return Ok(base);
    }
    for n in 2..100 {
        let suffix = format!("-{n}");
        let cut: String = base
            .chars()
            .take(names::MAX_HANDLE - suffix.len())
            .collect();
        let candidate = format!("{}{suffix}", cut.trim_end_matches('-'));
        if !taken.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Ok(base)
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers/probe",
    summary = "Checks an MCP server's address before adding it: whether it answers, which MCP\n\
    version it speaks, how to sign in and which tools it lists.",
    description = "Kaeso stores nothing it learns here. The address goes through the same checks\n\
    as a saved server's (`mcp_servers.allowed_networks`).",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = McpServerProbe,
    responses((status = 200, body = McpServerProbeView))
)]
pub async fn probe(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<McpServerProbe>,
) -> ApiResult<Json<McpServerProbeView>> {
    let org = ctx.org()?.org.id;
    let url = checked_url(&state, &req.url).await?;
    if let Some(h) = &req.header {
        signin::check_header(&h.name, &h.value).map_err(header_problem)?;
    }
    let found = service::probe(
        &state,
        &url,
        req.header.as_ref().map(|h| (h.name.as_str(), &h.value)),
    )
    .await;
    let named = found
        .server_info
        .as_ref()
        .and_then(|i| i.get("title").or_else(|| i.get("name")))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            url.host_str()
                .map(|h| h.trim_start_matches("mcp.").to_owned())
        })
        .unwrap_or_default();
    let (problem, http_status) = match found.problem {
        None => (None, None),
        Some(ProbeProblem::Status(s)) => (Some(McpProbeProblem::Status), Some(s)),
        Some(p) => (
            Some(match p {
                ProbeProblem::Refused => McpProbeProblem::Refused,
                ProbeProblem::Timeout => McpProbeProblem::Timeout,
                ProbeProblem::NotMcp => McpProbeProblem::NotMcp,
                ProbeProblem::HeaderRefused => McpProbeProblem::HeaderRefused,
                ProbeProblem::SignIn => McpProbeProblem::SignIn,
                ProbeProblem::Unreachable | ProbeProblem::Status(_) => McpProbeProblem::Unreachable,
            }),
            None,
        ),
    };
    let sign_in = found.sign_in.map(|s| match s {
        SignIn::None => McpSignInOffer {
            method: McpAuthMethod::None,
            issuer: None,
            client_metadata_documents: false,
            registration: false,
            scopes: Vec::new(),
        },
        SignIn::Header => McpSignInOffer {
            method: McpAuthMethod::Header,
            issuer: None,
            client_metadata_documents: false,
            registration: false,
            scopes: Vec::new(),
        },
        SignIn::OAuth(o) => McpSignInOffer {
            method: McpAuthMethod::Oauth,
            issuer: Some(o.issuer),
            client_metadata_documents: o.client_metadata_documents,
            registration: o.registration,
            scopes: o.scopes,
        },
    });
    Ok(Json(McpServerProbeView {
        url: url.to_string(),
        problem,
        http_status,
        protocol_version: found.protocol_version.map(str::to_owned),
        server_info: found.server_info,
        sign_in,
        tools: found
            .tools
            .map(|t| t.iter().filter_map(probed_tool).collect()),
        handle: free_handle(&state, org, &named).await?,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(
    description = "An OAuth client an admin registered with the server's authorization server."
)]
pub struct McpOAuthClient {
    #[doc = "Its client id."]
    pub client_id: String,
    #[doc = "Its secret, for a confidential client; absent for a public one."]
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub client_secret: Option<SecretString>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new MCP server.")]
pub struct McpServerCreate {
    #[doc = "The short name that prefixes its tools, as in `x__create_post`: 1 to 24 lowercase\n\
        letters, digits and hyphens, unique in the organization."]
    #[schema(example = "x")]
    pub handle: String,
    #[doc = "The name people see, 1 to 100 characters."]
    #[schema(example = "X")]
    pub name: String,
    #[doc = "Its Streamable HTTP address: `https`, or `http` for a network the instance allows."]
    #[schema(example = "https://mcp.example.com/mcp")]
    pub url: String,
    #[doc = "How Kaeso signs in to it."]
    pub auth_method: McpAuthMethod,
    #[doc = "Whose account a call uses; `shared` when absent. `per_user` needs a sign-in."]
    #[serde(default)]
    pub credential_mode: Option<McpCredentialMode>,
    #[doc = "For `oauth`: a client registered by hand, for an authorization server that takes\n\
        neither client metadata documents nor registration. Absent lets Kaeso introduce itself."]
    #[serde(default)]
    pub oauth_client: Option<McpOAuthClient>,
    #[doc = "For `oauth`: the scopes to ask for; absent asks for what the server offers."]
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[doc = "What happens when it changes a tool; `hold` when absent."]
    #[serde(default)]
    pub change_policy: Option<McpChangePolicy>,
    #[doc = "Projects that use it from the start."]
    #[serde(default)]
    pub projects: Option<Vec<ProjectId>>,
}

fn after_sync(state: &AppState, server: ServerRow) {
    let st = state.clone();
    state.in_flight.finish(async move {
        if let Err(e) = sync::sync(&st, &server).await {
            tracing::info!(server = %server.id, error = %e, "an MCP server couldn't be synced");
        }
    });
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers",
    summary = "Adds an MCP server to the organization.",
    description = "Kaeso checks the address now and on every connection: private, loopback,\n\
    link-local and cloud metadata addresses are refused unless the instance allows the network\n\
    (`mcp_servers.allowed_networks`). A server without sign-in lists its tools right away; others\n\
    do once connected (`POST …/connection`).",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = McpServerCreate,
    responses((status = 201, body = McpServerView, headers(("ETag" = String, description = "The server's revision as an entity tag."))))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<McpServerCreate>,
) -> ApiResult<(StatusCode, Tagged, Json<McpServerView>)> {
    let org = ctx.org()?.org.id;
    let handle = checked_handle(&req.handle)?.to_owned();
    let name = checked_name(&req.name)?;
    let url = checked_url(&state, &req.url).await?;
    let mode = req.credential_mode.unwrap_or(McpCredentialMode::Shared);
    if mode == McpCredentialMode::PerUser && req.auth_method == McpAuthMethod::None {
        return Err(ApiError::invalid(
            "credential_mode",
            "A server without sign-in has no accounts; use `shared`.",
        ));
    }
    let oauth = req.auth_method == McpAuthMethod::Oauth;
    if !oauth && (req.oauth_client.is_some() || req.scopes.is_some()) {
        return Err(ApiError::invalid(
            "auth_method",
            "`oauth_client` and `scopes` are only for `oauth` servers.",
        ));
    }
    let scopes = req.scopes.unwrap_or_default();
    if !service::valid_scopes(&scopes) {
        return Err(ApiError::invalid(
            "scopes",
            "`scopes` takes at most 30 scopes without spaces or quotes.",
        ));
    }
    if let Some(c) = &req.oauth_client
        && !(1..=2048).contains(&c.client_id.len())
    {
        return Err(ApiError::invalid(
            "oauth_client.client_id",
            "`client_id` must be 1 to 2048 characters.",
        ));
    }
    let projects = org_projects(&state, org, req.projects.as_deref().unwrap_or_default()).await?;
    let policy = req.change_policy.unwrap_or(McpChangePolicy::Hold);
    let id = Uuid::now_v7();
    let (kind, by) = creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    let added = store::insert(
        &mut *tx,
        &NewServer {
            id,
            org_id: org,
            handle: &handle,
            name: &name,
            url: url.as_str(),
            auth_method: req.auth_method.as_str(),
            credential_mode: mode.as_str(),
            oauth_registration: oauth.then_some(if req.oauth_client.is_some() {
                "manual"
            } else {
                "cimd"
            }),
            scopes: &scopes,
            change_policy: policy.as_str(),
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if !added {
        return Err(handle_taken(&handle));
    }
    store::set_projects(&mut tx, org, id, &projects).await?;
    let row = store::get(&mut *tx, org, id)
        .await?
        .ok_or_else(|| ApiError::internal("new MCP server vanished"))?;
    let shown = view(&row, ServerTools::default(), None, &projects);
    ctx.audit(Some(ctx.org()?.id()), Action::McpServerCreate)
        .target("mcp_server", McpServerId::from_uuid(id))
        .after(&shown)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    for p in &projects {
        crate::portal::snapshot::forget(&state, *p).await;
    }
    if let Some(c) = &req.oauth_client {
        signin::set_manual_client(&state, id, &c.client_id, c.client_secret.as_ref())
            .await
            .map_err(ApiError::internal)?;
    }
    let row = store::get(&state.pg, org, id)
        .await?
        .ok_or_else(|| ApiError::internal("new MCP server vanished"))?;
    if req.auth_method == McpAuthMethod::None {
        after_sync(&state, row.clone());
    }
    let shown = view(&row, ServerTools::default(), None, &projects);
    Ok((StatusCode::CREATED, etag(row.revision), Json(shown)))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Changes to an MCP server; absent fields stay as they are.")]
pub struct McpServerUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new handle; the tools' names outside Kaeso change with it."]
    pub handle: Option<String>,
    #[doc = "A new policy for changed tools. Switching to `accept` doesn't approve tools held now."]
    pub change_policy: Option<McpChangePolicy>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/mcp-servers/{server}",
    summary = "Renames an MCP server, changes its handle or its policy for changed tools.",
    description = "With `If-Match`, only while the server is still at that revision; a newer one\n\
    answers 412.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    request_body = McpServerUpdate,
    responses((status = 200, body = McpServerView, headers(("ETag" = String, description = "The server's revision as an entity tag."))))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
    if_match: IfMatch,
    Body(req): Body<McpServerUpdate>,
) -> ApiResult<(Tagged, Json<McpServerView>)> {
    let org = ctx.org()?.org.id;
    let before_row = server_row(&state, &ctx, &server).await?;
    if_match.check(before_row.revision, "MCP server")?;
    let name = req.name.as_deref().map(checked_name).transpose()?;
    let handle = req.handle.as_deref().map(checked_handle).transpose()?;
    let before = one(&state, &before_row).await?;
    let mut tx = state.pg.begin().await?;
    let row = match store::update(
        &mut *tx,
        org,
        before_row.id,
        &Changes {
            name: name.as_deref(),
            handle,
            change_policy: req.change_policy.map(McpChangePolicy::as_str),
        },
    )
    .await?
    {
        Updated::Done(row) => row,
        Updated::NotFound => return Err(ApiError::not_found("MCP server")),
        Updated::HandleTaken => return Err(handle_taken(handle.unwrap_or_default())),
    };
    let after = view(
        &row,
        ServerTools {
            total: before.tools.total,
            changed: before.tools.changed,
        },
        None,
        &[],
    );
    let after = McpServerView {
        connection: before.connection.clone(),
        projects: before.projects.clone(),
        ..after
    };
    ctx.audit(Some(ctx.org()?.id()), Action::McpServerUpdate)
        .target("mcp_server", McpServerId::from_uuid(row.id))
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::portal::snapshot::forget_server(&state, row.id).await;
    Ok((etag(row.revision), Json(after)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/mcp-servers/{server}",
    summary = "Deletes an MCP server with its sign-ins and tools; every project's portal stops\n\
    offering them.",
    description = "With `If-Match`, only while the server is still at that revision; a newer one\n\
    answers 412. The audit log keeps what was removed.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
    if_match: IfMatch,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?.org.id;
    let row = server_row(&state, &ctx, &server).await?;
    if_match.check(row.revision, "MCP server")?;
    let before = one(&state, &row).await?;
    let mut tx = state.pg.begin().await?;
    if !store::delete(&mut *tx, org, row.id).await? {
        return Err(ApiError::not_found("MCP server"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::McpServerDelete)
        .target("mcp_server", McpServerId::from_uuid(row.id))
        .before(&before)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    for p in &before.projects {
        crate::portal::snapshot::forget(&state, p.uuid()).await;
    }
    signin::forget(&state, row.id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Starts an OAuth sign-in.")]
pub struct McpOAuthStart {
    #[doc = "A client registered by hand, which replaces the one Kaeso has; absent keeps it."]
    #[serde(default)]
    pub client: Option<McpOAuthClient>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The shared sign-in: a secret header, or the start of an OAuth sign-in.")]
pub struct McpConnectionSet {
    #[doc = "For `header` servers: the header Kaeso sends from now on."]
    #[serde(default)]
    pub header: Option<McpHeader>,
    #[doc = "For `oauth` servers: start a sign-in in the browser."]
    #[serde(default)]
    pub oauth: Option<McpOAuthStart>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The server after a sign-in was set or started.")]
pub struct McpConnectionAnswer {
    #[doc = "For OAuth: open this address in the browser to sign in. Kaeso sends the person back\n\
        to the console afterwards. Valid for 10 minutes, for the person who started it."]
    pub redirect_url: Option<String>,
    #[doc = "The server, with its connection."]
    pub server: McpServerView,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers/{server}/connection",
    summary = "Sets the shared secret header of an MCP server, or starts its OAuth sign-in.",
    description = "A header replaces the one Kaeso had, and Kaeso lists the server's tools with it.\n\
    An OAuth sign-in answers with `redirect_url`; the person who started it finishes it in the\n\
    browser, signed in to Kaeso.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path)),
    request_body = McpConnectionSet,
    responses((status = 200, body = McpConnectionAnswer))
)]
pub async fn connect(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
    Body(req): Body<McpConnectionSet>,
) -> ApiResult<Json<McpConnectionAnswer>> {
    let row = server_row(&state, &ctx, &server).await?;
    if row.credential_mode == "per_user" {
        return Err(ApiError::Conflict(
            "Each person connects their own account to this server, under Account › Connections."
                .into(),
        ));
    }
    match (&req.header, &req.oauth, row.auth_method.as_str()) {
        (Some(h), None, "header") => {
            signin::set_header(&state, &row, None, &h.name, &h.value)
                .await
                .map_err(|e| match e {
                    SignInError::BadHeader(p) => header_problem(p),
                    other => ApiError::internal(other),
                })?;
            let mut tx = state.pg.begin().await?;
            ctx.audit(Some(ctx.org()?.id()), Action::McpServerConnect)
                .target("mcp_server", McpServerId::from_uuid(row.id))
                .after(serde_json::json!({ "kind": "header", "header_name": h.name }))
                .write(&mut tx)
                .await?;
            tx.commit().await?;
            if let Err(e) = sync::sync(&state, &row).await {
                tracing::info!(server = %row.id, error = %e, "an MCP server couldn't be synced");
            }
            let row = server_row(&state, &ctx, &server).await?;
            Ok(Json(McpConnectionAnswer {
                redirect_url: None,
                server: one(&state, &row).await?,
            }))
        }
        (None, Some(start), "oauth") => {
            let Some(user) = ctx.acting_user() else {
                return Err(ApiError::Conflict(
                    "A person signs in to an MCP server; start it from a console session.".into(),
                ));
            };
            if let Some(c) = &start.client {
                if !(1..=2048).contains(&c.client_id.len()) {
                    return Err(ApiError::invalid(
                        "oauth.client.client_id",
                        "`client_id` must be 1 to 2048 characters.",
                    ));
                }
                signin::set_manual_client(&state, row.id, &c.client_id, c.client_secret.as_ref())
                    .await
                    .map_err(ApiError::internal)?;
            }
            let row = server_row(&state, &ctx, &server).await?;
            let begun = signin::begin(&state, &row, None, user.uuid(), None)
                .await
                .map_err(sign_in_refused)?;
            Ok(Json(McpConnectionAnswer {
                redirect_url: Some(begun.url),
                server: one(&state, &row).await?,
            }))
        }
        (None, None, _) | (Some(_), Some(_), _) => {
            Err(ApiError::bad_request("Send either `header` or `oauth`."))
        }
        _ => Err(ApiError::invalid(
            if req.header.is_some() {
                "header"
            } else {
                "oauth"
            },
            match row.auth_method.as_str() {
                "oauth" => "This server signs in with OAuth; send `oauth`.",
                "header" => "This server takes a secret header; send `header`.",
                _ => "This server needs no sign-in.",
            },
        )),
    }
}

fn sign_in_refused(e: SignInError) -> ApiError {
    let message = match &e {
        SignInError::Unreachable(_) => {
            "Kaeso couldn't reach the server's sign-in; check the address and try again."
        }
        SignInError::NoAuthServer => "The server names no OAuth sign-in Kaeso can use.",
        SignInError::NoPkce => "The server's sign-in doesn't support PKCE, which Kaeso requires.",
        SignInError::NoRegistration => {
            "The server's sign-in needs a client registered by hand; send `oauth.client`."
        }
        SignInError::Refused(_) => {
            "The server's sign-in refused Kaeso's client; check it and try again."
        }
        _ => return ApiError::internal(e),
    };
    tracing::info!(error = %e, "an MCP sign-in couldn't start");
    ApiError::Provider {
        status: StatusCode::BAD_GATEWAY,
        message: message.to_owned(),
        retry_after: None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Why Kaeso left a tool out.")]
pub enum McpSkipReason {
    #[doc = "It has no name, or one longer than 256 characters."]
    NoName,
    #[doc = "The server lists the name twice."]
    ListedTwice,
    #[doc = "It has no input schema."]
    NoInputSchema,
    #[doc = "An `x-mcp-header` annotation in its input schema is one clients refuse."]
    HeaderAnnotation,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool Kaeso left out.")]
pub struct McpSkippedTool {
    #[doc = "Its name, as far as it has one."]
    pub name: String,
    #[doc = "Why."]
    pub reason: McpSkipReason,
    #[doc = "The header annotation at fault, for `header_annotation`."]
    pub header: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "What a fresh look at an MCP server's tools found.")]
pub struct McpRefreshView {
    #[doc = "New tools."]
    pub added: Vec<String>,
    #[doc = "Tools whose definition changed and that are held until someone approves them."]
    pub held: Vec<String>,
    #[doc = "Tools whose change was approved as it came, under the policy `accept`."]
    pub accepted: Vec<String>,
    #[doc = "How many tools the server no longer lists."]
    pub removed: usize,
    #[doc = "Tools Kaeso left out."]
    pub skipped: Vec<McpSkippedTool>,
    #[doc = "The server after the check."]
    pub server: McpServerView,
}

fn skipped(name: String, skip: Skip) -> McpSkippedTool {
    let (reason, header) = match skip {
        Skip::NoName => (McpSkipReason::NoName, None),
        Skip::ListedTwice => (McpSkipReason::ListedTwice, None),
        Skip::NoInputSchema => (McpSkipReason::NoInputSchema, None),
        Skip::Header(
            ParamError::Name(h)
            | ParamError::Duplicate(h)
            | ParamError::NotPrimitive(h)
            | ParamError::Unreachable(h),
        ) => (McpSkipReason::HeaderAnnotation, Some(h)),
    };
    McpSkippedTool {
        name,
        reason,
        header,
    }
}

fn sync_failed(e: &SyncError) -> ApiError {
    let message = match e.status() {
        "signed_out" => "Kaeso has no working sign-in for this server; connect it again.",
        "unreachable" => "Kaeso couldn't reach this server; check its address and try again.",
        _ => "The server answered with an error; see its last error and try again later.",
    };
    ApiError::Provider {
        status: StatusCode::BAD_GATEWAY,
        message: message.to_owned(),
        retry_after: None,
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers/{server}/refresh",
    summary = "Asks an MCP server for its tools now, instead of waiting for the next check.",
    description = "New tools are added, removed ones marked, and changed ones held under the\n\
    policy `hold`. A server Kaeso can't reach or sign in to answers 502, and its `status` and\n\
    `last_error` say why.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path)),
    responses((status = 200, body = McpRefreshView))
)]
pub async fn refresh(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
) -> ApiResult<Json<McpRefreshView>> {
    let row = server_row(&state, &ctx, &server).await?;
    let synced = match sync::sync(&state, &row).await {
        Ok(s) => s,
        Err(SyncError::Store(e)) => return Err(e.into()),
        Err(e) => {
            tracing::info!(server = %row.id, error = %e, "an MCP server couldn't be refreshed");
            return Err(sync_failed(&e));
        }
    };
    let row = server_row(&state, &ctx, &server).await?;
    Ok(Json(McpRefreshView {
        added: synced.added,
        held: synced.changed,
        accepted: synced.accepted,
        removed: synced.removed,
        skipped: synced
            .skipped
            .into_iter()
            .map(|(n, s)| skipped(n, s))
            .collect(),
        server: one(&state, &row).await?,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Where a tool stands.")]
pub enum McpToolState {
    #[doc = "Its definition is approved; projects can offer it."]
    Active,
    #[doc = "The server changed it; it's held until someone approves the change."]
    Changed,
    #[doc = "The server no longer lists it."]
    Removed,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool of an MCP server, as the server describes it now.")]
pub struct McpToolView {
    #[doc = "Id."]
    pub id: ToolId,
    #[doc = "Its name on the server."]
    #[schema(example = "create_post")]
    pub name: String,
    #[doc = "Its name for clients of a portal: the server's handle, two underscores and the\n\
        name, cut to 64 characters."]
    #[schema(example = "x__create_post")]
    pub exposed_name: String,
    #[doc = "Its title."]
    pub title: Option<String>,
    #[doc = "Its description, as clients and their models read it."]
    pub description: String,
    #[doc = "Its description is longer than 2,000 characters; read it all before approving."]
    pub long_description: bool,
    #[doc = "Its input schema."]
    #[schema(value_type = Object)]
    pub input_schema: Value,
    #[doc = "Its output schema."]
    #[schema(value_type = Option<Object>)]
    pub output_schema: Option<Value>,
    #[doc = "Hints such as `readOnlyHint`; claims of the server, not guarantees."]
    #[schema(value_type = Option<Object>)]
    pub annotations: Option<Value>,
    #[doc = "Where it stands."]
    pub state: McpToolState,
    #[doc = "SHA-256 of its definition (name, title, description, schemas and annotations as\n\
        canonical JSON), in hex."]
    pub hash: String,
    #[doc = "The hash of the definition last approved; differs from `hash` while it's held."]
    pub approved_hash: Option<String>,
    #[doc = "First listed."]
    #[serde(with = "time::serde::rfc3339")]
    pub first_seen_at: OffsetDateTime,
    #[doc = "Last changed by the server."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub changed_at: Option<OffsetDateTime>,
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() != 64 {
        return None;
    }
    (0..32)
        .map(|i| {
            s.get(i * 2..i * 2 + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect()
}

pub(crate) fn tool_state(t: &ToolRow) -> McpToolState {
    match t.state.as_str() {
        "removed" => McpToolState::Removed,
        _ if t.is_approved() => McpToolState::Active,
        _ => McpToolState::Changed,
    }
}

pub(crate) fn tool_view(handle: &str, t: &ToolRow) -> McpToolView {
    McpToolView {
        id: ToolId::from_uuid(t.id),
        name: t.name.clone(),
        exposed_name: names::exposed(handle, &t.name),
        title: t.title.clone(),
        description: t.description.clone(),
        long_description: t.description.chars().count() > sync::LONG_DESCRIPTION,
        input_schema: t.input_schema.clone(),
        output_schema: t.output_schema.clone(),
        annotations: t.annotations.clone(),
        state: tool_state(t),
        hash: hex(&t.definition_hash),
        approved_hash: t.approved_hash.as_deref().map(hex),
        first_seen_at: t.first_seen_at,
        changed_at: t.changed_at,
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/mcp-servers/{server}/tools",
    summary = "An MCP server's tools with their definition, hash and state, by name.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path)),
    responses((status = 200, body = List<McpToolView>))
)]
pub async fn list_tools(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server)): Path<(String, String)>,
) -> ApiResult<Json<List<McpToolView>>> {
    let row = server_row(&state, &ctx, &server).await?;
    let mut rows = tools::of_server(&state.pg, row.id).await?;
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(List::complete(
        rows.iter().map(|t| tool_view(&row.handle, t)).collect(),
    )))
}

async fn tool_row(
    state: &AppState,
    ctx: &Ctx,
    server: &ServerRow,
    raw: &str,
) -> ApiResult<ToolRow> {
    let id: ToolId = id_param(raw, "tool")?;
    tools::get(&state.pg, ctx.org()?.org.id, id.uuid())
        .await?
        .filter(|t| t.server_id == server.id)
        .ok_or_else(|| ApiError::not_found("tool"))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool's approved and current definitions, to review a change.")]
pub struct McpToolChange {
    #[doc = "The tool."]
    pub tool: McpToolView,
    #[doc = "The definition last approved: `name`, `title`, `description`, `inputSchema`,\n\
        `outputSchema` and `annotations` as the server sent them; `null` if none was."]
    #[schema(value_type = Option<Object>)]
    pub approved: Option<Value>,
    #[doc = "The definition the server lists now, in the same form."]
    #[schema(value_type = Object)]
    pub current: Value,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/mcp-servers/{server}/tools/{tool}/change",
    summary = "A tool's approved and current definition, to review a change before approving it.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path), ("tool" = ToolId, Path)),
    responses((status = 200, body = McpToolChange))
)]
pub async fn change(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server, tool)): Path<(String, String, String)>,
) -> ApiResult<Json<McpToolChange>> {
    let row = server_row(&state, &ctx, &server).await?;
    let t = tool_row(&state, &ctx, &row, &tool).await?;
    Ok(Json(McpToolChange {
        tool: tool_view(&row.handle, &t),
        approved: t.approved.clone(),
        current: sync::definition_of(&t),
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The definition being approved.")]
pub struct McpToolApprove {
    #[doc = "The `hash` of the definition you reviewed; if the server changed the tool again\n\
        since, nothing is approved."]
    pub hash: String,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers/{server}/tools/{tool}/approve",
    summary = "Approves a tool's changed definition; projects that have it on offer it again.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path), ("tool" = ToolId, Path)),
    request_body = McpToolApprove,
    responses((status = 200, body = McpToolView))
)]
pub async fn approve(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server, tool)): Path<(String, String, String)>,
    Body(req): Body<McpToolApprove>,
) -> ApiResult<Json<McpToolView>> {
    let org = ctx.org()?.org.id;
    let row = server_row(&state, &ctx, &server).await?;
    let t = tool_row(&state, &ctx, &row, &tool).await?;
    let hash = unhex(&req.hash.to_ascii_lowercase()).ok_or_else(|| {
        ApiError::invalid("hash", "`hash` must be the tool's 64-character hex hash.")
    })?;
    if t.state == "removed" {
        return Err(ApiError::Conflict(
            "The server no longer lists this tool.".into(),
        ));
    }
    if !sync::approve(&state, org, t.id, &hash).await? {
        return Err(ApiError::Conflict(
            "The tool changed again since you reviewed it; review it again.".into(),
        ));
    }
    let after = tool_row(&state, &ctx, &row, &tool).await?;
    let mut tx = state.pg.begin().await?;
    ctx.audit(Some(ctx.org()?.id()), Action::ToolApprove)
        .target("tool", ToolId::from_uuid(t.id))
        .before(serde_json::json!({ "hash": hex(t.approved_hash.as_deref().unwrap_or_default()), "definition": t.approved }))
        .after(serde_json::json!({ "hash": hex(&after.definition_hash), "definition": sync::definition_of(&after) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(tool_view(&row.handle, &after)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/mcp-servers/{server}/tools/{tool}/reject",
    summary = "Keeps a changed tool off: it stays held and is switched off in every project.",
    description = "Turn it on again in a project once the server changes it back or someone\n\
    approves a later definition.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("server" = McpServerId, Path), ("tool" = ToolId, Path)),
    responses((status = 200, body = McpToolView))
)]
pub async fn reject(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, server, tool)): Path<(String, String, String)>,
) -> ApiResult<Json<McpToolView>> {
    let org = ctx.org()?.org.id;
    let row = server_row(&state, &ctx, &server).await?;
    let t = tool_row(&state, &ctx, &row, &tool).await?;
    if tool_state(&t) != McpToolState::Changed {
        return Err(ApiError::Conflict(
            "This tool has no change waiting for review.".into(),
        ));
    }
    let projects = tools::projects_using(&state.pg, t.id).await?;
    sync::reject(&state, org, t.id).await?;
    let mut tx = state.pg.begin().await?;
    ctx.audit(Some(ctx.org()?.id()), Action::ToolReject)
        .target("tool", ToolId::from_uuid(t.id))
        .after(serde_json::json!({
            "hash": hex(&t.definition_hash),
            "switched_off_in": projects.into_iter().map(ProjectId::from_uuid).collect::<Vec<_>>(),
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(tool_view(&row.handle, &t)))
}
