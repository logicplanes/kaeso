//! Cohere's errors (<https://docs.cohere.com/reference/rerank>): `{"id", "message"}` with the
//! status saying what went wrong, such as 400 or 422 for a body Cohere can't take, 401 for a
//! missing or wrong key (recorded 2026-11-14), 429 for too many requests and 498 for a blocked
//! token.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::DIALECT;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::wire::Short;

fn kind_for(status: u16) -> ErrorKind {
    match status {
        400 | 409 | 422 => ErrorKind::InvalidRequest,
        401 => ErrorKind::Authentication,
        402 => ErrorKind::Billing,
        403 => ErrorKind::PermissionDenied,
        404 | 410 => ErrorKind::NotFound,
        413 => ErrorKind::RequestTooLarge,
        429 => ErrorKind::RateLimited,
        // Cohere answers 498 for a request or reply with a blocked token.
        498 => ErrorKind::ContentFiltered,
        408 | 504 => ErrorKind::Timeout,
        503 => ErrorKind::Overloaded,
        _ => ErrorKind::ProviderError,
    }
}

/// Reads Cohere's error, or another envelope a gateway in front of it answers with.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let message = value
        .as_ref()
        .and_then(|v| v.get("message"))
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty());
    let Some(message) = message else {
        return crate::openai_responses::error::decode(DIALECT, status, headers, body);
    };
    let mut err = KppError::new(kind_for(status), Short(message, 2000).to_string())
        .with_status(status)
        .with_origin(Origin::new(DIALECT));
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

fn status_for(err: &KppError) -> u16 {
    match &err.kind {
        ErrorKind::InvalidRequest | ErrorKind::Unsupported { .. } | ErrorKind::ContextTooLong => {
            400
        }
        ErrorKind::Authentication => 401,
        ErrorKind::Billing => 402,
        ErrorKind::PermissionDenied => 403,
        ErrorKind::NotFound => 404,
        ErrorKind::RequestTooLarge => 413,
        ErrorKind::RateLimited => 429,
        ErrorKind::ContentFiltered => 498,
        ErrorKind::Overloaded => 503,
        ErrorKind::Timeout => 504,
        _ => err.status.filter(|s| (500..600).contains(s)).unwrap_or(500),
    }
}

/// An error for a Cohere Rerank app: Cohere's own error as it sent it, else `{"id", "message"}`
/// with the status the kind maps to and the gateway's request id.
pub(super) fn encode(err: &KppError, request_id: Option<&str>) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    let replay = own
        .then_some(err.raw.as_ref())
        .flatten()
        .filter(|raw| raw.get("message").is_some());
    let (status, body) = match replay {
        Some(raw) => (err.status.unwrap_or_else(|| status_for(err)), raw.clone()),
        None => {
            let body = match request_id {
                Some(id) => json!({"id": id, "message": err.message}),
                None => json!({"message": err.message}),
            };
            (status_for(err), body)
        }
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    EncodedError::new(status, headers, body)
}
