use std::net::IpAddr;

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::ProjectId;
use kaeso_core::rights::ProjectRight;
use serde_json::json;
use uuid::Uuid;

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
    })
}
