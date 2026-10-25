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
        if let Some(own) = other_shape(&raw, status) {
            let status = status.or(own.status);
            let kind = kind_for(
                status.or_else(|| http_status_code(own.code.as_ref())),
                own.ty.as_deref(),
                own.code.as_ref().and_then(Value::as_str),
            );
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

struct OtherShape {
    message: String,
    ty: Option<String>,
    code: Option<Value>,
    param: Option<String>,
    status: Option<u16>,
}

// Error bodies without OpenAI's `error` object: `{"object": "error", "message", "type", "param",
// "code"}` (Mistral, vLLM), FastAPI's `detail` (Mistral's `HTTPValidationError`), a bare `message`.
fn other_shape(raw: &Value, status: Option<u16>) -> Option<OtherShape> {
    let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
    if raw.get("error").is_some_and(|e| !e.is_null()) {
        return None;
    }
    let object_error = raw
        .get("object")
        .and_then(Value::as_str)
        .is_some_and(|o| o.eq_ignore_ascii_case("error"));
    let (message, param) = match (object_error, raw.get("message"), raw.get("detail")) {
        (true, Some(Value::String(m)), _) => (m.trim().to_owned(), text("param")),
        (true, Some(nested), _) => validation(nested.get("detail")?, status)?,
        (_, _, Some(detail)) => validation(detail, status)?,
        (false, Some(Value::String(m)), None) => (quoted(m, status), None),
        _ => return None,
    };
    if message.is_empty() {
        return None;
    }
    Some(OtherShape {
        message,
        ty: text("type"),
        code: raw.get("code").filter(|c| !c.is_null()).cloned(),
        param,
        status: raw
            .get("raw_status_code")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok()),
    })
}

fn quoted(message: &str, status: Option<u16>) -> String {
    let message = message.trim();
    let end = if message.ends_with(['.', '!', '?']) {
        ""
    } else {
        "."
    };
    format!(
        "The provider returned an error{}: {message}{end}",
        status.map(|s| format!(" (HTTP {s})")).unwrap_or_default()
    )
}

const ROLE_TAGS: &[&str] = &["system", "user", "assistant", "tool"];
const CHUNK_TAGS: &[&str] = &[
    "text",
    "image_url",
    "document_url",
    "reference",
    "file",
    "thinking",
    "input_audio",
    "resource",
    "resource_link",
];

fn union_tag(part: &str, after_index: bool) -> bool {
    part == "str"
        || part.starts_with("list[")
        || (after_index && (ROLE_TAGS.contains(&part) || CHUNK_TAGS.contains(&part)))
}

fn validation(detail: &Value, status: Option<u16>) -> Option<(String, Option<String>)> {
    let items = match detail {
        Value::String(text) => return Some((quoted(text, status), None)),
        Value::Object(fields) => {
            let text = fields.get("message")?.as_str()?;
            return Some((quoted(text, status), None));
        }
        Value::Array(items) => items,
        _ => return None,
    };
    let entries = items
        .iter()
        .map(|item| {
            let msg = item.get("msg")?.as_str()?;
            let loc = item.get("loc")?.as_array()?;
            let mut path = String::new();
            let mut after_index = false;
            for part in loc.iter().skip(1) {
                match part {
                    Value::Number(n) => path.push_str(&format!("[{n}]")),
                    // FastAPI names the union member it tried (`str`, `list[…]`) and the tag of a
                    // discriminated union (a role, a chunk type) after an index; the request has no such key.
                    Value::String(s) if union_tag(s, after_index) => {}
                    Value::String(s) if path.is_empty() => path.push_str(s),
                    Value::String(s) => path.push_str(&format!(".{s}")),
                    _ => return None,
                }
                after_index = part.is_number();
            }
            Some((msg.to_owned(), path))
        })
        .collect::<Option<Vec<(String, String)>>>()?;
    let lines: Vec<String> = entries
        .iter()
        .map(|(msg, path)| match path.as_str() {
            "" => msg.clone(),
            path => format!("{msg} (`{path}`)"),
        })
        .collect();
    let joined = lines.join("; ");
    let message = if joined.ends_with('.') {
        joined
    } else {
        format!("{joined}.")
    };
    let param = match entries.as_slice() {
        [(_, path)] if !path.is_empty() => Some(path.clone()),
        _ => None,
    };
    Some((message, param))
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
        object: Option<Box<serde_json::value::RawValue>>,
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
            let object = p.object.as_ref().map(|o| o.get().trim());
            ErrorProbe {
                error: error.is_some_and(|e| e.starts_with('{')) || object == Some("\"error\""),
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
