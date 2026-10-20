use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, DialectId, ErrorKind, KppError, Opaque,
    OpaqueKind, Origin, RawItem, RawJson, ReasoningHeader, RequestKind, ResponseDelta,
    ResponseStart, ResponseStop, StopReason, StreamEvent, StreamMode, ToolCallHeader, Usage,
};
use proptest::prelude::*;
use serde_json::{Value, json};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx, EncodeCtx};
use crate::test_support::{assert_sse_eq, fixtures};
use crate::testing::sse_items;

const D: &str = "openai_chat";

fn ectx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true))
}

fn decode_in(bytes: &[u8], sizes: &[usize]) -> Vec<StreamEvent> {
    let mut decoder = OpenAiChat::stream_decoder(&ectx());
    let mut events = Vec::new();
    let mut rest = bytes;
    let mut i = 0;
    while !rest.is_empty() {
        let size = sizes[i % sizes.len()].max(1).min(rest.len());
        let (head, tail) = rest.split_at(size);
        events.extend(decoder.feed(head));
        rest = tail;
        i += 1;
    }
    events.extend(decoder.finish());
    events
}

fn decode_whole(bytes: &[u8]) -> Vec<StreamEvent> {
    decode_in(bytes, &[usize::MAX])
}

fn include_usage(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).contains("\"usage\"")
}

fn encode(events: &[StreamEvent], include_usage: bool) -> Vec<u8> {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(include_usage))
        .with_created_at(1_700_000_000);
    let mut encoder = OpenAiChat::stream_encoder(&ctx);
    let mut out = Vec::new();
    for event in events {
        out.extend(
            encoder
                .encode(event)
                .unwrap_or_else(|e| panic!("{event:?}: {e:?}")),
        );
    }
    out.extend(encoder.finish().unwrap());
    out
}

fn frames(bytes: &[u8]) -> Vec<Value> {
    sse_items(bytes)
        .unwrap()
        .into_iter()
        .map(|item| match item {
            crate::testing::ComparableItem::Event { data, .. } => data.unwrap_or(Value::Null),
            crate::testing::ComparableItem::Comment(c) => Value::String(format!(": {c}")),
        })
        .collect()
}

#[test]
fn every_stream_fixture_round_trips() {
    for (name, bytes) in fixtures(D, "streams", "sse") {
        let events = decode_whole(&bytes);
        let out = encode(&events, include_usage(&bytes));
        assert_sse_eq(&name, &bytes, &out);
    }
}

#[test]
fn chunking_never_changes_the_events() {
    for (name, bytes) in fixtures(D, "streams", "sse") {
        let whole = decode_whole(&bytes);
        for sizes in [&[1][..], &[7], &[64], &[3, 11, 1, 29]] {
            assert_eq!(
                decode_in(&bytes, sizes),
                whole,
                "{name} in pieces of {sizes:?}"
            );
        }
    }
}

proptest! {
    #[test]
    fn random_splits_give_the_same_events(
        which in 0usize..64,
        cuts in proptest::collection::vec(any::<prop::sample::Index>(), 0..12),
    ) {
        let all = fixtures(D, "streams", "sse");
        let (_, bytes) = &all[which % all.len()];
        let whole = decode_whole(bytes);
        let mut points: Vec<usize> = cuts.iter().map(|c| c.index(bytes.len() + 1)).collect();
        points.sort_unstable();
        let mut decoder = OpenAiChat::stream_decoder(&ectx());
        let mut events = Vec::new();
        let mut last = 0;
        for p in points {
            events.extend(decoder.feed(&bytes[last..p]));
            last = p;
        }
        events.extend(decoder.feed(&bytes[last..]));
        events.extend(decoder.finish());
        prop_assert_eq!(events, whole);
    }
}

fn fixture(name: &str) -> Vec<u8> {
    fixtures(D, "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn kinds(events: &[StreamEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            StreamEvent::ResponseStart(_) => "start".to_owned(),
            StreamEvent::BlockStart(b) => format!("block_start {}", b.index),
            StreamEvent::BlockDelta(b) => format!("delta {}", b.index),
            StreamEvent::BlockStop(b) => format!("block_stop {}", b.index),
            StreamEvent::ResponseDelta(_) => "response_delta".to_owned(),
            StreamEvent::ResponseStop(_) => "stop".to_owned(),
            StreamEvent::Error(_) => "error".to_owned(),
            StreamEvent::Extra(_) => "extra".to_owned(),
            _ => "other".to_owned(),
        })
        .collect()
}

#[test]
fn parallel_tool_calls_map_chat_indexes_to_blocks() {
    let events = decode_whole(&fixture("tools-parallel"));
    let mut args: [String; 2] = Default::default();
    let mut ids = Vec::new();
    for event in &events {
        match event {
            StreamEvent::BlockStart(BlockStart {
                index,
                block: BlockHeader::ToolCall(h),
                ..
            }) => {
                ids.push((*index, h.id.clone(), h.name.clone()));
            }
            StreamEvent::BlockDelta(BlockDelta {
                index,
                delta: Delta::ToolArguments(a),
                ..
            }) => {
                args[*index as usize].push_str(a);
            }
            _ => {}
        }
    }
    assert_eq!(
        ids[0],
        (
            0,
            "call_kfCAiUZI9KHN37p4VH3N0E2a".into(),
            "get_weather".into()
        )
    );
    assert_eq!(
        ids[1],
        (
            1,
            "call_kfNMlUq01JIft0lr9lKKbGMM".into(),
            "get_local_time".into()
        )
    );
    assert_eq!(args[0], r#"{"location":"Paris, France","unit":null}"#);
    assert_eq!(args[1], r#"{"timezone":"Asia/Tokyo"}"#);
    let tail = kinds(&events);
    let n = tail.len();
    assert_eq!(
        &tail[n - 5..],
        [
            "block_stop 0",
            "block_stop 1",
            "response_delta",
            "response_delta",
            "stop"
        ]
    );
    let StreamEvent::ResponseDelta(stop) = &events[n - 3] else {
        panic!()
    };
    assert_eq!(stop.stop, Some(StopReason::ToolUse));
    let StreamEvent::ResponseDelta(usage) = &events[n - 2] else {
        panic!()
    };
    assert_eq!(usage.usage.as_ref().unwrap().input_tokens, 171);
}

#[test]
fn text_refusal_and_usage_are_typed() {
    let events = decode_whole(&fixture("stream-include-usage"));
    assert!(
        matches!(&events[0], StreamEvent::ResponseStart(s) if s.id == "chatcmpl-UL5s8yb6jcdxEOaPlRU1zPeJF5kBk")
    );
    assert!(matches!(
        &events[1],
        StreamEvent::BlockStart(BlockStart {
            index: 0,
            block: BlockHeader::Text,
            ..
        })
    ));
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(BlockDelta {
                delta: Delta::Text(t),
                ..
            }) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.starts_with("Hello"));
    let events = decode_whole(&fixture("refusal"));
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::BlockStart(BlockStart {
            block: BlockHeader::Refusal,
            ..
        })
    )));
    let events = decode_whole(&fixture("reasoning-cached-usage"));
    let usage = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage.clone(),
            _ => None,
        })
        .unwrap();
    assert_eq!(usage.cache_read_tokens, 1920);
    assert_eq!(usage.input_tokens, 128);
    assert_eq!(usage.reasoning_tokens, Some(1152));
    let events = decode_whole(&fixture("content-filter-partial"));
    assert!(events.iter().any(
        |e| matches!(e, StreamEvent::ResponseDelta(d) if d.stop == Some(StopReason::ContentFilter))
    ));
}

#[test]
fn a_mid_stream_error_ends_the_stream() {
    let events = decode_whole(&fixture("error-mid-stream"));
    let StreamEvent::Error(err) = events.last().unwrap() else {
        panic!("{events:?}")
    };
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert_eq!(err.status, None);
    assert!(err.message.starts_with("The server had an error"));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::ResponseStop(_)))
    );
}

#[test]
fn an_openrouter_error_chunk_with_choices_ends_the_stream_as_an_error() {
    for (name, message, kind) in [
        (
            "providers/openrouter/doc-error-mid-stream",
            "Rate limit exceeded",
            ErrorKind::RateLimited,
        ),
        (
            "providers/openrouter/doc-error-mid-stream-string-code",
            "Provider disconnected unexpectedly",
            ErrorKind::ProviderError,
        ),
    ] {
        let events = decode_whole(&fixture(name));
        let StreamEvent::Error(err) = events.last().unwrap() else {
            panic!("{name}: {events:?}")
        };
        assert_eq!(
            (err.message.as_str(), &err.kind),
            (message, &kind),
            "{name}"
        );
        assert_eq!(err.status, None, "{name}");
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::ResponseStop(_))),
            "{name}"
        );
    }
}

#[test]
fn a_chunk_whose_error_is_a_string_ends_the_stream_as_an_error() {
    let chunk = r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":"Hi"},"finish_reason":null}]}"#;
    let sse = format!("data: {chunk}\n\ndata: {{\"error\":\"rate limited\"}}\n\n");
    let events = decode_whole(sse.as_bytes());
    let StreamEvent::Error(err) = events.last().unwrap() else {
        panic!("{events:?}")
    };
    assert_eq!(err.message, "rate limited");
    assert_eq!(err.raw, Some(json!({"error": "rate limited"})));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::ResponseStop(_)))
    );
    let kept = r#"{"id":"c","object":"chat.completion.chunk","created":1,"model":"m","error":null,"choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}"#;
    let sse = format!("data: {chunk}\n\ndata: {kept}\n\ndata: [DONE]\n\n");
    let events = decode_whole(sse.as_bytes());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
}

#[test]
fn comments_and_unknown_fields_survive() {
    let events = decode_whole(&fixture("stream-doc"));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Extra(item) if item.raw.get().contains("comment")))
    );
    let events = decode_whole(&fixture("unknown-chunk-fields"));
    let StreamEvent::ResponseStart(start) = &events[0] else {
        panic!()
    };
    assert!(start.extras.fields.contains_key("x_future_field"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d) if d.extras.fields.contains_key("delta.x_future_delta"))));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d) if d.extras.fields.contains_key("choice.x_future_choice_field"))));
    let events = decode_whole(&fixture("logprobs-sdk-2024"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d) if d.extras.fields.contains_key("choice.logprobs"))));
}

#[test]
fn frames_with_sse_fields_stay_typed_and_frames_without_data_stay_dataless() {
    let sse = concat!(
        "retry: 3000\n\n",
        "event: error\n\n",
        "id: 1\nx-trace: abc\ndata: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
        "id: 2\n\n",
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        kinds(&events)
            .iter()
            .filter(|k| k.starts_with("extra"))
            .count(),
        3,
        "{events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, StreamEvent::Error(_))));
    assert!(
        events.iter().any(
            |e| matches!(e, StreamEvent::BlockDelta(d) if d.delta == Delta::Text("Hi".into()))
        )
    );
    let out = encode(&events, false);
    assert_sse_eq("framed", sse.as_bytes(), &out);
    assert!(
        String::from_utf8(out)
            .unwrap()
            .starts_with("retry: 3000\n\nevent: error\n\n")
    );
}

#[test]
fn truncated_and_malformed_streams_end_with_an_error() {
    let bytes = fixture("tools-single");
    let cut = &bytes[..bytes.len() / 2];
    let events = decode_whole(cut);
    assert!(
        matches!(events.last(), Some(StreamEvent::Error(_))),
        "{:?}",
        kinds(&events)
    );
    let events = decode_whole(b"data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}]}\n\ndata: {not json}\n\n");
    let StreamEvent::Error(err) = events.last().unwrap() else {
        panic!()
    };
    assert_eq!(err.kind, ErrorKind::ProviderError);
    let events = decode_whole(b"data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{}},{\"index\":1,\"delta\":{}}]}\n\n");
    let StreamEvent::Error(err) = events.last().unwrap() else {
        panic!()
    };
    assert_eq!(err.unsupported_feature(), Some("n"));
    let mut decoder = OpenAiChat::stream_decoder(&ectx());
    decoder.feed(b"data: {bad\n\n");
    assert!(
        decoder.feed(b"data: [DONE]\n\n").is_empty(),
        "inert after an error"
    );
    assert!(decoder.finish().is_empty());
}

#[test]
fn a_stream_without_done_but_finished_still_stops() {
    let bytes = fixture("stream-doc");
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("data: [DONE]\n\n", "");
    let events = decode_whole(text.as_bytes());
    assert!(matches!(events.last(), Some(StreamEvent::ResponseStop(_))));
    let out = encode(&events, false);
    assert!(!String::from_utf8(out).unwrap().contains("[DONE]"));
}

#[test]
fn rewritten_tool_names_are_mapped_back() {
    let mut ctx = ectx();
    ctx.tool_names
        .insert("get_weather".into(), "weather.get".into());
    let mut decoder = OpenAiChat::stream_decoder(&ctx);
    let events = decoder.feed(&fixture("tools-single"));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockStart(BlockStart { block: BlockHeader::ToolCall(h), .. }) if h.name == "weather.get")));
}

#[test]
fn usage_is_hidden_when_the_app_did_not_ask() {
    let bytes = fixture("tools-single");
    let events = decode_whole(&bytes);
    let out = encode(&events, false);
    let frames = frames(&out);
    assert!(
        frames.iter().all(|f| f.get("usage").is_none()),
        "{frames:?}"
    );
    assert_eq!(frames.last(), Some(&json!("[DONE]")));
    let with = frames.len();
    assert_eq!(
        with + 1,
        self::frames(&bytes).len(),
        "only the usage chunk is gone"
    );
}

#[test]
fn provider_origin_is_stamped_on_events() {
    let ctx = ectx().with_provider("openai");
    let mut decoder = OpenAiChat::stream_decoder(&ctx);
    let events = decoder.feed(&fixture("stream-include-usage"));
    let expected = Origin::new(DialectId::OpenAiChat).with_provider("openai");
    let StreamEvent::ResponseStart(start) = &events[0] else {
        panic!()
    };
    assert_eq!(start.extras.origin.as_ref(), Some(&expected));
}

fn anthropic() -> Origin {
    Origin::new(DialectId::AnthropicMessages)
}

fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

fn anthropic_events() -> Vec<StreamEvent> {
    let mut start = ResponseStart::new("msg_01", "claude-x");
    start.usage = Some(Usage::new(25, 1));
    start.extras = kaeso_kpp::Extras::new(anthropic()).with_field("type", raw("\"message\""));
    let mut usage = Usage::new(0, 40);
    usage.reasoning_tokens = None;
    vec![
        StreamEvent::ResponseStart(start),
        StreamEvent::Extra(RawItem::new(
            anthropic(),
            raw(r#"{"event":"ping","data":{"type":"ping"}}"#),
        )),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Reasoning(ReasoningHeader::new(anthropic())),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningText("Let me think".into()),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Opaque(Opaque::new(
                anthropic(),
                OpaqueKind::AnthropicThinkingSignature,
                "sig",
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("Checking".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text(" now.".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(
            1,
            Delta::Citation(kaeso_kpp::Citation::new(
                anthropic(),
                raw(r#"{"type":"char_location"}"#),
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStart(BlockStart::new(
            2,
            BlockHeader::ToolCall(ToolCallHeader::new("toolu_01", "get_weather")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            2,
            Delta::ToolArguments("{\"city\":".into()),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            2,
            Delta::ToolArguments("\"Paris\"}".into()),
        )),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::BlockStart(BlockStart::new(
            3,
            BlockHeader::ToolCall(ToolCallHeader::new("toolu_02", "get_time")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(3, Delta::ToolArguments("{}".into()))),
        StreamEvent::BlockStop(BlockStop::new(3)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::ToolUse), Some(usage))),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]
}

fn chunk(delta: Value, finish: Value) -> Value {
    json!({
        "id": "msg_01", "object": "chat.completion.chunk", "created": 1_700_000_000, "model": "claude-x",
        "choices": [{"index": 0, "delta": delta, "logprobs": null, "finish_reason": finish}]
    })
}

#[test]
fn an_anthropic_stream_becomes_a_chat_stream() {
    let out = encode(&anthropic_events(), false);
    let tool = |index: u32, id: &str, name: &str| {
        json!({"tool_calls": [{"index": index, "id": id, "type": "function",
            "function": {"name": name, "arguments": ""}}]})
    };
    let args = |index: u32, a: &str| json!({"tool_calls": [{"index": index, "function": {"arguments": a}}]});
    assert_eq!(
        frames(&out),
        vec![
            chunk(
                json!({"role": "assistant", "content": "", "refusal": null}),
                Value::Null
            ),
            json!(": keepalive"),
            chunk(json!({"content": "Checking"}), Value::Null),
            chunk(json!({"content": " now."}), Value::Null),
            chunk(tool(0, "toolu_01", "get_weather"), Value::Null),
            chunk(args(0, "{\"city\":"), Value::Null),
            chunk(args(0, "\"Paris\"}"), Value::Null),
            chunk(tool(1, "toolu_02", "get_time"), Value::Null),
            chunk(args(1, "{}"), Value::Null),
            chunk(json!({}), json!("tool_calls")),
            json!("[DONE]"),
        ]
    );
    let events = decode_whole(&out);
    assert!(matches!(events.last(), Some(StreamEvent::ResponseStop(_))));
}

#[test]
fn an_anthropic_stream_reports_usage_when_asked() {
    let out = encode(&anthropic_events(), true);
    let frames = frames(&out);
    assert!(
        frames[..frames.len() - 2]
            .iter()
            .all(|f| f["usage"].is_null())
    );
    let usage_chunk = &frames[frames.len() - 2];
    assert_eq!(usage_chunk["choices"], json!([]));
    assert_eq!(
        usage_chunk["usage"],
        json!({"prompt_tokens": 25, "completion_tokens": 40, "total_tokens": 65,
            "prompt_tokens_details": {"cached_tokens": 0}})
    );
    let events = decode_whole(&out);
    let usage = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage.clone(),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.input_tokens, usage.output_tokens), (25, 40));
}

#[test]
fn a_foreign_refusal_and_error_stream_is_valid_chat() {
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("resp_1", "gpt-x")),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Refusal)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Refusal("No.".into()))),
        StreamEvent::Error(
            KppError::new(ErrorKind::Overloaded, "busy").with_origin(DialectId::AnthropicMessages),
        ),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let out = encode(&events, false);
    assert_eq!(
        frames(&out),
        vec![
            json!({
                "id": "resp_1", "object": "chat.completion.chunk", "created": 1_700_000_000, "model": "gpt-x",
                "choices": [{"index": 0, "delta": {"role": "assistant", "content": "", "refusal": null},
                    "logprobs": null, "finish_reason": null}]
            }),
            json!({
                "id": "resp_1", "object": "chat.completion.chunk", "created": 1_700_000_000, "model": "gpt-x",
                "choices": [{"index": 0, "delta": {"refusal": "No."},
                    "logprobs": null, "finish_reason": null}]
            }),
            json!({"error": {"message": "busy", "type": "service_unavailable_error", "param": null, "code": "server_is_overloaded"}}),
        ]
    );
}

#[test]
fn a_foreign_stream_without_stop_is_closed_by_finish() {
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("x", "m")),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("hi".into()))),
    ];
    let out = encode(&events, false);
    assert_eq!(frames(&out).last(), Some(&json!("[DONE]")));
    let unknown = StreamEvent::BlockStart(BlockStart::new(
        1,
        BlockHeader::Unknown(RawItem::new(
            anthropic(),
            raw(r#"{"type":"server_tool_use"}"#),
        )),
    ));
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    let mut encoder = OpenAiChat::stream_encoder(&ctx);
    assert!(encoder.encode(&unknown).unwrap().is_empty());
    let args = StreamEvent::BlockDelta(BlockDelta::new(1, Delta::ToolArguments("{}".into())));
    assert!(encoder.encode(&args).unwrap().is_empty());
}

#[test]
fn items_before_a_bad_line_are_kept_whatever_the_chunking() {
    let mut bytes = fixture("stream-include-usage");
    let cut = bytes.len() / 2;
    let at = cut + bytes[cut..].windows(2).position(|w| w == b"\n\n").unwrap() + 2;
    bytes.truncate(at);
    bytes.extend_from_slice(b"data: \xFF\n\n");
    let whole = decode_whole(&bytes);
    assert!(matches!(whole.last(), Some(StreamEvent::Error(_))));
    assert!(whole.len() > 3, "events before the bad line are kept");
    for sizes in [&[1][..], &[7], &[64]] {
        assert_eq!(decode_in(&bytes, sizes), whole, "pieces of {sizes:?}");
    }
}

#[test]
fn anthropic_fixture_streams_become_valid_chat_streams() {
    use crate::anthropic_messages::AnthropicMessages;
    for (name, bytes) in fixtures("anthropic_messages", "streams", "sse") {
        let mut decoder = AnthropicMessages::stream_decoder(&ectx());
        let mut events = decoder.feed(&bytes);
        events.extend(decoder.finish());
        let source_failed = events.iter().any(|e| matches!(e, StreamEvent::Error(_)));
        let out = encode(&events, true);
        let back = decode_whole(&out);
        let failed = back.iter().any(|e| matches!(e, StreamEvent::Error(_)));
        assert_eq!(
            failed,
            source_failed,
            "{name}: {}",
            String::from_utf8_lossy(&out)
        );
        if !source_failed {
            assert!(
                matches!(back.last(), Some(StreamEvent::ResponseStop(_))),
                "{name}"
            );
        }
    }
}

fn tool_headers(events: &[StreamEvent]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockStart(BlockStart {
                block: BlockHeader::ToolCall(h),
                ..
            }) => Some((h.id.clone(), h.name.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn id_model_and_tool_headers_that_arrive_late_reach_kpp() {
    let bytes = fixture("late-envelope-and-tool-header");
    let events = decode_whole(&bytes);
    let StreamEvent::ResponseStart(start) = &events[0] else {
        panic!("{events:?}")
    };
    assert_eq!(start.id, "chatcmpl-6TnLateEnvelope0x1");
    assert_eq!(start.model, "gpt-4.1-2025-04-14");
    assert_eq!(
        tool_headers(&events),
        [(
            "call_Lt7aQe2mVb9xRc4nWk1pZs8y".to_owned(),
            "get_weather".to_owned()
        )]
    );
    let mut ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(false));
    ctx.created_at = Some(1);
    let mut encoder = crate::anthropic_messages::AnthropicMessages::stream_encoder(&ctx);
    let mut out = Vec::new();
    for event in &events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"id\":\"chatcmpl-6TnLateEnvelope0x1\""),
        "{text}"
    );
    assert!(
        text.contains(
            "\"id\":\"call_Lt7aQe2mVb9xRc4nWk1pZs8y\",\"input\":{},\"name\":\"get_weather\""
        ),
        "{text}"
    );
}

#[test]
fn empty_ids_and_missing_ids_are_written_back_as_they_came() {
    let chunk = |delta: Value, finish: Value| {
        let c = json!({"id":"c","object":"chat.completion.chunk","created":1,"model":"m",
            "choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        format!("data: {c}\n\n")
    };
    let sse = [
        chunk(
            json!({"role":"assistant","tool_calls":[{"index":0,"id":"","type":"function","function":{"name":"","arguments":""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls":[{"index":0,"id":"call_1","function":{"name":"f","arguments":"{}"}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls":[{"index":1,"type":"function","function":{"name":"g","arguments":"{}"}}]}),
            Value::Null,
        ),
        chunk(json!({"content":"after"}), Value::Null),
        chunk(json!({}), json!("tool_calls")),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat();
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        tool_headers(&events),
        [
            ("call_1".to_owned(), "f".to_owned()),
            ("call_kaeso_c_1".to_owned(), "g".to_owned())
        ]
    );
    for sizes in [&[1][..], &[5], &[3, 17]] {
        assert_eq!(decode_in(sse.as_bytes(), sizes), events, "{sizes:?}");
    }
    assert_sse_eq("late", sse.as_bytes(), &encode(&events, false));
}

#[test]
fn a_held_tool_call_is_released_when_the_stream_breaks() {
    let sse = concat!(
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {not json\n\n",
    );
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "error"],
        "{events:?}"
    );
    let cut = &sse.as_bytes()[..sse.find("data: {not").unwrap()];
    let events = decode_whole(cut);
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "error"]
    );
}

fn running_usage_stream(final_usage: bool) -> String {
    let chunk = |delta: Value, finish: Value, completion: Option<u64>| {
        let mut c = json!({"id":"c","object":"chat.completion.chunk","created":1,"model":"m",
            "choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        if let Some(n) = completion {
            c["usage"] = json!({"prompt_tokens":5,"completion_tokens":n,"total_tokens":5 + n});
        }
        format!("data: {c}\n\n")
    };
    [
        chunk(
            json!({"role":"assistant","content":"Hel"}),
            Value::Null,
            Some(1),
        ),
        chunk(json!({"content":"lo"}), Value::Null, Some(2)),
        chunk(json!({}), json!("stop"), final_usage.then_some(3)),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat()
}

fn usage_deltas(events: &[StreamEvent]) -> Vec<(Option<StopReason>, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ResponseDelta(d) => {
                Some((d.stop.clone(), d.usage.as_ref()?.output_tokens))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn running_usage_is_reported_once_after_the_last_block_stops() {
    let sse = running_usage_stream(true);
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        kinds(&events),
        [
            "start",
            "block_start 0",
            "delta 0",
            "delta 0",
            "block_stop 0",
            "response_delta",
            "stop"
        ]
    );
    assert_eq!(usage_deltas(&events), [(Some(StopReason::EndTurn), 3)]);
    assert_sse_eq("replay", sse.as_bytes(), &encode(&events, true));
}

#[test]
fn running_usage_without_a_final_report_goes_out_before_the_stream_stops() {
    let sse = running_usage_stream(false);
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        kinds(&events),
        [
            "start",
            "block_start 0",
            "delta 0",
            "delta 0",
            "block_stop 0",
            "response_delta",
            "response_delta",
            "stop"
        ]
    );
    let stops: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ResponseDelta(d) => Some(d.stop.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(stops, [Some(StopReason::EndTurn), None]);
    assert_eq!(usage_deltas(&events), [(None, 2)]);
    assert_sse_eq("replay", sse.as_bytes(), &encode(&events, true));
}

#[test]
fn a_stream_with_running_usage_replays_byte_for_byte() {
    let bytes = fixture("usage-every-chunk");
    let events = decode_whole(&bytes);
    let last_block_stop = events
        .iter()
        .rposition(|e| matches!(e, StreamEvent::BlockStop(_)))
        .unwrap();
    let with_usage: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, StreamEvent::ResponseDelta(d) if d.usage.is_some()))
        .map(|(i, _)| i)
        .collect();
    assert!(
        with_usage.iter().all(|i| *i > last_block_stop),
        "{events:?}"
    );
    assert_sse_eq("usage-every-chunk", &bytes, &encode(&events, true));
    let hidden = String::from_utf8(encode(&events, false)).unwrap();
    assert!(!hidden.contains("usage"), "{hidden}");
    assert!(hidden.contains("help?"), "{hidden}");
}

#[test]
fn running_usage_on_a_chunk_without_content_is_held_and_hidden_from_apps_that_did_not_ask() {
    let sse = [
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":1,\"total_tokens\":5}}\n\n",
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    ]
    .concat();
    let events = decode_whole(sse.as_bytes());
    assert_eq!(usage_deltas(&events), [(None, 1)]);
    assert_sse_eq("replay", sse.as_bytes(), &encode(&events, true));
    let hidden = encode(&events, false);
    let text = String::from_utf8(hidden.clone()).unwrap();
    assert!(!text.contains("usage"), "{text}");
    let chunks: Vec<Value> = frames(&hidden)
        .into_iter()
        .filter(Value::is_object)
        .collect();
    assert_eq!(chunks.len(), 2, "{text}");
    assert!(
        chunks
            .iter()
            .all(|c| c["choices"].as_array().is_some_and(|a| !a.is_empty())),
        "{text}"
    );
}

#[test]
fn an_error_inside_a_chunk_with_choices_ends_the_stream_with_an_error() {
    let bytes = fixture("error-in-chunk");
    let events = decode_whole(&bytes);
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "delta 0", "error"]
    );
    let StreamEvent::Error(err) = events.last().unwrap() else {
        unreachable!()
    };
    assert_eq!(err.message, "Rate limit exceeded");
    assert!(!events.iter().any(|e| matches!(
        e,
        StreamEvent::ResponseDelta(_) | StreamEvent::ResponseStop(_)
    )));
    assert_sse_eq("error-in-chunk", &bytes, &encode(&events, false));
}

#[test]
fn finish_reason_error_without_details_is_a_provider_error() {
    let sse = concat!(
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"error\"}]}\n\n",
    );
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "error"]
    );
    let StreamEvent::Error(err) = events.last().unwrap() else {
        unreachable!()
    };
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert!(err.message.contains("finish_reason"), "{}", err.message);
    assert_sse_eq("finish-error", sse.as_bytes(), &encode(&events, false));
}

#[test]
fn a_second_tool_call_at_the_same_index_starts_a_block_of_its_own() {
    let chunk = |delta: Value, finish: Value| {
        let c = json!({"id":"c","object":"chat.completion.chunk","created":1,"model":"m",
            "choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        format!("data: {c}\n\n")
    };
    let call = |id: &str, name: &str, args: &str| json!({"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":args}}]});
    let sse = [
        chunk(json!({"role":"assistant","content":null}), Value::Null),
        chunk(call("call_1", "get_weather", "{\"city\":"), Value::Null),
        chunk(
            json!({"tool_calls":[{"index":0,"function":{"arguments":"\"Oslo\"}"}}]}),
            Value::Null,
        ),
        chunk(
            call("call_2", "get_time", "{\"zone\":\"UTC\"}"),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat();
    let events = decode_whole(sse.as_bytes());
    assert_eq!(
        tool_headers(&events),
        [
            ("call_1".to_owned(), "get_weather".to_owned()),
            ("call_2".to_owned(), "get_time".to_owned())
        ]
    );
    let first_stop = events
        .iter()
        .position(|e| matches!(e, StreamEvent::BlockStop(s) if s.index == 0))
        .unwrap();
    let second_start = events
        .iter()
        .position(|e| matches!(e, StreamEvent::BlockStart(s) if s.index == 1))
        .unwrap();
    assert!(first_stop < second_start, "{events:?}");
    assert_sse_eq("same-index", sse.as_bytes(), &encode(&events, false));
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
    let mut enc = crate::anthropic_messages::AnthropicMessages::stream_encoder(&ctx);
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("\"id\":\"call_1\""), "{text}");
    assert!(text.contains("\"id\":\"call_2\""), "{text}");
    assert!(text.contains("{\\\"city\\\":\\\"Oslo\\\"}"), "{text}");
}

#[test]
fn a_stream_that_ends_without_a_finish_reason_is_not_a_finished_reply() {
    let chunk = |finish: Value| {
        let c = json!({"id":"c","object":"chat.completion.chunk","created":1,"model":"m",
            "choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":finish}]});
        format!("data: {c}\n\n")
    };
    let truncated = [chunk(Value::Null), "data: [DONE]\n\n".to_owned()].concat();
    let events = decode_whole(truncated.as_bytes());
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "error"]
    );
    let empty = [
        chunk(json!("")),
        chunk(json!("")),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat();
    let events = decode_whole(empty.as_bytes());
    assert_eq!(
        kinds(&events),
        ["start", "block_start 0", "delta 0", "delta 0", "error"]
    );
    let finished = [
        chunk(json!("")),
        chunk(json!("stop")),
        "data: [DONE]\n\n".to_owned(),
    ]
    .concat();
    let events = decode_whole(finished.as_bytes());
    assert_eq!(
        kinds(&events),
        [
            "start",
            "block_start 0",
            "delta 0",
            "delta 0",
            "block_stop 0",
            "response_delta",
            "stop"
        ]
    );
    assert_sse_eq("empty finish", finished.as_bytes(), &encode(&events, false));
}
