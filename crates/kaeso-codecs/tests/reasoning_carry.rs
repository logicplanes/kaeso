#![cfg(test)]

use std::path::Path;

use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind, StreamEvent, StreamMode};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, folder: &str, name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(folder)
        .join(name);
    std::fs::read(path).unwrap()
}

fn provider(dialect: DialectId) -> &'static str {
    match dialect {
        A => "anthropic",
        _ => "openai",
    }
}

fn reply_for_app(source: DialectId, body: &[u8], app: DialectId) -> Value {
    let ctx =
        EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider(provider(source));
    let reply = dispatch::decode_response(source, 200, &HeaderList::new(), body, &ctx).unwrap();
    let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
    serde_json::from_slice(&dispatch::encode_response(app, &reply, &enc).unwrap()).unwrap()
}

fn stream_for_app(source: DialectId, sse: &[u8], app: DialectId) -> Value {
    let mode = StreamMode::on(true);
    let ctx = EgressCtx::for_kind(RequestKind::Generate, mode).with_provider(provider(source));
    let mut dec = dispatch::stream_decoder(source, &ctx).unwrap();
    let mut events: Vec<StreamEvent> = dec.feed(sse);
    events.extend(dec.finish());
    let mut enc = dispatch::stream_encoder(
        app,
        &EncodeCtx::new(RequestKind::Generate, mode).with_created_at(1),
    )
    .unwrap();
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    let items = kaeso_codecs::sse::parse_all(&out).unwrap();
    let datas: Vec<Value> = items
        .into_iter()
        .filter_map(|i| match i {
            kaeso_codecs::sse::SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
            _ => None,
        })
        .collect();
    match app {
        R => datas
            .iter()
            .find(|d| d["type"] == "response.completed")
            .map(|d| d["response"].clone())
            .unwrap(),
        _ => {
            let mut blocks: Vec<Value> = Vec::new();
            let mut json_parts: Vec<String> = Vec::new();
            for d in &datas {
                match d["type"].as_str() {
                    Some("content_block_start") => {
                        blocks.push(d["content_block"].clone());
                        json_parts.push(String::new());
                    }
                    Some("content_block_delta") => {
                        let i = d["index"].as_u64().unwrap() as usize;
                        let delta = &d["delta"];
                        let b = &mut blocks[i];
                        match delta["type"].as_str() {
                            Some("thinking_delta") => {
                                let t = b["thinking"].as_str().unwrap().to_owned();
                                b["thinking"] = json!(t + delta["thinking"].as_str().unwrap());
                            }
                            Some("signature_delta") => b["signature"] = delta["signature"].clone(),
                            Some("text_delta") => {
                                let t = b["text"].as_str().unwrap().to_owned();
                                b["text"] = json!(t + delta["text"].as_str().unwrap());
                            }
                            Some("input_json_delta") => {
                                json_parts[i].push_str(delta["partial_json"].as_str().unwrap());
                            }
                            _ => {}
                        }
                    }
                    Some("content_block_stop") => {
                        let i = d["index"].as_u64().unwrap() as usize;
                        if blocks[i]["type"] == "tool_use" && !json_parts[i].is_empty() {
                            blocks[i]["input"] = serde_json::from_str(&json_parts[i]).unwrap();
                        }
                    }
                    _ => {}
                }
            }
            json!({ "content": blocks })
        }
    }
}

fn next_request(app: DialectId, body: &Value, target: DialectId) -> Value {
    let req = dispatch::decode_request(
        app,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req).with_provider(provider(target));
    if target == A {
        ctx = ctx.with_adaptive_thinking(false);
    }
    let enc = dispatch::encode_request(target, &req, &mut ctx).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

fn responses_next_turn(output: &Value) -> Value {
    let mut input = vec![json!({"role": "user", "content": "Weather?"})];
    for item in output.as_array().unwrap() {
        input.push(item.clone());
        if item["type"] == "function_call" {
            input.push(json!({"type": "function_call_output", "call_id": item["call_id"], "output": "sunny"}));
        }
    }
    json!({"model": "claude-sonnet-4-6", "max_output_tokens": 4096, "input": input,
        "reasoning": {"effort": "medium"},
        "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object"}}]})
}

fn anthropic_reasoning(body: &[u8]) -> Vec<Value> {
    let v: Value = serde_json::from_slice(body).unwrap();
    v["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["type"] == "thinking" || b["type"] == "redacted_thinking")
        .cloned()
        .collect()
}

#[test]
fn claude_thinking_comes_back_exactly_through_a_responses_app() {
    for name in ["thinking-redacted.json", "recorded-thinking-redacted.json"] {
        let original = fixture(A, "responses", name);
        let response = reply_for_app(A, &original, R);
        let output = &response["output"];
        let first = &output[0];
        assert_eq!(first["type"], "reasoning", "{name}: {output}");
        assert!(first["summary"][0]["text"].as_str().unwrap().len() > 10);
        let body = next_request(R, &responses_next_turn(output), A);
        let assistant = &body["messages"][1]["content"];
        let want = anthropic_reasoning(&original);
        assert_eq!(
            assistant.as_array().unwrap()[..want.len()],
            want[..],
            "{name}: {body}"
        );
        assert_eq!(body["thinking"]["type"], "enabled", "{name}: {body}");
    }
}

#[test]
fn streamed_claude_thinking_comes_back_exactly_through_a_responses_app() {
    let sse = fixture(A, "streams", "thinking-long-tool-loop.sse");
    let response = stream_for_app(A, &sse, R);
    let output = &response["output"];
    assert_eq!(output[0]["type"], "reasoning");
    let body = next_request(R, &responses_next_turn(output), A);
    let thinking = &body["messages"][1]["content"][0];
    assert_eq!(thinking["type"], "thinking", "{body}");
    assert_eq!(thinking["thinking"].as_str().unwrap().len(), 4000);
    assert!(
        thinking["signature"]
            .as_str()
            .unwrap()
            .starts_with("EqQBCkYIBhgCKkBkZWVwLXRoaW5raW5n")
    );
    assert_eq!(body["messages"][1]["content"][1]["type"], "tool_use");
    assert_eq!(body["thinking"]["type"], "enabled", "{body}");
}

fn anthropic_next_turn(content: &Value) -> Value {
    let calls: Vec<Value> = content
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| json!({"type": "tool_result", "tool_use_id": b["id"], "content": "sunny"}))
        .collect();
    let next = if calls.is_empty() {
        json!({"role": "user", "content": "And tomorrow?"})
    } else {
        json!({"role": "user", "content": calls})
    };
    json!({"model": "gpt-5.5", "max_tokens": 4096,
        "thinking": {"type": "enabled", "budget_tokens": 2048},
        "messages": [{"role": "user", "content": "Umbrella?"},
            {"role": "assistant", "content": content}, next]})
}

fn openai_reasoning(body: &[u8]) -> Vec<Value> {
    let v: Value = serde_json::from_slice(body).unwrap();
    v["output"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "reasoning")
        .cloned()
        .collect()
}

#[test]
fn openai_reasoning_comes_back_exactly_through_an_anthropic_app() {
    let original = fixture(R, "responses", "reasoning-encrypted.json");
    let message = reply_for_app(R, &original, A);
    let content = &message["content"];
    assert_eq!(content[0]["type"], "thinking");
    assert!(
        content[0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("**Interpreting the forecast**")
    );
    let body = next_request(A, &anthropic_next_turn(content), R);
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    let items: Vec<Value> = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "reasoning")
        .cloned()
        .collect();
    assert_eq!(items, openai_reasoning(&original), "{body}");
}

#[test]
fn streamed_openai_reasoning_comes_back_exactly_through_an_anthropic_app() {
    let sse = fixture(R, "streams", "reasoning-summary-stream.sse");
    let message = stream_for_app(R, &sse, A);
    let content = &message["content"];
    assert_eq!(content[0]["type"], "thinking", "{content}");
    let body = next_request(A, &anthropic_next_turn(content), R);
    let item = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "reasoning")
        .cloned()
        .unwrap_or_else(|| panic!("{body}"));
    let original = kaeso_codecs::sse::parse_all(&sse)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            kaeso_codecs::sse::SseItem::Event(e) => serde_json::from_str::<Value>(&e.data).ok(),
            _ => None,
        })
        .find(|d| d["type"] == "response.completed")
        .unwrap();
    let want = original["response"]["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "reasoning")
        .cloned()
        .unwrap();
    for key in ["id", "encrypted_content", "summary"] {
        assert_eq!(item[key], want[key], "{key}: {body}");
    }
}

#[test]
fn reasoning_of_another_provider_is_never_replayed() {
    let original = fixture(A, "responses", "thinking-redacted.json");
    let response = reply_for_app(A, &original, R);
    let mut next = responses_next_turn(&response["output"]);
    next["model"] = json!("gpt-5.5");
    let body = next_request(R, &next, R);
    assert!(
        !body["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "reasoning"),
        "{body}"
    );
    let req = dispatch::decode_request(
        R,
        RequestKind::Generate,
        &serde_json::to_vec(&next).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(r) = &req else { panic!() };
    let origin = r
        .messages
        .iter()
        .flat_map(|m| &m.blocks)
        .find_map(|b| match b {
            kaeso_kpp::Block::Reasoning(r) => r.origin.clone(),
            _ => None,
        })
        .unwrap();
    assert!(origin.is(A));
    assert_eq!(origin.provider.as_deref(), Some("anthropic"));
}

fn chat_next_turn(message: &Value, model: &str) -> Value {
    let mut messages = vec![
        json!({"role": "user", "content": "Weather?"}),
        message.clone(),
    ];
    for call in message["tool_calls"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        messages.push(json!({"role": "tool", "tool_call_id": call["id"], "content": "sunny"}));
    }
    json!({"model": model, "max_completion_tokens": 4096, "reasoning_effort": "medium",
        "messages": messages,
        "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object"}}}]})
}

fn chat_next_request(
    body: &Value,
    kept: &[kaeso_codecs::continuity::CarriedReasoning],
    target: DialectId,
) -> Value {
    use kaeso_codecs::continuity::{restore_reasoning, tool_calls_missing_reasoning};
    let mut req = dispatch::decode_request(
        DialectId::OpenAiChat,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(r) = &mut req else {
        panic!()
    };
    let missing = tool_calls_missing_reasoning(r);
    let found: Vec<_> = kept
        .iter()
        .filter(|k| missing.contains(&k.tool_call_id))
        .cloned()
        .collect();
    assert!(restore_reasoning(r, &found) > 0, "nothing restored");
    let mut ctx = EgressCtx::new(&req).with_provider(provider(target));
    if target == A {
        ctx = ctx.with_adaptive_thinking(false);
    }
    let enc = dispatch::encode_request(target, &req, &mut ctx).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn a_chat_app_keeps_claude_thinking_through_the_gateway() {
    use kaeso_codecs::continuity::{ReasoningCollector, reasoning_for_tool_calls};
    let original = fixture(A, "responses", "thinking-redacted.json");
    let ctx =
        EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider("anthropic");
    let reply = dispatch::decode_response(A, 200, &HeaderList::new(), &original, &ctx).unwrap();
    let kaeso_kpp::Reply::Generate(resp) = &reply else {
        panic!()
    };
    let kept = reasoning_for_tool_calls(&resp.blocks);
    assert_eq!(kept.len(), 1);
    let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
    let chat: Value = serde_json::from_slice(
        &dispatch::encode_response(DialectId::OpenAiChat, &reply, &enc).unwrap(),
    )
    .unwrap();
    let message = &chat["choices"][0]["message"];
    assert!(message.get("reasoning").is_none());
    let body = chat_next_request(&chat_next_turn(message, "claude-haiku-4-5"), &kept, A);
    let want = anthropic_reasoning(&original);
    let content = body["messages"][1]["content"].as_array().unwrap();
    assert_eq!(content[..want.len()], want[..], "{body}");
    assert_eq!(
        body["thinking"]["type"], "enabled",
        "no reasoning is carried: {body}"
    );

    let sse = fixture(A, "streams", "thinking-long-tool-loop.sse");
    let mode = StreamMode::on(false);
    let ctx = EgressCtx::for_kind(RequestKind::Generate, mode).with_provider("anthropic");
    let mut dec = dispatch::stream_decoder(A, &ctx).unwrap();
    let mut collector = ReasoningCollector::new();
    for event in dec.feed(&sse).into_iter().chain(dec.finish()) {
        collector.observe(&event);
    }
    let kept = collector.finish();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].tool_call_id, "toolu_01KeepAliveWeatherParis01");
    assert_eq!(kept[0].blocks[0].text.concat().len(), 4000);
    let message = json!({"role": "assistant", "content": "", "tool_calls": [
        {"id": "toolu_01KeepAliveWeatherParis01", "type": "function",
         "function": {"name": "get_weather", "arguments": "{\"city\": \"Paris\"}"}}]});
    let body = chat_next_request(&chat_next_turn(&message, "claude-haiku-4-5"), &kept, A);
    let content = &body["messages"][1]["content"];
    assert_eq!(content[0]["type"], "thinking", "{body}");
    assert_eq!(content[0]["thinking"].as_str().unwrap().len(), 4000);
    assert_eq!(content[1]["type"], "tool_use", "{body}");
    assert_eq!(body["thinking"]["type"], "enabled");
}

#[test]
fn a_chat_app_keeps_openai_reasoning_through_the_gateway() {
    use kaeso_codecs::continuity::reasoning_for_tool_calls;
    let mut response: Value =
        serde_json::from_slice(&fixture(R, "responses", "reasoning-encrypted.json")).unwrap();
    let reasoning = response["output"][0].clone();
    response["output"] = json!([reasoning, {"id": "fc_1", "type": "function_call",
        "status": "completed", "call_id": "call_1", "name": "get_weather",
        "arguments": "{\"city\":\"Paris\"}"}]);
    let body = serde_json::to_vec(&response).unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider("openai");
    let reply = dispatch::decode_response(R, 200, &HeaderList::new(), &body, &ctx).unwrap();
    let kaeso_kpp::Reply::Generate(resp) = &reply else {
        panic!()
    };
    let kept = reasoning_for_tool_calls(&resp.blocks);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].tool_call_id, "call_1");
    let enc = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
    let chat: Value = serde_json::from_slice(
        &dispatch::encode_response(DialectId::OpenAiChat, &reply, &enc).unwrap(),
    )
    .unwrap();
    let next = chat_next_request(
        &chat_next_turn(&chat["choices"][0]["message"], "gpt-5.5"),
        &kept,
        R,
    );
    assert_eq!(next["include"], json!(["reasoning.encrypted_content"]));
    let items = next["input"].as_array().unwrap();
    let at = items.iter().position(|i| i["type"] == "reasoning").unwrap();
    for key in ["id", "summary", "encrypted_content"] {
        assert_eq!(items[at][key], reasoning[key], "{key}: {next}");
    }
    assert_eq!(items[at + 1]["type"], "function_call", "{next}");
}
