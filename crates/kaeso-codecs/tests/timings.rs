#![allow(clippy::unwrap_used, clippy::print_stderr)]

use std::sync::Arc;
use std::time::Instant;

use kaeso_codecs::{EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Reply, RequestKind};
use serde_json::json;

fn long_conversation(turns: usize) -> Vec<u8> {
    let messages: Vec<_> = (0..turns)
        .map(|i| {
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            json!({ "role": role, "content": format!("turn {i}: {}", "lorem ipsum ".repeat(80)) })
        })
        .collect();
    serde_json::to_vec(&json!({ "model": "gpt-4o", "messages": messages })).unwrap()
}

fn reply() -> Reply {
    let body = json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "gpt-4o",
        "choices": [{ "index": 0, "finish_reason": "stop",
            "message": { "role": "assistant", "content": "Done." } }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12 }
    });
    let ctx = kaeso_codecs::EgressCtx::for_kind(RequestKind::Generate, kaeso_kpp::StreamMode::Off);
    dispatch::decode_response(
        DialectId::OpenAiChat,
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap()
}

#[test]
#[ignore = "timing, run with --ignored --nocapture"]
fn responses_reply_to_a_long_conversation() {
    for turns in [2, 2_000] {
        let body = long_conversation(turns);
        let req = dispatch::decode_request(
            DialectId::OpenAiChat,
            RequestKind::Generate,
            &body,
            &HeaderList::new(),
        )
        .unwrap();
        let ctx = EncodeCtx::for_request(Arc::new(req));
        let reply = reply();
        let rounds = 20;
        let started = Instant::now();
        for _ in 0..rounds {
            dispatch::encode_response(DialectId::OpenAiResponses, &reply, &ctx).unwrap();
        }
        let each = started.elapsed() / rounds;
        eprintln!(
            "a Responses reply to a {} KB request: {each:?} each",
            body.len() / 1024
        );
    }
}

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(path),
    )
    .unwrap()
}

#[test]
#[ignore = "timing, run with --ignored --nocapture"]
fn streamed_events_through_each_encoder() {
    let sources = [
        (
            DialectId::AnthropicMessages,
            "anthropic_messages/streams/text-simple.sse",
        ),
        (
            DialectId::OpenAiChat,
            "openai_chat/streams/stream-include-usage.sse",
        ),
        (
            DialectId::OpenAiResponses,
            "openai_responses/streams/text-basic-stream.sse",
        ),
    ];
    for (from, path) in sources {
        let ectx = kaeso_codecs::EgressCtx::for_kind(
            RequestKind::Generate,
            kaeso_kpp::StreamMode::on(true),
        )
        .with_provider(if from == DialectId::AnthropicMessages {
            "anthropic"
        } else {
            "openai"
        });
        let mut decoder = dispatch::stream_decoder(from, &ectx).unwrap();
        let mut events = decoder.feed(&fixture(path));
        events.extend(decoder.finish());
        let deltas: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, kaeso_kpp::StreamEvent::BlockDelta(_)))
            .cloned()
            .collect();
        let (head, tail): (Vec<_>, Vec<_>) = events.iter().cloned().partition(|e| {
            matches!(
                e,
                kaeso_kpp::StreamEvent::ResponseStart(_) | kaeso_kpp::StreamEvent::BlockStart(_)
            )
        });
        let tail: Vec<_> = tail
            .into_iter()
            .filter(|e| !matches!(e, kaeso_kpp::StreamEvent::BlockDelta(_)))
            .collect();
        for to in [
            DialectId::AnthropicMessages,
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
        ] {
            let ctx = EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::on(true));
            let mut encoder = dispatch::stream_encoder(to, &ctx).unwrap();
            for e in &head {
                encoder.encode(e).unwrap();
            }
            let rounds = 20_000 / deltas.len().max(1);
            let started = Instant::now();
            let mut bytes = 0;
            for _ in 0..rounds {
                for e in &deltas {
                    bytes += encoder.encode(e).unwrap().len();
                }
            }
            let each = started.elapsed() / u32::try_from(rounds * deltas.len()).unwrap();
            for e in &tail {
                encoder.encode(e).unwrap();
            }
            eprintln!("{from} -> {to}: {each:?} per text delta ({bytes} bytes)");
        }
    }
}
