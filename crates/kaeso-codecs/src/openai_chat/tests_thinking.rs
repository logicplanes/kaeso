use kaeso_kpp::{
    Block, BlockHeader, Delta, DialectId, Reply, RequestKind, StreamEvent, StreamMode,
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

// Mistral's content arrays reach Chat apps as the string `content` and `reasoning_content` that
// other Chat providers send, because Chat clients read `content` as a string.
fn as_chat_text(message: &mut Value, whole: bool) {
    let Some(parts) = message["content"].as_array() else {
        return;
    };
    let mut text: Option<String> = None;
    let mut thought = String::new();
    for part in parts {
        match part["type"].as_str() {
            Some("text") => text
                .get_or_insert_with(String::new)
                .push_str(part["text"].as_str().unwrap()),
            Some("thinking") => {
                for inner in part["thinking"].as_array().unwrap() {
                    thought.push_str(inner["text"].as_str().unwrap());
                }
            }
            _ => return,
        }
    }
    let object = message.as_object_mut().unwrap();
    match text {
        Some(text) => object.insert("content".to_owned(), Value::from(text)),
        None if whole => object.insert("content".to_owned(), Value::Null),
        None => object.remove("content"),
    };
    if !thought.is_empty() {
        object.insert("reasoning_content".to_owned(), Value::from(thought));
    }
}

fn as_chat_text_stream(bytes: &[u8]) -> Vec<u8> {
    let mut out = String::new();
    for line in std::str::from_utf8(bytes).unwrap().split_inclusive('\n') {
        match line
            .strip_prefix("data: ")
            .and_then(|d| serde_json::from_str::<Value>(d.trim_end()).ok())
        {
            Some(mut chunk) if chunk["choices"][0]["delta"].is_object() => {
                as_chat_text(&mut chunk["choices"][0]["delta"], false);
                out.push_str(&format!("data: {chunk}\n"));
            }
            _ => out.push_str(line),
        }
    }
    out.into_bytes()
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
        let mut expected: Value = serde_json::from_slice(&body).unwrap();
        as_chat_text(&mut expected["choices"][0]["message"], true);
        assert_json_eq(&name, expected.to_string().as_bytes(), &out);
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
fn a_thinking_part_it_cannot_read_whole_stays_as_sent() {
    let body = json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", "content": [
            {"type": "thinking", "thinking": [{"type": "text", "text": "a"}], "signature": "s"},
            {"type": "thinking", "thinking": [{"type": "reference", "reference_ids": [1]}]},
            {"type": "text", "text": "b"}
        ]}}]
    })
    .to_string();
    let Reply::Generate(resp) = reply(body.as_bytes()) else {
        panic!("not a reply")
    };
    assert!(matches!(resp.blocks[0], Block::Unknown(_)));
    assert!(matches!(resp.blocks[1], Block::Unknown(_)));
    let out = OpenAiChat::encode_response(
        &Reply::Generate(resp),
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq("unreadable thinking", body.as_bytes(), &out);
}

#[test]
fn streamed_thinking_is_reasoning_that_ends_before_the_answer_and_reaches_chat_apps_as_reasoning_content()
 {
    for (name, bytes) in fixtures("openai_chat", "streams", "sse") {
        if !name.starts_with("providers/mistral/") {
            continue;
        }
        let out = encode_stream(DialectId::OpenAiChat, &stream(&bytes));
        assert_sse_eq(&name, &as_chat_text_stream(&bytes), &out);
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
    let sent = |profile: ChatProfile| -> Value {
        let mut ctx = EgressCtx::new(&req).with_chat_profile(profile);
        OpenAiChat::check(&req, &ctx).unwrap();
        let enc = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
        serde_json::from_slice(&enc.body).unwrap()
    };
    let original: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(sent(ChatProfile::OPENAI)["messages"], original["messages"]);

    let mistral = sent(THINKING);
    let thinking =
        |text: &str| json!({"type": "thinking", "thinking": [{"type": "text", "text": text}]});
    let call = &mistral["messages"][2];
    assert_eq!(
        call["content"],
        json!([thinking("The user wants echo ok.")])
    );
    assert_eq!(call["tool_calls"], original["messages"][2]["tool_calls"]);
    assert!(call.get("reasoning_content").is_none(), "{call}");
    let answer = &mistral["messages"][4];
    assert_eq!(
        answer["content"],
        json!([thinking("Answer briefly."), {"type": "text", "text": "ok"}])
    );
    assert!(answer.get("reasoning_content").is_none(), "{answer}");
    assert_eq!(mistral["messages"][3], original["messages"][3]);
    assert_eq!(mistral["messages"][5], original["messages"][5]);
}

#[test]
fn reasoning_content_mistral_cannot_take_as_a_chunk_is_refused() {
    let body = json!({
        "model": "m",
        "messages": [
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": "ok", "reasoning_content": ["not", "text"]},
            {"role": "user", "content": "again"}
        ]
    });
    let req = OpenAiChat::decode_request(
        RequestKind::Generate,
        body.to_string().as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req).with_chat_profile(THINKING);
    let err = OpenAiChat::encode_request(&req, &mut ctx).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("messages[1].reasoning_content"));
    assert!(err.message.contains("Example Cloud"), "{}", err.message);
}
