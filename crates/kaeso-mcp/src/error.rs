use std::borrow::Cow;

use serde_json::value::RawValue;

use crate::raw::{null, write_string};
use crate::version::Version;

pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const HEADER_MISMATCH: i64 = -32020;
    pub const MISSING_REQUIRED_CLIENT_CAPABILITY: i64 = -32021;
    pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
    // https://modelcontextprotocol.io/specification/2025-11-25/server/resources#error-handling
    pub const RESOURCE_NOT_FOUND: i64 = -32002;
}

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: Cow<'static, str>,
    pub data: Option<Box<RawValue>>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Box<RawValue>) -> Self {
        self.data = Some(data);
        self
    }

    pub fn parse_error() -> Self {
        Self::new(code::PARSE_ERROR, "The request isn't valid JSON.")
    }

    pub fn invalid_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(code::INVALID_REQUEST, message)
    }

    pub fn method_not_found() -> Self {
        Self::new(code::METHOD_NOT_FOUND, "Method not found.")
    }

    pub fn invalid_params(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(code::INVALID_PARAMS, message)
    }

    pub fn header_mismatch(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(code::HEADER_MISMATCH, message)
    }

    pub fn missing_capability(capability: &str) -> Self {
        Self::new(
            code::MISSING_REQUIRED_CLIENT_CAPABILITY,
            format!("This needs the client capability `{capability}`."),
        )
    }

    pub fn unsupported_version(requested: &str) -> Self {
        let data = serde_json::json!({ "supported": Version::names(), "requested": requested });
        let error = Self::new(
            code::UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version.",
        );
        match serde_json::value::to_raw_value(&data) {
            Ok(raw) => error.with_data(raw),
            Err(_) => error,
        }
    }

    pub fn response(&self, id: Option<&RawValue>) -> Vec<u8> {
        let mut out = Vec::with_capacity(96 + self.message.len());
        out.extend_from_slice(br#"{"jsonrpc":"2.0","id":"#);
        match id {
            Some(id) => out.extend_from_slice(id.get().as_bytes()),
            None => out.extend_from_slice(null().get().as_bytes()),
        }
        out.extend_from_slice(br#","error":{"code":"#);
        out.extend_from_slice(self.code.to_string().as_bytes());
        out.extend_from_slice(br#","message":"#);
        write_string(&mut out, &self.message);
        if let Some(data) = &self.data {
            out.extend_from_slice(br#","data":"#);
            out.extend_from_slice(data.get().as_bytes());
        }
        out.extend_from_slice(b"}}");
        out
    }
}

pub fn result_response(id: &RawValue, result: &RawValue) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + id.get().len() + result.get().len());
    out.extend_from_slice(br#"{"jsonrpc":"2.0","id":"#);
    out.extend_from_slice(id.get().as_bytes());
    out.extend_from_slice(br#","result":"#);
    out.extend_from_slice(result.get().as_bytes());
    out.push(b'}');
    out
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn parsed(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[test]
    fn codes_match_the_specification() {
        assert_eq!(code::HEADER_MISMATCH, -32020);
        assert_eq!(code::MISSING_REQUIRED_CLIENT_CAPABILITY, -32021);
        assert_eq!(code::UNSUPPORTED_PROTOCOL_VERSION, -32022);
        assert_eq!(RpcError::parse_error().code, -32700);
        assert_eq!(RpcError::invalid_request("x").code, -32600);
        assert_eq!(RpcError::method_not_found().code, -32601);
        assert_eq!(RpcError::invalid_params("x").code, -32602);
        assert_eq!(RpcError::header_mismatch("x").code, -32020);
    }

    #[test]
    fn unsupported_version_lists_what_kaeso_speaks() {
        let id = RawValue::from_string("7".to_owned()).unwrap();
        let bytes = RpcError::unsupported_version("1900-01-01").response(Some(&id));
        assert_eq!(
            parsed(&bytes),
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "error": {
                    "code": -32022,
                    "message": "Unsupported protocol version.",
                    "data": {
                        "supported": ["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"],
                        "requested": "1900-01-01"
                    }
                }
            })
        );
    }

    #[test]
    fn an_error_without_an_id_answers_with_null() {
        let bytes = RpcError::missing_capability("elicitation").response(None);
        assert_eq!(
            parsed(&bytes),
            json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32021, "message": "This needs the client capability `elicitation`." }
            })
        );
    }

    #[test]
    fn messages_are_escaped() {
        let bytes = RpcError::invalid_params("Unknown tool \"a\".").response(None);
        assert_eq!(
            parsed(&bytes)["error"]["message"],
            json!("Unknown tool \"a\".")
        );
    }

    #[test]
    fn a_result_keeps_the_id_and_result_as_written() {
        let id = RawValue::from_string(r#""req-1""#.to_owned()).unwrap();
        let result = RawValue::from_string(r#"{"tools": [ ]}"#.to_owned()).unwrap();
        assert_eq!(
            result_response(&id, &result),
            br#"{"jsonrpc":"2.0","id":"req-1","result":{"tools": [ ]}}"#
        );
    }
}
