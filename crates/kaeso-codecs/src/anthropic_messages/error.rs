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

// Z.ai's Anthropic endpoint writes its own numeric codes as `error.type`, and answers some of
// them with a status Anthropic uses for something else, such as 429 for an empty balance
// (https://docs.z.ai/api-reference/api-code).
fn kind_for_zai_code(code: &str) -> Option<ErrorKind> {
    Some(match code {
        "1000" | "1001" | "1003" | "1005" => ErrorKind::Authentication,
        "1113" | "1309" | "1314" => ErrorKind::Billing,
        "1211" => ErrorKind::NotFound,
        "1220" | "1311" | "1315" => ErrorKind::PermissionDenied,
        "1261" => ErrorKind::ContextTooLong,
        "1301" => ErrorKind::ContentFiltered,
        "1305" => ErrorKind::Overloaded,
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

// Anthropic's context overflow message, which Claude Code reads to compact the conversation.
const CONTEXT_TOO_LONG: &str = "prompt is too long";
// Anthropic answers an empty credit balance with 400 `invalid_request_error`, not the 402
// `billing_error` its errors page documents (fixtures/anthropic_messages/errors/billing-credit-balance.json).
const CREDIT_BALANCE: &str = "Your credit balance is too low";

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
    if crate::gemini::is_google_error(body) {
        return crate::gemini::decode_error_value(
            status,
            headers,
            body,
            kaeso_kpp::DialectId::Gemini,
        );
    }
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
            if kind == ErrorKind::InvalidRequest {
                let said = e.known.error.message.as_str();
                if said.starts_with(CONTEXT_TOO_LONG) {
                    kind = ErrorKind::ContextTooLong;
                } else if said.starts_with(CREDIT_BALANCE) {
                    kind = ErrorKind::Billing;
                }
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
            if let Some((ty, message)) =
                openai_shaped(raw.as_ref()).or_else(|| failed(raw.as_ref()))
            {
                let kind = kind_for_type(&ty)
                    .or_else(|| kind_for_zai_code(&ty))
                    .unwrap_or_else(|| status.map_or(ErrorKind::ProviderError, kind_for_status));
                let mut err = KppError::new(kind, message.to_owned()).with_origin(origin);
                err.raw = raw;
                return err;
            }
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

// An Anthropic-compatible server that answers with OpenAI's error shape, `{"error": {"message",
// "type"}}`, as Vercel AI Gateway and Z.ai do on `/v1/messages`: its type and message.
fn openai_shaped(raw: Option<&Value>) -> Option<(String, &str)> {
    let error = raw?.get("error")?;
    let message = error.get("message")?.as_str().filter(|m| !m.is_empty())?;
    let ty = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Some((ty.to_owned(), message))
}

// Z.ai answers a path it doesn't serve, `/v1/models` among them, with HTTP 200 and its platform's
// envelope, `{"code": 1000, "msg": "…", "success": false}`, before it checks the key.
fn failed(raw: Option<&Value>) -> Option<(String, &str)> {
    let raw = raw?;
    if raw.get("success").and_then(Value::as_bool) != Some(false) {
        return None;
    }
    let message = raw.get("msg")?.as_str().filter(|m| !m.is_empty())?;
    let code = match raw.get("code") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    Some((code, message))
}

pub(crate) fn is_failure_envelope(body: &[u8]) -> bool {
    let raw: Option<Value> = serde_json::from_slice(body).ok();
    failed(raw.as_ref()).is_some()
}

fn own_body(err: &KppError) -> Option<&serde_json::Map<String, Value>> {
    match &err.raw {
        Some(Value::Object(map))
            if err.origin.as_ref().is_some_and(|o| o.is(D))
                && map.get("type").and_then(Value::as_str) == Some("error")
                && map.get("error").is_some_and(Value::is_object) =>
        {
            Some(map)
        }
        _ => None,
    }
}

fn error_object(err: &KppError) -> ErrorObjectW {
    let (_, ty) = wire_for_kind(&err.kind);
    let message =
        if err.kind == ErrorKind::ContextTooLong && !err.message.starts_with(CONTEXT_TOO_LONG) {
            format!("{CONTEXT_TOO_LONG}: {}", err.message)
        } else {
            err.message.clone()
        };
    ErrorObjectW {
        ty: ty.into(),
        message,
    }
}

fn body_value(err: &KppError, request_id: Option<&str>) -> Value {
    match own_body(err) {
        Some(map) => {
            let mut map = map.clone();
            if let Some(id) = request_id {
                map.insert("request_id".to_owned(), Value::from(id));
            }
            Value::Object(map)
        }
        None => serde_json::to_value(ErrorResponseW {
            ty: "error".into(),
            error: error_object(err),
            request_id: request_id.map(str::to_owned),
        })
        .unwrap_or(Value::Null),
    }
}

fn stream_value(err: &KppError) -> Value {
    match own_body(err) {
        Some(map) => Value::Object(map.clone()),
        None => serde_json::json!({ "type": "error", "error": error_object(err) }),
    }
}

pub(crate) fn encode_error(err: &KppError, request_id: Option<&str>) -> EncodedError {
    let (status, _) = wire_for_kind(&err.kind);
    let own = err.origin.as_ref().is_some_and(|o| o.is(D));
    // A body in another shape keeps its status only when the status says what the kind says:
    // Z.ai answers an empty balance with 429, which apps would retry as a rate limit.
    let status = match err.status {
        Some(s)
            if own
                && (400..=599).contains(&s)
                && (own_body(err).is_some() || kind_for_status(s) == err.kind) =>
        {
            s
        }
        Some(s) if err.kind == ErrorKind::ProviderError && (500..=599).contains(&s) => s,
        _ => status,
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, false);
    }
    let body = serde_json::to_vec(&body_value(err, request_id)).unwrap_or_default();
    EncodedError::new(status, headers, body)
}

pub(crate) fn stream_error_data(err: &KppError) -> String {
    serde_json::to_string(&stream_value(err)).unwrap_or_default()
}
