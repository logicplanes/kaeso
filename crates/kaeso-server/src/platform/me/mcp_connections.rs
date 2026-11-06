use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{McpServerId, OrgId};
use kaeso_store::pg::mcp_connections as connections;
use kaeso_store::pg::mcp_servers::{self as servers, ServerRow};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::{self, Action};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::orgs::mcp_servers::{
    McpAuthMethod, McpConnectionView, McpHeader, connection_view, header_problem, server_param,
    sign_in_refused,
};
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::services::mcp_connections::{self as signin, SignInError};
use crate::services::mcp_tools as sync;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An MCP server where each person signs in with their own account.")]
pub struct PersonConnectionView {
    #[doc = "The server."]
    pub server: McpServerId,
    #[doc = "Its name."]
    #[schema(example = "X")]
    pub server_name: String,
    #[doc = "The prefix of its tools' names."]
    #[schema(example = "x")]
    pub server_handle: String,
    #[doc = "Its organization."]
    pub org: OrgId,
    #[doc = "How you sign in to it: `header` or `oauth`."]
    pub auth_method: McpAuthMethod,
    #[doc = "Your connection, without its secret; `null` until you connect."]
    pub connection: Option<McpConnectionView>,
}

fn method(s: &str) -> McpAuthMethod {
    if s == "header" {
        McpAuthMethod::Header
    } else {
        McpAuthMethod::Oauth
    }
}

async fn views(state: &AppState, user: Uuid) -> ApiResult<Vec<PersonConnectionView>> {
    let mine = connections::of_user(&state.pg, user).await?;
    Ok(servers::per_person_of_member(&state.pg, user)
        .await?
        .into_iter()
        .map(|s| PersonConnectionView {
            server: McpServerId::from_uuid(s.id),
            connection: mine
                .iter()
                .find(|c| c.connection.server_id == s.id)
                .map(|c| connection_view(&c.connection)),
            server_name: s.name,
            server_handle: s.handle,
            org: OrgId::from_uuid(s.org_id),
            auth_method: method(&s.auth_method),
        })
        .collect())
}

async fn own_server(state: &AppState, user: Uuid, raw: &str) -> ApiResult<ServerRow> {
    let id = server_param(raw)?;
    servers::per_person_of_member(&state.pg, user)
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ApiError::not_found("MCP server"))
}

#[utoipa::path(
    get,
    path = "/me/mcp-connections",
    summary = "The MCP servers where you sign in with your own account, with your connection.",
    tag = "me",
    responses((status = 200, body = List<PersonConnectionView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<List<PersonConnectionView>>> {
    let user = ctx.user_id()?.uuid();
    Ok(Json(List::complete(views(&state, user).await?)))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Your own sign-in to an MCP server: a secret header, or an OAuth sign-in.")]
pub struct PersonConnectionSet {
    #[doc = "For `header` servers: the header Kaeso sends for you."]
    #[serde(default)]
    pub header: Option<McpHeader>,
    #[doc = "For `oauth` servers: `true` starts a sign-in in the browser."]
    #[serde(default)]
    pub oauth: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Your connection after it was set, or the sign-in that sets it.")]
pub struct PersonConnectionAnswer {
    #[doc = "For OAuth: open this address to sign in; Kaeso sends you back to Account ›\n\
        Connections afterwards. Valid for 10 minutes."]
    pub redirect_url: Option<String>,
    #[doc = "The server with your connection."]
    pub connection: PersonConnectionView,
}

async fn record(
    state: &AppState,
    server: &ServerRow,
    user: Uuid,
    action: Action,
    after: serde_json::Value,
) -> ApiResult<()> {
    let mut conn = state.pg.acquire().await?;
    audit::user_event(
        &mut conn,
        OrgId::from_uuid(server.org_id),
        kaeso_core::id::UserId::from_uuid(user),
        action,
        ("mcp_server", McpServerId::from_uuid(server.id).to_string()),
        Some(after),
        state.clock.now(),
    )
    .await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/me/mcp-connections/{server}",
    summary = "Connects your own account to an MCP server where each person signs in.",
    description = "A header replaces the one Kaeso had for you, and Kaeso lists the server's\n\
    tools with it. An OAuth sign-in answers with `redirect_url`.",
    tag = "me",
    params(("server" = McpServerId, Path)),
    request_body = PersonConnectionSet,
    responses((status = 200, body = PersonConnectionAnswer))
)]
pub async fn connect(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(server): Path<String>,
    Body(req): Body<PersonConnectionSet>,
) -> ApiResult<Json<PersonConnectionAnswer>> {
    let user = ctx.user_id()?.uuid();
    let row = own_server(&state, user, &server).await?;
    let redirect_url = match (&req.header, req.oauth, row.auth_method.as_str()) {
        (Some(h), false, "header") => {
            signin::set_header(&state, &row, Some(user), &h.name, &h.value)
                .await
                .map_err(|e| match e {
                    SignInError::BadHeader(p) => header_problem(p),
                    other => ApiError::internal(other),
                })?;
            record(
                &state,
                &row,
                user,
                Action::McpServerConnect,
                serde_json::json!({ "kind": "header", "person": true }),
            )
            .await?;
            if let Err(e) = sync::sync(&state, &row).await {
                tracing::info!(server = %row.id, error = %e, "an MCP server couldn't be synced");
            }
            None
        }
        (None, true, "oauth") => Some(
            signin::begin(&state, &row, Some(user), user, None)
                .await
                .map_err(sign_in_refused)?
                .url,
        ),
        (None, false, _) | (Some(_), true, _) => {
            return Err(ApiError::bad_request("Send either `header` or `oauth`."));
        }
        _ => {
            return Err(ApiError::invalid(
                if req.header.is_some() {
                    "header"
                } else {
                    "oauth"
                },
                if row.auth_method == "oauth" {
                    "This server signs in with OAuth; send `oauth`."
                } else {
                    "This server takes a secret header; send `header`."
                },
            ));
        }
    };
    let connection = views(&state, user)
        .await?
        .into_iter()
        .find(|v| v.server.uuid() == row.id)
        .ok_or_else(|| ApiError::not_found("MCP server"))?;
    Ok(Json(PersonConnectionAnswer {
        redirect_url,
        connection,
    }))
}

#[utoipa::path(
    delete,
    path = "/me/mcp-connections/{server}",
    summary = "Removes your own connection to an MCP server; its tools stop working for you.",
    tag = "me",
    params(("server" = McpServerId, Path)),
    responses((status = 204))
)]
pub async fn disconnect(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(server): Path<String>,
) -> ApiResult<StatusCode> {
    let user = ctx.user_id()?.uuid();
    let row = own_server(&state, user, &server).await?;
    if !connections::delete(&state.pg, row.id, Some(user)).await? {
        return Err(ApiError::not_found("connection"));
    }
    signin::forget(&state, row.id).await;
    record(
        &state,
        &row,
        user,
        Action::McpServerDisconnect,
        serde_json::json!({ "person": true }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
