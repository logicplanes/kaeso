//! ElevenLabs' errors (<https://elevenlabs.io/docs/eleven-api/resources/errors>):
//! `{"detail": {"type", "code", "message", "status", "request_id", "param"}}`, the older
//! `{"detail": {"status", "message"}}`, and FastAPI's validation `{"detail": [{loc, msg}]}`.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::D;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of(status: u16, ty: Option<&str>, code: Option<&str>) -> ErrorKind {
    match (ty, code) {
        (_, Some("insufficient_credits" | "quota_exceeded")) => ErrorKind::Billing,
        (Some("authentication_error"), _) | (_, Some("invalid_api_key" | "missing_api_key")) => {
            ErrorKind::Authentication
        }
        (Some("authorization_error"), _) => ErrorKind::PermissionDenied,
        (Some("payment_required"), _) => ErrorKind::Billing,
        (Some("rate_limit_error"), _) => ErrorKind::RateLimited,
        (Some("not_found"), _) => ErrorKind::NotFound,
        (Some("validation_error" | "invalid_request"), _) => ErrorKind::InvalidRequest,
        (Some("service_unavailable"), _) => ErrorKind::Overloaded,
        _ => match status {
            400 | 409 | 413 | 422 => ErrorKind::InvalidRequest,
            401 => ErrorKind::Authentication,
            402 => ErrorKind::Billing,
            403 => ErrorKind::PermissionDenied,
            404 => ErrorKind::NotFound,
            429 => ErrorKind::RateLimited,
            503 => ErrorKind::Overloaded,
            _ => ErrorKind::ProviderError,
        },
    }
}

pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let detail = value.as_ref().and_then(|v| v.get("detail"));
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let mut err = match detail {
        Some(d @ Value::Object(_)) => {
            let ty = text(d, "type");
            let code = text(d, "code").or_else(|| text(d, "status"));
            let message = text(d, "message")
                .unwrap_or_else(|| format!("ElevenLabs returned HTTP {status} without a message."));
            let mut err = KppError::new(kind_of(status, ty.as_deref(), code.as_deref()), message);
            if let Some(p) = text(d, "param") {
                err = err.with_param(p);
            }
            err
        }
        Some(Value::Array(items)) => {
            let first = items.first();
            let message = first
                .and_then(|i| text(i, "msg"))
                .unwrap_or_else(|| "ElevenLabs refused the request's fields.".to_owned());
            let param = first
                .and_then(|i| i.get("loc"))
                .and_then(Value::as_array)
                .and_then(|l| l.last())
                .and_then(Value::as_str)
                .map(str::to_owned);
            let err = KppError::new(ErrorKind::InvalidRequest, message);
            match param {
                Some(p) => err.with_param(p),
                None => err,
            }
        }
        Some(Value::String(m)) => KppError::new(kind_of(status, None, None), m.clone()),
        _ => KppError::new(
            kind_of(status, None, None),
            format!(
                "ElevenLabs returned HTTP {status} with a body Kaeso could not read: {}",
                crate::wire::Short(String::from_utf8_lossy(body).trim(), 300)
            ),
        ),
    };
    err = err.with_status(status).with_origin(Origin::new(D));
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

fn wire_for(err: &KppError) -> (u16, &'static str, &'static str) {
    match &err.kind {
        ErrorKind::InvalidRequest | ErrorKind::ContextTooLong => {
            (400, "invalid_request", "invalid_parameters")
        }
        ErrorKind::Unsupported { .. } => (400, "invalid_request", "unsupported_parameter"),
        ErrorKind::RequestTooLarge => (413, "invalid_request", "request_too_large"),
        ErrorKind::Authentication => (401, "authentication_error", "invalid_api_key"),
        ErrorKind::PermissionDenied => (403, "authorization_error", "permission_denied"),
        ErrorKind::Billing => (402, "payment_required", "insufficient_credits"),
        ErrorKind::NotFound => (404, "not_found", "not_found"),
        ErrorKind::RateLimited => (429, "rate_limit_error", "rate_limit_exceeded"),
        ErrorKind::Overloaded => (503, "service_unavailable", "system_busy"),
        _ => (
            err.status.filter(|s| (500..600).contains(s)).unwrap_or(500),
            "internal_error",
            "internal_error",
        ),
    }
}

/// An error in ElevenLabs' documented shape. An error ElevenLabs itself sent is passed on as
/// it came, with its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, ty, code) = wire_for(err);
    let (status, body) = match (&err.raw, own) {
        (Some(raw), true) if raw.get("detail").is_some() => {
            (err.status.unwrap_or(status), raw.clone())
        }
        _ => (
            status,
            json!({"detail": {
                "type": ty,
                "code": code,
                "message": err.message,
                "status": code,
                "param": err.param.as_deref().filter(|_| own || err.origin.is_none()),
            }}),
        ),
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    EncodedError::new(
        status,
        headers,
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}
