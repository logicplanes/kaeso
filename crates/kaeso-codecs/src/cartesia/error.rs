//! Cartesia's errors (<https://docs.cartesia.ai/use-the-api/api-errors>): from version
//! 2026-03-01 on, JSON `{error_code, title, message, request_id, doc_url}`; an error from
//! before the version is known, such as a missing `Cartesia-Version`, is plain text.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::{D, PROVIDER};
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of_code(code: &str) -> Option<(ErrorKind, Option<&'static str>)> {
    Some(match code {
        "voice_not_found" => (ErrorKind::NotFound, Some("voice")),
        "voice_model_mismatch" => (ErrorKind::InvalidRequest, Some("voice")),
        "model_not_found" | "model_sunsetted" => (ErrorKind::NotFound, Some("model")),
        "language_not_supported" => (ErrorKind::InvalidRequest, Some("language")),
        "unsupported_audio_format" => (ErrorKind::InvalidRequest, Some("file")),
        "file_too_large" => (ErrorKind::RequestTooLarge, Some("file")),
        "quota_exceeded" => (ErrorKind::Billing, None),
        "concurrency_limited" => (ErrorKind::RateLimited, None),
        "plan_upgrade_required" => (ErrorKind::PermissionDenied, None),
        _ => return None,
    })
}

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

/// The error of an answer Cartesia sent with a status other than 2xx.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok().filter(Value::is_object);
    let by_status = kind_of_status(status);
    let mut err = match &value {
        Some(v) => {
            let found = text(v, "error_code").and_then(|c| kind_of_code(&c));
            let message = text(v, "message")
                .or_else(|| text(v, "title"))
                .unwrap_or_else(|| format!("{PROVIDER} returned HTTP {status}."));
            let param = found.as_ref().and_then(|(_, p)| *p);
            let err = KppError::new(found.map_or(by_status, |(k, _)| k), message);
            match param {
                Some(p) => err.with_param(p),
                None => err,
            }
        }
        None => {
            let text = String::from_utf8_lossy(body);
            let text = text.trim();
            let message = if text.is_empty() {
                format!("{PROVIDER} returned HTTP {status}.")
            } else {
                crate::wire::Short(text, 300).to_string()
            };
            KppError::new(by_status, message)
        }
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

fn code_for(err: &KppError) -> Value {
    let code = match (&err.kind, err.param.as_deref()) {
        (ErrorKind::NotFound, Some("voice")) => "voice_not_found",
        (ErrorKind::NotFound, Some("model")) => "model_not_found",
        (ErrorKind::Billing, _) => "quota_exceeded",
        (ErrorKind::RateLimited, _) => "concurrency_limited",
        (ErrorKind::RequestTooLarge, _) => "file_too_large",
        _ => return Value::Null,
    };
    Value::from(code)
}

/// An error in Cartesia's JSON shape. An error Cartesia itself sent is passed on as it came,
/// with its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, body) = match (&err.raw, own) {
        (Some(raw), true) if raw.get("message").is_some() => {
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
                _ => 400,
            });
            let title = match status {
                401 => "Unauthorized",
                404 => "Not Found",
                429 => "Too Many Requests",
                400..=499 => "Bad Request",
                _ => "Unexpected error",
            };
            (
                status,
                json!({"error_code": code_for(err), "title": title, "message": err.message, "request_id": null}),
            )
        }
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
