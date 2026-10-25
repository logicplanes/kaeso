#![cfg(test)]

use std::path::Path;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind};
use serde_json::Value;

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn grok_cli_request() -> (Vec<u8>, Request) {
    let body = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/openai_chat/requests/grok-cli-model-id.json"),
    )
    .unwrap();
    let mut req =
        dispatch::decode_request(C, RequestKind::Generate, &body, &HeaderList::new()).unwrap();
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(4096);
    }
    (body, req)
}

fn send(target: DialectId, req: &Request) -> Value {
    send_with(target, req, EgressCtx::new(req))
}

fn send_with(target: DialectId, req: &Request, mut ctx: EgressCtx) -> Value {
    dispatch::check(target, req, &ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
    let enc = dispatch::encode_request(target, req, &mut ctx).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn replayed_model_ids_are_left_out_for_other_formats() {
    let (_, req) = grok_cli_request();
    for target in [A, R] {
        let body = send(target, &req);
        let text = body.to_string();
        assert!(!text.contains("model_id"), "{target}: {text}");
        assert!(text.contains("call_kaeso_1"), "{target}: {text}");
    }
}

#[test]
fn replayed_model_ids_reach_a_chat_completions_provider_unchanged() {
    let (body, req) = grok_cli_request();
    let sent = send(C, &req);
    let original: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(sent["messages"], original["messages"]);
}

#[test]
fn a_chat_provider_that_forbids_undeclared_message_fields_gets_them_without_model_ids() {
    let (body, req) = grok_cli_request();
    let strict = ChatProfile::compatible("Example Cloud").without_message_bookkeeping();
    let sent = send_with(C, &req, EgressCtx::new(&req).with_chat_profile(strict));
    let mut expected: Value = serde_json::from_slice(&body).unwrap();
    for message in expected["messages"].as_array_mut().unwrap() {
        message.as_object_mut().unwrap().remove("model_id");
    }
    assert_eq!(sent["messages"], expected["messages"]);
}
