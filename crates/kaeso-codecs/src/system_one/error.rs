//! System One's errors (<https://docs.typesafe.ai/api/errors>): `{"detail": …}` with a string or
//! an object for a broken rule (400), `{"detail": [{"loc", "msg", "type"}]}` for a body that fails
//! validation (422), and 401, 413, 415, 429 and 529 for the rest.

use kaeso_kpp::{ErrorKind, KppError, Origin};
use serde_json::{Value, json};

use super::DIALECT;
use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::wire::Short;

fn kind_for(status: u16, text: &str) -> ErrorKind {
    if text.contains("max_tokens_exceeded") {
        return ErrorKind::ContextTooLong;
    }
    match status {
        400 | 415 | 422 => ErrorKind::InvalidRequest,
        401 => ErrorKind::Authentication,
        402 => ErrorKind::Billing,
        403 => ErrorKind::PermissionDenied,
        404 | 410 => ErrorKind::NotFound,
        413 => ErrorKind::RequestTooLarge,
        429 => ErrorKind::RateLimited,
        408 | 504 => ErrorKind::Timeout,
        503 | 529 => ErrorKind::Overloaded,
        _ => ErrorKind::ProviderError,
    }
}

// A 422's first problem: its message, and its location without the leading `body`.
fn first_problem(list: &[Value]) -> Option<(String, Option<String>)> {
    let first = list.first()?;
    let msg = first.get("msg").and_then(Value::as_str)?.to_owned();
    let path: Vec<String> = first
        .get("loc")
        .and_then(Value::as_array)
        .map(|loc| {
            loc.iter()
                .skip_while(|p| p.as_str() == Some("body"))
                .map(|p| match p {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let param = (!path.is_empty()).then(|| path.join("."));
    let message = match &param {
        Some(p) => format!("`{p}`: {msg}"),
        None => msg,
    };
    Some((message, param))
}

/// Reads a System One provider's error, or another envelope its gateway answers with, such as
/// OpenRouter's, Perplexity's or Cloudflare's.
pub(crate) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let detail = value.as_ref().and_then(|v| v.get("detail"));
    let Some(detail) = detail else {
        return crate::openai_responses::error::decode(DIALECT, status, headers, body);
    };
    let (message, param) = match detail {
        Value::String(s) => (s.clone(), None),
        Value::Array(list) => first_problem(list).unwrap_or_else(|| (detail.to_string(), None)),
        Value::Object(map) => (
            map.get("message")
                .or_else(|| map.get("msg"))
                .and_then(Value::as_str)
                .map_or_else(|| detail.to_string(), str::to_owned),
            None,
        ),
        other => (other.to_string(), None),
    };
    let message = if message.trim().is_empty() {
        format!("The provider returned HTTP {status} without a message.")
    } else {
        Short(&message, 2000).to_string()
    };
    let text = String::from_utf8_lossy(body);
    let mut err = KppError::new(kind_for(status, &text), message)
        .with_status(status)
        .with_origin(Origin::new(DIALECT));
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

fn status_for(err: &KppError) -> u16 {
    match &err.kind {
        ErrorKind::InvalidRequest if err.param.is_some() => 422,
        ErrorKind::InvalidRequest
        | ErrorKind::Unsupported { .. }
        | ErrorKind::ContextTooLong
        | ErrorKind::ContentFiltered => 400,
        ErrorKind::Authentication => 401,
        ErrorKind::Billing => 402,
        ErrorKind::PermissionDenied => 403,
        ErrorKind::NotFound => 404,
        ErrorKind::RequestTooLarge => 413,
        ErrorKind::RateLimited => 429,
        ErrorKind::Overloaded => 529,
        ErrorKind::Timeout => 504,
        _ => err
            .status
            .filter(|s| (500..600).contains(s) && *s != 529 && *s != 503)
            .unwrap_or(500),
    }
}

fn location(param: &str) -> Vec<Value> {
    let mut loc = vec![Value::from("body")];
    for piece in param.split('.') {
        let mut rest = piece;
        if let Some(open) = rest.find('[') {
            let (head, tail) = rest.split_at(open);
            if !head.is_empty() {
                loc.push(Value::from(head));
            }
            rest = tail;
            while let Some(after) = rest.strip_prefix('[') {
                let Some(close) = after.find(']') else {
                    break;
                };
                let (index, more) = after.split_at(close);
                loc.push(match index.parse::<u64>() {
                    Ok(n) => Value::from(n),
                    Err(_) => Value::from(index),
                });
                rest = more.get(1..).unwrap_or_default();
            }
            continue;
        }
        if !rest.is_empty() {
            loc.push(Value::from(rest));
        }
    }
    loc
}

/// An error for a System One app: a provider's own System One error as it sent it, else
/// `{"detail": …}` with the status the kind maps to.
pub(crate) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    let replay = own
        .then_some(err.raw.as_ref())
        .flatten()
        .filter(|raw| raw.get("detail").is_some());
    let (status, body) = match replay {
        Some(raw) => (err.status.unwrap_or_else(|| status_for(err)), raw.clone()),
        None => {
            let status = status_for(err);
            let foreign = err.origin.as_ref().is_some_and(|o| !o.is(DIALECT));
            let param = err.param.as_deref().filter(|_| !foreign);
            let body = match (status, param) {
                (422, Some(p)) => json!({"detail": [{
                    "loc": location(p),
                    "msg": err.message,
                    "type": "value_error",
                }]}),
                _ => json!({"detail": err.message}),
            };
            (
                if status == 422 && param.is_none() {
                    400
                } else {
                    status
                },
                body,
            )
        }
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, true);
    }
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    EncodedError::new(status, headers, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_follow_kpp_params() {
        assert_eq!(
            location("questions.tone.criteria"),
            vec![
                json!("body"),
                json!("questions"),
                json!("tone"),
                json!("criteria")
            ]
        );
        assert_eq!(
            location("questions[2].instructions"),
            vec![
                json!("body"),
                json!("questions"),
                json!(2),
                json!("instructions")
            ]
        );
        assert_eq!(
            location("x[a]"),
            vec![json!("body"), json!("x"), json!("a")]
        );
    }

    #[test]
    fn kinds_follow_the_status_and_the_context_code() {
        assert_eq!(kind_for(529, ""), ErrorKind::Overloaded);
        assert_eq!(kind_for(415, ""), ErrorKind::InvalidRequest);
        assert_eq!(
            kind_for(400, "max_tokens_exceeded"),
            ErrorKind::ContextTooLong
        );
        assert_eq!(kind_for(418, ""), ErrorKind::ProviderError);
    }
}
