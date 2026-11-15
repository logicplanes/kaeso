//! Replicate's errors: problem details (`{"title", "detail", "status"}`, mostly as
//! `application/problem+json`; <https://replicate.com/docs/reference/http>), a bare `{"detail":
//! "…"}` from some refusals and the rate limit
//! (<https://replicate.com/docs/topics/predictions/rate-limits>), and a prediction that ended
//! as `failed`, `canceled` or `aborted`, which answers 200 with an `error` string
//! (<https://replicate.com/docs/reference/error-codes>).

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::{ABORTED, CANCELED, D, PROVIDER};
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of_status(status: u16) -> ErrorKind {
    match status {
        400 | 409 | 422 => ErrorKind::InvalidRequest,
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

/// The error of an answer Replicate sent with a status other than 2xx.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let message = value
        .as_ref()
        .and_then(|v| match (text(v, "title"), text(v, "detail")) {
            (Some(t), Some(d)) => Some(format!("{t}: {d}")),
            (t, d) => d.or(t),
        });
    let message = message.unwrap_or_else(|| {
        format!(
            "{PROVIDER} returned HTTP {status} with a body Kaeso could not read: {}",
            crate::wire::Short(String::from_utf8_lossy(body).trim(), 300)
        )
    });
    let mut err = KppError::new(kind_of_status(status), message)
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

/// The error of a prediction that ended without its output.
pub(super) fn failed(v: &Value) -> KppError {
    let status = text(v, "status").unwrap_or_default();
    let message = text(v, "error").unwrap_or_else(|| match status.as_str() {
        CANCELED => format!("The prediction was canceled at {PROVIDER}."),
        ABORTED => format!("{PROVIDER} aborted the prediction before it started."),
        _ => format!("{PROVIDER} could not finish the prediction."),
    });
    KppError::new(ErrorKind::ProviderError, message)
        .with_status(502)
        .with_origin(Origin::new(D))
        .with_raw(v.clone())
}

/// An error as problem details. An error Replicate itself sent is passed on as it came, with
/// its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, body) = match (&err.raw, own) {
        (Some(raw), true) if raw.get("detail").is_some() || raw.get("title").is_some() => {
            (err.status.unwrap_or(400), raw.clone())
        }
        _ => {
            let status = err.status.unwrap_or(match err.kind {
                ErrorKind::Authentication => 401,
                ErrorKind::Billing => 402,
                ErrorKind::PermissionDenied => 403,
                ErrorKind::NotFound => 404,
                ErrorKind::RequestTooLarge => 413,
                ErrorKind::RateLimited => 429,
                ErrorKind::ProviderError => 500,
                ErrorKind::Overloaded => 503,
                ErrorKind::Timeout => 504,
                _ => 422,
            });
            let title = match status {
                401 => "Unauthenticated",
                404 => "Not found",
                429 => "Too many requests",
                400..=499 => "Invalid input",
                _ => "Internal server error",
            };
            (
                status,
                json!({"title": title, "detail": err.message, "status": status}),
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
