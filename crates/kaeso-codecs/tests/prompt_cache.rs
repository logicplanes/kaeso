#![cfg(test)]

use std::path::Path;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind, StreamEvent, StreamMode};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture_path(dialect: DialectId, folder: &str, name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(folder)
        .join(name)
}

fn fixture(dialect: DialectId, name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(fixture_path(dialect, "requests", name)).unwrap())
        .unwrap()
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
                    if !inner.is_null() {
                        found.push(inner.clone());
                    }
                } else {
                    collect(inner, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect(i, found)),
        _ => {}
    }
}

fn without_cache_control(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| *key != "cache_control")
                .map(|(key, inner)| (key.clone(), without_cache_control(inner)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_cache_control).collect()),
        other => other.clone(),
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

fn last_message_block(body: &Value) -> &Value {
    let messages = body["messages"].as_array().unwrap();
    messages.last().unwrap()["content"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
}

fn assert_same_prefix(first: &Value, second: &Value, label: &str) {
    assert_eq!(first["tools"], second["tools"], "{label}");
    assert_eq!(first["system"], second["system"], "{label}");
    assert_eq!(first["thinking"], second["thinking"], "{label}");
    let earlier = without_cache_control(&first["messages"]);
    let later = without_cache_control(&second["messages"]);
    let (earlier, later) = (earlier.as_array().unwrap(), later.as_array().unwrap());
    assert!(later.len() > earlier.len(), "{label}");
    assert_eq!(earlier[..], later[..earlier.len()], "{label}");
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
    assert_eq!(last_function_tool(&body)["cache_control"], ephemeral);
    assert_eq!(last_system_block(&body)["cache_control"], ephemeral);
    assert_eq!(last_message_block(&body)["cache_control"], ephemeral);
    assert_eq!(breakpoints(&body).len(), 3);
}

#[test]
fn breakpoints_sit_on_blocks_so_anthropic_compatible_providers_take_them() {
    for name in [
        "codex-developer-resume.json",
        "codex-model-switch.json",
        "codex-namespace-tools.json",
        "codex-web-search-resume.json",
    ] {
        let body = send(R, A, &fixture(R, name));
        assert_eq!(body.get("cache_control"), None, "{name}");
        assert_eq!(
            last_message_block(&body)["cache_control"],
            json!({"type": "ephemeral"}),
            "{name}"
        );
        assert!(breakpoints(&body).len() <= 4, "{name}");
    }
}

#[test]
fn a_codex_turn_repeats_the_previous_turn() {
    let turn = fixture(R, "codex-developer-resume.json");
    let first = send(R, A, &turn);
    let second = send(R, A, &codex_next_turn(&turn));
    assert_same_prefix(&first, &second, "codex");
    assert_eq!(
        last_message_block(&second)["cache_control"],
        json!({"type": "ephemeral"})
    );
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
    assert_eq!(body["tools"][0].get("cache_control"), None);
    assert_eq!(body["tools"][1]["cache_control"], ephemeral);
    assert_eq!(last_system_block(&body)["cache_control"], ephemeral);
    assert_eq!(last_message_block(&body)["cache_control"], ephemeral);
}

#[test]
fn a_tool_loop_caches_up_to_the_latest_tool_result() {
    let body = send(
        R,
        A,
        &json!({"model": "claude", "input": [
            {"role": "user", "content": "Weather in Oslo?"},
            {"type": "function_call", "call_id": "toolu_01", "name": "get_weather", "arguments": "{\"city\":\"Oslo\"}"},
            {"type": "function_call_output", "call_id": "toolu_01", "output": "rain"}],
            "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object", "properties": {}}}]}),
    );
    let last = last_message_block(&body);
    assert_eq!(last["type"], "tool_result");
    assert_eq!(last["cache_control"], json!({"type": "ephemeral"}));
}

#[test]
fn a_request_without_tools_or_system_prompt_still_caches_the_conversation() {
    let body = send(R, A, &json!({"model": "claude", "input": "Hello"}));
    assert_eq!(
        last_message_block(&body)["cache_control"],
        json!({"type": "ephemeral"})
    );
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
    assert_eq!(breakpoints(&body), vec![json!({"type": "ephemeral"})]);
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn no_more_breakpoints_reach_claude_than_it_accepts() {
    let marked = |text: &str| json!({"type": "input_text", "text": text, "prompt_cache_breakpoint": {"mode": "explicit"}});
    let body = send(
        R,
        A,
        &json!({"model": "claude", "input": [{"type": "message", "role": "user", "content": [
            marked("one"), marked("two"), marked("three"), marked("four"), marked("five"), marked("six")]}]}),
    );
    let content = body["messages"][0]["content"].as_array().unwrap();
    let kept: Vec<bool> = content
        .iter()
        .map(|b| b.get("cache_control").is_some())
        .collect();
    assert_eq!(kept, [false, false, true, true, true, true]);
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

fn decode_anthropic_stream(bytes: &[u8]) -> Vec<StreamEvent> {
    let ctx =
        EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true)).with_provider("anthropic");
    let mut dec = dispatch::stream_decoder(A, &ctx).unwrap();
    let mut events = dec.feed(bytes);
    events.extend(dec.finish());
    events
}

fn app_stream(dialect: DialectId, request: &[u8], events: &[StreamEvent]) -> Vec<Value> {
    let app = dispatch::decode_request(dialect, RequestKind::Generate, request, &HeaderList::new())
        .unwrap();
    let mut enc =
        dispatch::stream_encoder(dialect, &EncodeCtx::for_request(app).with_created_at(1)).unwrap();
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            SseItem::Event(e) if e.data != "[DONE]" => {
                Some(serde_json::from_str::<Value>(&e.data).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn claude_turn_as_responses_output(stream: &str) -> Vec<Value> {
    let events =
        decode_anthropic_stream(&std::fs::read(fixture_path(A, "streams", stream)).unwrap());
    let responses = app_stream(
        R,
        br#"{"model":"claude","input":"q","reasoning":{"effort":"medium","summary":"auto"},"stream":true}"#,
        &events,
    );
    responses.last().unwrap()["response"]["output"]
        .as_array()
        .unwrap()
        .clone()
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
        assert_same_prefix(&first, &second, &format!("{binding:?}"));
        assert_eq!(breakpoints(&second).len(), 3, "{binding:?}");
    }
}

#[test]
fn openai_apps_see_what_claude_read_from_the_cache() {
    let recorded = String::from_utf8(
        std::fs::read(fixture_path(
            A,
            "streams",
            "recorded-thinking-display-omitted.sse",
        ))
        .unwrap(),
    )
    .unwrap();
    let cached = recorded
        .replace(
            r#""cache_creation_input_tokens":0,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_5m_input_tokens":0,"#,
            r#""cache_creation_input_tokens":300,"cache_read_input_tokens":5000,"cache_creation":{"ephemeral_5m_input_tokens":300,"#,
        )
        .replace(
            r#""cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens""#,
            r#""cache_creation_input_tokens":300,"cache_read_input_tokens":5000,"output_tokens""#,
        );
    assert_ne!(cached, recorded);
    let events = decode_anthropic_stream(cached.as_bytes());

    let responses = app_stream(
        R,
        br#"{"model":"claude","input":"q","stream":true}"#,
        &events,
    );
    let usage = &responses.last().unwrap()["response"]["usage"];
    assert_eq!(usage["input_tokens"], 5317);
    assert_eq!(usage["input_tokens_details"]["cached_tokens"], 5000);

    let chat = app_stream(
        C,
        br#"{"model":"claude","messages":[{"role":"user","content":"q"}],"stream":true,"stream_options":{"include_usage":true}}"#,
        &events,
    );
    let usage = chat
        .iter()
        .rev()
        .find_map(|c| c.get("usage").filter(|u| !u.is_null()))
        .unwrap();
    assert_eq!(usage["prompt_tokens"], 5317);
    assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], 5000);
}

#[test]
fn hints_on_empty_text_neither_reach_claude_nor_count() {
    let marked = |text: &str| json!({"type": "input_text", "text": text, "prompt_cache_breakpoint": {"mode": "explicit"}});
    let body = send(
        R,
        A,
        &json!({"model": "claude", "input": [{"type": "message", "role": "user", "content": [
            marked(""), marked("one"), marked(""), marked("two"), marked("three"), marked("four"), marked("five")]}]}),
    );
    let content = body["messages"][0]["content"].as_array().unwrap();
    let sent: Vec<(&str, bool)> = content
        .iter()
        .map(|b| {
            (
                b["text"].as_str().unwrap(),
                b.get("cache_control").is_some(),
            )
        })
        .collect();
    assert_eq!(
        sent,
        [
            ("one", false),
            ("two", true),
            ("three", true),
            ("four", true),
            ("five", true)
        ]
    );

    let body = send(
        R,
        A,
        &json!({"model": "claude", "input": [{"type": "message", "role": "user", "content": [
            marked(""), {"type": "input_text", "text": "Hello"}]}]}),
    );
    assert_eq!(
        body["messages"][0]["content"],
        json!([{"type": "text", "text": "Hello", "cache_control": {"type": "ephemeral"}}])
    );
}

fn reported_breakpoints(from: DialectId, to: DialectId, body: &Value) -> (Value, Option<usize>) {
    let mut req = dispatch::decode_request(
        from,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(64_000);
    }
    let mut ctx = EgressCtx::new(&req).with_provider("anthropic");
    let enc = dispatch::encode_request(to, &req, &mut ctx).unwrap();
    (
        serde_json::from_slice(&enc.body).unwrap(),
        ctx.cache_breakpoints,
    )
}

#[test]
fn the_breakpoints_sent_to_claude_are_reported() {
    let cases = [
        (R, fixture(R, "codex-developer-resume.json")),
        (C, fixture(C, "prompt-cache-breakpoint.json")),
        (R, json!({"model": "claude", "input": "Hello"})),
        (
            R,
            json!({"model": "claude", "input": "Hello",
            "prompt_cache_options": {"mode": "explicit"}}),
        ),
        (A, fixture(A, "system-blocks-cache-ttl.json")),
        (A, fixture(A, "text-simple.json")),
        (A, fixture(A, "explicit-nulls.json")),
        (A, fixture(A, "cache-prewarm.json")),
        (A, fixture(A, "system-string-multi-turn.json")),
        (A, fixture(A, "claude-code-tool-search.json")),
        (A, fixture(A, "claude-code-tool-search-loaded.json")),
        (A, fixture(A, "claude-code-mid-conversation-system.json")),
        (A, fixture(A, "mid-conversation-system-tool-results.json")),
        (A, fixture(A, "web-search-server-tool.json")),
        (A, fixture(A, "web-search-result-replay.json")),
        (A, fixture(A, "document-custom-content.json")),
    ];
    for (from, request) in cases {
        let (body, reported) = reported_breakpoints(from, A, &request);
        assert_eq!(reported, Some(breakpoints(&body).len()), "{request}");
    }
    let (_, none) = reported_breakpoints(A, A, &fixture(A, "text-simple.json"));
    assert_eq!(none, Some(0));

    let marked = |text: &str| json!({"type": "input_text", "text": text, "prompt_cache_breakpoint": {"mode": "explicit"}});
    let (_, capped) = reported_breakpoints(
        R,
        A,
        &json!({"model": "claude", "input": [{"type": "message", "role": "user", "content": [
            marked("one"), marked("two"), marked("three"), marked("four"), marked("five"), marked("six")]}]}),
    );
    assert_eq!(capped, Some(4));
}

#[test]
fn automatic_caching_counts_as_a_breakpoint() {
    let (body, reported) = reported_breakpoints(
        A,
        A,
        &json!({"model": "claude", "max_tokens": 100, "cache_control": {"type": "ephemeral"},
            "messages": [{"role": "user", "content": "Hello"}]}),
    );
    assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
    assert_eq!(reported, Some(1));
}

#[test]
fn breakpoints_on_server_tools_and_passed_through_blocks_count() {
    let marked = json!({"type": "ephemeral"});
    let (_, reported) = reported_breakpoints(
        A,
        A,
        &json!({"model": "claude", "max_tokens": 100,
            "tools": [{"type": "web_search_20250305", "name": "web_search", "cache_control": marked}],
            "messages": [{"role": "user", "content": "Hello"}]}),
    );
    assert_eq!(reported, Some(1));

    let request = json!({"model": "claude", "max_tokens": 100,
    "tools": [
        {"type": "web_search_20250305", "name": "web_search", "cache_control": marked},
        {"type": "bash_20250124", "name": "bash", "cache_control": null}
    ],
    "messages": [
        {"role": "user", "content": [
            {"type": "search_result", "source": "https://docs.example.com", "title": "Docs",
                "content": [{"type": "text", "text": "Kaeso"}], "cache_control": marked},
            {"type": "text", "text": "Summarise this."}
        ]},
        {"role": "assistant", "content": [
            {"type": "tool_use", "id": "toolu_01", "name": "lookup", "input": {}}
        ]},
        {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "toolu_01", "content": [
                {"type": "tool_reference", "tool_name": "lookup", "cache_control": marked}
            ]}
        ]}
    ]});
    let (body, reported) = reported_breakpoints(A, A, &request);
    assert_eq!(breakpoints(&body).len(), 3);
    assert_eq!(reported, Some(3));
}

#[test]
fn formats_that_cache_by_themselves_report_no_breakpoints() {
    for to in [C, R] {
        let (_, reported) =
            reported_breakpoints(A, to, &fixture(A, "system-blocks-cache-ttl.json"));
        assert_eq!(reported, None, "{to:?}");
    }
}
