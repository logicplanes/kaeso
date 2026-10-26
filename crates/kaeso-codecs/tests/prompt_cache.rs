#![cfg(test)]

use std::path::Path;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind, StreamEvent, StreamMode};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join("requests")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn try_send(
    from: DialectId,
    to: DialectId,
    kind: RequestKind,
    body: &Value,
) -> Result<Value, KppError> {
    try_send_with(from, to, kind, body, None)
}

fn try_send_with(
    from: DialectId,
    to: DialectId,
    kind: RequestKind,
    body: &Value,
    thinking_binding: Option<bool>,
) -> Result<Value, KppError> {
    let mut req = dispatch::decode_request(
        from,
        kind,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )?;
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(64_000);
    }
    let provider = if to == A { "anthropic" } else { "openai" };
    let mut ctx = EgressCtx::new(&req)
        .with_provider(provider)
        .with_adaptive_thinking(true)
        .with_reasoning(true);
    ctx.thinking_binding = thinking_binding;
    dispatch::check(to, &req, &EgressCtx::new(&req))?;
    let enc = dispatch::encode_request(to, &req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn send(from: DialectId, to: DialectId, body: &Value) -> Value {
    try_send(from, to, RequestKind::Generate, body).unwrap()
}

fn breakpoints(value: &Value) -> Vec<Value> {
    let mut found = Vec::new();
    collect(value, &mut found);
    found
}

fn collect(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key == "cache_control" {
                    found.push(inner.clone());
                } else {
                    collect(inner, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect(i, found)),
        _ => {}
    }
}

fn last_function_tool(body: &Value) -> &Value {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|t| t.get("input_schema").is_some())
        .unwrap()
}

fn last_system_block(body: &Value) -> &Value {
    body["system"].as_array().unwrap().last().unwrap()
}

fn codex_next_turn(turn: &Value) -> Value {
    let mut next = turn.clone();
    let input = next["input"].as_array_mut().unwrap();
    input.extend([
        json!({"type": "function_call", "call_id": "call_next_1", "name": "exec_command",
            "arguments": "{\"cmd\":\"ls\"}"}),
        json!({"type": "function_call_output", "call_id": "call_next_1", "output": "Cargo.toml\nsrc"}),
        json!({"type": "message", "role": "assistant",
            "content": [{"type": "output_text", "text": "Two entries."}]}),
        json!({"type": "message", "role": "user",
            "content": [{"type": "input_text", "text": "Count them again."}]}),
    ]);
    next
}

#[test]
fn codex_on_claude_caches_the_tools_the_system_prompt_and_the_conversation() {
    let body = send(R, A, &fixture(R, "codex-developer-resume.json"));
    let ephemeral = json!({"type": "ephemeral"});
    assert_eq!(body["cache_control"], ephemeral);
    assert_eq!(last_function_tool(&body)["cache_control"], ephemeral);
    assert_eq!(last_system_block(&body)["cache_control"], ephemeral);
    assert_eq!(breakpoints(&body).len(), 3);
}

#[test]
fn every_codex_turn_on_claude_is_cached() {
    for name in [
        "codex-developer-resume.json",
        "codex-model-switch.json",
        "codex-namespace-tools.json",
        "codex-web-search-resume.json",
    ] {
        let body = send(R, A, &fixture(R, name));
        assert_eq!(
            body["cache_control"],
            json!({"type": "ephemeral"}),
            "{name}"
        );
        assert!(breakpoints(&body).len() <= 4, "{name}");
    }
}

#[test]
fn a_codex_turn_repeats_the_previous_turn_byte_for_byte() {
    let turn = fixture(R, "codex-developer-resume.json");
    let first = send(R, A, &turn);
    let second = send(R, A, &codex_next_turn(&turn));
    assert_eq!(first["tools"], second["tools"]);
    assert_eq!(first["system"], second["system"]);
    assert_eq!(first["thinking"], second["thinking"]);
    let earlier = first["messages"].as_array().unwrap();
    let later = second["messages"].as_array().unwrap();
    assert!(later.len() > earlier.len());
    assert_eq!(earlier[..], later[..earlier.len()]);
}

#[test]
fn chat_apps_on_claude_are_cached() {
    let body = send(
        C,
        A,
        &json!({"model": "claude", "messages": [
            {"role": "system", "content": "You review pull requests."},
            {"role": "user", "content": "Review #12."}],
            "tools": [
                {"type": "function", "function": {"name": "read_file", "parameters": {"type": "object"}}},
                {"type": "function", "function": {"name": "list_files", "parameters": {"type": "object"}}}]}),
    );
    let ephemeral = json!({"type": "ephemeral"});
    assert_eq!(body["cache_control"], ephemeral);
    assert_eq!(body["tools"][0].get("cache_control"), None);
    assert_eq!(body["tools"][1]["cache_control"], ephemeral);
    assert_eq!(last_system_block(&body)["cache_control"], ephemeral);
}

#[test]
fn a_request_without_tools_or_system_prompt_still_caches_the_conversation() {
    let body = send(R, A, &json!({"model": "claude", "input": "Hello"}));
    assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
    assert_eq!(breakpoints(&body).len(), 1);
}

#[test]
fn an_app_that_places_its_own_breakpoints_keeps_only_those() {
    let body = send(
        R,
        A,
        &json!({"model": "claude", "instructions": "Be brief.", "input": [
            {"type": "message", "role": "user", "content": [
                {"type": "input_text", "text": "A long shared document.", "prompt_cache_breakpoint": {"mode": "explicit"}},
                {"type": "input_text", "text": "Summarise it."}]}]}),
    );
    assert_eq!(body.get("cache_control"), None);
    assert_eq!(breakpoints(&body), vec![json!({"type": "ephemeral"})]);
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn explicit_caching_without_breakpoints_caches_nothing() {
    let body = send(
        R,
        A,
        &json!({"model": "claude", "input": "Hello",
            "prompt_cache_options": {"mode": "explicit", "ttl": "30m"}}),
    );
    assert!(breakpoints(&body).is_empty());
}

#[test]
fn a_cache_meant_to_outlive_five_minutes_is_kept_for_an_hour() {
    let hour = json!({"type": "ephemeral", "ttl": "1h"});
    let five = json!({"type": "ephemeral"});
    let cases = [
        (R, json!({"prompt_cache_retention": "24h"}), &hour),
        (R, json!({"prompt_cache_retention": "in_memory"}), &five),
        (R, json!({"prompt_cache_options": {"ttl": "30m"}}), &hour),
        (R, json!({"prompt_cache_options": {"ttl": "5m"}}), &five),
        (R, json!({"prompt_cache_options": {"ttl": "300s"}}), &five),
        (R, json!({"prompt_cache_options": {"ttl": "301s"}}), &hour),
        (R, json!({"prompt_cache_options": {"ttl": "soon"}}), &five),
        (C, json!({"prompt_cache_retention": "24h"}), &hour),
        (
            C,
            json!({"prompt_cache_options": {"mode": "implicit", "ttl": "1d"}}),
            &hour,
        ),
    ];
    for (from, settings, expected) in cases {
        let mut request = if from == R {
            json!({"model": "claude", "instructions": "Be brief.", "input": "Hello",
                "tools": [{"type": "function", "name": "f", "parameters": {"type": "object"}}]})
        } else {
            json!({"model": "claude", "messages": [
                {"role": "system", "content": "Be brief."}, {"role": "user", "content": "Hello"}],
                "tools": [{"type": "function", "function": {"name": "f", "parameters": {"type": "object"}}}]})
        };
        request
            .as_object_mut()
            .unwrap()
            .extend(settings.as_object().unwrap().clone());
        let body = send(from, A, &request);
        assert_eq!(breakpoints(&body), vec![expected.clone(); 3], "{settings}");
    }
}

#[test]
fn cache_settings_are_still_refused_where_they_cannot_be_honoured() {
    let err = try_send(
        R,
        C,
        RequestKind::Generate,
        &json!({"model": "gpt", "input": "Hello", "prompt_cache_retention": "24h"}),
    )
    .unwrap_err();
    assert!(err.message.contains("`prompt_cache_retention`"), "{err:?}");
}

#[test]
fn token_counts_carry_no_breakpoints() {
    let body = try_send(
        R,
        A,
        RequestKind::CountTokens,
        &json!({"model": "claude", "instructions": "Be brief.", "input": "Hello",
            "tools": [{"type": "function", "name": "f", "parameters": {"type": "object"}}]}),
    )
    .unwrap();
    assert!(breakpoints(&body).is_empty());
}

#[test]
fn anthropic_apps_keep_their_own_caching() {
    let own = json!({"model": "claude", "max_tokens": 100, "system": "Be brief.",
        "tools": [{"name": "f", "input_schema": {"type": "object"}}],
        "messages": [{"role": "user", "content": "Hello"}]});
    assert_eq!(send(A, A, &own), own);
}

#[test]
fn the_cache_key_follows_a_session_between_chat_and_responses() {
    let from_chat = send(
        C,
        R,
        &json!({"model": "gpt", "messages": [{"role": "user", "content": "Hello"}],
            "prompt_cache_key": "session-42"}),
    );
    assert_eq!(from_chat["prompt_cache_key"], "session-42");
    let from_responses = send(
        R,
        C,
        &json!({"model": "gpt", "input": "Hello", "prompt_cache_key": "session-42"}),
    );
    assert_eq!(from_responses["prompt_cache_key"], "session-42");
    let codex = send(R, C, &fixture(R, "codex-developer-resume.json"));
    assert_eq!(
        codex["prompt_cache_key"],
        fixture(R, "codex-developer-resume.json")["prompt_cache_key"]
    );
}

#[test]
fn no_cache_key_is_made_up() {
    let body = send(
        A,
        R,
        &json!({"model": "gpt", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}),
    );
    assert_eq!(body.get("prompt_cache_key"), None);
    let body = send(
        C,
        R,
        &json!({"model": "gpt", "messages": [{"role": "user", "content": "Hello"}]}),
    );
    assert_eq!(body.get("prompt_cache_key"), None);
}

fn claude_turn_as_responses_output(stream: &str) -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(A.as_str())
        .join("streams")
        .join(stream);
    let ctx =
        EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true)).with_provider("anthropic");
    let mut dec = dispatch::stream_decoder(A, &ctx).unwrap();
    let mut events: Vec<StreamEvent> = dec.feed(&std::fs::read(path).unwrap());
    events.extend(dec.finish());
    let app = dispatch::decode_request(
        R,
        RequestKind::Generate,
        br#"{"model":"claude","input":"q","reasoning":{"effort":"medium","summary":"auto"},"stream":true}"#,
        &HeaderList::new(),
    )
    .unwrap();
    let mut enc =
        dispatch::stream_encoder(R, &EncodeCtx::for_request(app).with_created_at(1)).unwrap();
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    let last = parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            SseItem::Event(e) => Some(serde_json::from_str::<Value>(&e.data).unwrap()),
            _ => None,
        })
        .next_back()
        .unwrap();
    last["response"]["output"].as_array().unwrap().clone()
}

#[test]
fn a_tool_loop_with_replayed_claude_thinking_keeps_every_turn_cacheable() {
    let mut input = vec![json!({"role": "user", "content": "Can I see the aurora tonight?"})];
    for item in claude_turn_as_responses_output("thinking-redacted.sse") {
        input.push(item.clone());
        if item["type"] == "function_call" {
            input.push(
                json!({"type": "function_call_output", "call_id": item["call_id"],
                "output": "{\"cloud_cover\":0.1}"}),
            );
        }
    }
    let request = |input: &[Value]| {
        json!({"model": "claude", "input": input, "stream": true,
            "reasoning": {"effort": "medium", "summary": "auto"},
            "include": ["reasoning.encrypted_content"], "store": false,
            "instructions": "You are a weather assistant.",
            "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object", "properties": {}}}]})
    };
    let tool_turn = request(&input);
    input.push(json!({"type": "message", "role": "assistant",
        "content": [{"type": "output_text", "text": "Yes, the sky is clear."}]}));
    input.push(json!({"role": "user", "content": "And tomorrow?"}));
    let next_turn = request(&input);
    for binding in [None, Some(true)] {
        let first = try_send_with(R, A, RequestKind::Generate, &tool_turn, binding).unwrap();
        let second = try_send_with(R, A, RequestKind::Generate, &next_turn, binding).unwrap();
        assert_eq!(first["tools"], second["tools"], "{binding:?}");
        assert_eq!(first["system"], second["system"], "{binding:?}");
        assert_eq!(first["thinking"], second["thinking"], "{binding:?}");
        let earlier = first["messages"].as_array().unwrap();
        let later = second["messages"].as_array().unwrap();
        assert_eq!(earlier[..], later[..earlier.len()], "{binding:?}");
        assert_eq!(breakpoints(&second).len(), 3, "{binding:?}");
    }
}

#[test]
fn an_app_breakpoint_keeps_the_lifetime_the_app_asked_for() {
    let body = send(C, A, &fixture(C, "prompt-cache-breakpoint.json"));
    assert_eq!(
        breakpoints(&body),
        vec![json!({"type": "ephemeral", "ttl": "1h"})]
    );
    assert_eq!(
        last_system_block(&body)["cache_control"],
        json!({"type": "ephemeral", "ttl": "1h"})
    );
}
