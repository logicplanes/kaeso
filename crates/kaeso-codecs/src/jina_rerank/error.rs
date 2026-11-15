//! Jina AI's errors (<https://docs.jina.ai/api.md>, Errors; `ErrorResponse` in
//! <https://api.jina.ai/openapi.json>): `{"detail", "code", "request_id"}`, with `errors` on a
//! 422. The code says more than the status where they differ: an empty balance and a resource
//! limit both answer 403, a token limit and an unknown model both 400.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::DIALECT;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::wire::Short;

/// Reads Jina AI's error, or another envelope a proxy in front of it answers with.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let detail = value
        .as_ref()
        .and_then(|v| v.get("detail"))
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty());
    let Some(detail) = detail else {
        return crate::openai_responses::error::decode(DIALECT, status, headers, body);
    };
    let code = value
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(Value::as_str);
    let kind = crate::openai_responses::error::kind_for(Some(status), None, code);
    let mut err = KppError::new(kind, Short(detail, 2000).to_string())
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
        ErrorKind::InvalidRequest
        | ErrorKind::Unsupported { .. }
        | ErrorKind::ContextTooLong
        | ErrorKind::ContentFiltered => 400,
        ErrorKind::Authentication => 401,
        ErrorKind::Billing | ErrorKind::PermissionDenied => 403,
        ErrorKind::NotFound => 404,
        ErrorKind::RequestTooLarge => 413,
        ErrorKind::RateLimited => 429,
        ErrorKind::Overloaded => 503,
        ErrorKind::Timeout => 504,
        _ => err.status.filter(|s| (500..600).contains(s)).unwrap_or(500),
    }
}

/// An error in Jina AI's shape: its own as it sent it, else `{"detail": …}` with the status the
/// kind maps to.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    let replay = own
        .then_some(err.raw.as_ref())
        .flatten()
        .filter(|raw| raw.get("detail").is_some());
    let (status, body) = match replay {
        Some(raw) => (err.status.unwrap_or_else(|| status_for(err)), raw.clone()),
        None => (status_for(err), json!({"detail": err.message})),
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    EncodedError::new(status, headers, body)
}
