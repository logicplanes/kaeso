use std::collections::{BTreeMap, BTreeSet};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{
    ApiKeyId, McpServerId, OrgId, PortalGrantId, ProjectId, ServiceAccountId, ToolId, UserId,
};
use kaeso_mcp::names;
use kaeso_store::pg::catalogue::Subject;
use kaeso_store::pg::mcp_connections as connections;
use kaeso_store::pg::mcp_servers as servers;
use kaeso_store::pg::mcp_tools::{self as tools, ProjectToolRow};
use kaeso_store::pg::portal_grants::{self as grants, GrantRow};
use kaeso_store::pg::{keys, members, service_accounts, tool_access};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::orgs::mcp_servers::{
    McpAuthMethod, McpCredentialMode, McpServerStatus, McpToolState, tool_state,
};
use crate::platform::pagination::List;
use crate::platform::params::{Path, id_param};
use crate::state::AppState;

const TOOLS_ARCHIVED: &str = "The project is archived; its tools cannot change.";

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An MCP server a project uses.")]
pub struct ProjectMcpServerView {
    #[doc = "Id."]
    pub id: McpServerId,
    #[doc = "The prefix of its tools' names."]
    #[schema(example = "x")]
    pub handle: String,
    #[doc = "The name people see."]
    #[schema(example = "X")]
    pub name: String,
    #[doc = "How Kaeso signs in to it."]
    pub auth_method: McpAuthMethod,
    #[doc = "Whose account a call uses."]
    pub credential_mode: McpCredentialMode,
    #[doc = "Whether a shared server has a working sign-in. Always `true` for servers without\n\
        sign-in; for `per_user` servers each person connects their own."]
    pub connected: bool,
    #[doc = "How its last check went."]
    pub status: McpServerStatus,
    #[doc = "Its tools the server lists now."]
    pub tools: i64,
    #[doc = "Of those, tools this project has on."]
    pub tools_on: i64,
    #[doc = "Of those, tools held because their definition changed."]
    pub tools_held: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A tool of one of the project's MCP servers.")]
pub struct ProjectToolView {
    #[doc = "Id."]
    pub id: ToolId,
    #[doc = "Its server."]
    pub server: McpServerId,
    #[doc = "Its server's handle."]
    #[schema(example = "x")]
    pub server_handle: String,
    #[doc = "Its server's name."]
    #[schema(example = "X")]
    pub server_name: String,
    #[doc = "Its name on the server."]
    #[schema(example = "create_post")]
    pub name: String,
    #[doc = "Its name for the portal's clients."]
    #[schema(example = "x__create_post")]
    pub exposed_name: String,
    #[doc = "Its title."]
    pub title: Option<String>,
    #[doc = "Its description."]
    pub description: String,
    #[doc = "Its input schema."]
    #[schema(value_type = Object)]
    pub input_schema: Value,
    #[doc = "Hints such as `readOnlyHint`; claims of the server, not guarantees."]
    #[schema(value_type = Option<Object>)]
    pub annotations: Option<Value>,
    #[doc = "Where it stands at its server."]
    pub state: McpToolState,
    #[doc = "The project has it on."]
    pub enabled: bool,
    #[doc = "The portal offers it: on and `active`."]
    pub offered: bool,
}

fn tool_view(r: &ProjectToolRow) -> ProjectToolView {
    let state = tool_state(&r.tool);
    ProjectToolView {
        id: ToolId::from_uuid(r.tool.id),
        server: McpServerId::from_uuid(r.tool.server_id),
        server_handle: r.server_handle.clone(),
        server_name: r.server_name.clone(),
        name: r.tool.name.clone(),
        exposed_name: names::exposed(&r.server_handle, &r.tool.name),
        title: r.tool.title.clone(),
        description: r.tool.description.clone(),
        input_schema: r.tool.input_schema.clone(),
        annotations: r.tool.annotations.clone(),
        state,
        enabled: r.enabled,
        offered: r.enabled && state == McpToolState::Active,
    }
}

async fn server_views(state: &AppState, project: Uuid) -> ApiResult<Vec<ProjectMcpServerView>> {
    let rows = servers::of_project(&state.pg, project).await?;
    let project_tools = tools::of_project(&state.pg, project).await?;
    let mut counts: BTreeMap<Uuid, (i64, i64, i64)> = BTreeMap::new();
    for t in &project_tools {
        let c = counts.entry(t.tool.server_id).or_default();
        c.0 += 1;
        c.1 += i64::from(t.enabled);
        c.2 += i64::from(tool_state(&t.tool) == McpToolState::Changed);
    }
    let mut out = Vec::with_capacity(rows.len());
    for s in rows {
        let connected = match (s.auth_method.as_str(), s.credential_mode.as_str()) {
            ("none", _) | (_, "per_user") => true,
            _ => connections::of_server(&state.pg, s.id)
                .await?
                .iter()
                .any(|c| c.user_id.is_none() && c.auth_error.is_none()),
        };
        let (total, on, held) = counts.get(&s.id).copied().unwrap_or_default();
        out.push(ProjectMcpServerView {
            id: McpServerId::from_uuid(s.id),
            handle: s.handle,
            name: s.name,
            auth_method: match s.auth_method.as_str() {
                "header" => McpAuthMethod::Header,
                "oauth" => McpAuthMethod::Oauth,
                _ => McpAuthMethod::None,
            },
            credential_mode: if s.credential_mode == "per_user" {
                McpCredentialMode::PerUser
            } else {
                McpCredentialMode::Shared
            },
            connected,
            status: match s.status.as_str() {
                "ok" => McpServerStatus::Ok,
                "unreachable" => McpServerStatus::Unreachable,
                "signed_out" => McpServerStatus::SignedOut,
                "error" => McpServerStatus::Error,
                _ => McpServerStatus::Unchecked,
            },
            tools: total,
            tools_on: on,
            tools_held: held,
        });
    }
    Ok(out)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/mcp-servers",
    summary = "The MCP servers the project uses, by name.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<ProjectMcpServerView>))
)]
pub async fn list_servers(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
) -> ApiResult<Json<List<ProjectMcpServerView>>> {
    let project = ctx.project()?.project.id;
    Ok(Json(List::complete(server_views(&state, project).await?)))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The MCP servers a project uses.")]
pub struct ProjectMcpServersSet {
    #[doc = "Every server the project uses from now on; servers left out are turned off, with\n\
        their tools."]
    pub servers: Vec<McpServerId>,
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/projects/{project}/mcp-servers",
    summary = "Sets the MCP servers the project uses.",
    description = "A server the project starts using has all its tools off; turn them on with\n\
    `PATCH …/tools`. A server it stops using takes its tools out of the portal.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectMcpServersSet,
    responses((status = 200, body = List<ProjectMcpServerView>))
)]
pub async fn set_servers(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
    Body(req): Body<ProjectMcpServersSet>,
) -> ApiResult<Json<List<ProjectMcpServerView>>> {
    let project = super::ensure_active(&ctx, TOOLS_ARCHIVED)?;
    let org = ctx.org()?.org.id;
    let known: BTreeSet<Uuid> = servers::list(&state.pg, org)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();
    let mut wanted = Vec::new();
    for id in &req.servers {
        if !known.contains(&id.uuid()) {
            return Err(ApiError::invalid(
                "servers",
                format!("`{id}` is not an MCP server of this organization."),
            ));
        }
        wanted.push(id.uuid());
    }
    wanted.sort_unstable();
    wanted.dedup();
    let before: Vec<McpServerId> = servers::of_project(&state.pg, project)
        .await?
        .into_iter()
        .map(|s| McpServerId::from_uuid(s.id))
        .collect();
    let mut tx = state.pg.begin().await?;
    servers::set_project_servers(&mut tx, org, project, &wanted).await?;
    ctx.audit(Some(ctx.org()?.id()), Action::ProjectMcpServersUpdate)
        .target("project", ctx.project()?.id())
        .before(serde_json::json!({ "servers": before }))
        .after(serde_json::json!({
            "servers": wanted.iter().copied().map(McpServerId::from_uuid).collect::<Vec<_>>()
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::portal::snapshot::forget(&state, project).await;
    Ok(Json(List::complete(server_views(&state, project).await?)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/tools",
    summary = "The tools of the project's MCP servers, by server handle and name.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<ProjectToolView>))
)]
pub async fn list_tools(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
) -> ApiResult<Json<List<ProjectToolView>>> {
    let project = ctx.project()?.project.id;
    let rows = tools::of_project(&state.pg, project).await?;
    Ok(Json(List::complete(rows.iter().map(tool_view).collect())))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A tool turned on or off.")]
pub struct ProjectToolSwitch {
    #[doc = "The tool."]
    pub id: ToolId,
    #[doc = "On or off."]
    pub enabled: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Tools to turn on or off; others stay as they are.")]
pub struct ProjectToolsUpdate {
    #[doc = "The tools, each with its new switch."]
    pub tools: Vec<ProjectToolSwitch>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/tools",
    summary = "Turns tools of the project's MCP servers on or off.",
    description = "A tool that is on but held because its definition changed stays out of the\n\
    portal until someone approves the change.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectToolsUpdate,
    responses((status = 200, body = List<ProjectToolView>))
)]
pub async fn update_tools(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
    Body(req): Body<ProjectToolsUpdate>,
) -> ApiResult<Json<List<ProjectToolView>>> {
    let project = super::ensure_active(&ctx, TOOLS_ARCHIVED)?;
    let rows = tools::of_project(&state.pg, project).await?;
    let by_id: BTreeMap<Uuid, &ProjectToolRow> = rows.iter().map(|r| (r.tool.id, r)).collect();
    let mut on = Vec::new();
    let mut off = Vec::new();
    let mut changes = Vec::new();
    for s in &req.tools {
        let Some(row) = by_id.get(&s.id.uuid()) else {
            return Err(ApiError::invalid(
                "tools",
                format!("`{}` is not a tool of the project's MCP servers.", s.id),
            ));
        };
        if row.enabled == s.enabled {
            continue;
        }
        changes.push(serde_json::json!({
            "tool": s.id,
            "name": names::exposed(&row.server_handle, &row.tool.name),
            "enabled": s.enabled,
        }));
        if s.enabled {
            on.push(s.id.uuid());
        } else {
            off.push(s.id.uuid());
        }
    }
    if !changes.is_empty() {
        let mut tx = state.pg.begin().await?;
        tools::set_enabled(&mut *tx, project, &on, true).await?;
        tools::set_enabled(&mut *tx, project, &off, false).await?;
        ctx.audit(Some(ctx.org()?.id()), Action::ProjectToolsUpdate)
            .target("project", ctx.project()?.id())
            .after(serde_json::json!({ "tools": changes }))
            .write(&mut tx)
            .await?;
        tx.commit().await?;
        crate::portal::snapshot::forget(&state, project).await;
    }
    let rows = tools::of_project(&state.pg, project).await?;
    Ok(Json(List::complete(rows.iter().map(tool_view).collect())))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The tools a member, service account or key may call.")]
pub struct ToolAccess {
    #[doc = "Every tool the level above allows."]
    pub all: bool,
    #[doc = "The chosen tools (empty with `all`)."]
    pub tools: Vec<ToolId>,
    #[doc = "The tools it may call now: on in the project, approved, and allowed at every level\n\
        above."]
    pub effective: Vec<ToolId>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new tool list: either `all: true`, or `tools`.")]
pub struct ToolAccessUpdate {
    #[doc = "Every tool the level above allows."]
    pub all: Option<bool>,
    #[doc = "The chosen tools."]
    pub tools: Option<Vec<ToolId>>,
}

enum Principal {
    Member(UserId, Uuid),
    Account(ServiceAccountId),
    Key(keys::KeyRow),
}

impl Principal {
    fn subject(&self) -> Subject {
        match self {
            Principal::Member(_, membership) => Subject::Membership(*membership),
            Principal::Account(id) => Subject::ServiceAccount(id.uuid()),
            Principal::Key(k) => Subject::Key(k.id),
        }
    }

    fn target(&self) -> (&'static str, String) {
        match self {
            Principal::Member(user, _) => ("user", user.to_string()),
            Principal::Account(id) => ("service_account", id.to_string()),
            Principal::Key(k) => ("key", ApiKeyId::from_uuid(k.id).to_string()),
        }
    }
}

const PRINCIPAL: &str = "member, service account or key";

async fn principal(state: &AppState, project: Uuid, raw: &str) -> ApiResult<Principal> {
    if let Ok(user) = raw.parse::<UserId>() {
        let m = members::get_project(&state.pg, project, user.uuid())
            .await?
            .ok_or_else(|| ApiError::not_found(PRINCIPAL))?;
        return Ok(Principal::Member(user, m.id));
    }
    if let Ok(account) = raw.parse::<ServiceAccountId>() {
        service_accounts::get(&state.pg, project, account.uuid())
            .await?
            .ok_or_else(|| ApiError::not_found(PRINCIPAL))?;
        return Ok(Principal::Account(account));
    }
    let key: ApiKeyId = id_param(raw, PRINCIPAL)?;
    let row = keys::get(&state.pg, project, key.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found(PRINCIPAL))?;
    Ok(Principal::Key(row))
}

async fn list_of(
    state: &AppState,
    project: Uuid,
    subject: Subject,
) -> ApiResult<Option<BTreeSet<Uuid>>> {
    let mut conn = state.pg.acquire().await?;
    Ok(tool_access::access(&mut conn, project, subject).await?)
}

// What the level above a principal allows: the project's offered tools, narrowed for a key by
// its member's or service account's own list.
async fn above(
    state: &AppState,
    project: Uuid,
    p: &Principal,
    offered: &BTreeSet<Uuid>,
) -> ApiResult<BTreeSet<Uuid>> {
    let Principal::Key(key) = p else {
        return Ok(offered.clone());
    };
    let owner = match (key.user_id, key.service_account_id) {
        (Some(user), _) => members::get_project(&state.pg, project, user)
            .await?
            .map(|m| Subject::Membership(m.id)),
        (None, Some(account)) => Some(Subject::ServiceAccount(account)),
        (None, None) => None,
    };
    let list = match owner {
        Some(s) => list_of(state, project, s).await?,
        None => None,
    };
    Ok(match list {
        None => offered.clone(),
        Some(l) => l.intersection(offered).copied().collect(),
    })
}

async fn project_tools(
    state: &AppState,
    project: Uuid,
) -> ApiResult<(BTreeSet<Uuid>, BTreeSet<Uuid>)> {
    let rows = tools::of_project(&state.pg, project).await?;
    let all = rows.iter().map(|r| r.tool.id).collect();
    let offered = rows
        .iter()
        .filter(|r| r.enabled && tool_state(&r.tool) == McpToolState::Active)
        .map(|r| r.tool.id)
        .collect();
    Ok((all, offered))
}

fn ids(set: &BTreeSet<Uuid>) -> Vec<ToolId> {
    set.iter().copied().map(ToolId::from_uuid).collect()
}

fn access_view(list: Option<&BTreeSet<Uuid>>, above: &BTreeSet<Uuid>) -> ToolAccess {
    let effective = match list {
        None => above.clone(),
        Some(l) => l.intersection(above).copied().collect(),
    };
    ToolAccess {
        all: list.is_none(),
        tools: list.map(ids).unwrap_or_default(),
        effective: ids(&effective),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/tool-access/{principal}",
    summary = "The tools a project member, service account or key may call.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("principal" = String, Path, description = "A member's user id (`usr_…`), a service account (`svc_…`) or a key (`key_…`).")),
    responses((status = 200, body = ToolAccess))
)]
pub async fn get_access(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, raw)): Path<(String, String, String)>,
) -> ApiResult<Json<ToolAccess>> {
    let project = ctx.project()?.project.id;
    let p = principal(&state, project, &raw).await?;
    let list = list_of(&state, project, p.subject()).await?;
    let (_, offered) = project_tools(&state, project).await?;
    let above = above(&state, project, &p, &offered).await?;
    Ok(Json(access_view(list.as_ref(), &above)))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/projects/{project}/tool-access/{principal}",
    summary = "Sets the tools a project member, service account or key may call.",
    description = "A key's list stays within its member's or service account's. Access only narrows:\n\
    a tool must also be on in the project and approved.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("principal" = String, Path, description = "A member's user id (`usr_…`), a service account (`svc_…`) or a key (`key_…`).")),
    request_body = ToolAccessUpdate,
    responses((status = 200, body = ToolAccess))
)]
pub async fn set_access(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, raw)): Path<(String, String, String)>,
    Body(req): Body<ToolAccessUpdate>,
) -> ApiResult<Json<ToolAccess>> {
    let project = super::ensure_active(&ctx, TOOLS_ARCHIVED)?;
    let p = principal(&state, project, &raw).await?;
    if let Principal::Key(k) = &p
        && k.archived_at.is_some()
    {
        return Err(ApiError::Conflict(
            "The key is archived and cannot change.".into(),
        ));
    }
    let list: Option<BTreeSet<Uuid>> = match (req.all, &req.tools) {
        (Some(true), None) => None,
        (None | Some(false), Some(t)) => Some(t.iter().map(|id| id.uuid()).collect()),
        _ => {
            return Err(ApiError::bad_request(
                "Send either `all: true` or a `tools` list.",
            ));
        }
    };
    let (all, offered) = project_tools(&state, project).await?;
    let above = above(&state, project, &p, &offered).await?;
    if let Some(l) = &list {
        let limit = if matches!(p, Principal::Key(_)) {
            &above
        } else {
            &all
        };
        if let Some(id) = l.iter().find(|id| !limit.contains(id)) {
            let message = if matches!(p, Principal::Key(_)) {
                format!(
                    "`{}` is not among the tools the key's member may call.",
                    ToolId::from_uuid(*id)
                )
            } else {
                format!(
                    "`{}` is not a tool of the project's MCP servers.",
                    ToolId::from_uuid(*id)
                )
            };
            return Err(ApiError::invalid("tools", message));
        }
    }
    let as_json = |l: Option<&BTreeSet<Uuid>>| match l {
        None => serde_json::json!({ "all": true }),
        Some(l) => serde_json::json!({ "tools": ids(l) }),
    };
    let mut tx = state.pg.begin().await?;
    let before = tool_access::access(&mut tx, project, p.subject()).await?;
    if !tool_access::set_access(&mut tx, project, p.subject(), list.as_ref()).await? {
        return Err(ApiError::not_found(PRINCIPAL));
    }
    let (kind, id) = p.target();
    ctx.audit(Some(ctx.org()?.id()), Action::ToolAccessUpdate)
        .target(kind, id)
        .before(as_json(before.as_ref()))
        .after(as_json(list.as_ref()))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::portal::snapshot::forget(&state, project).await;
    Ok(Json(access_view(list.as_ref(), &above)))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A client connected to the project's MCP portal on someone's behalf.")]
pub struct PortalGrantView {
    #[doc = "Id."]
    pub id: PortalGrantId,
    #[doc = "The person who connected it."]
    pub user: UserId,
    #[doc = "The client's name, as it calls itself."]
    #[schema(example = "Claude")]
    pub client_name: String,
    #[doc = "Where the client receives its sign-in."]
    #[schema(example = "https://claude.ai/api/mcp/auth_callback")]
    pub redirect_uri: String,
    #[doc = "Connected."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last used."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

fn grant_view(g: &GrantRow) -> PortalGrantView {
    PortalGrantView {
        id: PortalGrantId::from_uuid(g.id),
        user: UserId::from_uuid(g.user_id),
        client_name: g.client_name.clone(),
        redirect_uri: g.redirect_uri.clone(),
        created_at: g.created_at,
        last_used_at: g.last_used_at,
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/portal-grants",
    summary = "The clients connected to the project's MCP portal, newest first.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<PortalGrantView>))
)]
pub async fn list_grants(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p)): Path<(String, String)>,
) -> ApiResult<Json<List<PortalGrantView>>> {
    let project = ctx.project()?.project.id;
    let rows = grants::of_project(&state.pg, project).await?;
    Ok(Json(List::complete(rows.iter().map(grant_view).collect())))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/portal-grants/{grant}",
    summary = "Disconnects a client from the project's MCP portal; its tokens stop working now.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("grant" = PortalGrantId, Path)),
    responses((status = 204))
)]
pub async fn revoke_grant(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, grant)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    let project = ctx.project()?.project.id;
    let id: PortalGrantId = id_param(&grant, "portal client")?;
    let row = grants::get(&state.pg, project, id.uuid())
        .await?
        .filter(|g| g.revoked_at.is_none())
        .ok_or_else(|| ApiError::not_found("portal client"))?;
    let mut tx = state.pg.begin().await?;
    if !grants::revoke(&mut tx, project, row.id, ctx.now).await? {
        return Err(ApiError::not_found("portal client"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::PortalGrantRevoke)
        .target("portal_grant", id)
        .before(grant_view(&row))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
