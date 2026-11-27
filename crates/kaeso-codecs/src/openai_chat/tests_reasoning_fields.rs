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
        "baseten",
        ChatProfile::compatible("Baseten").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "alibaba",
        ChatProfile::compatible("QwenCloud").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "cerebras",
        ChatProfile::compatible("Cerebras").reasoning_fields(&["reasoning"]),
    ),
    (
        "deepinfra",
        ChatProfile::compatible("DeepInfra").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "deepseek",
        ChatProfile::compatible("DeepSeek").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "fireworks-ai",
        ChatProfile::compatible("Fireworks AI").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "groq",
        ChatProfile::compatible("Groq").reasoning_fields(&["reasoning"]),
    ),
    (
        "huggingface",
        ChatProfile::compatible("Hugging Face")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
    ),
    (
        "ionos",
        ChatProfile::compatible("IONOS AI Model Hub")
            .reasoning_fields(&["reasoning", "reasoning_content"]),
    ),
    (
        "moonshotai",
        ChatProfile::compatible("Moonshot").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "nebius",
        ChatProfile::compatible("Nebius Token Factory")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
    ),
    (
        "novita-ai",
        ChatProfile::compatible("Novita AI").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "nvidia",
        ChatProfile::compatible("NVIDIA").reasoning_fields(&["reasoning_content"]),
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
        "ovhcloud",
        ChatProfile::compatible("OVHcloud AI Endpoints").reasoning_fields(&["reasoning"]),
    ),
    (
        "sambanova",
        ChatProfile::compatible("SambaNova").reasoning_fields(&["reasoning"]),
    ),
    (
        "scaleway",
        ChatProfile::compatible("Scaleway").reasoning_fields(&["reasoning_content", "reasoning"]),
    ),
    (
        "standardcompute",
        ChatProfile::compatible("Standard Compute").reasoning_fields(&["reasoning"]),
    ),
    (
        "siliconflow",
        ChatProfile::compatible("SiliconFlow").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "tencent-tokenhub",
        ChatProfile::compatible("Tencent TokenHub").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "stepfun-ai",
        ChatProfile::compatible("StepFun").reasoning_fields(&["reasoning", "reasoning_content"]),
    ),
    (
        "togetherai",
        ChatProfile::compatible("Together AI")
            .reasoning_fields(&["reasoning_content", "reasoning"]),
    ),
    (
        "upstage",
        ChatProfile::compatible("Upstage").reasoning_fields(&["reasoning"]),
    ),
    (
        "volcengine",
        ChatProfile::compatible("Volcengine Ark")
            .reasoning_fields(&["reasoning_content"])
            .encrypted_reasoning_field("encrypted_content"),
    ),
    (
        "xai",
        ChatProfile::compatible("SpaceX").reasoning_fields(&["reasoning_content"]),
    ),
    (
        "venice",
        ChatProfile::compatible("Venice").reasoning_fields(&["reasoning_content"]),
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
            let expected = crate::test_support::chat_usage_in_its_own_chunk(&bytes);
            assert_sse_eq(&name, &expected, &out);
        }
    }
}

#[test]
fn a_reasoning_field_is_a_reasoning_block_before_the_answer() {
    let cases = [
        ("alibaba", "recorded-thinking", "reasoning_content"),
        ("alibaba", "recorded-reasoning-effort", "reasoning_content"),
        (
            "alibaba",
            "recorded-reasoning-sent-back",
            "reasoning_content",
        ),
        ("baseten", "reasoning-doc", "reasoning_content"),
        ("cerebras", "recorded-basic", "reasoning"),
        ("cerebras", "recorded-reasoning", "reasoning"),
        ("cerebras", "recorded-reasoning-sent-back", "reasoning"),
        ("cerebras", "recorded-system-mid-conversation", "reasoning"),
        ("deepinfra", "greeting-published", "reasoning_content"),
        ("venice", "cached-published", "reasoning_content"),
        (
            "deepseek",
            "recorded-thinking-reasoning-content",
            "reasoning_content",
        ),
        ("groq", "recorded-plain-developer-string", "reasoning"),
        ("groq", "recorded-reasoning-format-parsed", "reasoning"),
        ("huggingface", "provider-suffix", "reasoning"),
        ("ionos", "gpt-oss-reasoning-doc", "reasoning"),
        ("ionos", "qwen-reasoning-doc", "reasoning"),
        ("moonshotai", "recorded-reasoning", "reasoning_content"),
        (
            "moonshotai",
            "recorded-reasoning-effort-low",
            "reasoning_content",
        ),
        ("moonshotai", "recorded-tool-result", "reasoning_content"),
        ("nvidia", "reasoning", "reasoning_content"),
        ("ollama-cloud", "recorded-reasoning-effort", "reasoning"),
        ("ollama-cloud", "recorded-reasoning-object", "reasoning"),
        ("ollama-cloud", "recorded-reasoning-sent-back", "reasoning"),
        ("openrouter", "recorded-free-reasoning", "reasoning"),
        ("ovhcloud", "recorded-greeting", "reasoning"),
        ("siliconflow", "reasoning-doc", "reasoning_content"),
        ("stepfun-ai", "reasoning-effort-schema", "reasoning"),
        ("stepfun-ai", "both-reasoning-fields-schema", "reasoning"),
        ("tencent-tokenhub", "introduce-doc", "reasoning_content"),
        ("tencent-tokenhub", "thinking-doc", "reasoning_content"),
        ("tencent-tokenhub", "json-schema-doc", "reasoning_content"),
        ("togetherai", "recorded-reasoning-reply", "reasoning"),
        (
            "togetherai",
            "recorded-sampling-extras",
            "reasoning_content",
        ),
        ("volcengine", "reasoning-doc", "reasoning_content"),
        (
            "volcengine",
            "audio-encrypted-content-doc",
            "reasoning_content",
        ),
        ("upstage", "greeting-reasoning-doc", "reasoning"),
        ("upstage", "reasoning-medium-doc", "reasoning"),
        ("upstage", "structured-output-doc", "reasoning"),
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
fn standard_compute_routed_reasoning_reaches_anthropic_apps_as_thinking_before_the_answer() {
    let bytes = recorded(
        "streams",
        "sse",
        "standardcompute",
        "recorded-routed-reasoning-stream",
    );
    let events = stream("standardcompute", &bytes);
    let got = shape(&events);
    assert_eq!(got.reasoning, streamed_field(&bytes, "reasoning"));
    assert!(!got.reasoning.is_empty());
    assert_eq!(got.order[..2], ["reasoning 0", "stop 0"]);
    let anthropic =
        String::from_utf8(encode_stream(DialectId::AnthropicMessages, &events)).unwrap();
    assert!(
        anthropic.contains(r#""type":"thinking_delta""#),
        "{anthropic}"
    );
    assert!(anthropic.contains("No, 91 is"), "{anthropic}");
    let resp = reply(
        "standardcompute",
        &recorded(
            "responses",
            "json",
            "standardcompute",
            "recorded-json-schema",
        ),
    );
    assert!(
        matches!(&resp.blocks[0], Block::Reasoning(_)),
        "{:?}",
        resp.blocks
    );
    assert_eq!(
        encode_reply(DialectId::AnthropicMessages, &resp)["content"][0]["type"],
        "thinking"
    );
}

#[test]
fn cerebras_reasoning_comes_before_its_tool_call_in_replies_and_streams() {
    let resp = reply(
        "cerebras",
        &recorded("responses", "json", "cerebras", "recorded-tools"),
    );
    assert!(
        matches!(&resp.blocks[0], Block::Reasoning(r) if r.text == ["We need to call get_weather."]),
        "{:?}",
        resp.blocks
    );
    assert!(
        matches!(&resp.blocks[1], Block::ToolCall(c) if c.name == "get_weather"),
        "{:?}",
        resp.blocks
    );
    let bytes = recorded("streams", "sse", "cerebras", "recorded-stream-tools");
    let got = shape(&stream("cerebras", &bytes));
    assert_eq!(got.reasoning, streamed_field(&bytes, "reasoning"));
    assert_eq!(got.order[..2], ["reasoning 0", "stop 0"]);
    assert_eq!(got.order[2], "tool call 1");
}

#[test]
fn ovhcloud_reasoning_comes_before_its_tool_call_and_a_reply_cut_while_thinking_is_reasoning_only()
{
    let resp = reply(
        "ovhcloud",
        &recorded("responses", "json", "ovhcloud", "recorded-tools"),
    );
    assert!(
        matches!(&resp.blocks[0], Block::Reasoning(r) if r.text == ["We need to call function."]),
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
    let resp = reply(
        "ovhcloud",
        &recorded("responses", "json", "ovhcloud", "recorded-thinking-length"),
    );
    assert!(
        matches!(&resp.blocks[..], [Block::Reasoning(_)]),
        "{:?}",
        resp.blocks
    );
    let anthropic = encode_reply(DialectId::AnthropicMessages, &resp);
    assert_eq!(anthropic["content"][0]["type"], "thinking");
    assert_eq!(anthropic["stop_reason"], "max_tokens");
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
fn moonshot_reasoning_comes_before_its_tool_call_and_thinking_off_opens_no_reasoning() {
    let resp = reply(
        "moonshotai",
        &recorded(
            "responses",
            "json",
            "moonshotai",
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
        "moonshotai",
        "recorded-reasoning-tools-stream",
    );
    let got = shape(&stream("moonshotai", &bytes));
    assert_eq!(got.reasoning, streamed_field(&bytes, "reasoning_content"));
    assert_eq!(got.order[..2], ["reasoning 0", "stop 0"]);
    assert!(
        got.order.iter().any(|o| o.starts_with("tool call")),
        "{:?}",
        got.order
    );

    for name in ["recorded-thinking-disabled", "recorded-image"] {
        let resp = reply(
            "moonshotai",
            &recorded("responses", "json", "moonshotai", name),
        );
        assert!(
            !resp.blocks.iter().any(|b| matches!(b, Block::Reasoning(_))),
            "{name}: {:?}",
            resp.blocks
        );
    }
    let bytes = recorded(
        "streams",
        "sse",
        "moonshotai",
        "recorded-stream-thinking-off",
    );
    let got = shape(&stream("moonshotai", &bytes));
    assert!(got.reasoning.is_empty(), "{:?}", got.order);
    assert_eq!(got.text, streamed_field(&bytes, "content"));
}

#[test]
fn alibaba_reasoning_comes_before_its_tool_call() {
    let resp = reply(
        "alibaba",
        &recorded("responses", "json", "alibaba", "recorded-thinking-tools"),
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
            "alibaba",
            "recorded-thinking-stream",
            "reasoning_content",
            answered,
        ),
        ("cerebras", "recorded-stream", "reasoning", answered),
        (
            "cerebras",
            "recorded-stream-json-object",
            "reasoning",
            answered,
        ),
        (
            "cerebras",
            "recorded-stream-reasoning",
            "reasoning",
            answered,
        ),
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
            "huggingface",
            "reasoning-stream",
            "reasoning_content",
            answered,
        ),
        (
            "moonshotai",
            "recorded-reasoning-stream",
            "reasoning_content",
            answered,
        ),
        ("nvidia", "reasoning-stream", "reasoning_content", answered),
        (
            "ollama-cloud",
            "recorded-reasoning-stream",
            "reasoning",
            answered,
        ),
        (
            "ovhcloud",
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
            "siliconflow",
            "reasoning-stream-doc",
            "reasoning_content",
            answered,
        ),
        (
            "stepfun-ai",
            "reasoning-stream-schema",
            "reasoning",
            answered,
        ),
        (
            "togetherai",
            "recorded-reasoning-stream",
            "reasoning",
            &["reasoning 0", "stop 0"],
        ),
        ("upstage", "reasoning-stream-doc", "reasoning", answered),
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

#[test]
fn volcengine_encrypted_reasoning_reaches_chat_apps_as_written_and_goes_back_from_anthropic_apps() {
    let body = recorded(
        "responses",
        "json",
        "volcengine",
        "tools-encrypted-content-doc",
    );
    let encrypted = message(&body)["encrypted_content"].clone();
    let resp = reply("volcengine", &body);
    let Block::Reasoning(thought) = &resp.blocks[0] else {
        panic!("{:?}", resp.blocks)
    };
    assert_eq!(thought.opaque.len(), 1, "{thought:?}");
    assert_eq!(Some(thought.opaque[0].data.as_str()), encrypted.as_str());

    let chat = encode_reply(DialectId::OpenAiChat, &resp);
    assert_eq!(
        chat["choices"][0]["message"]["encrypted_content"],
        encrypted
    );

    let anthropic = encode_reply(DialectId::AnthropicMessages, &resp);
    assert!(
        !anthropic.to_string().contains(encrypted.as_str().unwrap()),
        "only inside Kaeso's signature: {anthropic}"
    );
    let signature = anthropic["content"][0]["signature"].as_str().unwrap();
    let carried = crate::continuity::unwrap_reasoning(signature).unwrap();
    let call = resp
        .blocks
        .iter()
        .find(|b| matches!(b, Block::ToolCall(_)))
        .unwrap()
        .clone();
    let mut req = kaeso_kpp::GenerateRequest::new("doubao-seed-2-0-lite-260215");
    req.extras.origin = Some(kaeso_kpp::Origin::new(DialectId::AnthropicMessages));
    req.messages = vec![
        kaeso_kpp::Message::user("今天北京天气怎么样"),
        kaeso_kpp::Message::new(
            kaeso_kpp::Role::Assistant,
            vec![Block::Reasoning(carried), call],
        ),
    ];
    let req = kaeso_kpp::Request::Generate(req);
    let mut ctx = EgressCtx::new(&req).with_chat_profile(profile("volcengine"));
    let sent: Value =
        serde_json::from_slice(&OpenAiChat::encode_request(&req, &mut ctx).unwrap().body).unwrap();
    let assistant = &sent["messages"][1];
    assert_eq!(assistant["encrypted_content"], encrypted, "{assistant}");
    assert_eq!(
        assistant["reasoning_content"],
        message(&body)["reasoning_content"]
    );

    let mut ctx = EgressCtx::new(&req).with_chat_profile(profile("deepseek"));
    let other: Value =
        serde_json::from_slice(&OpenAiChat::encode_request(&req, &mut ctx).unwrap().body).unwrap();
    assert!(
        other["messages"][1].get("encrypted_content").is_none(),
        "{other}"
    );
}

#[test]
fn volcengine_streams_its_encrypted_reasoning_into_the_open_reasoning_block() {
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"\",\"reasoning_content\":\"Think.\",\"role\":\"assistant\"},\"index\":0}],\"created\":1,\"id\":\"0217\",\"model\":\"doubao-seed-2-0-lite-260428\",\"service_tier\":\"default\",\"object\":\"chat.completion.chunk\",\"usage\":null}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"\",\"reasoning_content\":\"\",\"encrypted_content\":\"djF+abc\",\"role\":\"assistant\"},\"index\":0}],\"created\":1,\"id\":\"0217\",\"model\":\"doubao-seed-2-0-lite-260428\",\"service_tier\":\"default\",\"object\":\"chat.completion.chunk\",\"usage\":null}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\",\"role\":\"assistant\"},\"finish_reason\":\"stop\",\"index\":0}],\"created\":1,\"id\":\"0217\",\"model\":\"doubao-seed-2-0-lite-260428\",\"service_tier\":\"default\",\"object\":\"chat.completion.chunk\",\"usage\":null}\n\n",
        "data: [DONE]\n\n",
    );
    let events = stream("volcengine", sse.as_bytes());
    let opaque: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::Opaque(o) => Some(o.data.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(opaque, ["djF+abc"]);
    let chat = String::from_utf8(encode_stream(DialectId::OpenAiChat, &events)).unwrap();
    assert_eq!(chat.matches("djF+abc").count(), 1, "{chat}");
    let anthropic =
        String::from_utf8(encode_stream(DialectId::AnthropicMessages, &events)).unwrap();
    assert!(!anthropic.contains("djF+abc"), "{anthropic}");
    let signature = anthropic
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .find_map(|v| v["delta"]["signature"].as_str().map(str::to_owned))
        .unwrap();
    let carried = crate::continuity::unwrap_reasoning(&signature).unwrap();
    assert_eq!(carried.opaque[0].data, "djF+abc");
}
