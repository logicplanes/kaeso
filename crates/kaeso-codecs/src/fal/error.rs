//! fal's errors: model errors as `{"detail": [{loc, msg, type, url, ctx, input, categories}]}`
//! (<https://fal.ai/docs/documentation/model-apis/errors>), request errors as `{"detail":
//! "…", "error_type": "…"}` (<https://fal.ai/docs/documentation/model-apis/request-errors>),
//! the queue's own refusals as `{"detail": "…"}`, and a failed request's queue status, which is
//! `COMPLETED` with `error` and `error_type`.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::{D, PROVIDER};
use crate::codec::{EncodedError, HeaderList, parse_retry_after};

fn kind_of_type(ty: &str) -> Option<ErrorKind> {
    Some(match ty {
        "content_policy_violation" => ErrorKind::ContentFiltered,
        "generation_timeout" | "request_timeout" | "startup_timeout" => ErrorKind::Timeout,
        "runner_scheduling_failure"
        | "runner_connection_timeout"
        | "runner_disconnected"
        | "runner_connection_refused"
        | "runner_connection_error"
        | "downstream_service_unavailable" => ErrorKind::Overloaded,
        "internal_server_error"
        | "internal_error"
        | "downstream_service_error"
        | "runner_incomplete_response"
        | "runner_server_error"
        | "no_media_generated" => ErrorKind::ProviderError,
        "concurrent_requests_limit" => ErrorKind::RateLimited,
        "bad_request" | "client_disconnected" | "client_cancelled" => ErrorKind::InvalidRequest,
        _ => return None,
    })
}

fn kind_of_status(status: u16) -> ErrorKind {
    match status {
        400 | 405 | 409 | 422 => ErrorKind::InvalidRequest,
        401 => ErrorKind::Authentication,
        402 => ErrorKind::Billing,
        // A key limited to other models, a locked account or a spending limit (the Queue
        // API's `Forbidden` response).
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
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

// A field fal names in `loc`, as `["body", "prompt"]`; `["body"]` names the whole input.
fn param_of(item: &Value) -> Option<String> {
    let loc = item.get("loc")?.as_array()?;
    let mut names = loc.iter().filter_map(Value::as_str);
    match names.next() {
        Some("body") => names.next().map(str::to_owned),
        other => other.map(str::to_owned),
    }
}

/// The error of an answer fal sent with a status other than 2xx.
pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let detail = value.as_ref().and_then(|v| v.get("detail"));
    let by_status = kind_of_status(status);
    let mut err = match detail {
        Some(Value::Array(items)) => {
            let first = items.first();
            let ty = first.and_then(|i| text(i, "type"));
            let kind = ty.as_deref().and_then(kind_of_type).unwrap_or(by_status);
            let message = first
                .and_then(|i| text(i, "msg"))
                .unwrap_or_else(|| format!("{PROVIDER} refused the request's input."));
            let err = KppError::new(kind, message);
            match first.and_then(param_of) {
                Some(p) => err.with_param(p),
                None => err,
            }
        }
        Some(Value::String(m)) => {
            let ty = value
                .as_ref()
                .and_then(|v| text(v, "error_type").or_else(|| text(v, "type")))
                .or_else(|| headers.get("x-fal-error-type").map(str::to_owned));
            let kind = ty.as_deref().and_then(kind_of_type).unwrap_or(by_status);
            KppError::new(kind, m.clone())
        }
        _ => match value
            .as_ref()
            .and_then(|v| v.get("error"))
            .filter(|e| e.is_object())
        {
            Some(e) => KppError::new(
                by_status,
                text(e, "message").unwrap_or_else(|| format!("{PROVIDER} returned HTTP {status}.")),
            ),
            None => KppError::new(
                by_status,
                format!(
                    "{PROVIDER} returned HTTP {status} with a body Kaeso could not read: {}",
                    crate::wire::Short(String::from_utf8_lossy(body).trim(), 300)
                ),
            ),
        },
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

/// The error of a request fal's queue reports as completed with an error, such as a request
/// cancelled before it ran (`client_cancelled`).
pub(super) fn failed(v: &Value) -> KppError {
    let ty = text(v, "error_type");
    let message = text(v, "error").unwrap_or_else(|| match ty.as_deref() {
        Some("client_cancelled") => format!("The request was cancelled at {PROVIDER}."),
        _ => format!("{PROVIDER} could not finish the request."),
    });
    let kind = ty
        .as_deref()
        .and_then(kind_of_type)
        .unwrap_or(ErrorKind::ProviderError);
    let status = match kind {
        ErrorKind::ContentFiltered | ErrorKind::InvalidRequest => 400,
        ErrorKind::Timeout => 504,
        ErrorKind::Overloaded => 503,
        _ => 502,
    };
    KppError::new(kind, message)
        .with_status(status)
        .with_origin(Origin::new(D))
        .with_raw(v.clone())
}

fn type_for(kind: &ErrorKind) -> &'static str {
    match kind {
        ErrorKind::ContentFiltered => "content_policy_violation",
        ErrorKind::Timeout => "generation_timeout",
        ErrorKind::ProviderError | ErrorKind::Overloaded => "internal_server_error",
        _ => "value_error",
    }
}

/// An error in fal's model-error shape. An error fal itself sent is passed on as it came, with
/// its status.
pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    let (status, body) = match (&err.raw, own) {
        (Some(raw), true) if raw.get("detail").is_some() => {
            (err.status.unwrap_or(400), raw.clone())
        }
        _ => {
            let status = err.status.unwrap_or(match err.kind {
                ErrorKind::Authentication => 401,
                ErrorKind::PermissionDenied => 403,
                ErrorKind::NotFound => 404,
                ErrorKind::RequestTooLarge => 413,
                ErrorKind::RateLimited => 429,
                ErrorKind::ProviderError => 500,
                ErrorKind::Overloaded => 503,
                ErrorKind::Timeout => 504,
                _ => 422,
            });
            let mut loc = vec![json!("body")];
            if let Some(p) = &err.param {
                loc.push(json!(p));
            }
            (
                status,
                json!({"detail": [{"loc": loc, "msg": err.message, "type": type_for(&err.kind)}]}),
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
