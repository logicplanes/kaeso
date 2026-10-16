use std::time::Duration;

use kaeso_kpp::{ErrorKind, KppError};
use serde_json::{Value, json};

use super::DIALECT;
use super::wire::ErrorResponse;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::openai_responses::error::{kind_for_code, wire_for_kind};
use crate::wire::{Ext, Short};

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let mut err = decode_body(Some(status), body);
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

pub(crate) fn decode_body(status: Option<u16>, body: &[u8]) -> KppError {
    let origin = kaeso_kpp::Origin::new(DIALECT);
    let parsed: Option<Ext<ErrorResponse>> = Ext::from_slice(body).ok();
    let raw: Value = serde_json::from_slice(body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned()));
    let Some(parsed) = parsed else {
        let text = String::from_utf8_lossy(body);
        let message = match raw.get("error") {
            Some(Value::String(e)) if !e.trim().is_empty() => e.clone(),
            _ => format!(
                "The provider returned an error{} that is not in the OpenAI error format: {}",
                status.map(|s| format!(" (HTTP {s})")).unwrap_or_default(),
                Short(text.trim(), 500)
            ),
        };
        let mut err = KppError::new(decode_kind(status, None, None), message)
            .with_origin(origin)
            .with_raw(raw);
        if let Some(status) = status {
            err = err.with_status(status);
        }
        return err;
    };
    let body = &parsed.known.error.known;
    let code = body.code.as_ref().and_then(Value::as_str);
    let ty = body.ty.as_deref();
    let kind = decode_kind(status, ty, code);
    let message = body
        .message
        .clone()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "The provider returned an error without a message.".to_owned());
    let mut err = KppError::new(kind, message)
        .with_origin(origin)
        .with_raw(raw);
    if let Some(status) = status {
        err = err.with_status(status);
    }
    if let Some(param) = body.param.as_ref().and_then(Value::as_str) {
        err = err.with_param(param);
    }
    err
}

fn decode_kind(status: Option<u16>, ty: Option<&str>, code: Option<&str>) -> ErrorKind {
    let by_code = |strong_only: bool| {
        [code, ty].into_iter().flatten().find_map(|c| {
            kind_for_code(c).filter(|k| {
                !strong_only
                    || !matches!(
                        k,
                        ErrorKind::ProviderError
                            | ErrorKind::InvalidRequest
                            | ErrorKind::RateLimited
                    )
            })
        })
    };
    if let Some(kind) = by_code(true) {
        return kind;
    }
    match status {
        Some(400 | 409 | 422) => ErrorKind::InvalidRequest,
        Some(401) => ErrorKind::Authentication,
        Some(403) => ErrorKind::PermissionDenied,
        Some(404) => ErrorKind::NotFound,
        Some(413) => ErrorKind::RequestTooLarge,
        Some(429) => ErrorKind::RateLimited,
        Some(408 | 504) => ErrorKind::Timeout,
        Some(503) => ErrorKind::Overloaded,
        Some(_) => ErrorKind::ProviderError,
        None => by_code(false).unwrap_or(ErrorKind::ProviderError),
    }
}

pub(crate) fn error_body(err: &KppError) -> Value {
    if let Some(raw) = own_raw(err) {
        return raw.clone();
    }
    let (_, ty, code) = wire_for_kind(err);
    json!({
        "error": {
            "message": err.message,
            "type": ty,
            "param": err.param,
            "code": code,
        }
    })
}

fn own_raw(err: &KppError) -> Option<&Value> {
    let from_chat = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    err.raw
        .as_ref()
        .filter(|raw| from_chat && raw.get("error").is_some_and(Value::is_object))
}

pub(crate) fn encode_error(err: &KppError) -> EncodedError {
    let from_chat = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    let (kind_status, _, _) = wire_for_kind(err);
    let status = match err.status {
        Some(s) if from_chat => s,
        _ => kind_status,
    };
    let body = serde_json::to_vec(&error_body(err)).unwrap_or_default();
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        for (name, value) in retry_headers(retry) {
            headers.push(name, value);
        }
    }
    EncodedError::new(status, headers, body)
}

fn retry_headers(retry: Duration) -> Vec<(&'static str, String)> {
    let whole = retry.subsec_nanos() == 0;
    let secs = if whole {
        retry.as_secs()
    } else {
        retry.as_secs().saturating_add(1)
    };
    let mut headers = vec![("retry-after", secs.to_string())];
    if !whole {
        let ms = retry.as_nanos().div_ceil(1_000_000);
        headers.push(("retry-after-ms", ms.to_string()));
    }
    headers
}
