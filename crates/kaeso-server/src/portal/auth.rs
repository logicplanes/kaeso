use std::net::IpAddr;

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::ProjectId;
use kaeso_core::rights::ProjectRight;
use serde_json::json;
use uuid::Uuid;

use kaeso_core::rights::effective_project_rights;
use kaeso_core::secret;
use kaeso_store::pg::sessions::SessionRow;
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
    pub standing: Standing,
}

/// Whether a grant's person may call the project's tools now, decided as for their sessions:
/// the project open, the organization active, its sign-in rules met by the sign-in that approved
/// the grant, and `tools.use` held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    Allowed,
    Closed,
    NoRight,
    NeedsSso,
    NeedsOtherSignIn,
    Needs2fa,
}

impl Standing {
    fn refusal(self) -> Option<&'static str> {
        match self {
            Standing::Allowed => None,
            Standing::Closed => Some(
                "This project is archived or its organization is closing; its tools can't be called.",
            ),
            Standing::NoRight => {
                Some("You may not call tools in this project; the right `tools.use` is missing.")
            }
            Standing::NeedsSso => {
                Some("This organization requires signing in with SSO; connect the portal again.")
            }
            Standing::NeedsOtherSignIn => {
                Some("This organization needs another sign-in; connect the portal again.")
            }
            Standing::Needs2fa => Some(
                "This organization requires two-factor authentication; connect the portal again.",
            ),
        }
    }
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

async fn standing(
    state: &AppState,
    a: &portal_grants::PortalAuth,
) -> Result<Standing, kaeso_store::StoreError> {
    use crate::platform::access::{Area, PolicyGap, policy_gap};
    let mut conn = state.pg.acquire().await?;
    let Some(found) =
        members::access_context(&mut conn, a.org_id, Some(a.user_id), Some(a.project_id)).await?
    else {
        return Ok(Standing::NoRight);
    };
    drop(conn);
    let Some(project) = &found.project else {
        return Ok(Standing::NoRight);
    };
    if project.archived_at.is_some() || found.org.state != "active" {
        return Ok(Standing::Closed);
    }
    let org_grant = found
        .org_grant
        .as_ref()
        .and_then(|g| crate::platform::grants::org_from_store(g).ok());
    let project_grant = found
        .project_grant
        .as_ref()
        .and_then(|g| crate::platform::grants::project_from_store(g).ok());
    if org_grant.is_none()
        || !effective_project_rights(org_grant.as_ref(), project_grant.as_ref())
            .contains(ProjectRight::ToolsUse)
    {
        return Ok(Standing::NoRight);
    }
    let sign_in = SessionRow {
        id: a.grant_id,
        user_id: a.user_id,
        method: a.sign_in_method.clone(),
        second_factor: a.second_factor,
        oidc_org_id: a.oidc_org_id,
        signed_in_at: a.created_at,
        last_used_at: a.created_at,
        expires_at: a.token_expires_at.unwrap_or(a.created_at),
        user_state: a.user_state.clone(),
        is_instance_admin: false,
    };
    let area = Area::of_org(&found.org);
    let manager = crate::services::managed::of_user(&state.pg, a.user_id)
        .await?
        .map(|m| m.org_id);
    Ok(match policy_gap(&sign_in, manager, area) {
        None => Standing::Allowed,
        Some(PolicyGap::NeedsSso) => Standing::NeedsSso,
        Some(PolicyGap::NeedsOtherSignIn) => Standing::NeedsOtherSignIn,
        Some(PolicyGap::Needs2fa) => Standing::Needs2fa,
    })
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
            let standing = standing(&st, &a).await?;
            Ok::<_, kaeso_store::StoreError>(Some(TokenAuth {
                grant: a.grant_id,
                user: a.user_id,
                org: a.org_id,
                project: a.project_id,
                client: a.client_name,
                resource: a.resource,
                expires_at: a.token_expires_at,
                active: a.user_state == "active",
                standing,
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
    if let Some(message) = a.standing.refusal() {
        return Err(refused(StatusCode::FORBIDDEN, "permission_denied", message));
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
