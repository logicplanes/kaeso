#![cfg(test)]

use std::path::Path;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn grok_cli_body() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/openai_chat/requests/grok-cli-model-id.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn decode(dialect: DialectId, body: &Value) -> Request {
    let mut req = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(4096);
    }
    req
}

fn send_with(target: DialectId, req: &Request, profile: ChatProfile) -> Result<Value, KppError> {
    let mut ctx = EgressCtx::new(req).with_chat_profile(profile);
    dispatch::check(target, req, &ctx)?;
    let enc = dispatch::encode_request(target, req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn send(target: DialectId, req: &Request) -> Value {
    send_with(target, req, ChatProfile::OPENAI).unwrap_or_else(|e| panic!("{target}: {e}"))
}

fn without_model_ids(messages: &Value) -> Value {
    let mut messages = messages.clone();
    for message in messages.as_array_mut().unwrap() {
        message.as_object_mut().unwrap().remove("model_id");
    }
    messages
}

fn has_model_id(messages: &Value) -> bool {
    messages
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m.get("model_id").is_some())
}

#[test]
fn replayed_model_ids_are_left_out_for_other_formats() {
    let req = decode(C, &grok_cli_body());
    let anthropic = send(A, &req);
    assert!(!has_model_id(&anthropic["messages"]), "{anthropic}");
    assert_eq!(anthropic["messages"][1]["content"][0]["id"], "call_kaeso_1");
    let responses = send(R, &req);
    assert!(!has_model_id(&responses["input"]), "{responses}");
    assert!(
        responses["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["call_id"] == "call_kaeso_1"),
        "{responses}"
    );
}

#[test]
fn chat_providers_get_model_ids_only_when_their_profile_takes_them() {
    let body = grok_cli_body();
    let req = decode(C, &body);
    for profile in [
        ChatProfile::OPENAI,
        ChatProfile::compatible("Example Cloud"),
    ] {
        let sent = send_with(C, &req, profile).unwrap();
        assert_eq!(sent["messages"], without_model_ids(&body["messages"]));
    }
    let keeps = ChatProfile::compatible("Example Cloud").with_message_bookkeeping();
    let sent = send_with(C, &req, keeps).unwrap();
    assert_eq!(sent["messages"], body["messages"]);
}

#[test]
fn a_null_model_id_is_left_out_like_a_set_one() {
    let mut body = grok_cli_body();
    body["messages"][4]["model_id"] = Value::Null;
    let req = decode(C, &body);
    let sent = send(C, &req);
    assert_eq!(sent["messages"], without_model_ids(&body["messages"]));
    let keeps = ChatProfile::compatible("Example Cloud").with_message_bookkeeping();
    let kept = send_with(C, &req, keeps).unwrap();
    assert_eq!(kept["messages"][4]["model_id"], Value::Null);
    assert!(!has_model_id(&send(A, &req)["messages"]));
}

#[test]
fn a_model_id_outside_a_chat_assistant_message_is_still_refused() {
    let mut on_user = grok_cli_body();
    on_user["messages"][1]["model_id"] = json!("claude-haiku-5-5");
    let mut on_tool = grok_cli_body();
    on_tool["messages"][3]["model_id"] = json!("claude-haiku-5-5");
    for (body, param) in [(on_user, "messages[0]"), (on_tool, "messages[2].blocks[0]")] {
        let req = decode(C, &body);
        for target in [A, R] {
            let err = send_with(target, &req, ChatProfile::OPENAI).unwrap_err();
            assert_eq!(
                (err.unsupported_feature(), err.param.as_deref()),
                (Some(format!("{param}.model_id").as_str()), Some(param)),
                "{target}: {err}"
            );
        }
    }

    let anthropic = json!({"model": "m", "max_tokens": 64, "messages": [
        {"role": "user", "content": "hi"},
        {"role": "assistant", "content": "hello", "model_id": "m"},
        {"role": "user", "content": "again"}
    ]});
    let req = decode(A, &anthropic);
    for target in [C, R] {
        let err = send_with(target, &req, ChatProfile::OPENAI).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some("messages[1].model_id"));
    }
}
