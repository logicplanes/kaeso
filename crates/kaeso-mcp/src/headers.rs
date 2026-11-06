use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::error::RpcError;
use crate::message::Message;
use crate::version::Version;

pub const PROTOCOL_VERSION: &str = "mcp-protocol-version";
pub const METHOD: &str = "mcp-method";
pub const NAME: &str = "mcp-name";
pub const SESSION_ID: &str = "mcp-session-id";
pub const PARAM_PREFIX: &str = "mcp-param-";

const SENTINEL_START: &str = "=?base64?";
const SENTINEL_END: &str = "?=";

#[derive(Debug, Clone, Copy, Default)]
pub struct RequestHeaders<'h> {
    pub protocol_version: Option<&'h [u8]>,
    pub method: Option<&'h [u8]>,
    pub name: Option<&'h [u8]>,
}

pub fn name_source(method: &str) -> Option<&'static str> {
    match method {
        "tools/call" | "prompts/get" => Some("name"),
        "resources/read" => Some("uri"),
        _ => None,
    }
}

pub fn version_of(
    headers: &RequestHeaders<'_>,
    message: &Message<'_>,
) -> Result<Version, RpcError> {
    let header = match headers.protocol_version {
        None => None,
        Some(raw) => Some(plain(raw).ok_or_else(invalid_characters)?),
    };
    match (message.protocol_version(), header) {
        (Some(declared), header) => {
            let version = Version::parse(&declared)
                .ok_or_else(|| RpcError::unsupported_version(&declared))?;
            match header {
                None => Err(RpcError::header_mismatch(
                    "Send the `MCP-Protocol-Version` header with the version in `_meta`.",
                )),
                Some(h) if h != declared => Err(RpcError::header_mismatch(
                    "The `MCP-Protocol-Version` header doesn't match the version in `_meta`.",
                )),
                Some(_) => Ok(version),
            }
        }
        (None, Some(h)) => {
            let version = Version::parse(h).ok_or_else(|| RpcError::unsupported_version(h))?;
            if version.is_stateless() && message.method() != Some("initialize") {
                return Err(RpcError::header_mismatch(
                    "Put the protocol version in the request's `_meta` too.",
                ));
            }
            Ok(version)
        }
        (None, None) => Ok(Version::WITHOUT_HEADER),
    }
}

pub fn check(
    version: Version,
    headers: &RequestHeaders<'_>,
    message: &Message<'_>,
) -> Result<(), RpcError> {
    let required = version.is_stateless();
    let method = message.method().unwrap_or_default();
    match headers.method {
        None if required => {
            return Err(RpcError::header_mismatch(
                "Send the `Mcp-Method` header with the request's method.",
            ));
        }
        None => {}
        Some(raw) if plain(raw).ok_or_else(invalid_characters)? != method => {
            return Err(RpcError::header_mismatch(
                "The `Mcp-Method` header doesn't match the request's method.",
            ));
        }
        Some(_) => {}
    }
    if name_source(method).is_none() {
        return Ok(());
    }
    match headers.name {
        None if required => Err(RpcError::header_mismatch(
            "Send the `Mcp-Name` header with the tool, prompt or resource the request names.",
        )),
        None => Ok(()),
        Some(raw) => {
            let decoded = decode(raw).ok_or_else(invalid_characters)?;
            if message.name().as_deref() == Some(decoded.as_ref()) {
                Ok(())
            } else {
                Err(RpcError::header_mismatch(
                    "The `Mcp-Name` header doesn't match the request's `name` or `uri`.",
                ))
            }
        }
    }
}

fn invalid_characters() -> RpcError {
    RpcError::header_mismatch("A header holds characters it can't; encode its value as base64.")
}

fn header_safe(byte: u8) -> bool {
    byte == b'\t' || (0x20..=0x7e).contains(&byte)
}

fn plain(raw: &[u8]) -> Option<&str> {
    if raw.iter().copied().all(header_safe) {
        std::str::from_utf8(raw).ok()
    } else {
        None
    }
}

pub fn decode(raw: &[u8]) -> Option<Cow<'_, str>> {
    let text = plain(raw)?;
    match text
        .strip_prefix(SENTINEL_START)
        .and_then(|rest| rest.strip_suffix(SENTINEL_END))
    {
        Some(encoded) => {
            let bytes = STANDARD.decode(encoded).ok()?;
            String::from_utf8(bytes).ok().map(Cow::Owned)
        }
        None => Some(Cow::Borrowed(text)),
    }
}

pub fn encode(value: &str) -> Cow<'_, str> {
    let safe = !value.is_empty()
        && value.bytes().all(header_safe)
        && !value.starts_with([' ', '\t'])
        && !value.ends_with([' ', '\t'])
        && !(value.starts_with(SENTINEL_START) && value.ends_with(SENTINEL_END));
    if safe {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(format!(
            "{SENTINEL_START}{}{SENTINEL_END}",
            STANDARD.encode(value)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::code;

    const MODERN_CALL: &[u8] = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_weather","arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
    const LEGACY_CALL: &[u8] =
        br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_weather","arguments":{}}}"#;

    fn headers<'h>(
        version: Option<&'h str>,
        method: Option<&'h str>,
        name: Option<&'h str>,
    ) -> RequestHeaders<'h> {
        RequestHeaders {
            protocol_version: version.map(str::as_bytes),
            method: method.map(str::as_bytes),
            name: name.map(str::as_bytes),
        }
    }

    fn outcome(body: &[u8], h: RequestHeaders<'_>) -> Result<Version, (i64, String)> {
        let m = Message::parse(body).unwrap();
        let v = version_of(&h, &m).map_err(|e| (e.code, e.message.to_string()))?;
        check(v, &h, &m).map_err(|e| (e.code, e.message.to_string()))?;
        Ok(v)
    }

    #[test]
    fn a_complete_modern_request_passes() {
        assert_eq!(
            outcome(
                MODERN_CALL,
                headers(Some("2026-07-28"), Some("tools/call"), Some("get_weather"))
            ),
            Ok(Version::V2026_07_28)
        );
    }

    #[test]
    fn a_modern_request_needs_every_header() {
        for h in [
            headers(None, Some("tools/call"), Some("get_weather")),
            headers(Some("2026-07-28"), None, Some("get_weather")),
            headers(Some("2026-07-28"), Some("tools/call"), None),
        ] {
            assert_eq!(
                outcome(MODERN_CALL, h).unwrap_err().0,
                code::HEADER_MISMATCH
            );
        }
    }

    #[test]
    fn headers_must_match_the_body() {
        for h in [
            headers(Some("2025-11-25"), Some("tools/call"), Some("get_weather")),
            headers(Some("2026-07-28"), Some("tools/list"), Some("get_weather")),
            headers(Some("2026-07-28"), Some("tools/call"), Some("delete_all")),
        ] {
            assert_eq!(
                outcome(MODERN_CALL, h).unwrap_err().0,
                code::HEADER_MISMATCH
            );
        }
    }

    #[test]
    fn an_encoded_name_is_decoded_before_comparing() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wetter_für_köln","_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        let encoded = encode("wetter_für_köln");
        assert!(encoded.starts_with("=?base64?"));
        assert_eq!(
            outcome(
                body.as_bytes(),
                headers(Some("2026-07-28"), Some("tools/call"), Some(&encoded))
            ),
            Ok(Version::V2026_07_28)
        );
    }

    #[test]
    fn unknown_versions_are_refused_with_the_supported_list() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2099-01-01"}}}"#;
        let (code, _) =
            outcome(body, headers(Some("2099-01-01"), Some("tools/list"), None)).unwrap_err();
        assert_eq!(code, code::UNSUPPORTED_PROTOCOL_VERSION);
        let (code, _) = outcome(LEGACY_CALL, headers(Some("2024-11-05"), None, None)).unwrap_err();
        assert_eq!(code, code::UNSUPPORTED_PROTOCOL_VERSION);
    }

    #[test]
    fn legacy_requests_take_the_header_or_the_first_version() {
        assert_eq!(
            outcome(LEGACY_CALL, headers(Some("2025-06-18"), None, None)),
            Ok(Version::V2025_06_18)
        );
        assert_eq!(
            outcome(LEGACY_CALL, headers(None, None, None)),
            Ok(Version::V2025_03_26)
        );
    }

    #[test]
    fn legacy_requests_check_headers_they_send() {
        assert_eq!(
            outcome(
                LEGACY_CALL,
                headers(Some("2025-11-25"), Some("tools/call"), Some("get_weather"))
            ),
            Ok(Version::V2025_11_25)
        );
        assert_eq!(
            outcome(
                LEGACY_CALL,
                headers(Some("2025-11-25"), Some("tools/call"), Some("other"))
            )
            .unwrap_err()
            .0,
            code::HEADER_MISMATCH
        );
        assert_eq!(
            outcome(
                LEGACY_CALL,
                headers(Some("2025-11-25"), Some("initialize"), None)
            )
            .unwrap_err()
            .0,
            code::HEADER_MISMATCH
        );
    }

    #[test]
    fn the_newest_version_needs_meta_except_for_initialize() {
        assert_eq!(
            outcome(
                LEGACY_CALL,
                headers(Some("2026-07-28"), Some("tools/call"), Some("get_weather"))
            )
            .unwrap_err()
            .0,
            code::HEADER_MISMATCH
        );
        let init = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#;
        let m = Message::parse(init).unwrap();
        assert_eq!(
            version_of(&headers(Some("2026-07-28"), None, None), &m).unwrap(),
            Version::V2026_07_28
        );
    }

    #[test]
    fn methods_without_a_name_need_no_name_header() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        assert_eq!(
            outcome(body, headers(Some("2026-07-28"), Some("tools/list"), None)),
            Ok(Version::V2026_07_28)
        );
    }

    #[test]
    fn control_characters_in_headers_are_refused() {
        let m = Message::parse(MODERN_CALL).unwrap();
        let bad = RequestHeaders {
            protocol_version: Some(b"2026-07-28\r\n"),
            method: Some(b"tools/call"),
            name: Some(b"get_weather"),
        };
        assert_eq!(
            version_of(&bad, &m).unwrap_err().code,
            code::HEADER_MISMATCH
        );
        let bad_name = RequestHeaders {
            protocol_version: Some(b"2026-07-28"),
            method: Some(b"tools/call"),
            name: Some("wetter\u{e9}".as_bytes()),
        };
        assert_eq!(
            check(Version::V2026_07_28, &bad_name, &m).unwrap_err().code,
            code::HEADER_MISMATCH
        );
        let bad_method = RequestHeaders {
            protocol_version: Some(b"2026-07-28"),
            method: Some(b"tools/call\x00"),
            name: Some(b"get_weather"),
        };
        assert_eq!(
            check(Version::V2026_07_28, &bad_method, &m)
                .unwrap_err()
                .code,
            code::HEADER_MISMATCH
        );
    }

    #[test]
    fn encoding_follows_the_specification_table() {
        assert_eq!(encode("us-west1"), "us-west1");
        assert_eq!(encode("Hello, 世界"), "=?base64?SGVsbG8sIOS4lueVjA==?=");
        assert_eq!(encode(" padded "), "=?base64?IHBhZGRlZCA=?=");
        assert_eq!(encode("line1\nline2"), "=?base64?bGluZTEKbGluZTI=?=");
        assert_eq!(
            encode("=?base64?literal?="),
            "=?base64?PT9iYXNlNjQ/bGl0ZXJhbD89?="
        );
        assert_eq!(encode(""), "=?base64??=");
    }

    #[test]
    fn decoding_undoes_encoding() {
        for value in [
            "plain",
            "Hello, 世界",
            " padded ",
            "a\tb",
            "=?base64?literal?=",
            "",
        ] {
            assert_eq!(decode(encode(value).as_bytes()).as_deref(), Some(value));
        }
        assert_eq!(decode(b"=?base64?!!!?="), None);
        assert_eq!(decode(b"=?base64?/w==?="), None);
    }

    #[test]
    fn names_come_from_the_method() {
        assert_eq!(name_source("tools/call"), Some("name"));
        assert_eq!(name_source("prompts/get"), Some("name"));
        assert_eq!(name_source("resources/read"), Some("uri"));
        assert_eq!(name_source("tools/list"), None);
    }
}
