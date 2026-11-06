use std::net::IpAddr;

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::ProjectId;
use kaeso_core::rights::ProjectRight;
use serde_json::json;
use uuid::Uuid;

use kaeso_core::rights::effective_project_rights;
use kaeso_core::secret;
use kaeso_store::pg::{members, portal_grants};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::auth::keys::{self, KeyCaller, LookupError};
use crate::mcp::oauth;
use crate::state::AppState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub org: Uuid,
    pub project: Uuid,
    pub key: Option<Uuid>,
    pub grant: Option<Uuid>,
    pub user: Option<Uuid>,
    pub service_account: Option<Uuid>,
    pub client: Option<String>,
}

// What a portal access token stands for, kept by the token's hash so a warm call reads no row;
// a revoked grant drops its tokens here, and anything else reaches every instance within 30 s.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAuth {
    pub grant: Uuid,
    pub user: Uuid,
    pub org: Uuid,
    pub project: Uuid,
    pub client: String,
    pub resource: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    pub active: bool,
    pub tools_use: bool,
}

fn token_key(hash: &[u8]) -> String {
    crate::cache::hex(hash)
}

pub async fn forget(state: &AppState, hash: &[u8]) {
    state
        .caches
        .portal_tokens
        .invalidate(&token_key(hash))
        .await;
}

pub async fn forget_grant(state: &AppState, grant: Uuid) {
    match portal_grants::access_hashes(&state.pg, grant).await {
        Ok(hashes) => {
            for h in hashes {
                forget(state, &h).await;
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "a revoked portal grant's tokens couldn't be dropped from the cache; they end within 30 seconds");
        }
    }
}

async fn person_uses_tools(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    user: Uuid,
) -> Result<bool, kaeso_store::StoreError> {
    let mut conn = state.pg.acquire().await?;
    let Some(found) = members::access_context(&mut conn, org, Some(user), Some(project)).await?
    else {
        return Ok(false);
    };
    let org_grant = found
        .org_grant
        .as_ref()
        .and_then(|g| crate::platform::grants::org_from_store(g).ok());
    let project_grant = found
        .project_grant
        .as_ref()
        .and_then(|g| crate::platform::grants::project_from_store(g).ok());
    Ok(org_grant.is_some()
        && found.project.is_some()
        && effective_project_rights(org_grant.as_ref(), project_grant.as_ref())
            .contains(ProjectRight::ToolsUse))
}

async fn token(state: &AppState, presented: &str, project: Uuid) -> Result<Caller, Response> {
    let hash = secret::hash(presented);
    let st = state.clone();
    let looked = state
        .caches
        .portal_tokens
        .get_or_load(&token_key(&hash), || async move {
            let Some(a) = portal_grants::authenticate(&st.pg, &hash, st.clock.now()).await? else {
                return Ok(None);
            };
            let tools_use = person_uses_tools(&st, a.org_id, a.project_id, a.user_id).await?;
            Ok::<_, kaeso_store::StoreError>(Some(TokenAuth {
                grant: a.grant_id,
                user: a.user_id,
                org: a.org_id,
                project: a.project_id,
                client: a.client_name,
                resource: a.resource,
                expires_at: a.token_expires_at,
                active: a.user_state == "active",
                tools_use,
            }))
        })
        .await;
    let found = match looked {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "a portal token couldn't be checked");
            return Err(refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                "Kaeso can't check sign-ins right now; try again shortly.",
            ));
        }
    };
    let now = state.clock.now();
    let Some(a) = found.filter(|a| a.expires_at.is_none_or(|e| now < e) && a.active) else {
        metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "portal", "reason" => "refused").increment(1);
        return Err(challenge(
            state,
            project,
            true,
            "This sign-in has ended or was revoked; sign in to the portal again.",
        ));
    };
    if a.resource != oauth::portal_resource(state, project) || a.project != project {
        metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "portal", "reason" => "resource").increment(1);
        return Err(challenge(
            state,
            project,
            true,
            "This sign-in is for another resource; sign in to this project's portal.",
        ));
    }
    if !a.tools_use {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "You may not call tools in this project; the right `tools.use` is missing.",
        ));
    }
    Ok(Caller {
        org: a.org,
        project,
        key: None,
        grant: Some(a.grant),
        user: Some(a.user),
        service_account: None,
        client: Some(a.client.clone()),
    })
}

impl Caller {
    pub fn kind(&self) -> &'static str {
        if self.grant.is_some() {
            "portal_grant"
        } else {
            "api_key"
        }
    }

    pub fn id(&self) -> Uuid {
        self.key.or(self.grant).unwrap_or_default()
    }
}

pub fn resource_metadata(state: &AppState, project: Uuid) -> String {
    oauth::endpoint(
        state,
        &format!(
            ".well-known/oauth-protected-resource/mcp/{}",
            ProjectId::from_uuid(project)
        ),
    )
}

pub fn refused(status: StatusCode, kind: &str, message: &str) -> Response {
    let mut r = (
        status,
        axum::Json(json!({ "error": { "type": kind, "message": message } })),
    )
        .into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

// RFC 9728 §5.1: the challenge names this portal's own metadata, so a client signs in for this
// project and no other.
pub fn challenge(state: &AppState, project: Uuid, invalid: bool, message: &str) -> Response {
    let mut value = format!(
        "Bearer resource_metadata=\"{}\"",
        resource_metadata(state, project)
    );
    if invalid {
        value.push_str(", error=\"invalid_token\"");
    }
    let mut r = refused(StatusCode::UNAUTHORIZED, "authentication", message);
    if let Ok(v) = HeaderValue::from_str(&value) {
        r.headers_mut().insert(header::WWW_AUTHENTICATE, v);
    }
    r
}

pub async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    ip: Option<IpAddr>,
    project: Uuid,
) -> Result<Caller, Response> {
    let Some(secret) = crate::model_api::auth::presented(headers).filter(|s| !s.is_empty()) else {
        return Err(challenge(
            state,
            project,
            false,
            "Sign in to this project's MCP portal, or send an API key with tool access.",
        ));
    };
    if secret.starts_with(oauth::PORTAL_ACCESS_PREFIX) {
        return token(state, secret, project).await;
    }
    let found = match keys::authenticate(state, secret, ip).await {
        Ok(found) => found,
        Err(LookupError::Throttled(t)) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "portal", "reason" => "throttled").increment(1);
            let mut r = refused(StatusCode::TOO_MANY_REQUESTS, "rate_limited", &t.message());
            r.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(t.retry_after));
            return Err(r);
        }
        Err(LookupError::Store(e)) => {
            tracing::error!(error = %e, "a portal key couldn't be checked");
            return Err(refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                "Kaeso can't check keys right now; try again shortly.",
            ));
        }
    };
    let key = match found {
        Ok(KeyCaller::Project(k)) => k,
        Ok(KeyCaller::Org(_)) => {
            return Err(refused(
                StatusCode::FORBIDDEN,
                "permission_denied",
                "Org platform keys can't call tools; use a project key with tool access.",
            ));
        }
        Err(refusal) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "portal", "reason" => "refused").increment(1);
            return Err(challenge(state, project, true, refusal.message()));
        }
    };
    if key.project_id != project {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "This API key belongs to another project; use a key of this project.",
        ));
    }
    if !key.tools {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "This API key has no tool access; create a key with tool access for the portal.",
        ));
    }
    if !keys::project_rights(&key).contains(ProjectRight::ToolsUse) {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "The owner of this API key may not call tools in its project; the right `tools.use` is missing.",
        ));
    }
    Ok(Caller {
        org: key.org_id,
        project,
        key: Some(key.id),
        grant: None,
        user: key.user_id,
        service_account: key.service_account_id,
        client: None,
    })
}
