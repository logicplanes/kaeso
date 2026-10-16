use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dialect::{DialectId, Origin};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
#[non_exhaustive]
pub struct KppError {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

impl KppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        KppError {
            kind,
            message: message.into(),
            status: None,
            param: None,
            retry_after: None,
            origin: None,
            raw: None,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        KppError::new(ErrorKind::InvalidRequest, message)
    }

    pub fn unsupported(feature: impl Into<String>, message: impl Into<String>) -> Self {
        KppError::new(
            ErrorKind::Unsupported {
                feature: feature.into(),
            },
            message,
        )
    }

    pub fn unsupported_by(feature: impl Into<String>, target: DialectId, hint: &str) -> Self {
        let feature = feature.into();
        let named = if feature.contains('`') {
            feature.clone()
        } else {
            format!("`{feature}`")
        };
        let mut message = format!("{named} is not supported by {}.", target.display_name());
        if !hint.is_empty() {
            message.push(' ');
            message.push_str(hint);
        }
        KppError::unsupported(feature, message)
    }

    pub fn provider(message: impl Into<String>) -> Self {
        KppError::new(ErrorKind::ProviderError, message)
    }

    #[must_use]
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    #[must_use]
    pub fn with_retry_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = Some(retry_after);
        self
    }

    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<Origin>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    #[must_use]
    pub fn with_raw(mut self, raw: Value) -> Self {
        self.raw = Some(raw);
        self
    }

    pub fn unsupported_feature(&self) -> Option<&str> {
        match &self.kind {
            ErrorKind::Unsupported { feature } => Some(feature),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidRequest,
    Authentication,
    PermissionDenied,
    NotFound,
    RequestTooLarge,
    RateLimited,
    Overloaded,
    ContextTooLong,
    ContentFiltered,
    Unsupported { feature: String },
    Timeout,
    Billing,
    ProviderError,
}

impl ErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorKind::InvalidRequest => "invalid_request",
            ErrorKind::Authentication => "authentication",
            ErrorKind::PermissionDenied => "permission_denied",
            ErrorKind::NotFound => "not_found",
            ErrorKind::RequestTooLarge => "request_too_large",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::Overloaded => "overloaded",
            ErrorKind::ContextTooLong => "context_too_long",
            ErrorKind::ContentFiltered => "content_filtered",
            ErrorKind::Unsupported { .. } => "unsupported",
            ErrorKind::Timeout => "timeout",
            ErrorKind::Billing => "billing",
            ErrorKind::ProviderError => "provider_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_messages_name_feature_and_target() {
        let e = KppError::unsupported_by(
            "top_k",
            DialectId::OpenAiChat,
            "Remove `top_k` or route the request to an Anthropic model.",
        );
        assert_eq!(e.unsupported_feature(), Some("top_k"));
        assert_eq!(
            e.to_string(),
            "`top_k` is not supported by OpenAI Chat Completions. Remove `top_k` or route the request to an Anthropic model."
        );
    }

    #[test]
    fn features_that_name_a_value_are_not_wrapped_twice() {
        let e = KppError::unsupported_by(
            "text document media type `text/markdown`",
            DialectId::AnthropicMessages,
            "",
        );
        assert_eq!(
            e.to_string(),
            "text document media type `text/markdown` is not supported by Anthropic Messages."
        );
    }

    #[test]
    fn stable_json_form_round_trips() {
        let e = KppError::new(ErrorKind::RateLimited, "slow down")
            .with_status(429)
            .with_retry_after(Duration::from_secs(20))
            .with_origin(DialectId::AnthropicMessages);
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<KppError>(&json).unwrap(), e);
    }
}
