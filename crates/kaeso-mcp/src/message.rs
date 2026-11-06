use std::borrow::Cow;

use serde_json::value::RawValue;

use crate::error::RpcError;
use crate::meta;
use crate::raw::{RawObject, json_string, string};

const JSONRPC: &str = "jsonrpc";
const ID: &str = "id";
const METHOD: &str = "method";
const PARAMS: &str = "params";
const RESULT: &str = "result";
const ERROR: &str = "error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Request,
    Notification,
    Response,
    Error,
}

#[derive(Debug, Clone)]
pub struct Message<'a> {
    object: RawObject<'a>,
    kind: Kind,
    method: Option<Cow<'a, str>>,
    params: Option<RawObject<'a>>,
}

impl<'a> Message<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, RpcError> {
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'[') {
            return Err(RpcError::invalid_request(
                "Send one JSON-RPC message per request.",
            ));
        }
        let object = RawObject::parse(bytes).map_err(|e| {
            if e.is_syntax() || e.is_eof() {
                RpcError::parse_error()
            } else if e.to_string().starts_with("duplicate member") {
                RpcError::invalid_request("A member appears twice in the message.")
            } else {
                RpcError::invalid_request("Send one JSON-RPC message per request.")
            }
        })?;
        if object.get(JSONRPC).map(RawValue::get) != Some(r#""2.0""#) {
            return Err(RpcError::invalid_request("Set `jsonrpc` to \"2.0\"."));
        }
        if let Some(id) = object.get(ID)
            && !valid_id(id)
        {
            return Err(RpcError::invalid_request(
                "`id` must be a string or an integer.",
            ));
        }
        let method = match object.borrowed(METHOD) {
            None => None,
            Some(raw) => Some(
                borrowed_string(raw)
                    .ok_or_else(|| RpcError::invalid_request("`method` must be a string."))?,
            ),
        };
        let params = match object.borrowed(PARAMS) {
            None => None,
            Some(raw) => Some(
                RawObject::of(raw)
                    .map_err(|_| RpcError::invalid_params("`params` must be an object."))?,
            ),
        };
        let has_id = object.contains(ID);
        let kind = match (
            method.is_some(),
            has_id,
            object.contains(RESULT),
            object.contains(ERROR),
        ) {
            (true, true, false, false) => Kind::Request,
            (true, false, false, false) => Kind::Notification,
            (false, true, true, false) => Kind::Response,
            (false, true, false, true) => Kind::Error,
            _ => {
                return Err(RpcError::invalid_request(
                    "The message is neither a request, a notification nor a response.",
                ));
            }
        };
        Ok(Self {
            object,
            kind,
            method,
            params,
        })
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn id(&self) -> Option<&RawValue> {
        self.object.get(ID)
    }

    pub fn method(&self) -> Option<&str> {
        self.method.as_deref()
    }

    pub fn params(&self) -> Option<&RawObject<'a>> {
        self.params.as_ref()
    }

    pub fn result(&self) -> Option<&RawValue> {
        self.object.get(RESULT)
    }

    pub fn error(&self) -> Option<&RawValue> {
        self.object.get(ERROR)
    }

    pub fn meta(&self) -> Option<RawObject<'_>> {
        self.params
            .as_ref()?
            .get(meta::KEY)
            .and_then(|raw| RawObject::of(raw).ok())
    }

    pub fn protocol_version(&self) -> Option<String> {
        self.meta().as_ref().and_then(meta::protocol_version)
    }

    pub fn name_field(&self) -> Option<&'static str> {
        self.method().and_then(crate::headers::name_source)
    }

    pub fn name(&self) -> Option<String> {
        let field = self.name_field()?;
        self.params.as_ref()?.string(field)
    }

    pub fn set_name(&mut self, name: &str) {
        let Some(field) = self.name_field() else {
            return;
        };
        if let Some(params) = self.params.as_mut() {
            params.set(field, json_string(name));
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.push(b'{');
        for (i, (key, value)) in self.object.members().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            crate::raw::write_string(&mut out, key);
            out.push(b':');
            match (key, &self.params) {
                (PARAMS, Some(params)) => params.write(&mut out),
                _ => out.extend_from_slice(value.get().as_bytes()),
            }
        }
        out.push(b'}');
        out
    }
}

// MCP narrows JSON-RPC: a request id is a string or an integer, never null or a fraction.
fn valid_id(raw: &RawValue) -> bool {
    let text = raw.get();
    match text.as_bytes().first() {
        Some(b'"') => string(raw).is_some(),
        Some(b'-' | b'0'..=b'9') => text.parse::<i64>().is_ok() || text.parse::<u64>().is_ok(),
        _ => false,
    }
}

fn borrowed_string(raw: &RawValue) -> Option<Cow<'_, str>> {
    match serde_json::from_str::<&str>(raw.get()) {
        Ok(s) => Some(Cow::Borrowed(s)),
        Err(_) => string(raw).map(Cow::Owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::code;

    fn kind_of(body: &str) -> Result<Kind, i64> {
        Message::parse(body.as_bytes())
            .map(|m| m.kind())
            .map_err(|e| e.code)
    }

    #[test]
    fn tells_the_four_kinds_apart() {
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#),
            Ok(Kind::Request)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Ok(Kind::Notification)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":"a","result":{}}"#),
            Ok(Kind::Response)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":"a","error":{"code":1,"message":"x"}}"#),
            Ok(Kind::Error)
        );
    }

    #[test]
    fn refuses_what_is_not_one_jsonrpc_message() {
        assert_eq!(kind_of("{"), Err(code::PARSE_ERROR));
        assert_eq!(kind_of("not json"), Err(code::PARSE_ERROR));
        assert_eq!(
            kind_of(r#" [{"jsonrpc":"2.0","id":1,"method":"a"}]"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(kind_of(r#""text""#), Err(code::INVALID_REQUEST));
        assert_eq!(
            kind_of(r#"{"id":1,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"1.0","id":1,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":null,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1.5,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1,"method":7}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1,"method":"a","params":[1]}"#),
            Err(code::INVALID_PARAMS)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{}}"#),
            Err(code::INVALID_REQUEST)
        );
        assert_eq!(kind_of(r#"{"jsonrpc":"2.0"}"#), Err(code::INVALID_REQUEST));
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1,"id":2,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
    }

    #[test]
    fn large_and_negative_integer_ids_are_ids() {
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":18446744073709551615,"method":"a"}"#),
            Ok(Kind::Request)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":-3,"method":"a"}"#),
            Ok(Kind::Request)
        );
        assert_eq!(
            kind_of(r#"{"jsonrpc":"2.0","id":1e3,"method":"a"}"#),
            Err(code::INVALID_REQUEST)
        );
    }

    #[test]
    fn unknown_members_pass_through_byte_for_byte() {
        let body = br#"{"jsonrpc":"2.0","id":"r-1","method":"tools/call","x-vendor":{"keep" : [1.0, "\u00e9"]},"params":{"name":"x__create_post","arguments":{"text":"hi" , "n": 1.50},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","com.example/extra":{"a" :1}},"future":true}}"#;
        let m = Message::parse(body).unwrap();
        let out = String::from_utf8(m.to_vec()).unwrap();
        assert!(out.contains(r#""x-vendor":{"keep" : [1.0, "\u00e9"]}"#));
        assert!(out.contains(r#""arguments":{"text":"hi" , "n": 1.50}"#));
        assert!(out.contains(r#""com.example/extra":{"a" :1}"#));
        assert!(out.contains(r#""future":true"#));
        let again = Message::parse(out.as_bytes()).unwrap();
        assert_eq!(again.to_vec(), m.to_vec());
    }

    #[test]
    fn renaming_a_tool_changes_only_its_name() {
        let body = br#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"x__create_post","arguments":{"text":"hi"},"_meta":{"progressToken":"p"}}}"#;
        let mut m = Message::parse(body).unwrap();
        assert_eq!(m.name().as_deref(), Some("x__create_post"));
        m.set_name("create_post");
        assert_eq!(
            String::from_utf8(m.to_vec()).unwrap(),
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_post","arguments":{"text":"hi"},"_meta":{"progressToken":"p"}}}"#
        );
    }

    #[test]
    fn a_resource_read_is_named_by_its_uri() {
        let body =
            br#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"ui://x/post"}}"#;
        let mut m = Message::parse(body).unwrap();
        assert_eq!(m.name_field(), Some("uri"));
        assert_eq!(m.name().as_deref(), Some("ui://x/post"));
        m.set_name("ui://post");
        assert!(
            String::from_utf8(m.to_vec())
                .unwrap()
                .contains(r#""uri":"ui://post""#)
        );
    }

    #[test]
    fn methods_without_a_name_ignore_renames() {
        let body = br#"{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"cursor":"c"}}"#;
        let mut m = Message::parse(body).unwrap();
        m.set_name("x");
        assert_eq!(m.name(), None);
        assert_eq!(m.to_vec(), body);
    }

    #[test]
    fn reads_the_protocol_version_from_meta() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        let m = Message::parse(body).unwrap();
        assert_eq!(m.protocol_version().as_deref(), Some("2026-07-28"));
        assert_eq!(m.id().unwrap().get(), "1");
        assert_eq!(m.method(), Some("server/discover"));
        let no_params = Message::parse(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).unwrap();
        assert_eq!(no_params.protocol_version(), None);
        assert!(no_params.params().is_none());
    }

    #[test]
    fn responses_expose_their_result_or_error() {
        let ok = Message::parse(br#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#).unwrap();
        assert_eq!(ok.result().unwrap().get(), r#"{"tools":[]}"#);
        assert!(ok.error().is_none());
        let failed =
            Message::parse(br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"x"}}"#)
                .unwrap();
        assert!(failed.error().is_some());
        assert!(failed.method().is_none());
    }

    #[test]
    fn escaped_methods_are_read() {
        let m = Message::parse(br#"{"jsonrpc":"2.0","id":1,"method":"tools\/list"}"#).unwrap();
        assert_eq!(m.method(), Some("tools/list"));
    }
}
