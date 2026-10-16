use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::Value;

use super::common::D;
use super::wire::{ErrorObjectW, ErrorResponseW};
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::wire::{Ext, Short};

fn kind_for_type(ty: &str) -> Option<ErrorKind> {
    Some(match ty {
        "invalid_request_error" | "conflict_error" => ErrorKind::InvalidRequest,
        "authentication_error" => ErrorKind::Authentication,
        "billing_error" => ErrorKind::Billing,
        "permission_error" => ErrorKind::PermissionDenied,
        "not_found_error" => ErrorKind::NotFound,
        "request_too_large" => ErrorKind::RequestTooLarge,
        "rate_limit_error" => ErrorKind::RateLimited,
        "api_error" => ErrorKind::ProviderError,
        "timeout_error" => ErrorKind::Timeout,
        "overloaded_error" => ErrorKind::Overloaded,
        _ => return None,
    })
}

fn kind_for_status(status: u16) -> ErrorKind {
    match status {
        400 | 409 | 422 => ErrorKind::InvalidRequest,
        401 => ErrorKind::Authentication,
        402 => ErrorKind::Billing,
        403 => ErrorKind::PermissionDenied,
        404 => ErrorKind::NotFound,
        413 => ErrorKind::RequestTooLarge,
        429 => ErrorKind::RateLimited,
        408 | 504 => ErrorKind::Timeout,
        503 | 529 => ErrorKind::Overloaded,
        _ => ErrorKind::ProviderError,
    }
}

fn wire_for_kind(kind: &ErrorKind) -> (u16, &'static str) {
    match kind {
        ErrorKind::InvalidRequest
        | ErrorKind::ContextTooLong
        | ErrorKind::ContentFiltered
        | ErrorKind::Unsupported { .. } => (400, "invalid_request_error"),
        ErrorKind::Authentication => (401, "authentication_error"),
        ErrorKind::Billing => (402, "billing_error"),
        ErrorKind::PermissionDenied => (403, "permission_error"),
        ErrorKind::NotFound => (404, "not_found_error"),
        ErrorKind::RequestTooLarge => (413, "request_too_large"),
        ErrorKind::RateLimited => (429, "rate_limit_error"),
        ErrorKind::Timeout => (504, "timeout_error"),
        ErrorKind::Overloaded => (529, "overloaded_error"),
        _ => (500, "api_error"),
    }
}

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let mut err = decode_body(body, Some(status));
    err.status = Some(status);
    err.retry_after = parse_retry_after(headers);
    err
}

pub(crate) fn decode_stream_error(data: &str) -> KppError {
    let mut err = decode_body(data.as_bytes(), None);
    err.status = None;
    err
}

fn decode_body(body: &[u8], status: Option<u16>) -> KppError {
    let origin = Origin::new(D);
    let raw: Option<Value> = serde_json::from_slice(body).ok();
    let parsed = Ext::<ErrorResponseW>::from_slice(body)
        .ok()
        .filter(|e| e.known.ty == "error");
    match parsed {
        Some(e) => {
            let mut kind = kind_for_type(&e.known.error.ty)
                .unwrap_or_else(|| status.map_or(ErrorKind::ProviderError, kind_for_status));
            if kind == ErrorKind::InvalidRequest
                && e.known.error.message.starts_with("prompt is too long")
            {
                kind = ErrorKind::ContextTooLong;
            }
            let message = if e.known.error.message.is_empty() {
                format!("Anthropic returned `{}`.", e.known.error.ty)
            } else {
                e.known.error.message.clone()
            };
            let mut err = KppError::new(kind, message).with_origin(origin);
            err.raw = raw;
            err
        }
        None => {
            let text = String::from_utf8_lossy(body);
            let message = match status {
                Some(s) => format!(
                    "The provider returned HTTP {s} with a body that is not an Anthropic error: {}",
                    Short(&text, 200)
                ),
                None => format!(
                    "The provider sent a stream error that is not an Anthropic error: {}",
                    Short(&text, 200)
                ),
            };
            let kind = status.map_or(ErrorKind::ProviderError, kind_for_status);
            let mut err = KppError::new(kind, message).with_origin(origin);
            err.raw = Some(raw.unwrap_or_else(|| Value::String(text.into_owned())));
            err
        }
    }
}

fn body_value(err: &KppError) -> Value {
    if err.origin.as_ref().is_some_and(|o| o.is(D))
        && let Some(raw @ Value::Object(map)) = &err.raw
        && map.get("type").and_then(Value::as_str) == Some("error")
        && map.get("error").is_some_and(Value::is_object)
    {
        return raw.clone();
    }
    let (_, ty) = wire_for_kind(&err.kind);
    let body = ErrorResponseW {
        ty: "error".into(),
        error: ErrorObjectW {
            ty: ty.into(),
            message: err.message.clone(),
        },
        request_id: None,
    };
    serde_json::to_value(&body).unwrap_or(Value::Null)
}

pub(crate) fn encode_error(err: &KppError) -> EncodedError {
    let (status, _) = wire_for_kind(&err.kind);
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let status = match err.status {
        Some(s) if own && (400..=599).contains(&s) => s,
        Some(s) if err.kind == ErrorKind::ProviderError && (500..=599).contains(&s) => s,
        _ => status,
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    let body = serde_json::to_vec(&body_value(err)).unwrap_or_default();
    EncodedError::new(status, headers, body)
}

pub(crate) fn stream_error_data(err: &KppError) -> String {
    serde_json::to_string(&body_value(err)).unwrap_or_default()
}
