use kaeso_kpp::{
    Block, BlockHeader, Delta, DialectId, Reply, RequestKind, Response, StreamEvent, StreamMode,
};
use serde_json::Value;

use super::{ChatProfile, OpenAiChat};
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::dispatch;
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures};

const PROFILES: &[(&str, ChatProfile)] = &[
    (
        "deepseek",
        ChatProfile::compatible("DeepSeek").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "groq",
        ChatProfile::compatible("Groq").reasoning_fields(&["reasoning"]),
    ),
    (
        "ollama-cloud",
        ChatProfile::compatible("Ollama Cloud").reasoning_fields(&["reasoning"]),
    ),
    (
        "openrouter",
        ChatProfile::compatible("OpenRouter").reasoning_fields(&["reasoning"]),
    ),
    (
        "togetherai",
        ChatProfile::compatible("Together AI")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
    ),
    (
        "xai",
        ChatProfile::compatible("SpaceX").reasoning_fields(&["reasoning_content"]),
    ),
];

fn profile(provider: &str) -> ChatProfile {
    PROFILES
        .iter()
        .find(|(id, _)| *id == provider)
        .map(|(_, p)| *p)
        .unwrap()
}

fn ctx(provider: &str, stream: bool) -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(stream))
        .with_chat_profile(profile(provider))
}

fn recorded(folder: &str, ext: &str, provider: &str, name: &str) -> Vec<u8> {
    let wanted = format!("providers/{provider}/{name}");
    fixtures("openai_chat", folder, ext)
        .into_iter()
        .find(|(n, _)| *n == wanted)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("no fixture {folder}/{wanted}"))
}

fn reply(provider: &str, body: &[u8]) -> Response {
    match OpenAiChat::decode_response(200, &HeaderList::new(), body, &ctx(provider, false)) {
        Ok(Reply::Generate(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn stream(provider: &str, bytes: &[u8]) -> Vec<StreamEvent> {
    let mut decoder = OpenAiChat::stream_decoder(&ctx(provider, true));
    let mut events = decoder.feed(bytes);
    events.extend(decoder.finish());
    events
}

fn encode_reply(dialect: DialectId, resp: &Response) -> Value {
    let ectx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    let out = dispatch::encode_response(dialect, &Reply::Generate(resp.clone()), &ectx).unwrap();
    serde_json::from_slice(&out).unwrap()
}

fn encode_stream(dialect: DialectId, events: &[StreamEvent]) -> Vec<u8> {
    let ectx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
    let mut encoder = dispatch::stream_encoder(dialect, &ectx).unwrap();
    let mut out = Vec::new();
    for event in events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    out
}

fn message(body: &[u8]) -> Value {
    let body: Value = serde_json::from_slice(body).unwrap();
    body["choices"][0]["message"].clone()
}

fn streamed_field(bytes: &[u8], field: &str) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .filter_map(|c| c["choices"][0]["delta"][field].as_str().map(str::to_owned))
        .collect()
}

struct Shape {
    order: Vec<String>,
    reasoning: String,
    text: String,
    extras: usize,
}

fn shape(events: &[StreamEvent]) -> Shape {
    let mut shape = Shape {
        order: Vec::new(),
        reasoning: String::new(),
        text: String::new(),
        extras: 0,
    };
    for event in events {
        match event {
            StreamEvent::BlockStart(s) => shape.order.push(match s.block {
                BlockHeader::Reasoning(_) => format!("reasoning {}", s.index),
                BlockHeader::Text => format!("text {}", s.index),
                BlockHeader::ToolCall(_) => format!("tool call {}", s.index),
                _ => format!("other {}", s.index),
            }),
            StreamEvent::BlockStop(s) => shape.order.push(format!("stop {}", s.index)),
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::ReasoningText(t) => shape.reasoning.push_str(t),
                Delta::Text(t) => shape.text.push_str(t),
                _ => {}
            },
            StreamEvent::Extra(_) => shape.extras += 1,
            StreamEvent::Error(e) => panic!("{e:?}"),
            _ => {}
        }
    }
    shape
}

#[test]
fn every_reply_and_stream_of_these_providers_round_trips_under_its_profile() {
    for (provider, _) in PROFILES {
        let prefix = format!("providers/{provider}/");
        for (name, body) in fixtures("openai_chat", "responses", "json") {
            if !name.starts_with(&prefix) {
                continue;
            }
            let out = OpenAiChat::encode_response(
                &Reply::Generate(reply(provider, &body)),
                &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
            )
            .unwrap();
            assert_json_eq(&name, &body, &out);
        }
        for (name, bytes) in fixtures("openai_chat", "streams", "sse") {
            if !name.starts_with(&prefix) {
                continue;
            }
            let out = encode_stream(DialectId::OpenAiChat, &stream(provider, &bytes));
            assert_sse_eq(&name, &bytes, &out);
        }
    }
}

#[test]
fn a_reasoning_field_is_a_reasoning_block_before_the_answer() {
    let cases = [
        (
            "deepseek",
            "recorded-thinking-reasoning-content",
            "reasoning_content",
        ),
        ("groq", "recorded-plain-developer-string", "reasoning"),
        ("groq", "recorded-reasoning-format-parsed", "reasoning"),
        ("ollama-cloud", "recorded-reasoning-effort", "reasoning"),
        ("ollama-cloud", "recorded-reasoning-object", "reasoning"),
        ("ollama-cloud", "recorded-reasoning-sent-back", "reasoning"),
        ("openrouter", "recorded-free-reasoning", "reasoning"),
        ("togetherai", "recorded-reasoning-reply", "reasoning"),
        (
            "togetherai",
            "recorded-sampling-extras",
            "reasoning_content",
        ),
        ("xai", "recorded-reasoning-content", "reasoning_content"),
    ];
    for (provider, name, field) in cases {
        let body = recorded("responses", "json", provider, name);
        let sent = message(&body);
        let resp = reply(provider, &body);
        let Block::Reasoning(thought) = &resp.blocks[0] else {
            panic!("{name}: {:?}", resp.blocks)
        };
        assert_eq!(thought.text, [sent[field].as_str().unwrap()], "{name}");
        assert!(
            matches!(&resp.blocks[1], Block::Text(t) if Some(t.text.as_str()) == sent["content"].as_str()),
            "{name}: {:?}",
            resp.blocks
        );
        assert!(
            !serde_json::to_string(&resp.extras)
                .unwrap()
                .contains(&format!("\"choice.message.{field}\"")),
            "{name}: the reasoning is not an extra as well"
        );

        let anthropic = encode_reply(DialectId::AnthropicMessages, &resp);
        assert_eq!(anthropic["content"][0]["type"], "thinking", "{name}");
        assert_eq!(anthropic["content"][0]["thinking"], sent[field], "{name}");
        let blob = anthropic["content"][0]["signature"].as_str().unwrap();
        assert_eq!(
            crate::continuity::unwrap_reasoning(blob).unwrap().text,
            thought.text
        );
        assert_eq!(anthropic["content"][1]["type"], "text", "{name}");

        let responses = encode_reply(DialectId::OpenAiResponses, &resp);
        assert_eq!(responses["output"][0]["type"], "reasoning", "{name}");
        assert_eq!(
            responses["output"][0]["summary"][0]["text"], sent[field],
            "{name}"
        );
        assert_eq!(responses["output"][1]["type"], "message", "{name}");
    }
}

#[test]
fn reasoning_before_a_tool_call_comes_first() {
    let body = recorded(
        "responses",
        "json",
        "deepseek",
        "recorded-thinking-tools-auto",
    );
    let resp = reply("deepseek", &body);
    assert!(
        matches!(resp.blocks[0], Block::Reasoning(_)),
        "{:?}",
        resp.blocks
    );
    assert!(
        resp.blocks
            .iter()
            .any(|b| matches!(b, Block::ToolCall(c) if c.name == "get_weather")),
        "{:?}",
        resp.blocks
    );
}

#[test]
fn ollama_cloud_reasoning_comes_before_its_tool_call_in_replies_and_streams() {
    let resp = reply(
        "ollama-cloud",
        &recorded(
            "responses",
            "json",
            "ollama-cloud",
            "recorded-reasoning-tools",
        ),
    );
    assert!(
        matches!(resp.blocks[0], Block::Reasoning(_)),
        "{:?}",
        resp.blocks
    );
    assert!(
        resp.blocks
            .iter()
            .any(|b| matches!(b, Block::ToolCall(c) if c.name == "get_weather")),
        "{:?}",
        resp.blocks
    );

    let bytes = recorded(
        "streams",
        "sse",
        "ollama-cloud",
        "recorded-reasoning-tools-stream",
    );
    let got = shape(&stream("ollama-cloud", &bytes));
    assert_eq!(got.reasoning, streamed_field(&bytes, "reasoning"));
    assert_eq!(got.order[..2], ["reasoning 0", "stop 0"]);
    assert!(
        got.order.contains(&"tool call 1".to_owned()),
        "{:?}",
        got.order
    );
}

#[test]
fn without_the_setting_and_with_an_empty_field_the_reasoning_stays_as_sent() {
    let body = recorded(
        "responses",
        "json",
        "deepseek",
        "recorded-thinking-reasoning-content",
    );
    let plain = OpenAiChat::decode_response(
        200,
        &HeaderList::new(),
        &body,
        &EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let Reply::Generate(plain) = plain else {
        panic!("not a reply")
    };
    assert!(
        !plain
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Reasoning(_)))
    );

    let mut empty: Value = serde_json::from_slice(&body).unwrap();
    empty["choices"][0]["message"]["reasoning_content"] = Value::from("");
    let empty = serde_json::to_vec(&empty).unwrap();
    let resp = reply("deepseek", &empty);
    assert!(!resp.blocks.iter().any(|b| matches!(b, Block::Reasoning(_))));
    let out = OpenAiChat::encode_response(
        &Reply::Generate(resp),
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq("empty reasoning", &empty, &out);
}

#[test]
fn streamed_reasoning_fields_are_one_reasoning_block_that_ends_before_the_answer() {
    let answered: &[&str] = &["reasoning 0", "stop 0", "text 1", "stop 1"];
    let cases = [
        (
            "deepseek",
            "recorded-thinking-stream",
            "reasoning_content",
            answered,
        ),
        ("groq", "recorded-stream-doc", "reasoning", answered),
        (
            "groq",
            "recorded-stream-reasoning-parsed",
            "reasoning",
            answered,
        ),
        (
            "ollama-cloud",
            "recorded-reasoning-stream",
            "reasoning",
            answered,
        ),
        (
            "openrouter",
            "recorded-free-reasoning-stream",
            "reasoning",
            answered,
        ),
        (
            "togetherai",
            "recorded-reasoning-stream",
            "reasoning",
            &["reasoning 0", "stop 0"],
        ),
        (
            "xai",
            "recorded-reasoning-content-stream",
            "reasoning_content",
            answered,
        ),
    ];
    for (provider, name, field, order) in cases {
        let bytes = recorded("streams", "sse", provider, name);
        let events = stream(provider, &bytes);
        let got = shape(&events);
        assert_eq!(got.reasoning, streamed_field(&bytes, field), "{name}");
        assert_eq!(got.text, streamed_field(&bytes, "content"), "{name}");
        assert_eq!(got.order, order, "{name}");
        assert!(
            got.extras <= 1,
            "{name}: {} chunks stayed stream extras",
            got.extras
        );

        let anthropic =
            String::from_utf8(encode_stream(DialectId::AnthropicMessages, &events)).unwrap();
        assert!(
            anthropic.contains(r#""type":"thinking_delta""#),
            "{name}: {anthropic}"
        );
        let responses =
            String::from_utf8(encode_stream(DialectId::OpenAiResponses, &events)).unwrap();
        assert!(
            responses.contains("response.reasoning_summary_text.delta"),
            "{name}: {responses}"
        );
    }
}

#[test]
fn streamed_reasoning_before_a_tool_call_ends_when_the_call_starts() {
    let bytes = recorded(
        "streams",
        "sse",
        "deepseek",
        "recorded-thinking-tools-stream",
    );
    let got = shape(&stream("deepseek", &bytes));
    assert_eq!(got.reasoning, streamed_field(&bytes, "reasoning_content"));
    assert_eq!(got.order[..2], ["reasoning 0", "stop 0"]);
    assert_eq!(got.order[2], "tool call 1");
}

#[test]
fn reasoning_an_app_in_another_format_sends_back_returns_in_the_providers_field() {
    let body = serde_json::json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "deepseek-reasoner",
        "choices": [{"index": 0, "finish_reason": "tool_calls", "message": {
            "role": "assistant", "content": "", "reasoning_content": "R1",
            "tool_calls": [{"id": "call_1", "type": "function",
                "function": {"name": "f", "arguments": "{}"}}]}}]
    })
    .to_string();
    let ctx = ctx("deepseek", false).with_provider("deepseek");
    let resp = OpenAiChat::decode_response(200, &HeaderList::new(), body.as_bytes(), &ctx).unwrap();
    let anthropic = encode_reply(
        DialectId::AnthropicMessages,
        match &resp {
            Reply::Generate(r) => r,
            _ => panic!(),
        },
    );
    let next = serde_json::json!({"model": "m", "max_tokens": 64, "messages": [
        {"role": "user", "content": "go"},
        {"role": "assistant", "content": anthropic["content"]},
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_1", "content": "done"}]}
    ]});
    let req = dispatch::decode_request(
        DialectId::AnthropicMessages,
        kaeso_kpp::RequestKind::Generate,
        next.to_string().as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let sent = |profile: ChatProfile| -> Value {
        let mut ctx = EgressCtx::new(&req)
            .with_provider("deepseek")
            .with_chat_profile(profile);
        let enc = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
        serde_json::from_slice(&enc.body).unwrap()
    };
    assert_eq!(
        sent(profile("deepseek"))["messages"][1]["reasoning_content"],
        "R1"
    );
    assert!(
        sent(ChatProfile::OPENAI)["messages"][1]
            .get("reasoning_content")
            .is_none()
    );
}
