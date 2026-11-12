use std::time::Duration;

use kaeso_kpp::{DialectId, ErrorKind, KppError};
use serde_json::{Value, json};

use crate::codec::{EncodedError, HeaderList, parse_retry_after, push_retry_headers};

const RETRY_INFO: &str = "type.googleapis.com/google.rpc.RetryInfo";
const ERROR_INFO: &str = "type.googleapis.com/google.rpc.ErrorInfo";
const BAD_REQUEST: &str = "type.googleapis.com/google.rpc.BadRequest";

// google.rpc.Code names, as the Gemini API writes them in `error.status`
// (https://aistudio.google.com/docs/troubleshooting, "Error codes").
fn kind_of(status: Option<&str>, http: u16, details: &[Value], message: &str) -> ErrorKind {
    let reason = details
        .iter()
        .filter(|d| d["@type"] == ERROR_INFO)
        .find_map(|d| d["reason"].as_str());
    match status {
        Some("INVALID_ARGUMENT") if reason == Some("API_KEY_INVALID") => ErrorKind::Authentication,
        Some("INVALID_ARGUMENT") if message.contains("exceeds the maximum number of tokens") => {
            ErrorKind::ContextTooLong
        }
        Some("INVALID_ARGUMENT" | "FAILED_PRECONDITION" | "OUT_OF_RANGE") => {
            ErrorKind::InvalidRequest
        }
        Some("UNAUTHENTICATED") => ErrorKind::Authentication,
        Some("PERMISSION_DENIED") => ErrorKind::PermissionDenied,
        Some("NOT_FOUND") => ErrorKind::NotFound,
        Some("RESOURCE_EXHAUSTED") => ErrorKind::RateLimited,
        Some("UNAVAILABLE") => ErrorKind::Overloaded,
        Some("DEADLINE_EXCEEDED") => ErrorKind::Timeout,
        Some("INTERNAL" | "UNKNOWN" | "DATA_LOSS" | "ABORTED" | "CANCELLED" | "UNIMPLEMENTED") => {
            ErrorKind::ProviderError
        }
        _ => match http {
            400 => ErrorKind::InvalidRequest,
            401 => ErrorKind::Authentication,
            403 => ErrorKind::PermissionDenied,
            404 | 410 => ErrorKind::NotFound,
            413 => ErrorKind::RequestTooLarge,
            429 => ErrorKind::RateLimited,
            503 => ErrorKind::Overloaded,
            504 => ErrorKind::Timeout,
            _ => ErrorKind::ProviderError,
        },
    }
}

fn status_name(kind: &ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Authentication => "UNAUTHENTICATED",
        ErrorKind::PermissionDenied => "PERMISSION_DENIED",
        ErrorKind::NotFound => "NOT_FOUND",
        ErrorKind::RateLimited | ErrorKind::Billing => "RESOURCE_EXHAUSTED",
        ErrorKind::Overloaded => "UNAVAILABLE",
        ErrorKind::Timeout => "DEADLINE_EXCEEDED",
        ErrorKind::ProviderError => "INTERNAL",
        _ => "INVALID_ARGUMENT",
    }
}

fn default_code(kind: &ErrorKind) -> u16 {
    match kind {
        ErrorKind::Authentication => 401,
        ErrorKind::PermissionDenied => 403,
        ErrorKind::NotFound => 404,
        ErrorKind::RequestTooLarge => 413,
        ErrorKind::RateLimited | ErrorKind::Billing => 429,
        ErrorKind::Overloaded => 503,
        ErrorKind::Timeout => 504,
        ErrorKind::ProviderError => 500,
        _ => 400,
    }
}

/// A `google.protobuf.Duration` in JSON, such as `"23s"` or `"1.5s"`.
fn parse_delay(text: &str) -> Option<Duration> {
    let secs: f64 = text.strip_suffix('s')?.parse().ok()?;
    Duration::try_from_secs_f64(secs).ok()
}

fn format_delay(d: Duration) -> String {
    if d.subsec_nanos() == 0 {
        format!("{}s", d.as_secs())
    } else {
        format!("{}s", d.as_secs_f64())
    }
}

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    decode_error_value(status, headers, body, DialectId::Gemini)
}

// Google's error shape (https://google.aip.dev/193): `{"error": {"code": 404, "message": "…",
// "status": "NOT_FOUND"}}`, which Gemini Enterprise Agent Platform answers in front of the
// partner models it serves in their own formats.
pub(crate) fn is_google_error(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    value.get("error").is_some_and(|e| {
        e.get("status").is_some_and(Value::is_string) && e.get("code").is_some_and(Value::is_u64)
    })
}

pub(crate) fn decode_error_value(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    dialect: DialectId,
) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let error = value.as_ref().and_then(|v| v.get("error"));
    let Some(error) = error.filter(|e| e.is_object()) else {
        let text = String::from_utf8_lossy(body);
        let text: String = text.trim().chars().take(500).collect();
        let message = if text.is_empty() {
            format!("The provider answered with HTTP {status} and no body.")
        } else {
            text
        };
        let mut e = KppError::new(kind_of(None, status, &[], ""), message)
            .with_status(status)
            .with_origin(dialect);
        if let Some(v) = value {
            e = e.with_raw(v);
        }
        if let Some(retry) = parse_retry_after(headers) {
            e = e.with_retry_after(retry);
        }
        return e;
    };
    let details: Vec<Value> = error["details"].as_array().cloned().unwrap_or_default();
    let message = error["message"].as_str().unwrap_or_default().to_owned();
    let kind = kind_of(error["status"].as_str(), status, &details, &message);
    let mut e = KppError::new(kind, message)
        .with_status(status)
        .with_origin(dialect);
    if let Some(field) = details
        .iter()
        .filter(|d| d["@type"] == BAD_REQUEST)
        .find_map(|d| {
            d.get("fieldViolations")
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("field"))
                .and_then(Value::as_str)
        })
    {
        e = e.with_param(field);
    }
    let retry = details
        .iter()
        .filter(|d| d["@type"] == RETRY_INFO)
        .find_map(|d| d["retryDelay"].as_str().and_then(parse_delay))
        .or_else(|| parse_retry_after(headers));
    if let Some(retry) = retry {
        e = e.with_retry_after(retry);
    }
    if let Some(v) = value {
        e = e.with_raw(v);
    }
    e
}

// A status another format's provider chose, such as Anthropic's 529, means nothing to a
// Gemini app; its error kind decides the Google code instead.
fn code_for(err: &KppError) -> u16 {
    let google = err.origin.as_ref().is_none_or(|o| {
        matches!(
            o.dialect,
            DialectId::Gemini | DialectId::GeminiEmbeddings | DialectId::GeminiDecisions
        )
    });
    match err.status {
        Some(status) if google => status,
        _ => default_code(&err.kind),
    }
}

pub(crate) fn error_body(err: &KppError, dialect: DialectId) -> Value {
    if let (Some(origin), Some(raw)) = (&err.origin, &err.raw)
        && origin.is(dialect)
        && raw.get("error").is_some_and(Value::is_object)
    {
        return raw.clone();
    }
    let code = code_for(err);
    let mut error = json!({
        "code": code,
        "message": err.message,
        "status": status_name(&err.kind),
    });
    if let Some(retry) = err.retry_after
        && matches!(err.kind, ErrorKind::RateLimited | ErrorKind::Overloaded)
        && let Some(fields) = error.as_object_mut()
    {
        fields.insert(
            "details".into(),
            json!([{ "@type": RETRY_INFO, "retryDelay": format_delay(retry) }]),
        );
    }
    json!({ "error": error })
}

pub(crate) fn encode_error(err: &KppError, dialect: DialectId) -> EncodedError {
    let body = error_body(err, dialect);
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(Value::as_u64)
        .and_then(|c| u16::try_from(c).ok())
        .filter(|_| err.origin.as_ref().is_some_and(|o| o.is(dialect)))
        .unwrap_or_else(|| code_for(err));
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if let Some(retry) = err.retry_after {
        push_retry_headers(&mut headers, retry, false);
    }
    EncodedError::new(code, headers, serde_json::to_vec(&body).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_round_trip_through_their_json_spelling() {
        assert_eq!(parse_delay("23s"), Some(Duration::from_secs(23)));
        assert_eq!(parse_delay("1.5s"), Some(Duration::from_millis(1500)));
        assert_eq!(parse_delay("soon"), None);
        assert_eq!(parse_delay("-1s"), None);
        assert_eq!(format_delay(Duration::from_secs(23)), "23s");
        assert_eq!(format_delay(Duration::from_millis(1500)), "1.5s");
    }

    #[test]
    fn statuses_and_http_codes_name_the_kind() {
        let k = |s: Option<&str>, http| kind_of(s, http, &[], "");
        assert_eq!(k(Some("UNAUTHENTICATED"), 401), ErrorKind::Authentication);
        assert_eq!(
            k(Some("PERMISSION_DENIED"), 403),
            ErrorKind::PermissionDenied
        );
        assert_eq!(k(Some("DEADLINE_EXCEEDED"), 504), ErrorKind::Timeout);
        assert_eq!(k(Some("INTERNAL"), 500), ErrorKind::ProviderError);
        assert_eq!(
            k(Some("FAILED_PRECONDITION"), 400),
            ErrorKind::InvalidRequest
        );
        for (http, kind) in [
            (400, ErrorKind::InvalidRequest),
            (401, ErrorKind::Authentication),
            (403, ErrorKind::PermissionDenied),
            (404, ErrorKind::NotFound),
            (410, ErrorKind::NotFound),
            (413, ErrorKind::RequestTooLarge),
            (429, ErrorKind::RateLimited),
            (503, ErrorKind::Overloaded),
            (504, ErrorKind::Timeout),
            (502, ErrorKind::ProviderError),
        ] {
            assert_eq!(k(None, http), kind, "{http}");
        }
        assert_eq!(
            kind_of(
                Some("INVALID_ARGUMENT"),
                400,
                &[],
                "The input token count (2000000) exceeds the maximum number of tokens allowed (1048576)."
            ),
            ErrorKind::ContextTooLong
        );
    }

    #[test]
    fn errors_of_other_formats_get_the_google_shape() {
        for (kind, code, status) in [
            (ErrorKind::Authentication, 401, "UNAUTHENTICATED"),
            (ErrorKind::PermissionDenied, 403, "PERMISSION_DENIED"),
            (ErrorKind::NotFound, 404, "NOT_FOUND"),
            (ErrorKind::RequestTooLarge, 413, "INVALID_ARGUMENT"),
            (ErrorKind::Billing, 429, "RESOURCE_EXHAUSTED"),
            (ErrorKind::Overloaded, 503, "UNAVAILABLE"),
            (ErrorKind::Timeout, 504, "DEADLINE_EXCEEDED"),
            (ErrorKind::ProviderError, 500, "INTERNAL"),
            (ErrorKind::unsupported("x"), 400, "INVALID_ARGUMENT"),
        ] {
            let out = encode_error(&KppError::new(kind, "No."), DialectId::Gemini);
            assert_eq!(out.status, code);
            let v: Value = serde_json::from_slice(&out.body).unwrap();
            assert_eq!(v["error"]["status"], status);
            assert_eq!(v["error"]["code"], code);
            assert_eq!(v["error"]["message"], "No.");
        }
        let limited = KppError::new(ErrorKind::RateLimited, "Slow down.")
            .with_retry_after(Duration::from_millis(2500));
        let out = encode_error(&limited, DialectId::Gemini);
        assert_eq!(out.headers.get("retry-after"), Some("3"));
        let v: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(v["error"]["details"][0]["retryDelay"], "2.5s");
        let back = decode_error(429, &HeaderList::new(), &out.body);
        assert_eq!(back.retry_after, Some(Duration::from_millis(2500)));
    }

    #[test]
    fn bodies_without_a_google_error_keep_their_text() {
        let e = decode_error(
            502,
            &HeaderList::new().with("retry-after", "4"),
            b"<html>bad gateway</html>",
        );
        assert_eq!(e.kind, ErrorKind::ProviderError);
        assert_eq!(e.message, "<html>bad gateway</html>");
        assert_eq!(e.retry_after, Some(Duration::from_secs(4)));
        let e = decode_error(503, &HeaderList::new(), b"");
        assert_eq!(e.kind, ErrorKind::Overloaded);
        assert!(e.message.contains("503"), "{}", e.message);
        let e = decode_error(400, &HeaderList::new(), br#"{"error":"bad"}"#);
        assert_eq!(e.kind, ErrorKind::InvalidRequest);
        assert!(e.raw.is_some());
    }
}
