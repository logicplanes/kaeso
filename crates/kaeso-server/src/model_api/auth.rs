use std::net::IpAddr;
use std::sync::Arc;

use axum::http::HeaderMap;
use kaeso_core::rights::ProjectRight;
use kaeso_kpp::{ErrorKind, KppError};
use kaeso_store::pg::keys::KeyAuth;

use super::errors::error;
use crate::auth::keys::{self, KeyCaller, LookupError};
use crate::state::AppState;

pub fn presented(headers: &HeaderMap) -> Option<&str> {
    if let Some(v) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        return Some(v.trim());
    }
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
}

pub async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    client: Option<IpAddr>,
) -> Result<Arc<KeyAuth>, KppError> {
    let Some(secret) = presented(headers).filter(|s| !s.is_empty()) else {
        metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "missing").increment(1);
        return Err(error(
            ErrorKind::Authentication,
            401,
            "Send a Kaeso API key as `Authorization: Bearer <key>` or `x-api-key: <key>`.",
        ));
    };
    let found = match keys::authenticate(state, secret, client).await {
        Ok(found) => found,
        Err(LookupError::Throttled(t)) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "throttled").increment(1);
            return Err(
                error(ErrorKind::RateLimited, 429, t.message()).with_retry_after(t.duration())
            );
        }
        Err(LookupError::Store(e)) => {
            tracing::error!(error = %e, "key lookup failed");
            return Err(error(
                ErrorKind::ProviderError,
                503,
                "Kaeso cannot check API keys right now. Try again shortly.",
            ));
        }
    };
    let key = match found {
        Ok(KeyCaller::Project(k)) => k,
        Ok(KeyCaller::Org(_)) => {
            return Err(error(
                ErrorKind::PermissionDenied,
                403,
                "Org platform keys manage Kaeso through the Platform API and cannot call models. Use a project API key.",
            ));
        }
        Err(refusal) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "refused").increment(1);
            return Err(error(ErrorKind::Authentication, 401, refusal.message()));
        }
    };
    if !key.inference {
        return Err(error(
            ErrorKind::PermissionDenied,
            403,
            "This API key has Platform API access only. Create a key with inference access to call models.",
        ));
    }
    if !keys::project_rights(&key).contains(ProjectRight::ApiRequest) {
        return Err(error(
            ErrorKind::PermissionDenied,
            403,
            "The owner of this API key may not call models in its project (the right `api.request` is missing).",
        ));
    }
    Ok(key)
}
