use kaeso_kpp::KppError;
use serde_json::{Value, json};

use super::DIALECT;
use super::wire::ErrorResponse;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::openai_responses::error::{kind_for, wire_for_kind};
use crate::wire::{Ext, Short};

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let mut err = decode_body(Some(status), body);
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

pub(crate) fn with_detail(mut err: KppError, path: &[&str]) -> KppError {
    if path.is_empty() {
        return err;
    }
    let detail = err
        .raw
        .as_ref()
        .and_then(|raw| path.iter().try_fold(raw, |v, key| v.get(*key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty() && !err.message.contains(*d))
        .map(str::to_owned);
    if let Some(detail) = detail {
        err.message = format!("{}: {detail}", err.message.trim_end_matches('.'));
    }
    err
}

pub(crate) fn decode_body(status: Option<u16>, body: &[u8]) -> KppError {
    let origin = kaeso_kpp::Origin::new(DIALECT);
    let parsed: Option<Ext<ErrorResponse>> = Ext::from_slice(body).ok();
    let raw: Value = serde_json::from_slice(body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body).into_owned()));
    let Some(parsed) = parsed else {
        if let Some(own) = mistral_error(&raw) {
            let kind = kind_for(status, own.ty.as_deref(), own.code.as_deref());
            let mut err = KppError::new(kind, own.message)
                .with_origin(origin)
                .with_raw(raw);
            if let Some(status) = status {
                err = err.with_status(status);
            }
            if let Some(param) = own.param {
                err = err.with_param(param);
            }
            return err;
        }
        let text = String::from_utf8_lossy(body);
        let message = match raw.get("error") {
            Some(Value::String(e)) if !e.trim().is_empty() => e.clone(),
            _ => format!(
                "The provider returned an error{} that is not in the OpenAI error format: {}",
                status.map(|s| format!(" (HTTP {s})")).unwrap_or_default(),
                Short(text.trim(), 500)
            ),
        };
        let mut err = KppError::new(kind_for(status, None, None), message)
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
    let kind = kind_for(
        status.or_else(|| http_status_code(body.code.as_ref())),
        ty,
        code,
    );
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

struct MistralError {
    message: String,
    ty: Option<String>,
    code: Option<String>,
    param: Option<String>,
}

// Mistral's shapes: `{"object": "error", "message", "type", "param", "code"}` from its
// gateway, and FastAPI's `HTTPValidationError` (`detail`) in Mistral's OpenAPI document.
fn mistral_error(raw: &Value) -> Option<MistralError> {
    let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
    let message = match (
        raw.get("object").and_then(Value::as_str),
        raw.get("message"),
    ) {
        (Some("error"), Some(Value::String(m))) => m.clone(),
        (Some("error"), Some(nested)) => validation_message(nested.get("detail")?)?,
        _ => validation_message(raw.get("detail")?)?,
    };
    let message = message.trim().to_owned();
    if message.is_empty() {
        return None;
    }
    Some(MistralError {
        message,
        ty: text("type"),
        code: text("code"),
        param: text("param"),
    })
}

fn validation_message(detail: &Value) -> Option<String> {
    let items = match detail {
        Value::String(text) => return Some(text.clone()),
        Value::Array(items) => items,
        _ => return None,
    };
    let lines = items
        .iter()
        .map(|item| {
            let msg = item.get("msg")?.as_str()?;
            let mut path = String::new();
            for part in item.get("loc")?.as_array()?.iter().skip(1) {
                match part {
                    Value::Number(n) => path.push_str(&format!("[{n}]")),
                    Value::String(s) if path.is_empty() => path.push_str(s),
                    Value::String(s) => path.push_str(&format!(".{s}")),
                    _ => return None,
                }
            }
            Some(if path.is_empty() {
                msg.to_owned()
            } else {
                format!("{msg} (`{path}`)")
            })
        })
        .collect::<Option<Vec<String>>>()?;
    let joined = lines.join("; ");
    Some(if joined.ends_with('.') {
        joined
    } else {
        format!("{joined}.")
    })
}

pub(crate) struct ErrorProbe {
    pub(crate) error: bool,
    pub(crate) choices: bool,
}

pub(crate) fn probe(json: &str) -> ErrorProbe {
    #[derive(serde::Deserialize)]
    struct Probe {
        #[serde(default)]
        error: Option<Box<serde_json::value::RawValue>>,
        #[serde(default)]
        choices: Option<serde::de::IgnoredAny>,
    }
    serde_json::from_str::<Probe>(json).map_or(
        ErrorProbe {
            error: false,
            choices: false,
        },
        |p| {
            let error = p.error.as_ref().map(|e| e.get().trim());
            ErrorProbe {
                error: error.is_some_and(|e| e.starts_with('{')),
                choices: p.choices.is_some(),
            }
        },
    )
}

fn http_status_code(code: Option<&Value>) -> Option<u16> {
    code.and_then(Value::as_u64)
        .and_then(|c| u16::try_from(c).ok())
        .filter(|c| (400..600).contains(c))
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
    err.raw.as_ref().filter(|raw| {
        from_chat
            && (raw.get("error").is_some_and(Value::is_object)
                || raw.get("choices").is_some_and(Value::is_array))
    })
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
        crate::codec::push_retry_headers(&mut headers, retry, true);
    }
    EncodedError::new(status, headers, body)
}
