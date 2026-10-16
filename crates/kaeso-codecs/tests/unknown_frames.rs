#![cfg(test)]

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, dispatch};
use kaeso_kpp::{DialectId, RequestKind, StreamEvent, StreamMode};

fn live() -> StreamMode {
    StreamMode::on(true)
}

fn round_trip(dialect: DialectId, sse: &str) -> (Vec<StreamEvent>, Vec<u8>) {
    let mut decoder =
        dispatch::stream_decoder(dialect, &EgressCtx::for_kind(RequestKind::Generate, live()))
            .unwrap();
    let events = decoder.feed(sse.as_bytes());
    let mut encoder =
        dispatch::stream_encoder(dialect, &EncodeCtx::new(RequestKind::Generate, live())).unwrap();
    let mut out = Vec::new();
    for event in &events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    (events, out)
}

fn custom_items(bytes: &[u8]) -> Vec<SseItem> {
    parse_all(bytes)
        .unwrap()
        .into_iter()
        .filter(|item| match item {
            SseItem::Event(e) => e.event.as_deref() == Some("custom"),
            SseItem::Comment(c) => c == "note",
            _ => false,
        })
        .collect()
}

fn assert_kept(dialect: DialectId, sse: &str) {
    let (events, out) = round_trip(dialect, sse);
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{dialect:?}: {events:?}"
    );
    assert_eq!(
        custom_items(&out),
        custom_items(sse.as_bytes()),
        "{dialect:?}: {}",
        String::from_utf8_lossy(&out)
    );
}

const STREAMING: [DialectId; 3] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
];

#[test]
fn unknown_frames_with_a_json_string_keep_their_quotes() {
    for dialect in STREAMING {
        assert_kept(dialect, "event: custom\ndata: \"abc\"\n\n");
    }
}

#[test]
fn unknown_frames_keep_json_framing_and_comments() {
    for dialect in STREAMING {
        assert_kept(
            dialect,
            ": note\n\nevent: custom\nid: 7\nretry: 30\nx-field: y\ndata: {\"a\": [1, 2]}\n\nevent: custom\n\n",
        );
    }
}

#[test]
fn unknown_frames_with_text_keep_it_unquoted() {
    for dialect in [DialectId::AnthropicMessages, DialectId::OpenAiChat] {
        assert_kept(dialect, "event: custom\ndata: abc\ndata: def\n\n");
    }
}
