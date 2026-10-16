use kaeso_kpp::{
    Block, CacheTtl, DialectId, DocumentBlock, Extras, FunctionTool, GenerateRequest,
    JsonSchemaSpec, Message, OpaqueKind, Origin, OutputSpec, RawJson, ReasoningBlock,
    ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary, Request, RequestKind, Role,
    Source, StreamMode, ToolChoice, ToolDef,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIALECT: &str = "anthropic_messages";
const A: DialectId = DialectId::AnthropicMessages;
const CHAT: DialectId = DialectId::OpenAiChat;

fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

fn headers_for(folder: &str, name: &str) -> HeaderList {
    match sidecar(DIALECT, folder, name, "headers") {
        Some(Value::Object(map)) => map
            .into_iter()
            .map(|(k, v)| (k, v.as_str().unwrap().to_owned()))
            .collect(),
        _ => HeaderList::new(),
    }
}

fn decode(kind: RequestKind, body: &[u8]) -> Request {
    AnthropicMessages::decode_request(kind, body, &HeaderList::new()).unwrap()
}

fn generate(body: Value) -> GenerateRequest {
    match decode(RequestKind::Generate, &serde_json::to_vec(&body).unwrap()) {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn fixture_request(name: &str) -> GenerateRequest {
    let (_, body) = fixtures(DIALECT, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {name}"));
    match decode(RequestKind::Generate, &body) {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode(req: &Request) -> (EncodedRequest, EgressCtx) {
    let mut ctx = EgressCtx::new(req).with_provider("anthropic");
    let enc = AnthropicMessages::encode_request(req, &mut ctx).unwrap();
    (enc, ctx)
}

fn encode_value(req: GenerateRequest) -> Value {
    let (enc, _) = encode(&Request::Generate(req));
    serde_json::from_slice(&enc.body).unwrap()
}

fn rc(f: impl FnOnce(&mut ReasoningConfig)) -> ReasoningConfig {
    let mut c = ReasoningConfig::default();
    f(&mut c);
    c
}

fn feature(req: GenerateRequest) -> String {
    AnthropicMessages::check(&Request::Generate(req))
        .unwrap_err()
        .unsupported_feature()
        .unwrap_or("<not unsupported>")
        .to_owned()
}

fn chat_request() -> GenerateRequest {
    let mut r = GenerateRequest::new("claude-sonnet-4-6").with_message(Message::user("hi"));
    r.sampling.max_output_tokens = Some(100);
    r.extras = Extras::new(CHAT);
    r
}

mod check;
mod malformed;
mod regressions;
mod replies;
mod requests;
mod streams;
