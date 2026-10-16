use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::http::request_id;

const RECENT_SIGN_IN: &str = "This action needs a sign-in from the last 15 minutes; sign in again.";
const ORG_READ_ONLY: &str =
    "This organization is scheduled for deletion; nothing in it can be changed.";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{message}")]
    InvalidRequest {
        message: String,
        param: Option<String>,
    },
    #[error("{0}")]
    Authentication(String),
    #[error("{0}")]
    PermissionDenied(String),
    #[error("{message}")]
    MissingRight {
        right: &'static str,
        message: String,
    },
    #[error("{RECENT_SIGN_IN}")]
    RecentSignInRequired,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    MethodNotAllowed(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{ORG_READ_ONLY}")]
    OrgScheduledForDeletion,
    #[error("{message}")]
    RateLimited {
        message: String,
        retry_after: Option<u64>,
    },
    #[error("{0}")]
    TooLarge(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{message}")]
    Provider {
        status: StatusCode,
        message: String,
        retry_after: Option<u64>,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    pub fn invalid(param: &str, message: impl Into<String>) -> ApiError {
        ApiError::InvalidRequest {
            message: message.into(),
            param: Some(param.to_owned()),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> ApiError {
        ApiError::InvalidRequest {
            message: message.into(),
            param: None,
        }
    }

    pub fn not_found(what: &str) -> ApiError {
        ApiError::NotFound(format!("No such {what}, or you cannot see it."))
    }

    pub fn internal(cause: impl std::fmt::Display) -> ApiError {
        ApiError::Internal(cause.to_string())
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ApiError::InvalidRequest { .. } | ApiError::MethodNotAllowed(_) => "invalid_request",
            ApiError::Authentication(_) => "authentication",
            ApiError::PermissionDenied(_) | ApiError::MissingRight { .. } => "permission_denied",
            ApiError::RecentSignInRequired => "recent_sign_in_required",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::OrgScheduledForDeletion => "org_scheduled_for_deletion",
            ApiError::RateLimited { .. } => "rate_limited",
            ApiError::TooLarge(_) => "request_too_large",
            ApiError::Unavailable(_) => "unavailable",
            ApiError::Provider { .. } => "provider_error",
            ApiError::Internal(_) => "internal",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
            ApiError::Authentication(_) => StatusCode::UNAUTHORIZED,
            ApiError::PermissionDenied(_)
            | ApiError::MissingRight { .. }
            | ApiError::RecentSignInRequired => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::MethodNotAllowed(_) => StatusCode::METHOD_NOT_ALLOWED,
            ApiError::Conflict(_) | ApiError::OrgScheduledForDeletion => StatusCode::CONFLICT,
            ApiError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Provider { status, .. } => *status,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The error envelope.")]
pub struct ErrorEnvelope {
    #[doc = "The error."]
    pub error: ErrorBody,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An error.")]
pub struct ErrorBody {
    #[doc = "One of `invalid_request`, `authentication`, `permission_denied`,\n\
        `recent_sign_in_required`, `not_found`, `conflict`,\n\
        `org_scheduled_for_deletion`, `request_too_large`, `rate_limited`,\n\
        `unavailable`, `provider_error` (only the Playground's model\n\
        requests), `internal`."]
    #[schema(example = "not_found")]
    #[serde(rename = "type")]
    pub kind: String,
    #[doc = "What went wrong and what to do, for the person reading it."]
    #[schema(example = "No such project, or you cannot see it.")]
    pub message: String,
    #[doc = "The request field at fault, when there is one."]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[doc = "For `permission_denied`: the right the caller lacks, when one right\n\
        would allow the request (`orgs/{org}/rights/{right}/grantors` names\n\
        who can grant it)."]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "logs.export")]
    pub right: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let kind = self.kind();
        let request = request_id::current()
            .map(|id| id.to_string())
            .unwrap_or_default();
        let mut right = None;
        let (message, param, retry_after) = match self {
            ApiError::InvalidRequest { message, param } => (message, param, None),
            ApiError::MissingRight { right: r, message } => {
                right = Some(r.to_owned());
                (message, None, None)
            }
            ApiError::RateLimited {
                message,
                retry_after,
            }
            | ApiError::Provider {
                message,
                retry_after,
                ..
            } => (message, None, retry_after),
            ApiError::RecentSignInRequired => (RECENT_SIGN_IN.to_owned(), None, None),
            ApiError::OrgScheduledForDeletion => (ORG_READ_ONLY.to_owned(), None, None),
            ApiError::Internal(cause) => {
                tracing::error!(error = %cause, "internal error");
                (
                    format!(
                        "Something went wrong on our side; if it keeps happening, report request {request}."
                    ),
                    None,
                    None,
                )
            }
            ApiError::Authentication(m)
            | ApiError::PermissionDenied(m)
            | ApiError::NotFound(m)
            | ApiError::MethodNotAllowed(m)
            | ApiError::Conflict(m)
            | ApiError::TooLarge(m)
            | ApiError::Unavailable(m) => (m, None, None),
        };
        let body = ErrorEnvelope {
            error: ErrorBody {
                kind: kind.to_owned(),
                message,
                param,
                right,
            },
        };
        let mut response = (status, Json(body)).into_response();
        if let Some(secs) = retry_after
            && let Ok(v) = HeaderValue::from_str(&secs.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, v);
        }
        response
    }
}

impl From<kaeso_store::StoreError> for ApiError {
    fn from(e: kaeso_store::StoreError) -> ApiError {
        ApiError::Internal(e.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> ApiError {
        ApiError::Internal(e.to_string())
    }
}

impl From<kaeso_core::rights::GrantExceedsHolder> for ApiError {
    fn from(e: kaeso_core::rights::GrantExceedsHolder) -> ApiError {
        ApiError::PermissionDenied(e.to_string())
    }
}

impl From<kaeso_core::rights::MissingDependency> for ApiError {
    fn from(e: kaeso_core::rights::MissingDependency) -> ApiError {
        ApiError::invalid("rights", e.to_string())
    }
}

impl From<kaeso_core::rights::GrantCheckError> for ApiError {
    fn from(e: kaeso_core::rights::GrantCheckError) -> ApiError {
        match e {
            kaeso_core::rights::GrantCheckError::Exceeds(e) => e.into(),
            kaeso_core::rights::GrantCheckError::Dependency(e) => e.into(),
        }
    }
}

pub async fn unknown_endpoint() -> ApiError {
    ApiError::NotFound("No such endpoint in the Platform API.".into())
}

pub async fn method_not_allowed(method: axum::http::Method) -> ApiError {
    ApiError::MethodNotAllowed(format!(
        "This endpoint does not accept `{method}` requests."
    ))
}

pub type ApiResult<T> = Result<T, ApiError>;
