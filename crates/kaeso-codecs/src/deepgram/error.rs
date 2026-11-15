//! Deepgram's errors (<https://developers.deepgram.com/docs/errors>, the OpenAPI document's
//! `ErrorResponse`): `{"err_code", "err_msg", "request_id"}` (`err_code` is a code such as
//! `INVALID_AUTH` or an HTTP reason phrase such as `Bad Request`), the newer
//! `{"category", "message", "details", "request_id"}`, and plain text.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::D;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of(status: u16, code: Option<&str>) -> ErrorKind {
    match code {
        Some("INVALID_AUTH") => ErrorKind::Authentication,
        Some("INSUFFICIENT_PERMISSIONS") => ErrorKind::PermissionDenied,
        Some("ASR_PAYMENT_REQUIRED") => ErrorKind::Billing,
        Some("PAYLOAD_TOO_LARGE" | "Payload Too Large") => ErrorKind::RequestTooLarge,
        Some("TOO_MANY_REQUESTS" | "Too Many Requests") => ErrorKind::RateLimited,
        Some("PROJECT_NOT_FOUND") => ErrorKind::NotFound,
        _ => match status {
            400 | 409 | 415 | 422 => ErrorKind::InvalidRequest,
            401 => ErrorKind::Authentication,
            402 => ErrorKind::Billing,
            403 => ErrorKind::PermissionDenied,
            404 => ErrorKind::NotFound,
            413 => ErrorKind::RequestTooLarge,
            429 => ErrorKind::RateLimited,
            503 => ErrorKind::Overloaded,
            504 => ErrorKind::Timeout,
            _ => ErrorKind::ProviderError,
        },
    }
}

pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let text = |k: &str| {
        value
            .as_ref()
            .and_then(|v| v.get(k))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let code = text("err_code")
        .or_else(|| text("category"))
        // The errors page writes its 503 with `error_code`.
        .or_else(|| text("error_code"));
    let message = text("err_msg")
        .or_else(|| match (text("message"), text("details")) {
            (Some(m), Some(d)) => Some(format!("{m} {d}")),
            (m, _) => m,
        })
        .or_else(|| {
            let plain = String::from_utf8_lossy(body);
            let plain = plain.trim();
            (value.is_none() && !plain.is_empty())
                .then(|| crate::wire::Short(plain, 300).to_string())
        })
        .unwrap_or_else(|| format!("Deepgram returned HTTP {status} without a message."));
    let mut err = KppError::new(kind_of(status, code.as_deref()), message)
        .with_status(status)
        .with_origin(Origin::new(D));
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

fn wire_for(err: &KppError) -> (u16, &'static str) {
    match &err.kind {
        ErrorKind::InvalidRequest | ErrorKind::ContextTooLong | ErrorKind::Unsupported { .. } => {
            (400, "Bad Request")
        }
        ErrorKind::RequestTooLarge => (413, "PAYLOAD_TOO_LARGE"),
        ErrorKind::Authentication => (401, "INVALID_AUTH"),
        ErrorKind::PermissionDenied => (403, "INSUFFICIENT_PERMISSIONS"),
        ErrorKind::Billing => (402, "ASR_PAYMENT_REQUIRED"),
        ErrorKind::NotFound => (404, "Not Found"),
        ErrorKind::RateLimited => (429, "TOO_MANY_REQUESTS"),
        ErrorKind::Overloaded => (503, "Service Unavailable"),
        ErrorKind::Timeout => (504, "Gateway Timeout"),
        _ => (
            err.status.filter(|s| (500..600).contains(s)).unwrap_or(500),
            "Internal Server Error",
        ),
    }
}

/// An error in Deepgram's `err_code`/`err_msg` shape. An error Deepgram itself sent as JSON is
/// passed on as it came, with its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, code) = wire_for(err);
    let (status, body) = match (&err.raw, own) {
        (Some(raw @ Value::Object(_)), true) => (err.status.unwrap_or(status), raw.clone()),
        _ => (
            status,
            json!({"err_code": code, "err_msg": err.message, "request_id": ""}),
        ),
    };
    let mut headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    EncodedError::new(
        status,
        headers,
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}
