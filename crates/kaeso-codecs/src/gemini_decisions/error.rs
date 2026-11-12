//! Errors in Google's envelope. A request the input safety filters block is a 400 whose `code`
//! is one of the safety codes instead of the HTTP status (Decisions guide, "Request and response
//! reference"); KPP reads it as `ContentFiltered` and writes every `ContentFiltered` that way.

use kaeso_kpp::{ErrorKind, KppError};
use serde_json::{Value, json};

use super::DIALECT;
use crate::codec::{EncodedError, HeaderList};
use crate::gemini::{decode_error_value, encode_google_error};

/// The `code`s of a blocked request.
pub(super) const SAFETY_CODES: &[&str] = &[
    "safety",
    "prohibited_content",
    "blocklist",
    "image_safety",
    "content_blocked",
];

/// The code a blocked request is written with when nothing names a more specific one.
const DEFAULT_CODE: &str = "safety";

pub(super) fn decode(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let mut e = decode_error_value(status, headers, body, DIALECT);
    let code = e
        .raw
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|e| e.get("code"))
        .and_then(Value::as_str);
    if code.is_some_and(|c| SAFETY_CODES.contains(&c)) {
        e.kind = ErrorKind::ContentFiltered;
    }
    e
}

/// The safety code a `prompt_feedback.block_reason` is reported with as an error; `other` and
/// reasons the API may add later are `content_blocked`.
pub(super) fn code_of_block(reason: &str) -> &'static str {
    SAFETY_CODES
        .iter()
        .find(|c| **c == reason)
        .copied()
        .unwrap_or("content_blocked")
}

pub(super) fn encode(err: &KppError) -> EncodedError {
    let own = err.origin.as_ref().is_some_and(|o| o.is(DIALECT));
    let raw_error = err
        .raw
        .as_ref()
        .and_then(|v| v.get("error"))
        .is_some_and(Value::is_object);
    if err.kind != ErrorKind::ContentFiltered || (own && raw_error) {
        return encode_google_error(err, DIALECT);
    }
    let reason = err
        .raw
        .as_ref()
        .filter(|_| own)
        .and_then(|v| v.get("prompt_feedback"))
        .and_then(|f| f.get("block_reason"))
        .and_then(Value::as_str);
    let code = reason.map_or(DEFAULT_CODE, code_of_block);
    let body = json!({"error": {"code": code, "message": err.message}});
    EncodedError::new(
        400,
        HeaderList::new().with("content-type", "application/json"),
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}
