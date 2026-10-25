use kaeso_kpp::{
    Block, BlockHeader, Delta, DialectId, Reply, Request, RequestKind, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::{ChatProfile, OpenAiChat, ReplyReasoning};
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::dispatch;
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures};

const THINKING: ChatProfile = ChatProfile::compatible("Example Cloud").thinking_parts();
const ANSWER: &str = "No, 91 is not a prime number because it can be divided evenly by 7 and 13.";

fn ctx(stream: bool) -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(stream)).with_chat_profile(THINKING)
}

fn recorded(folder: &str, ext: &str, name: &str) -> Vec<u8> {
    fixtures("openai_chat", folder, ext)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, body)| body)
        .unwrap()
}

fn reply(body: &[u8]) -> Reply {
    OpenAiChat::decode_response(200, &HeaderList::new(), body, &ctx(false)).unwrap()
}

fn stream(bytes: &[u8]) -> Vec<StreamEvent> {
    let mut decoder = OpenAiChat::stream_decoder(&ctx(true));
    let mut events = decoder.feed(bytes);
    events.extend(decoder.finish());
    events
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

fn for_chat_apps(name: &str) -> Option<Vec<u8>> {
    let name = name.strip_prefix("providers/mistral/")?;
    let dir =
        crate::test_support::fixtures_dir().join("openai_chat/providers/mistral/for-chat-apps");
    ["json", "sse"]
        .iter()
        .find_map(|ext| std::fs::read(dir.join(format!("{name}.{ext}"))).ok())
}

fn encode_reply(resp: Reply) -> Value {
    let out = OpenAiChat::encode_response(
        &resp,
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    serde_json::from_slice(&out).unwrap()
}

fn content_reply(content: Value) -> Vec<u8> {
    json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{"index": 0, "finish_reason": "stop",
            "message": {"role": "assistant", "content": content}}]
    })
    .to_string()
    .into_bytes()
}

fn chat_request(messages: Value) -> Request {
    let body = json!({"model": "m", "messages": messages}).to_string();
    OpenAiChat::decode_request(RequestKind::Generate, body.as_bytes(), &HeaderList::new()).unwrap()
}

fn sent(req: &Request, profile: ChatProfile) -> Result<Value, kaeso_kpp::KppError> {
    let mut ctx = EgressCtx::new(req).with_chat_profile(profile);
    OpenAiChat::check(req, &ctx)?;
    let enc = OpenAiChat::encode_request(req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn thinking_chunk(text: &str) -> Value {
    json!({"type": "thinking", "thinking": [{"type": "text", "text": text}]})
}

#[test]
fn the_profile_reads_thinking_parts_only_when_it_says_so() {
    assert_eq!(ChatProfile::OPENAI.reply.reasoning, ReplyReasoning::AsSent);
    assert_eq!(THINKING.reply.reasoning, ReplyReasoning::ThinkingParts);
    let body = recorded(
        "responses",
        "json",
        "providers/mistral/recorded-reasoning-effort-high",
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
    assert!(matches!(plain.blocks[0], Block::Unknown(_)));
}

#[test]
fn a_thinking_part_is_reasoning_and_chat_apps_get_mistral_replies_as_text_and_reasoning_content() {
    for (name, body) in fixtures("openai_chat", "responses", "json") {
        if !name.starts_with("providers/mistral/") {
            continue;
        }
        let out = OpenAiChat::encode_response(
            &reply(&body),
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
        )
        .unwrap();
        let expected = for_chat_apps(&name).unwrap_or(body);
        assert_json_eq(&name, &expected, &out);
    }
    let body = recorded(
        "responses",
        "json",
        "providers/mistral/recorded-reasoning-effort-high",
    );
    let Reply::Generate(resp) = reply(&body) else {
        panic!("not a reply")
    };
    let Block::Reasoning(thought) = &resp.blocks[0] else {
        panic!("{:?}", resp.blocks[0])
    };
    assert!(thought.text[0].starts_with("I need to determine if 91 is a prime number."));
    assert!(matches!(&resp.blocks[1], Block::Text(t) if t.text == ANSWER));

    let anthropic = dispatch::encode_response(
        DialectId::AnthropicMessages,
        &Reply::Generate(resp.clone()),
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let anthropic: Value = serde_json::from_slice(&anthropic).unwrap();
    assert_eq!(anthropic["content"][0]["type"], "thinking");
    assert_eq!(anthropic["content"][0]["thinking"], json!(thought.text[0]));
    let blob = anthropic["content"][0]["signature"].as_str().unwrap();
    assert_eq!(
        crate::continuity::unwrap_reasoning(blob).unwrap().text,
        thought.text
    );
    assert_eq!(
        anthropic["content"][1],
        json!({"type": "text", "text": ANSWER})
    );

    let responses = dispatch::encode_response(
        DialectId::OpenAiResponses,
        &Reply::Generate(resp),
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let responses: Value = serde_json::from_slice(&responses).unwrap();
    assert_eq!(responses["output"][0]["type"], "reasoning");
    assert_eq!(responses["output"][1]["content"][0]["text"], ANSWER);
}

#[test]
fn chat_apps_never_get_mistral_chunks_they_cannot_read_as_an_array() {
    let body = content_reply(json!([
        {"type": "thinking", "thinking": [{"type": "text", "text": "a"}], "signature": "s"},
        {"type": "thinking", "thinking": [
            {"type": "text", "text": "b"}, {"type": "reference", "reference_ids": [1]}]},
        {"type": "reference", "reference_ids": [2]},
        {"type": "text", "text": "c"}
    ]));
    let Reply::Generate(resp) = reply(&body) else {
        panic!("not a reply")
    };
    let Block::Reasoning(signed) = &resp.blocks[0] else {
        panic!("{:?}", resp.blocks[0])
    };
    assert_eq!(signed.text, ["a"]);
    assert_eq!(
        signed
            .extras
            .form_value(DialectId::OpenAiChat, super::response::FORM_THINKING),
        Some(&json!({"signature": "s"}))
    );
    let Block::Reasoning(referring) = &resp.blocks[1] else {
        panic!("{:?}", resp.blocks[1])
    };
    assert_eq!(referring.text, ["b"]);
    assert!(matches!(resp.blocks[2], Block::Unknown(_)));
    let message = encode_reply(Reply::Generate(resp))["choices"][0]["message"].clone();
    assert_eq!(message["content"], "c");
    assert_eq!(message["reasoning_content"], "a\n\nb");

    let only_thinking =
        content_reply(json!([{"type": "thinking", "thinking": [], "closed": true}]));
    let message = encode_reply(reply(&only_thinking))["choices"][0]["message"].clone();
    assert_eq!(message["content"], Value::Null);
    assert!(message.get("reasoning_content").is_none(), "{message}");
}

#[test]
fn stream_chunks_that_carry_nothing_readable_reach_chat_apps_without_an_array() {
    let chunk = |delta: Value, finish: Value| {
        format!(
            "data: {}\n\n",
            json!({"id": "x", "object": "chat.completion.chunk", "created": 1, "model": "m",
                "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
        )
    };
    let call = json!({"index": 0, "id": "call_1", "type": "function",
        "function": {"name": "f", "arguments": "{}"}});
    let sse = [
        chunk(
            json!({"role": "assistant", "content": [thinking_chunk("Plan.")]}),
            Value::Null,
        ),
        chunk(
            json!({"content": [{"type": "thinking", "thinking": [], "closed": true}]}),
            Value::Null,
        ),
        chunk(json!({"content": []}), Value::Null),
        chunk(
            json!({"content": [{"type": "text", "text": ""}]}),
            Value::Null,
        ),
        chunk(
            json!({"content": [{"type": "reference", "reference_ids": [1]}]}),
            Value::Null,
        ),
        chunk(json!({"tool_calls": [call]}), json!("tool_calls")),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat();
    let out = String::from_utf8(encode_stream(
        DialectId::OpenAiChat,
        &stream(sse.as_bytes()),
    ))
    .unwrap();
    let deltas: Vec<Value> = out
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .map(|c| c["choices"][0]["delta"].clone())
        .collect();
    assert!(deltas.iter().all(|d| !d["content"].is_array()), "{out}");
    assert_eq!(deltas[0]["reasoning_content"], "Plan.");
    assert_eq!(deltas[3]["content"], "");
    assert_eq!(deltas[5]["tool_calls"][0]["id"], "call_1");
}

#[test]
fn streamed_thinking_is_reasoning_that_ends_before_the_answer_and_reaches_chat_apps_as_reasoning_content()
 {
    for (name, bytes) in fixtures("openai_chat", "streams", "sse") {
        if !name.starts_with("providers/mistral/") {
            continue;
        }
        let out = encode_stream(DialectId::OpenAiChat, &stream(&bytes));
        let expected = for_chat_apps(&name).unwrap_or_else(|| bytes.clone());
        assert_sse_eq(&name, &expected, &out);
    }
    let bytes = recorded(
        "streams",
        "sse",
        "providers/mistral/recorded-reasoning-effort-high-stream",
    );
    let events = stream(&bytes);
    let mut thinking = String::new();
    let mut answer = String::new();
    let mut order = Vec::new();
    for event in &events {
        match event {
            StreamEvent::BlockStart(s) if matches!(s.block, BlockHeader::Reasoning(_)) => {
                order.push(format!("reasoning start {}", s.index));
            }
            StreamEvent::BlockStart(s) if matches!(s.block, BlockHeader::Text) => {
                order.push(format!("text start {}", s.index));
            }
            StreamEvent::BlockStop(s) => order.push(format!("stop {}", s.index)),
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::ReasoningText(t) => thinking.push_str(t),
                Delta::Text(t) => answer.push_str(t),
                _ => {}
            },
            _ => {}
        }
    }
    assert_eq!(answer, ANSWER);
    assert!(
        thinking.starts_with("I need to determine if 91 is"),
        "{thinking}"
    );
    assert_eq!(
        order,
        ["reasoning start 0", "stop 0", "text start 1", "stop 1"]
    );

    let anthropic =
        String::from_utf8(encode_stream(DialectId::AnthropicMessages, &events)).unwrap();
    assert!(
        anthropic.contains(r#""type":"thinking_delta""#),
        "{anthropic}"
    );
    assert!(
        anthropic.contains(r#""type":"signature_delta""#),
        "{anthropic}"
    );
    let responses = String::from_utf8(encode_stream(DialectId::OpenAiResponses, &events)).unwrap();
    assert!(
        responses.contains("response.reasoning_summary_text.delta"),
        "{responses}"
    );
}

#[test]
fn replayed_reasoning_content_goes_back_to_mistral_as_a_thinking_chunk() {
    let (_, body) = fixtures("openai_chat", "requests", "json")
        .into_iter()
        .find(|(n, _)| n == "grok-cli-reasoning-content")
        .unwrap();
    let req = OpenAiChat::decode_request(RequestKind::Generate, &body, &HeaderList::new()).unwrap();
    let original: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        sent(&req, ChatProfile::OPENAI).unwrap()["messages"],
        original["messages"]
    );

    let mistral = sent(&req, THINKING).unwrap();
    let call = &mistral["messages"][2];
    assert_eq!(
        call["content"],
        json!([thinking_chunk("The user wants echo ok.")])
    );
    assert_eq!(call["tool_calls"], original["messages"][2]["tool_calls"]);
    assert!(call.get("reasoning_content").is_none(), "{call}");
    let answer = &mistral["messages"][4];
    assert_eq!(
        answer["content"],
        json!([thinking_chunk("Answer briefly."), {"type": "text", "text": "ok"}])
    );
    assert!(answer.get("reasoning_content").is_none(), "{answer}");
    assert_eq!(mistral["messages"][3], original["messages"][3]);
    assert_eq!(mistral["messages"][5], original["messages"][5]);

    let mut limited = req.clone();
    if let Request::Generate(g) = &mut limited {
        g.sampling.max_output_tokens = Some(1024);
    }
    for dialect in [DialectId::AnthropicMessages, DialectId::OpenAiResponses] {
        let mut ctx = EgressCtx::new(&limited);
        dispatch::check(dialect, &limited, &ctx).unwrap_or_else(|e| panic!("{dialect}: {e}"));
        let enc = dispatch::encode_request(dialect, &limited, &mut ctx).unwrap();
        let text = String::from_utf8(enc.body).unwrap();
        assert!(!text.contains("Answer briefly."), "{dialect}: {text}");
    }
}

#[test]
fn every_chat_reasoning_field_is_readable_and_goes_back_as_it_came() {
    let details = json!([{"type": "reasoning.text", "text": "Use the tool.", "format": "unknown", "index": 0}]);
    let req = chat_request(json!([
        {"role": "user", "content": "hi"},
        {"role": "assistant", "content": "ok", "reasoning": "Use the tool.", "reasoning_details": details},
        {"role": "assistant", "content": "ok", "reasoning_details": [
            {"type": "reasoning.summary", "summary": "Summed up.", "format": "unknown", "index": 0}]},
        {"role": "assistant", "content": "ok", "reasoning_details": [
            {"type": "reasoning.encrypted", "data": "opaque", "format": "unknown", "index": 0}]},
        {"role": "user", "content": "again"}
    ]));
    let Request::Generate(g) = &req else { panic!() };
    let Block::Reasoning(first) = &g.messages[1].blocks[0] else {
        panic!("{:?}", g.messages[1])
    };
    assert_eq!(first.text, ["Use the tool."]);

    let openai = sent(&req, ChatProfile::OPENAI).unwrap();
    assert_eq!(openai["messages"][1]["reasoning"], "Use the tool.");
    assert_eq!(openai["messages"][1]["reasoning_details"], details);
    assert_eq!(
        openai["messages"][3]["reasoning_details"][0]["data"],
        "opaque"
    );

    let mistral = sent(&req, THINKING).unwrap();
    assert_eq!(
        mistral["messages"][1]["content"],
        json!([thinking_chunk("Use the tool."), {"type": "text", "text": "ok"}])
    );
    assert_eq!(
        mistral["messages"][2]["content"],
        json!([thinking_chunk("Summed up."), {"type": "text", "text": "ok"}])
    );
    assert_eq!(
        mistral["messages"][3],
        json!({"role": "assistant", "content": "ok"}),
        "encrypted reasoning has nothing Mistral can read"
    );
}

#[test]
fn replayed_reasoning_without_readable_text_never_blocks_a_request() {
    let req = chat_request(json!([
        {"role": "user", "content": "hi"},
        {"role": "assistant", "content": "a", "reasoning_content": ""},
        {"role": "assistant", "content": "b", "reasoning_details": [
            {"type": "reasoning.encrypted", "data": "opaque", "format": "unknown", "index": 0}]},
        {"role": "assistant", "content": "c", "reasoning": "r", "reasoning_details": ["x"]},
        {"role": "assistant", "content": "d", "reasoning_content": ["not", "text"]},
        {"role": "user", "content": "again"}
    ]));
    let Request::Generate(g) = &req else { panic!() };
    for m in &g.messages[1..5] {
        assert!(matches!(m.blocks[0], Block::Reasoning(_)), "{m:?}");
        assert!(m.extras.fields.is_empty(), "{m:?}");
    }
    let original = json!([
        {"role": "assistant", "content": "a", "reasoning_content": ""},
        {"role": "assistant", "content": "b", "reasoning_details": [
            {"type": "reasoning.encrypted", "data": "opaque", "format": "unknown", "index": 0}]},
        {"role": "assistant", "content": "c", "reasoning": "r", "reasoning_details": ["x"]},
        {"role": "assistant", "content": "d", "reasoning_content": ["not", "text"]}
    ]);
    let openai = sent(&req, ChatProfile::OPENAI).unwrap();
    assert_eq!(
        openai["messages"].as_array().unwrap()[1..5],
        original.as_array().unwrap()[..]
    );

    let mistral = sent(&req, THINKING).unwrap();
    let contents: Vec<&Value> = (1..5).map(|i| &mistral["messages"][i]["content"]).collect();
    assert_eq!(
        contents,
        [
            &json!("a"),
            &json!("b"),
            &json!([thinking_chunk("r"), {"type": "text", "text": "c"}]),
            &json!("d")
        ]
    );

    let mut limited = req.clone();
    if let Request::Generate(g) = &mut limited {
        g.sampling.max_output_tokens = Some(1024);
    }
    for dialect in [DialectId::AnthropicMessages, DialectId::OpenAiResponses] {
        let ctx = EgressCtx::new(&limited);
        dispatch::check(dialect, &limited, &ctx).unwrap_or_else(|e| panic!("{dialect}: {e}"));
    }
}

#[test]
fn several_thinking_parts_in_one_delta_read_as_a_reply_reads_them() {
    let sse = format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        json!({"id": "x", "object": "chat.completion.chunk", "created": 1, "model": "m",
            "choices": [{"index": 0, "delta": {"role": "assistant",
                "content": [thinking_chunk("a"), thinking_chunk("b")]}, "finish_reason": null}]}),
        json!({"id": "x", "object": "chat.completion.chunk", "created": 1, "model": "m",
            "choices": [{"index": 0, "delta": {"content": "c"}, "finish_reason": "stop"}]})
    );
    let out = String::from_utf8(encode_stream(
        DialectId::OpenAiChat,
        &stream(sse.as_bytes()),
    ))
    .unwrap();
    let thought: String = out
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .filter_map(|c| {
            c["choices"][0]["delta"]["reasoning_content"]
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(thought, "a\n\nb");
    let body = content_reply(
        json!([thinking_chunk("a"), thinking_chunk("b"), {"type": "text", "text": "c"}]),
    );
    assert_eq!(
        encode_reply(reply(&body))["choices"][0]["message"]["reasoning_content"],
        "a\n\nb"
    );
}

#[test]
fn mistral_thinking_an_app_in_another_format_sends_back_returns_as_mistral_sent_it() {
    let body = recorded(
        "responses",
        "json",
        "providers/mistral/recorded-reasoning-effort-high",
    );
    let anthropic = dispatch::encode_response(
        DialectId::AnthropicMessages,
        &reply(&body),
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let content: Value = serde_json::from_slice::<Value>(&anthropic).unwrap()["content"].clone();
    let next = json!({"model": "m", "max_tokens": 256, "messages": [
        {"role": "user", "content": "Is 91 prime?"},
        {"role": "assistant", "content": content},
        {"role": "user", "content": "And 97?"}
    ]});
    let req = dispatch::decode_request(
        DialectId::AnthropicMessages,
        RequestKind::Generate,
        next.to_string().as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let mistral = sent(&req, THINKING).unwrap();
    let mut original: Value = serde_json::from_slice(&body).unwrap();
    let thinking = &mut original["choices"][0]["message"]["content"][0];
    assert_eq!(
        thinking.as_object_mut().unwrap().remove("closed"),
        Some(json!(true))
    );
    assert_eq!(
        mistral["messages"][1]["content"], original["choices"][0]["message"]["content"],
        "`closed` defaults to true"
    );
}
