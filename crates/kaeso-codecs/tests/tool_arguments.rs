#![cfg(test)]

use std::path::Path;
use std::sync::Arc;

use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn codex_thread() -> Value {
    serde_json::from_slice(&fixture(R, "requests/codex-desktop-blank-arguments.json")).unwrap()
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

fn ctx_for(target: DialectId, req: &Request) -> EgressCtx {
    match target {
        R => EgressCtx::new(req)
            .with_provider("openai")
            .with_responses_profile(ResponsesProfile::OPENAI),
        C => EgressCtx::new(req).with_provider("openai"),
        _ => EgressCtx::new(req).with_provider("anthropic"),
    }
}

fn try_send(target: DialectId, req: &Request) -> Result<(Value, Vec<String>), KppError> {
    let mut ctx = ctx_for(target, req);
    dispatch::check(target, req, &ctx)?;
    let enc = dispatch::encode_request(target, req, &mut ctx)?;
    Ok((serde_json::from_slice(&enc.body).unwrap(), ctx.adapted))
}

fn send(target: DialectId, req: &Request) -> (Value, Vec<String>) {
    try_send(target, req).unwrap_or_else(|e| panic!("{target}: {e}"))
}

fn tool_uses(body: &Value) -> Vec<Value> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter(|b| b["type"] == "tool_use")
        .map(|b| json!({"name": b["name"], "input": b["input"]}))
        .collect()
}

fn chat_arguments(body: &Value) -> Vec<Value> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["tool_calls"].as_array().cloned().unwrap_or_default())
        .map(|c| c["function"]["arguments"].clone())
        .collect()
}

fn responses_arguments(body: &Value) -> Vec<Value> {
    body["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "function_call")
        .map(|i| i["arguments"].clone())
        .collect()
}

fn claude_stream_to(app: DialectId, app_req: &Request) -> Vec<Value> {
    let ctx = ctx_for(A, app_req);
    let mut dec = dispatch::stream_decoder(A, &ctx).unwrap();
    let mut events = dec.feed(&fixture(A, "streams/tools-no-input.sse"));
    events.extend(dec.finish());
    let enc_ctx = EncodeCtx::for_request(Arc::new(app_req.clone()));
    let mut enc = dispatch::stream_encoder(app, &enc_ctx).unwrap();
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
            _ => None,
        })
        .collect()
}

fn of_type<'e>(events: &'e [Value], ty: &str) -> Vec<&'e Value> {
    events.iter().filter(|e| e["type"] == ty).collect()
}

#[test]
fn a_codex_thread_with_a_call_claude_made_without_arguments_continues_on_claude() {
    let req = decode(R, &codex_thread());
    let (body, adapted) = send(A, &req);
    assert_eq!(
        tool_uses(&body),
        vec![
            json!({"name": "exec_command", "input": {"cmd": "cat /home/user/.codex/attachments/00000000-0000-4000-8000-000000000001/pasted-text-1.txt"}}),
            json!({"name": "get_goal", "input": {}}),
            json!({"name": "exec_command", "input": {"cmd": "sed -n 150,330p /home/user/.codex/attachments/00000000-0000-4000-8000-000000000001/pasted-text-1.txt", "max_output_tokens": 20000}}),
        ]
    );
    assert!(adapted.is_empty(), "{adapted:?}");
}

#[test]
fn blank_responses_arguments_go_to_openai_as_sent_and_to_chat_as_an_empty_object() {
    let req = decode(R, &codex_thread());
    assert_eq!(responses_arguments(&send(R, &req).0)[1], json!(""));
    assert_eq!(chat_arguments(&send(C, &req).0)[1], json!("{}"));

    let mut spaced = codex_thread();
    spaced["input"][7]["arguments"] = json!("  ");
    let req = decode(R, &spaced);
    assert_eq!(responses_arguments(&send(R, &req).0)[1], json!("  "));
    assert_eq!(tool_uses(&send(A, &req).0)[1]["input"], json!({}));
}

#[test]
fn claude_calls_without_arguments_reach_responses_apps_as_an_empty_object() {
    let app = decode(R, &codex_thread());
    let events = claude_stream_to(R, &app);
    let deltas: String = of_type(&events, "response.function_call_arguments.delta")
        .iter()
        .map(|e| e["delta"].as_str().unwrap())
        .collect();
    assert_eq!(deltas, "{}");
    let done = of_type(&events, "response.function_call_arguments.done");
    assert_eq!(done.len(), 1);
    assert_eq!(done[0]["arguments"], "{}");
    let items: Vec<&Value> = of_type(&events, "response.output_item.done")
        .into_iter()
        .map(|e| &e["item"])
        .filter(|i| i["type"] == "function_call")
        .collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "get_goal");
    assert_eq!(items[0]["arguments"], "{}");
    let completed = of_type(&events, "response.completed");
    let output = completed[0]["response"]["output"].as_array().unwrap();
    let call = output
        .iter()
        .find(|i| i["type"] == "function_call")
        .unwrap();
    assert_eq!(call["arguments"], "{}");
}

#[test]
fn claude_calls_without_arguments_reach_chat_apps_as_an_empty_object() {
    let app = decode(
        C,
        &json!({"model": "claude-opus-5-5", "stream": true, "messages": [{"role": "user", "content": "What is my goal?"}]}),
    );
    let events = claude_stream_to(C, &app);
    let arguments: String = events
        .iter()
        .flat_map(|e| e["choices"].as_array().cloned().unwrap_or_default())
        .flat_map(|c| {
            c["delta"]["tool_calls"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|t| t["function"]["arguments"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(arguments, "{}");
}

#[test]
fn a_streamed_call_without_arguments_comes_back_to_claude_in_the_next_turn() {
    let mut thread = codex_thread();
    let app = decode(R, &thread);
    let events = claude_stream_to(R, &app);
    let input = thread["input"].as_array_mut().unwrap();
    for item in of_type(&events, "response.output_item.done") {
        input.push(item["item"].clone());
    }
    input.push(json!({"type": "function_call_output", "call_id": "toolu_016JRGCFdy7aegTehRXRsbmH", "output": "{\"goal\":null}"}));
    let (body, adapted) = send(A, &decode(R, &thread));
    assert_eq!(
        tool_uses(&body).last().unwrap(),
        &json!({"name": "get_goal", "input": {}})
    );
    assert!(adapted.is_empty(), "{adapted:?}");
}

fn chat_history(arguments: &[&str]) -> Value {
    let calls: Vec<Value> = arguments
        .iter()
        .enumerate()
        .map(|(i, a)| json!({"id": format!("call_{i}"), "type": "function", "function": {"name": "run", "arguments": a}}))
        .collect();
    let mut messages = vec![
        json!({"role": "user", "content": "Run it."}),
        json!({"role": "assistant", "content": null, "tool_calls": calls}),
    ];
    for i in 0..arguments.len() {
        messages.push(
            json!({"role": "tool", "tool_call_id": format!("call_{i}"), "content": "failed to parse arguments"}),
        );
    }
    json!({
        "model": "claude-opus-5-5",
        "max_tokens": 1024,
        "messages": messages,
        "tools": [{"type": "function", "function": {"name": "run", "parameters": {"type": "object", "properties": {"cmd": {"type": "string"}}}}}]
    })
}

#[test]
fn calls_from_other_formats_whose_arguments_are_not_an_object_reach_claude_as_one() {
    let req = decode(
        C,
        &chat_history(&[
            "{\"cmd\": \"sed -n 1,",
            "null",
            "[1, 2]",
            "42",
            "\"ls -la\"",
            "{\"cmd\": \"ls\"}",
            "",
        ]),
    );
    let (body, adapted) = send(A, &req);
    let inputs: Vec<Value> = tool_uses(&body)
        .into_iter()
        .map(|t| t["input"].clone())
        .collect();
    assert_eq!(
        inputs,
        vec![
            json!({"input": "{\"cmd\": \"sed -n 1,"}),
            json!({}),
            json!({"input": [1, 2]}),
            json!({"input": 42}),
            json!({"input": "ls -la"}),
            json!({"cmd": "ls"}),
            json!({}),
        ]
    );
    assert_eq!(
        adapted,
        [
            "tool call call_0",
            "tool call call_1",
            "tool call call_2",
            "tool call call_3",
            "tool call call_4"
        ]
    );
    assert_eq!(
        chat_arguments(&send(C, &req).0),
        vec![
            json!("{\"cmd\": \"sed -n 1,"),
            json!("null"),
            json!("[1, 2]"),
            json!("42"),
            json!("\"ls -la\""),
            json!("{\"cmd\": \"ls\"}"),
            json!(""),
        ]
    );
}

#[test]
fn token_counts_for_claude_take_the_same_arguments() {
    let mut thread = codex_thread();
    thread["input"][5]["arguments"] = json!("{\"cmd\": \"cat /home/user/");
    for key in [
        "stream",
        "store",
        "include",
        "prompt_cache_key",
        "client_metadata",
        "reasoning",
    ] {
        thread.as_object_mut().unwrap().remove(key);
    }
    let body = serde_json::to_vec(&thread).unwrap();
    let req = dispatch::decode_request(R, RequestKind::CountTokens, &body, &HeaderList::new())
        .unwrap_or_else(|e| panic!("{e}"));
    let (body, adapted) = send(A, &req);
    let inputs: Vec<Value> = tool_uses(&body)
        .into_iter()
        .map(|t| t["input"].clone())
        .collect();
    assert_eq!(inputs[0], json!({"input": "{\"cmd\": \"cat /home/user/"}));
    assert_eq!(inputs[1], json!({}));
    assert_eq!(adapted, ["tool call toolu_01GtrXhD5GoEnUhCCW1fRitv"]);
}

#[test]
fn anthropic_apps_still_get_a_tool_use_input_that_is_not_an_object_refused() {
    let req = decode(
        A,
        &json!({
            "model": "claude-opus-5-5",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "Run it."},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "run", "input": "ls"}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "ok"}]}
            ]
        }),
    );
    let err = try_send(A, &req).unwrap_err();
    assert_eq!(
        err.unsupported_feature(),
        Some("tool call arguments that are not a JSON object")
    );
    assert_eq!(err.param.as_deref(), Some("messages[1].blocks[0]"));
}
