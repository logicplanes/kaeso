use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
use serde_json::{Value, json};

use super::ChatProfile;
use crate::codec::{EgressCtx, HeaderList};
use crate::dispatch;
use crate::test_support::fixtures;

const A: DialectId = DialectId::AnthropicMessages;
const B: DialectId = DialectId::BedrockConverse;
const C: DialectId = DialectId::OpenAiChat;
const G: DialectId = DialectId::Gemini;
const R: DialectId = DialectId::OpenAiResponses;

fn decoded(body: &[u8]) -> Request {
    dispatch::decode_request(C, RequestKind::Generate, body, &HeaderList::new()).unwrap()
}

fn fixture(name: &str) -> Request {
    let (_, body) = fixtures("openai_chat", "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap();
    let mut req = decoded(&body);
    if let Request::Generate(r) = &mut req {
        r.sampling.max_output_tokens.get_or_insert(100);
    }
    req
}

fn chat(messages: Value) -> Request {
    let body = json!({"model": "m", "max_completion_tokens": 100, "messages": messages});
    decoded(body.to_string().as_bytes())
}

fn sent_with(
    target: DialectId,
    req: &Request,
    mut ctx: EgressCtx,
) -> Result<(Value, Vec<String>), KppError> {
    dispatch::check(target, req, &ctx)?;
    let out = dispatch::encode_request(target, req, &mut ctx)?;
    Ok((serde_json::from_slice(&out.body).unwrap(), ctx.left_out))
}

fn sent(target: DialectId, req: &Request) -> (Value, Vec<String>) {
    sent_with(target, req, EgressCtx::new(req)).unwrap_or_else(|e| panic!("{target}: {e}"))
}

fn refusing_names() -> ChatProfile {
    ChatProfile::compatible("Example Cloud").refusing_in_messages(&["name", "refusal"])
}

fn names_in(body: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(body, &mut found);
    found
}

fn walk(v: &Value, found: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "name" && matches!(v.as_str(), Some("user" | "alice")) {
                    found.push(v.to_string());
                }
                walk(v, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, found)),
        _ => {}
    }
}

#[test]
fn autogens_participant_names_are_left_out_for_every_other_format() {
    let req = fixture("autogen-user-name");
    for target in [A, B, G, R] {
        let (body, left_out) = sent(target, &req);
        assert_eq!(left_out, ["messages[1].name"], "{target}");
        assert!(names_in(&body).is_empty(), "{target}: {body}");
    }
    let (body, left_out) = sent(C, &req);
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(
        body["messages"][1]["name"], "user",
        "OpenAI gets them as sent"
    );
}

#[test]
fn crewais_tool_message_names_are_left_out_by_the_apps_message_index() {
    let req = fixture("crewai-tool-name");
    let (body, left_out) = sent(A, &req);
    assert_eq!(left_out, ["messages[3].name"]);
    let result = &body["messages"][2]["content"][0];
    assert_eq!(
        (&result["type"], &result["tool_use_id"]),
        (&json!("tool_result"), &json!("call_1"))
    );
    assert!(result.get("name").is_none(), "{result}");
    let (_, left_out) = sent(R, &req);
    assert_eq!(left_out, ["messages[3].name"]);
}

#[test]
fn leading_system_names_are_named_by_the_message_the_app_wrote() {
    let req = fixture("system-name-multiturn");
    let (body, left_out) = sent(A, &req);
    assert_eq!(
        left_out,
        ["messages[0].name", "messages[1].name", "messages[3].name"]
    );
    assert_eq!(
        body["system"][0]["text"],
        "You are a friendly cooking assistant. Answer in two sentences or fewer."
    );
    assert!(!body.to_string().contains("house_rules"), "{body}");
}

#[test]
fn every_role_keeps_its_place_in_the_named_paths() {
    let req = chat(json!([
        {"role": "system", "name": "rules", "content": "Be brief."},
        {"role": "developer", "name": "more_rules", "content": [{"type": "text", "text": "Use tools."}]},
        {"role": "user", "name": "alice", "content": "Weather in Paris and Rome?"},
        {"role": "assistant", "name": "planner", "content": null, "tool_calls": [
            {"id": "call_1", "type": "function", "function": {"name": "weather", "arguments": "{}"}},
            {"id": "call_2", "type": "function", "function": {"name": "weather", "arguments": "{}"}}
        ]},
        {"role": "tool", "tool_call_id": "call_1", "name": "weather", "content": "Sunny"},
        {"role": "tool", "tool_call_id": "call_2", "content": "Rain"},
        {"role": "system", "name": "late", "content": "Answer in French."},
        {"role": "user", "name": "alice", "content": "Thanks!"}
    ]));
    let all = [
        "messages[0].name",
        "messages[1].name",
        "messages[2].name",
        "messages[3].name",
        "messages[4].name",
        "messages[6].name",
        "messages[7].name",
    ];
    for target in [A, G, R] {
        let (body, left_out) = sent(target, &req);
        assert_eq!(left_out, all, "{target}");
        let text = body.to_string();
        for name in ["rules", "planner", "late", "alice"] {
            assert!(!text.contains(&format!("\"{name}\"")), "{target}: {text}");
        }
    }
}

#[test]
fn a_chat_provider_that_refuses_names_gets_none_and_tool_names_stay() {
    let req = fixture("system-name-multiturn");
    let ctx = EgressCtx::new(&req).with_chat_profile(refusing_names());
    let (body, left_out) = sent_with(C, &req, ctx).unwrap();
    assert_eq!(
        left_out,
        ["messages[0].name", "messages[1].name", "messages[3].name"]
    );
    for m in body["messages"].as_array().unwrap() {
        assert!(m.get("name").is_none(), "{m}");
    }
    let req = fixture("crewai-tool-name");
    let ctx = EgressCtx::new(&req).with_chat_profile(refusing_names());
    let (body, left_out) = sent_with(C, &req, ctx).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(
        body["messages"][3]["name"], "get_weather",
        "the provider takes names on tool messages"
    );
}

#[test]
fn a_name_sent_as_null_is_not_sent_to_a_provider_that_refuses_names() {
    let req = chat(json!([
        {"role": "system", "name": null, "content": "Be brief."},
        {"role": "user", "name": null, "content": "Hi"}
    ]));
    let ctx = EgressCtx::new(&req).with_chat_profile(refusing_names());
    let (body, left_out) = sent_with(C, &req, ctx).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    for m in body["messages"].as_array().unwrap() {
        assert!(m.get("name").is_none(), "{m}");
    }
    let (body, _) = sent(C, &req);
    assert_eq!(body["messages"][1].get("name"), Some(&Value::Null));
}

#[test]
fn other_message_fields_of_a_chat_app_are_still_refused() {
    let req = chat(json!([{"role": "user", "name": "alice", "x_future": 1, "content": "Hi"}]));
    let e = sent_with(A, &req, EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("messages[0].x_future"));
}
