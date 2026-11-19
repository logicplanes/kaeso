//! The Sync API's errors: RFC 9457 problem details (`{status, title, detail}`,
//! `application/problem+json`) for every error, by the OpenAPI document; a request the
//! service can't route, such as one without `X-AAI-Model`, gets a plain-text `Not found`.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::{D, PROVIDER};
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of_status(status: u16) -> ErrorKind {
    match status {
        400 | 415 | 422 => ErrorKind::InvalidRequest,
        401 => ErrorKind::Authentication,
        402 => ErrorKind::Billing,
        403 => ErrorKind::PermissionDenied,
        404 => ErrorKind::NotFound,
        413 => ErrorKind::RequestTooLarge,
        429 => ErrorKind::RateLimited,
        503 => ErrorKind::Overloaded,
        504 => ErrorKind::Timeout,
        _ => ErrorKind::ProviderError,
    }
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The error of an answer the Sync API sent with a status other than 2xx.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok().filter(Value::is_object);
    let (kind, message, param) = match &value {
        Some(v) => {
            let title = text(v, "title").unwrap_or_default();
            let detail = text(v, "detail");
            // A refused key answers 404 with `Invalid API key`, not the documented 401
            // (recorded 2026-11-14).
            let kind = if detail.as_deref() == Some("Invalid API key") {
                ErrorKind::Authentication
            } else {
                kind_of_status(status)
            };
            let param = matches!(
                title.as_str(),
                "Bad Audio" | "Audio Too Short" | "Audio Too Large" | "Unsupported Media Type"
            )
            .then_some("file");
            let message = detail
                .or_else(|| (!title.is_empty()).then_some(title))
                .unwrap_or_else(|| format!("{PROVIDER} returned HTTP {status}."));
            (kind, message, param)
        }
        None => {
            let t = String::from_utf8_lossy(body);
            let t = t.trim();
            let message = if t.is_empty() {
                format!("{PROVIDER} returned HTTP {status}.")
            } else {
                crate::wire::Short(t, 300).to_string()
            };
            (kind_of_status(status), message, None)
        }
    };
    let mut err = KppError::new(kind, message)
        .with_status(status)
        .with_origin(Origin::new(D));
    if let Some(p) = param {
        err = err.with_param(p);
    }
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

/// An error as problem details. An error AssemblyAI itself sent is passed on as it came, with
/// its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, body) = match (&err.raw, own) {
        (Some(raw), true) if raw.get("title").is_some() || raw.get("detail").is_some() => {
            (err.status.unwrap_or(400), raw.clone())
        }
        _ => {
            let status = err.status.unwrap_or(match err.kind {
                ErrorKind::Authentication => 401,
                ErrorKind::RequestTooLarge => 413,
                ErrorKind::RateLimited => 429,
                ErrorKind::Overloaded => 503,
                ErrorKind::Timeout => 504,
                ErrorKind::ProviderError => 500,
                _ => 400,
            });
            let title = match status {
                401 => "Unauthorized",
                413 => "Audio Too Large",
                415 => "Unsupported Media Type",
                429 => "Too Many Requests",
                503 => "Service Unavailable",
                504 => "Inference Timeout",
                400..=499 => "Bad Request",
                _ => "Inference Error",
            };
            (
                status,
                json!({"status": status, "title": title, "detail": err.message}),
            )
        }
    };
    let mut headers = HeaderList::new().with("content-type", "application/problem+json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    EncodedError::new(
        status,
        headers,
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}
