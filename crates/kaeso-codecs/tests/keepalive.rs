#![cfg(test)]

use std::path::Path;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HIDDEN_TEXT_PER_KEEPALIVE, dispatch};
use kaeso_kpp::{
    Delta, DialectId, RawItem, RawJson, RequestKind, ResponseStart, ResponseStop, StreamEvent,
    StreamMode,
};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn mode() -> StreamMode {
    StreamMode::on(false)
}

fn fixture(dialect: DialectId, name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join("streams")
        .join(format!("{name}.sse"));
    std::fs::read(path).unwrap()
}

fn decode(dialect: DialectId, bytes: &[u8]) -> Vec<StreamEvent> {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, mode());
    let mut dec = dispatch::stream_decoder(dialect, &ctx).unwrap();
    let mut events = dec.feed(bytes);
    events.extend(dec.finish());
    events
}

fn encode_each(target: DialectId, events: &[StreamEvent]) -> Vec<Vec<u8>> {
    let ctx = EncodeCtx::new(RequestKind::Generate, mode()).with_created_at(1_760_000_000);
    let mut enc = dispatch::stream_encoder(target, &ctx).unwrap();
    let mut out: Vec<Vec<u8>> = events.iter().map(|e| enc.encode(e).unwrap()).collect();
    out.push(enc.finish().unwrap());
    out
}

fn hidden_text(event: &StreamEvent) -> usize {
    match event {
        StreamEvent::BlockDelta(d) => match &d.delta {
            Delta::ReasoningText(t) | Delta::Text(t) | Delta::ToolArguments(t) => t.len(),
            Delta::ReasoningSummary(s) => s.text.len(),
            _ => 0,
        },
        _ => 0,
    }
}

fn longest_silence(events: &[StreamEvent], outputs: &[Vec<u8>]) -> usize {
    let (mut run, mut worst) = (0, 0);
    for (event, out) in events.iter().zip(outputs) {
        if out.is_empty() {
            run += hidden_text(event);
            worst = worst.max(run);
        } else {
            run = 0;
        }
    }
    worst
}

fn comments(bytes: &[u8]) -> usize {
    parse_all(bytes)
        .unwrap()
        .iter()
        .filter(|i| matches!(i, SseItem::Comment(c) if c == "keepalive"))
        .count()
}

fn pings(bytes: &[u8]) -> usize {
    parse_all(bytes)
        .unwrap()
        .iter()
        .filter(|i| matches!(i, SseItem::Event(e) if e.event.as_deref() == Some("ping")))
        .count()
}

#[test]
fn claude_thinking_keeps_a_chat_app_hearing_from_the_stream() {
    let events = decode(A, &fixture(A, "thinking-long-tool-loop"));
    let outputs = encode_each(C, &events);
    let first = String::from_utf8(outputs[0].clone()).unwrap();
    assert!(
        first.contains(r#""delta":{"role":"assistant","content":"","refusal":null}"#),
        "{first}"
    );
    let longest_delta = events.iter().map(hidden_text).max().unwrap();
    let silence = longest_silence(&events, &outputs);
    assert!(
        silence < HIDDEN_TEXT_PER_KEEPALIVE + longest_delta,
        "{silence} bytes hidden in a row"
    );
    let all = outputs.concat();
    assert_eq!(comments(&all), 4, "{}", String::from_utf8_lossy(&all));
    let back = decode(C, &all);
    let calls = back
        .iter()
        .filter(|e| {
            matches!(e, StreamEvent::BlockStart(s)
                if matches!(s.block, kaeso_kpp::BlockHeader::ToolCall(_)))
        })
        .count();
    assert_eq!(calls, 1);
    assert!(!back.iter().any(
        |e| matches!(e, StreamEvent::BlockDelta(d) if matches!(&d.delta, Delta::Text(t) if !t.is_empty()))
    ));
}

#[test]
fn foreign_extra_events_become_keepalives_in_every_dialect() {
    let ping = |origin: DialectId| {
        StreamEvent::Extra(RawItem::new(
            origin,
            RawJson::from_string(r#"{"event":"ping","data":{"type":"ping"}}"#.into()).unwrap(),
        ))
    };
    let start = StreamEvent::ResponseStart(ResponseStart::new("x", "m"));
    for (target, source) in [(C, A), (R, A), (A, R)] {
        let outputs = encode_each(target, &[start.clone(), ping(source), ping(source)]);
        let tail = [outputs[1].clone(), outputs[2].clone()].concat();
        let frames = if target == A {
            pings(&tail)
        } else {
            comments(&tail)
        };
        assert_eq!(frames, 2, "{target}: {}", String::from_utf8_lossy(&tail));
    }
}

#[test]
fn every_encoder_gives_the_gateway_a_keepalive_frame_until_the_stream_ends() {
    for target in [A, C, R] {
        let ctx = EncodeCtx::new(RequestKind::Generate, mode());
        let mut enc = dispatch::stream_encoder(target, &ctx).unwrap();
        let before = enc.keepalive();
        let frames = if target == A {
            pings(&before)
        } else {
            comments(&before)
        };
        assert_eq!(frames, 1, "{target}: {}", String::from_utf8_lossy(&before));
        let items = parse_all(&before).unwrap();
        assert_eq!(items.len(), 1);
        enc.encode(&StreamEvent::ResponseStart(ResponseStart::new("x", "m")))
            .unwrap();
        assert!(!enc.keepalive().is_empty(), "{target}: mid-stream");
        enc.encode(&StreamEvent::ResponseStop(ResponseStop::new()))
            .unwrap();
        enc.finish().unwrap();
        assert!(enc.keepalive().is_empty(), "{target}: after the end");
    }
}

#[test]
fn claude_thinking_streams_to_a_responses_app_as_reasoning_summary_text() {
    let events = decode(A, &fixture(A, "thinking-long-tool-loop"));
    let outputs = encode_each(R, &events);
    assert_eq!(longest_silence(&events, &outputs), 0);
    let all = String::from_utf8(outputs.concat()).unwrap();
    assert_eq!(
        all.matches("event: response.reasoning_summary_text.delta")
            .count(),
        40
    );
    assert_eq!(comments(all.as_bytes()), 2);
}
