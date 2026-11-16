use kaeso_kpp::{ErrorKind, KppError};
use serde_json::Value;

use super::D;
use crate::codec::{HeaderList, parse_retry_after};

// The error types Converse and AWS's common errors name
// (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html#API_runtime_Converse_Errors,
// https://docs.aws.amazon.com/bedrock/latest/APIReference/CommonErrors.html). In a stream the
// same types arrive in camel case, such as `throttlingException`.
fn kind_of(error_type: &str, status: u16, message: &str) -> ErrorKind {
    match error_type.to_ascii_lowercase().as_str() {
        "validationexception" if too_long(message) => ErrorKind::ContextTooLong,
        "validationexception" => ErrorKind::InvalidRequest,
        "throttlingexception" | "servicequotaexceededexception" => ErrorKind::RateLimited,
        "accessdeniedexception" => ErrorKind::PermissionDenied,
        "unrecognizedclientexception"
        | "invalidsignatureexception"
        | "incompletesignature"
        | "expiredtokenexception"
        | "missingauthenticationtokenexception" => ErrorKind::Authentication,
        "modelnotreadyexception" | "serviceunavailableexception" => ErrorKind::Overloaded,
        "modeltimeoutexception" => ErrorKind::Timeout,
        "resourcenotfoundexception" => ErrorKind::NotFound,
        "modelerrorexception" | "modelstreamerrorexception" | "internalserverexception" => {
            ErrorKind::ProviderError
        }
        _ => match status {
            400 => ErrorKind::InvalidRequest,
            401 => ErrorKind::Authentication,
            403 => ErrorKind::PermissionDenied,
            404 => ErrorKind::NotFound,
            408 => ErrorKind::Timeout,
            413 => ErrorKind::RequestTooLarge,
            429 => ErrorKind::RateLimited,
            503 => ErrorKind::Overloaded,
            _ => ErrorKind::ProviderError,
        },
    }
}

// Bedrock answers a prompt beyond the model's context with a ValidationException whose message
// says so, such as "Input is too long for requested model." or "… exceeds the context window".
fn too_long(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("too long") || m.contains("context window") || m.contains("context length")
}

// `x-amzn-errortype` may carry a namespace after a colon, and `__type` a prefix before `#`.
fn error_type(headers: &HeaderList, body: Option<&Value>) -> String {
    let from_header = headers
        .get("x-amzn-errortype")
        .map(|t| t.split(':').next().unwrap_or(t));
    let from_body = body
        .and_then(|v| v.get("__type"))
        .and_then(Value::as_str)
        .map(|t| t.rsplit('#').next().unwrap_or(t));
    from_header
        .or(from_body)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn message_of(body: Option<&Value>) -> Option<String> {
    let v = body?;
    v.get("message")
        .or_else(|| v.get("Message"))
        .and_then(Value::as_str)
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
}

pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let ty = error_type(headers, value.as_ref());
    let message = message_of(value.as_ref()).unwrap_or_else(|| {
        let text: String = String::from_utf8_lossy(body)
            .trim()
            .chars()
            .take(500)
            .collect();
        if text.is_empty() {
            format!("The provider answered with HTTP {status} and no body.")
        } else {
            text
        }
    });
    let mut e = KppError::new(kind_of(&ty, status, &message), message)
        .with_status(status)
        .with_origin(D);
    if let Some(v) = value {
        e = e.with_raw(v);
    }
    if let Some(retry) = parse_retry_after(headers) {
        e = e.with_retry_after(retry);
    }
    e
}

/// An exception message in the middle of a stream: its type and its JSON payload.
pub(super) fn in_stream(exception: &str, payload: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(payload).ok();
    let status = match exception.to_ascii_lowercase().as_str() {
        "throttlingexception" => 429,
        "validationexception" => 400,
        "serviceunavailableexception" => 503,
        "modelstreamerrorexception" => 424,
        _ => 500,
    };
    let message = message_of(value.as_ref())
        .unwrap_or_else(|| format!("Amazon Bedrock ended the stream with {exception}."));
    let mut e = KppError::new(kind_of(exception, status, &message), message)
        .with_status(status)
        .with_origin(D);
    if let Some(v) = value {
        e = e.with_raw(v);
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(ty: &str) -> HeaderList {
        let mut h = HeaderList::new();
        h.push("x-amzn-errortype", ty);
        h
    }

    #[test]
    fn the_error_type_names_the_kind() {
        let cases = [
            (
                "ThrottlingException:http://internal.amazon.com/coral/com.amazon.bedrock/",
                429,
                "Too many requests, please wait before trying again.",
                ErrorKind::RateLimited,
            ),
            (
                "ValidationException",
                400,
                "Input is too long for requested model.",
                ErrorKind::ContextTooLong,
            ),
            (
                "ValidationException",
                400,
                "The model returned the following errors: bad",
                ErrorKind::InvalidRequest,
            ),
            (
                "AccessDeniedException",
                403,
                "You don't have access to the model with the specified model ID.",
                ErrorKind::PermissionDenied,
            ),
            (
                "UnrecognizedClientException",
                403,
                "The security token included in the request is invalid.",
                ErrorKind::Authentication,
            ),
            (
                "ModelNotReadyException",
                429,
                "Model is not ready.",
                ErrorKind::Overloaded,
            ),
            (
                "ServiceUnavailableException",
                503,
                "Service unavailable.",
                ErrorKind::Overloaded,
            ),
            (
                "ModelTimeoutException",
                408,
                "Model timed out.",
                ErrorKind::Timeout,
            ),
            (
                "ResourceNotFoundException",
                404,
                "Model not found.",
                ErrorKind::NotFound,
            ),
            (
                "ModelErrorException",
                424,
                "The model failed.",
                ErrorKind::ProviderError,
            ),
        ];
        for (ty, status, message, kind) in cases {
            let body = serde_json::json!({ "message": message }).to_string();
            let e = decode(status, &headers(ty), body.as_bytes());
            assert_eq!(
                (&e.kind, e.message.as_str(), e.status),
                (&kind, message, Some(status)),
                "{ty}"
            );
        }
    }

    #[test]
    fn the_body_names_the_type_when_no_header_does_and_the_status_decides_otherwise() {
        let e = decode(
            400,
            &HeaderList::new(),
            br#"{"__type":"com.amazon.coral.validate#ValidationException","Message":"bad"}"#,
        );
        assert_eq!(
            (&e.kind, e.message.as_str()),
            (&ErrorKind::InvalidRequest, "bad")
        );
        for (status, kind) in [
            (401, ErrorKind::Authentication),
            (403, ErrorKind::PermissionDenied),
            (404, ErrorKind::NotFound),
            (408, ErrorKind::Timeout),
            (413, ErrorKind::RequestTooLarge),
            (429, ErrorKind::RateLimited),
            (503, ErrorKind::Overloaded),
            (502, ErrorKind::ProviderError),
            (400, ErrorKind::InvalidRequest),
        ] {
            assert_eq!(
                decode(status, &HeaderList::new(), b"").kind,
                kind,
                "{status}"
            );
        }
        assert_eq!(
            decode(502, &HeaderList::new(), b"").message,
            "The provider answered with HTTP 502 and no body."
        );
        assert_eq!(decode(502, &HeaderList::new(), b" oops ").message, "oops");
        let mut h = headers("ThrottlingException");
        h.push("retry-after", "3");
        assert!(decode(429, &h, b"{}").retry_after.is_some());
    }

    #[test]
    fn exceptions_in_a_stream_carry_their_status() {
        for (ty, status, kind) in [
            ("throttlingException", 429, ErrorKind::RateLimited),
            ("validationException", 400, ErrorKind::InvalidRequest),
            ("serviceUnavailableException", 503, ErrorKind::Overloaded),
            ("modelStreamErrorException", 424, ErrorKind::ProviderError),
            ("internalServerException", 500, ErrorKind::ProviderError),
        ] {
            let e = in_stream(ty, br#"{"message":"m"}"#);
            assert_eq!(
                (e.kind, e.status, e.message.as_str()),
                (kind, Some(status), "m"),
                "{ty}"
            );
        }
        assert_eq!(
            in_stream("internalServerException", b"").message,
            "Amazon Bedrock ended the stream with internalServerException."
        );
    }
}
