use kaeso_kpp::{DialectId, ErrorKind, KppError, Origin};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::codec::{EncodedError, HeaderList, parse_retry_after};
use crate::wire::Short;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorObject,
}

#[derive(Debug, Deserialize)]
struct ErrorObject {
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "type")]
    ty: Option<String>,
    #[serde(default)]
    param: Option<Value>,
    #[serde(default)]
    code: Option<Value>,
}

const BILLING: &[&str] = &[
    "insufficient_quota",
    "credit_balance_exhausted",
    "organization_spend_limit_exceeded",
    "project_spend_limit_exceeded",
    "organization_usage_limit_exceeded",
    "billing_hard_limit_reached",
    "billing_not_active",
];

pub(crate) fn kind_for_code(code: &str) -> Option<ErrorKind> {
    Some(match code {
        "context_length_exceeded" => ErrorKind::ContextTooLong,
        c if BILLING.contains(&c) => ErrorKind::Billing,
        "server_is_overloaded" | "service_unavailable_error" => ErrorKind::Overloaded,
        "content_policy_violation"
        | "content_filter"
        | "bio_policy"
        | "cyber_policy"
        | "misalignment_policy_violation"
        | "image_content_policy_violation" => ErrorKind::ContentFiltered,
        "rate_limit_exceeded" | "rate_limit_error" | "slow_down" => ErrorKind::RateLimited,
        "server_error" => ErrorKind::ProviderError,
        "vector_store_timeout" => ErrorKind::Timeout,
        "invalid_prompt" | "invalid_request_error" => ErrorKind::InvalidRequest,
        c if c.starts_with("invalid_image") || c.starts_with("image_") || c.ends_with("_image") => {
            ErrorKind::InvalidRequest
        }
        _ => return None,
    })
}

pub(crate) fn kind_for(status: Option<u16>, ty: Option<&str>, code: Option<&str>) -> ErrorKind {
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
        Some(402) => ErrorKind::Billing,
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

pub(crate) fn decode(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> KppError {
    let origin = Origin::new(dialect);
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let parsed = value
        .as_ref()
        .and_then(|v| ErrorResponse::deserialize(v).ok());
    let mut err = match parsed {
        Some(ErrorResponse { error: e }) => {
            let code = e.code.as_ref().and_then(Value::as_str);
            let kind = kind_for(Some(status), e.ty.as_deref(), code);
            let message = e.message.filter(|m| !m.is_empty()).unwrap_or_else(|| {
                format!("The provider returned HTTP {status} without a message.")
            });
            let mut err = KppError::new(kind, message);
            if let Some(Value::String(p)) = e.param {
                err = err.with_param(p);
            }
            err
        }
        None => match value.as_ref().and_then(|v| v.get("error")) {
            Some(Value::String(message)) if !message.trim().is_empty() => {
                KppError::new(kind_for(Some(status), None, None), message.clone())
            }
            _ => {
                let text = String::from_utf8_lossy(body);
                KppError::new(
                    kind_for(Some(status), None, None),
                    format!(
                        "The provider returned HTTP {status} with a body Kaeso could not read: {}",
                        Short(&text, 300)
                    ),
                )
            }
        },
    };
    err = err.with_status(status).with_origin(origin);
    err = err.with_raw(
        value.unwrap_or_else(|| Value::String(String::from_utf8_lossy(body).into_owned())),
    );
    if let Some(retry) = parse_retry_after(headers) {
        err = err.with_retry_after(retry);
    }
    err
}

pub(crate) fn wire_for_kind(err: &KppError) -> (u16, &'static str, Option<&'static str>) {
    match &err.kind {
        ErrorKind::InvalidRequest => (400, "invalid_request_error", None),
        ErrorKind::Authentication => (401, "invalid_request_error", None),
        ErrorKind::PermissionDenied => (403, "request_forbidden", None),
        ErrorKind::NotFound => (404, "invalid_request_error", None),
        ErrorKind::RequestTooLarge => (413, "invalid_request_error", None),
        ErrorKind::RateLimited => (429, "rate_limit_error", Some("rate_limit_exceeded")),
        ErrorKind::Overloaded => (
            503,
            "service_unavailable_error",
            Some("server_is_overloaded"),
        ),
        ErrorKind::ContextTooLong => (
            400,
            "invalid_request_error",
            Some("context_length_exceeded"),
        ),
        ErrorKind::ContentFiltered => (
            400,
            "invalid_request_error",
            Some("content_policy_violation"),
        ),
        ErrorKind::Unsupported { .. } => (400, "invalid_request_error", None),
        ErrorKind::Timeout => (504, "server_error", Some("timeout")),
        ErrorKind::Billing => (429, "insufficient_quota", Some("insufficient_quota")),
        _ => {
            let status = err
                .status
                .filter(|s| (500..600).contains(s) && *s != 503)
                .unwrap_or(500);
            (status, "server_error", None)
        }
    }
}

fn is_openai(origin: Option<&Origin>) -> bool {
    origin.is_some_and(|o| {
        matches!(
            o.dialect,
            DialectId::OpenAiChat | DialectId::OpenAiResponses | DialectId::OpenAiEmbeddings
        )
    })
}

pub(crate) fn encode(err: &KppError) -> EncodedError {
    let (kind_status, ty, code) = wire_for_kind(err);
    let replay = is_openai(err.origin.as_ref())
        && match &err.raw {
            Some(Value::Object(m))
                if m.get("object").and_then(Value::as_str) != Some("response") =>
            {
                match m.get("error").map(ErrorObject::deserialize) {
                    Some(Ok(e)) => {
                        let code = e.code.as_ref().and_then(Value::as_str);
                        let status = err.status.unwrap_or(kind_status);
                        kind_for(Some(status), e.ty.as_deref(), code) == err.kind
                            && e.message.as_deref() == Some(err.message.as_str())
                    }
                    _ => false,
                }
            }
            _ => false,
        };
    let unchanged_string = is_openai(err.origin.as_ref())
        && match &err.raw {
            Some(Value::Object(m)) => matches!(
                m.get("error"),
                Some(Value::String(message))
                    if *message == err.message
            ),
            _ => false,
        };
    let (status, body) = match (&err.raw, replay) {
        (Some(raw), true) => (err.status.unwrap_or(kind_status), raw.clone()),
        _ => (
            if unchanged_string {
                err.status.unwrap_or(kind_status)
            } else {
                kind_status
            },
            json!({
                "error": {
                    "message": err.message,
                    "type": ty,
                    "param": err.param,
                    "code": code,
                }
            }),
        ),
    };
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        crate::codec::push_retry_headers(&mut headers, retry, true);
    }
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    EncodedError::new(status, headers, body)
}

pub(crate) fn stream_error_event(err: &KppError) -> Value {
    let (_, ty, code) = wire_for_kind(err);
    json!({
        "type": "error",
        "code": code.unwrap_or(ty),
        "message": err.message,
        "param": err.param,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const D: DialectId = DialectId::OpenAiResponses;

    #[test]
    fn kinds_follow_status_type_and_code() {
        let k = |s, t: &str, c: Option<&str>| kind_for(Some(s), Some(t), c);
        assert_eq!(
            k(
                400,
                "invalid_request_error",
                Some("context_length_exceeded")
            ),
            ErrorKind::ContextTooLong
        );
        assert_eq!(
            k(429, "insufficient_quota", Some("insufficient_quota")),
            ErrorKind::Billing
        );
        assert_eq!(
            k(429, "tokens", Some("rate_limit_exceeded")),
            ErrorKind::RateLimited
        );
        assert_eq!(
            k(
                503,
                "service_unavailable_error",
                Some("server_is_overloaded")
            ),
            ErrorKind::Overloaded
        );
        assert_eq!(
            k(401, "invalid_request_error", Some("invalid_api_key")),
            ErrorKind::Authentication
        );
        assert_eq!(
            k(403, "request_forbidden", None),
            ErrorKind::PermissionDenied
        );
        assert_eq!(
            k(404, "invalid_request_error", Some("model_not_found")),
            ErrorKind::NotFound
        );
        assert_eq!(k(500, "server_error", None), ErrorKind::ProviderError);
        assert_eq!(k(504, "x", None), ErrorKind::Timeout);
        assert_eq!(k(413, "x", None), ErrorKind::RequestTooLarge);
        assert_eq!(
            k(402, "invalid_request_error", Some("invalid_request_error")),
            ErrorKind::Billing
        );
    }

    #[test]
    fn unknown_bodies_keep_the_kind_of_their_status() {
        let e = decode(D, 502, &HeaderList::new(), b"<html>bad gateway</html>");
        assert_eq!(e.kind, ErrorKind::ProviderError);
        assert_eq!(e.status, Some(502));
        assert_eq!(
            e.raw,
            Some(Value::String("<html>bad gateway</html>".into()))
        );
        let e = decode(D, 418, &HeaderList::new(), b"{\"x\":1}");
        assert_eq!(e.kind, ErrorKind::ProviderError);
        let e = decode(D, 429, &HeaderList::new(), b"Too Many Requests");
        assert_eq!(e.kind, ErrorKind::RateLimited);
        let e = decode(D, 401, &HeaderList::new(), b"<html>Unauthorized</html>");
        assert_eq!(e.kind, ErrorKind::Authentication);
    }

    #[test]
    fn changed_errors_are_not_replayed_from_their_raw_body() {
        let body = br#"{"error":{"message":"Rate limit reached","type":"tokens","param":null,"code":"rate_limit_exceeded"}}"#;
        let mut e = decode(D, 429, &HeaderList::new(), body);
        let same = encode(&e);
        assert_eq!(
            same.body,
            serde_json::to_vec(&serde_json::from_slice::<Value>(body).unwrap()).unwrap()
        );
        e.kind = ErrorKind::Overloaded;
        let out = encode(&e);
        assert_eq!(out.status, 503);
        let v: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(v["error"]["code"], "server_is_overloaded");
        let mut e = decode(D, 429, &HeaderList::new(), body);
        e.message = "Kaeso: try later".into();
        let v: Value = serde_json::from_slice(&encode(&e).body).unwrap();
        assert_eq!(v["error"]["message"], "Kaeso: try later");
    }

    #[test]
    fn string_errors_keep_their_message_in_the_openai_shape() {
        let body = br#"{"code":"invalid-argument","error":"Model grok-4.3 does not support parameter presencePenalty."}"#;
        let e = decode(D, 400, &HeaderList::new(), body);
        assert_eq!(e.kind, ErrorKind::InvalidRequest);
        assert_eq!(
            e.message,
            "Model grok-4.3 does not support parameter presencePenalty."
        );
        assert_eq!(e.raw, Some(serde_json::from_slice(body).unwrap()));
        let out = encode(&e);
        assert_eq!(out.status, 400);
        let v: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(v["error"]["message"], e.message);
        assert_eq!(v["error"]["type"], "invalid_request_error");
        let blank = decode(D, 400, &HeaderList::new(), br#"{"error":" "}"#);
        assert!(blank.message.contains("could not read"));
        let gone = decode(
            D,
            410,
            &HeaderList::new(),
            br#"{"error":"Live search is deprecated."}"#,
        );
        assert_eq!(gone.kind, ErrorKind::ProviderError);
        assert_eq!(encode(&gone).status, 410);
        let mut changed = gone;
        changed.message = "Kaeso: try later".into();
        assert_eq!(encode(&changed).status, 500);
        let mut key = decode(
            D,
            400,
            &HeaderList::new(),
            br#"{"code":"invalid-argument","error":"Incorrect API key provided. You can obtain an API key from https://console.x.ai."}"#,
        );
        assert_eq!(key.kind, ErrorKind::InvalidRequest);
        key.kind = ErrorKind::Authentication;
        let out = encode(&key);
        assert_eq!(out.status, 400);
        let v: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(v["error"]["message"], key.message);
    }

    #[test]
    fn foreign_errors_get_openai_shape_and_status() {
        let e = KppError::new(ErrorKind::Overloaded, "busy")
            .with_status(529)
            .with_origin(DialectId::AnthropicMessages)
            .with_raw(json!({"type": "error", "error": {"type": "overloaded_error"}}))
            .with_retry_after(Duration::from_millis(1500));
        let out = encode(&e);
        assert_eq!(out.status, 503);
        let body: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(body["error"]["code"], "server_is_overloaded");
        assert_eq!(out.headers.get("retry-after"), Some("2"));
        assert_eq!(out.headers.get("retry-after-ms"), Some("1500"));
        let u = encode(&KppError::unsupported("top_k", "no").with_param("top_k"));
        assert_eq!(u.status, 400);
        let body: Value = serde_json::from_slice(&u.body).unwrap();
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["param"], "top_k");
        assert_eq!(
            stream_error_event(&KppError::provider("x"))["code"],
            "server_error"
        );
    }
}
