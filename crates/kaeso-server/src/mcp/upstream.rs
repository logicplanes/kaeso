use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use kaeso_core::id::{McpServerId, OrgId, UserId};
use kaeso_store::pg::mcp_servers;

use super::oauth::endpoint;
use crate::auth::sessions;
use crate::platform::audit::{self, Action};
use crate::platform::cookies;
use crate::services::mcp_connections::{self, CallbackParams, Finished, SignInError};
use crate::services::mcp_tools;
use crate::state::AppState;

pub async fn client_document(State(state): State<AppState>) -> Response {
    let mut r = Json(mcp_connections::client_document(&state)).into_response();
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    r
}

fn back(state: &AppState, f: Finished, result: &str) -> String {
    let server = McpServerId::from_uuid(f.server);
    match f.user {
        Some(_) => endpoint(
            state,
            &format!("settings/account/connections?server={server}&result={result}"),
        ),
        None => endpoint(
            state,
            &format!(
                "orgs/{}/mcp-servers?server={server}&result={result}",
                OrgId::from_uuid(f.org)
            ),
        ),
    }
}

fn result_of(e: &SignInError) -> &'static str {
    match e {
        SignInError::Denied(_) => "declined",
        SignInError::Expired => "expired",
        _ => "failed",
    }
}

async fn connected(state: &AppState, f: Finished, by: uuid::Uuid) {
    let recorded = async {
        let mut conn = state.pg.acquire().await?;
        audit::user_event(
            &mut conn,
            OrgId::from_uuid(f.org),
            UserId::from_uuid(by),
            Action::McpServerConnect,
            ("mcp_server", McpServerId::from_uuid(f.server).to_string()),
            Some(serde_json::json!({ "kind": "oauth", "person": f.user.is_some() })),
            state.clock.now(),
        )
        .await
    };
    if let Err(e) = recorded.await {
        tracing::error!(error = %e, "an MCP sign-in couldn't be audited");
    }
    let st = state.clone();
    state.in_flight.finish(async move {
        match mcp_servers::get_by_id(&st.pg, f.server).await {
            Ok(Some(server)) => {
                if let Err(e) = mcp_tools::sync(&st, &server).await {
                    tracing::info!(server = %server.id, error = %e, "an MCP server couldn't be synced");
                }
            }
            Ok(None) => {}
            Err(e) => tracing::error!(error = %e, "an MCP server couldn't be loaded for a sync"),
        }
    });
}

pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let mut params = CallbackParams::default();
    for (k, v) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        let slot = match k.as_ref() {
            "code" => &mut params.code,
            "state" => &mut params.state,
            "iss" => &mut params.iss,
            "error" => &mut params.error,
            "error_description" => &mut params.error_description,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(v.into_owned());
        }
    }
    let user = match cookies::session_token(&state, &headers) {
        Some(token) => match sessions::authenticate(&state, &token).await {
            Ok(row) => row.map(|r| r.user_id),
            Err(e) => {
                tracing::error!(error = %e, "an MCP sign-in couldn't read the session");
                None
            }
        },
        None => None,
    };
    match mcp_connections::finish(&state, &params, user).await {
        Ok(f) => {
            if let Some(by) = user {
                connected(&state, f, by).await;
            }
            Redirect::to(&back(&state, f, "connected")).into_response()
        }
        Err((Some(f), e)) => {
            tracing::warn!(error = %e, server = %f.server, "an MCP sign-in didn't finish");
            Redirect::to(&back(&state, f, result_of(&e))).into_response()
        }
        Err((None, e)) => {
            tracing::warn!(error = %e, "an MCP sign-in didn't finish");
            Redirect::to(&endpoint(&state, "?mcp_sign_in=expired")).into_response()
        }
    }
}
