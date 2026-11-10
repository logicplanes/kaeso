#![cfg(test)]

use std::path::Path;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HIDDEN_TEXT_PER_KEEPALIVE, dispatch};
use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, DialectId, RawItem, RawJson,
    RequestKind, ResponseDelta, ResponseStart, ResponseStop, StopReason, StreamEvent, StreamMode,
    ToolCallHeader,
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
fn gemini_apps_hear_blank_lines_that_their_sdk_skips() {
    let g = DialectId::Gemini;
    let ping = StreamEvent::Extra(RawItem::new(
        A,
        RawJson::from_string(r#"{"event":"ping","data":{"type":"ping"}}"#.into()).unwrap(),
    ));
    let start = StreamEvent::ResponseStart(ResponseStart::new("x", "m"));
    let outputs = encode_each(g, &[start, ping.clone(), ping]);
    assert_eq!([outputs[1].clone(), outputs[2].clone()].concat(), b"\n\n");
    let mut enc =
        dispatch::stream_encoder(g, &EncodeCtx::new(RequestKind::Generate, mode())).unwrap();
    assert_eq!(enc.keepalive(), b"\n");
    assert!(
        parse_all(b"\n\n").unwrap().is_empty(),
        "a blank line is no event"
    );
    enc.encode(&StreamEvent::ResponseStop(ResponseStop::new()))
        .unwrap();
    assert!(enc.keepalive().is_empty(), "after the end");
    let events = decode(g, b"\n\ndata: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hi\"}]}, \"finishReason\": \"STOP\"}]}\n\n\n");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::Extra(_) | StreamEvent::Error(_))),
        "{events:?}"
    );
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

#[test]
fn a_tool_call_held_until_it_is_complete_keeps_the_app_hearing_from_the_stream() {
    let pieces = 4000;
    let mut events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("x", "m")),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::ToolCall(ToolCallHeader::new("call_1", "write_file")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ToolArguments(r#"{"text":""#.into()),
        )),
    ];
    events.extend(
        (0..pieces).map(|_| {
            StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ToolArguments("ab".into())))
        }),
    );
    events.extend([
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ToolArguments(r#""}"#.into()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::ToolUse), None)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]);
    let text = "ab".repeat(pieces);
    let held = events.iter().map(hidden_text).sum::<usize>();
    let frames = held / HIDDEN_TEXT_PER_KEEPALIVE;
    assert!(frames >= 7);

    let outputs = encode_each(A, &events);
    assert!(longest_silence(&events, &outputs) <= HIDDEN_TEXT_PER_KEEPALIVE);
    let all = outputs.concat();
    assert_eq!(pings(&all), frames);
    let deltas: Vec<serde_json::Value> = parse_all(&all)
        .unwrap()
        .iter()
        .filter_map(|i| match i {
            SseItem::Event(e) if e.event.as_deref() == Some("content_block_delta") => {
                serde_json::from_str(&e.data).ok()
            }
            _ => None,
        })
        .collect();
    assert_eq!(deltas.len(), 1);
    let sent: serde_json::Value =
        serde_json::from_str(deltas[0]["delta"]["partial_json"].as_str().unwrap()).unwrap();
    assert_eq!(sent, serde_json::json!({ "text": text }));

    let g = DialectId::Gemini;
    let outputs = encode_each(g, &events);
    assert!(longest_silence(&events, &outputs) <= HIDDEN_TEXT_PER_KEEPALIVE);
    let blank = outputs.iter().filter(|o| o.as_slice() == b"\n").count();
    assert_eq!(blank, frames);
    let back = decode(g, &outputs.concat());
    let arguments: String = back
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::ToolArguments(a) => Some(a.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let sent: serde_json::Value = serde_json::from_str(&arguments).unwrap();
    assert_eq!(sent, serde_json::json!({ "text": text }));
}
