use super::*;

use crate::test_support::assert_sse_eq;
use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ResponseDelta, ResponseStart,
    ResponseStop, StreamEvent, SummaryDelta, ToolCallHeader,
};

pub(super) fn stream_ctx() -> EgressCtx {
    EgressCtx::for_kind(
        RequestKind::Generate,
        StreamMode::On {
            include_usage: true,
        },
    )
    .with_provider("anthropic")
}

pub(super) fn decode_stream_chunked(
    bytes: &[u8],
    size: usize,
    ctx: &EgressCtx,
) -> Vec<StreamEvent> {
    let mut dec = AnthropicMessages::stream_decoder(ctx);
    let mut events = Vec::new();
    if size == 0 {
        events.extend(dec.feed(bytes));
    } else {
        for chunk in bytes.chunks(size) {
            events.extend(dec.feed(chunk));
        }
    }
    events.extend(dec.finish());
    events
}

pub(super) fn encode_stream(events: &[StreamEvent]) -> Vec<u8> {
    let ctx = EncodeCtx::new(
        RequestKind::Generate,
        StreamMode::On {
            include_usage: true,
        },
    );
    let mut enc = AnthropicMessages::stream_encoder(&ctx);
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap_or_else(|err| panic!("{e:?}: {err}")));
    }
    out.extend(enc.finish().unwrap());
    out
}

fn stream_fixture(name: &str) -> Vec<u8> {
    fixtures(DIALECT, "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no stream {name}"))
        .1
}

pub(super) fn accumulate(sse: &[u8]) -> Value {
    let mut message = Value::Null;
    let mut partial: std::collections::BTreeMap<u64, String> = Default::default();
    for item in crate::sse::parse_all(sse).unwrap() {
        let crate::sse::SseItem::Event(ev) = item else {
            continue;
        };
        let data: Value = serde_json::from_str(&ev.data).unwrap();
        assert_eq!(ev.event.as_deref(), data["type"].as_str(), "event name");
        match data["type"].as_str().unwrap() {
            "message_start" => message = data["message"].clone(),
            "content_block_start" => {
                let i = data["index"].as_u64().unwrap();
                let content = message["content"].as_array_mut().unwrap();
                assert_eq!(content.len() as u64, i, "indexes are sequential");
                content.push(data["content_block"].clone());
            }
            "content_block_delta" => {
                let i = data["index"].as_u64().unwrap();
                let block = &mut message["content"][i as usize];
                let d = &data["delta"];
                let append = |b: &mut Value, k: &str, s: &str| {
                    let old = b[k].as_str().unwrap_or_default().to_owned();
                    b[k] = Value::String(old + s);
                };
                match d["type"].as_str().unwrap() {
                    "text_delta" => append(block, "text", d["text"].as_str().unwrap()),
                    "thinking_delta" => append(block, "thinking", d["thinking"].as_str().unwrap()),
                    "signature_delta" => block["signature"] = d["signature"].clone(),
                    "citations_delta" => {
                        if !block["citations"].is_array() {
                            block["citations"] = json!([]);
                        }
                        block["citations"]
                            .as_array_mut()
                            .unwrap()
                            .push(d["citation"].clone());
                    }
                    "input_json_delta" => {
                        partial
                            .entry(i)
                            .or_default()
                            .push_str(d["partial_json"].as_str().unwrap());
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let i = data["index"].as_u64().unwrap();
                if let Some(json) = partial.remove(&i)
                    && !json.is_empty()
                {
                    message["content"][i as usize]["input"] = serde_json::from_str(&json).unwrap();
                }
            }
            "message_delta" => {
                for (k, v) in data["delta"].as_object().unwrap() {
                    message[k] = v.clone();
                }
                for (k, v) in data["usage"].as_object().unwrap() {
                    message["usage"][k] = v.clone();
                }
            }
            _ => {}
        }
    }
    message
}

#[test]
fn every_stream_fixture_round_trips_under_every_chunking() {
    let mut n = 0;
    for (name, bytes) in fixtures(DIALECT, "streams", "sse") {
        let ctx = stream_ctx();
        let whole = decode_stream_chunked(&bytes, 0, &ctx);
        for size in [1, 7, 64] {
            assert_eq!(
                decode_stream_chunked(&bytes, size, &ctx),
                whole,
                "{name}: chunks of {size}"
            );
        }
        let errors = whole
            .iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count();
        let expect_error = name == "error-overloaded-mid-stream";
        assert_eq!(errors, usize::from(expect_error), "{name}: {whole:?}");
        assert_sse_eq(&name, &bytes, &encode_stream(&whole));
        n += 1;
    }
    assert!(n >= 14, "{n}");
}

#[test]
fn streams_accumulate_to_their_paired_responses() {
    let responses: std::collections::HashMap<_, _> =
        fixtures(DIALECT, "responses", "json").into_iter().collect();
    let mut n = 0;
    for (name, bytes) in fixtures(DIALECT, "streams", "sse") {
        let Some(response) = responses.get(&name) else {
            continue;
        };
        let out = encode_stream(&decode_stream_chunked(&bytes, 0, &stream_ctx()));
        let expected: Value = serde_json::from_slice(response).unwrap();
        assert_eq!(accumulate(&out), expected, "{name}");
        n += 1;
    }
    assert_eq!(n, 10);
}

#[test]
fn stream_events_are_typed() {
    let events = decode_stream_chunked(&stream_fixture("tools-single"), 0, &stream_ctx());
    let StreamEvent::ResponseStart(start) = &events[0] else {
        panic!()
    };
    assert_eq!(start.id, "msg_01aZVoMeJ4jfgoELtozyLhH2");
    assert_eq!(start.usage.as_ref().unwrap().input_tokens, 578);
    assert!(
        matches!(&events[1], StreamEvent::BlockStart(s) if s.index == 0 && s.block == BlockHeader::Text)
    );
    assert!(matches!(&events[2], StreamEvent::Extra(i) if i.raw.get().contains("ping")));
    let args: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(d) if d.index == 1 => match &d.delta {
                Delta::ToolArguments(s) => Some(s.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        serde_json::from_str::<Value>(&args).unwrap(),
        json!({"location": "San Francisco, CA", "unit": "fahrenheit"})
    );
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockStart(s)
        if s.block == BlockHeader::ToolCall(ToolCallHeader::new("toolu_01VLLqZ1bfs3WXvVeYqZPtE7", "get_weather")))));
    let StreamEvent::ResponseDelta(d) = &events[events.len() - 2] else {
        panic!()
    };
    assert_eq!(d.stop, Some(kaeso_kpp::StopReason::ToolUse));
    let u = d.usage.as_ref().unwrap();
    assert_eq!((u.input_tokens, u.output_tokens), (578, 87));
    assert_eq!(
        u.cache_write_breakdown,
        Some(kaeso_kpp::CacheWriteBreakdown::new(0, 0)),
        "completed from message_start"
    );
    assert!(matches!(events.last(), Some(StreamEvent::ResponseStop(_))));

    let events = decode_stream_chunked(&stream_fixture("thinking-redacted"), 0, &stream_ctx());
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if matches!(&d.delta, Delta::Opaque(o) if o.kind == OpaqueKind::AnthropicThinkingSignature))));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockStart(s)
        if matches!(&s.block, BlockHeader::Reasoning(h) if h.opaque.len() == 1))));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if matches!(&d.delta, Delta::ReasoningText(t) if t.starts_with("Aurora")))));
    let u = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage.clone(),
            _ => None,
        })
        .unwrap();
    assert_eq!(u.reasoning_tokens, Some(181));

    let events =
        decode_stream_chunked(&stream_fixture("document-text-citations"), 0, &stream_ctx());
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if matches!(&d.delta, Delta::Citation(c) if c.origin.is(A)))));

    let events = decode_stream_chunked(&stream_fixture("web-search-server-tool"), 0, &stream_ctx());
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockStart(s)
        if s.index == 1 && matches!(&s.block, BlockHeader::Unknown(i) if i.raw.get().contains("server_tool_use")))));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if d.index == 1 && matches!(&d.delta, Delta::Unknown(i) if i.raw.get().contains("input_json_delta")))));

    let events = decode_stream_chunked(&stream_fixture("unknown-event"), 0, &stream_ctx());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Extra(i) if i.raw.get().contains("x_future_event")))
    );
    let events =
        decode_stream_chunked(&stream_fixture("unknown-block-and-delta"), 0, &stream_ctx());
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if matches!(&d.delta, Delta::Unknown(i) if i.raw.get().contains("future_delta")))));

    let events = decode_stream_chunked(&stream_fixture("refusal-mid-stream"), 0, &stream_ctx());
    let d = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::ResponseDelta(d) => Some(d),
            _ => None,
        })
        .unwrap();
    assert_eq!(d.stop, Some(kaeso_kpp::StopReason::Refusal));
    assert!(
        d.extras.fields["delta.stop_details"]
            .get()
            .contains("refusal")
    );
}

#[test]
fn mid_stream_errors_become_error_events_and_back() {
    let bytes = stream_fixture("error-overloaded-mid-stream");
    let events = decode_stream_chunked(&bytes, 0, &stream_ctx());
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!("{events:?}")
    };
    assert_eq!(e.kind, kaeso_kpp::ErrorKind::Overloaded);
    assert_eq!(e.message, "Overloaded");
    assert!(e.origin.as_ref().unwrap().is(A));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::BlockStop(_) | StreamEvent::ResponseStop(_)))
    );
    let out = encode_stream(&[
        StreamEvent::ResponseStart(ResponseStart::new("x", "m")),
        StreamEvent::Error(kaeso_kpp::KppError::new(
            kaeso_kpp::ErrorKind::RateLimited,
            "slow down",
        )),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]);
    let text = String::from_utf8(out).unwrap();
    assert!(text.ends_with(
        "event: error\ndata: {\"error\":{\"message\":\"slow down\",\"type\":\"rate_limit_error\"},\"type\":\"error\"}\n\n"
    ), "{text}");
}

#[test]
fn broken_streams_end_with_one_provider_error() {
    let ctx = stream_ctx();
    let full = stream_fixture("text-simple");
    let cut = &full[..full.len() / 2];
    let events = decode_stream_chunked(cut, 0, &ctx);
    let errors: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::Error(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind, kaeso_kpp::ErrorKind::ProviderError);
    assert!(matches!(events.last(), Some(StreamEvent::Error(_))));

    for bad in [
        &b"event: message_start\ndata: {not json\n\n"[..],
        b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":3,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}\n\n",
        b"",
    ] {
        let events = decode_stream_chunked(bad, 0, &ctx);
        assert!(
            matches!(&events[..], [StreamEvent::Error(e)] if e.kind == kaeso_kpp::ErrorKind::ProviderError),
            "{events:?}"
        );
    }
    let mut dec = AnthropicMessages::stream_decoder(&ctx);
    assert_eq!(dec.feed(b"event: message_start\ndata: [\n\n").len(), 1);
    assert!(dec.feed(&full).is_empty());
    assert!(dec.finish().is_empty());
}

#[test]
fn unusual_framing_and_start_content_round_trip() {
    let stream = b": hello\n\
event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"x\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1},\"extra\":true}}\n\n\
event: content_block_start\nid: 7\ndata: {\"type\":\"content_block_start\",\"index\":5,\"content_block\":{\"type\":\"text\",\"text\":\"Hi\",\"citations\":[]}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":5,\"delta\":{\"type\":\"text_delta\",\"text\":\" there\",\"x\":1}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":5}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":6,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"f\",\"input\":{\"a\":1}}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":6}\n\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: custom\ndata: plain text\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let events = decode_stream_chunked(stream, 0, &stream_ctx());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if d.index == 0 && d.delta == Delta::Text("Hi".into()))));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d)
        if d.index == 1 && d.delta == Delta::ToolArguments("{\"a\":1}".into()))));
    assert_sse_eq("framing", stream, &encode_stream(&events));
}

#[test]
fn rewritten_tool_names_map_back_in_streams() {
    let mut ctx = stream_ctx();
    ctx.tool_names
        .insert("files_read_x".into(), "files.read".into());
    ctx.tool_ids.insert("toolu_9".into(), "call:9".into());
    let stream = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"x\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_9\",\"name\":\"files_read_x\",\"input\":{}}}\n\n";
    let events = decode_stream_chunked(stream, 0, &ctx);
    assert!(matches!(&events[1], StreamEvent::BlockStart(s)
        if s.block == BlockHeader::ToolCall(ToolCallHeader::new("call:9", "files.read"))));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::BlockStart(s) if !s.extras.form.is_empty()))
    );
}

fn chat_like_stream() -> Vec<StreamEvent> {
    let ex = || Extras::new(CHAT).with_field("system_fingerprint", raw("\"fp\""));
    let mut start = ResponseStart::new("chatcmpl-1", "gpt-5");
    start.extras = ex();
    let delta = |i: u32, d: Delta| {
        let mut bd = BlockDelta::new(i, d);
        bd.extras = Extras::new(CHAT).with_form("chunk", json!(1));
        StreamEvent::BlockDelta(bd)
    };
    let mut usage = kaeso_kpp::Usage::new(20, 9);
    usage.cache_read_tokens = 100;
    usage.raw = Some(kaeso_kpp::RawUsage::new(
        CHAT,
        json!({"prompt_tokens": 120}),
    ));
    vec![
        StreamEvent::ResponseStart(start),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        delta(0, Delta::Text("Checking ".into())),
        delta(0, Delta::Text("both.".into())),
        StreamEvent::BlockStart(BlockStart::new(
            1,
            BlockHeader::ToolCall(ToolCallHeader::new("call_a", "get_weather")),
        )),
        delta(1, Delta::ToolArguments("{\"location\":".into())),
        StreamEvent::BlockStart(BlockStart::new(
            2,
            BlockHeader::ToolCall(ToolCallHeader::new("call_b", "get_time")),
        )),
        delta(2, Delta::ToolArguments("{\"timezone\":\"UTC\"}".into())),
        delta(1, Delta::ToolArguments("\"Oslo\"}".into())),
        StreamEvent::Extra(kaeso_kpp::RawItem::new(
            CHAT,
            raw(r#"{"event":null,"data":"[DONE]"}"#),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(kaeso_kpp::StopReason::ToolUse),
            None,
        )),
        StreamEvent::ResponseDelta(ResponseDelta::new(None, Some(usage))),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]
}

#[test]
fn a_foreign_stream_becomes_a_valid_sequential_anthropic_stream() {
    let out = encode_stream(&chat_like_stream());
    let names: Vec<String> = crate::sse::parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            crate::sse::SseItem::Event(e) => e.event,
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        [
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_delta",
            "ping",
            "content_block_stop",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop",
        ]
    );
    assert_eq!(
        accumulate(&out),
        json!({
            "id": "chatcmpl-1", "type": "message", "role": "assistant", "model": "gpt-5",
            "content": [
                {"type": "text", "text": "Checking both."},
                {"type": "tool_use", "id": "call_a", "name": "get_weather", "input": {"location": "Oslo"}},
                {"type": "tool_use", "id": "call_b", "name": "get_time", "input": {"timezone": "UTC"}},
            ],
            "stop_reason": "tool_use", "stop_sequence": null, "stop_details": null,
            "usage": {"input_tokens": 20, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 100,
                "cache_creation": null, "output_tokens": 9},
        })
    );
}

#[test]
fn foreign_reasoning_is_carried_unknown_blocks_left_out_and_refusals_mapped() {
    let origin = DialectId::OpenAiResponses;
    let mut header = kaeso_kpp::ReasoningHeader::new(origin);
    header.opaque.push(kaeso_kpp::Opaque::new(
        origin,
        OpaqueKind::OpenAiReasoningItemId,
        "rs_1",
    ));
    let mut carried = ReasoningBlock::new(origin);
    carried.summary = vec!["First.".into(), "Second.".into()];
    carried.opaque = header.opaque.clone();
    carried.opaque.push(kaeso_kpp::Opaque::new(
        origin,
        OpaqueKind::OpenAiEncryptedReasoning,
        "gAAA",
    ));
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("resp_1", "o4")),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Reasoning(header))),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningSummary(SummaryDelta::new(0, "First.")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningSummary(SummaryDelta::new(1, "Second.")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Opaque(kaeso_kpp::Opaque::new(
                origin,
                OpaqueKind::OpenAiEncryptedReasoning,
                "gAAA",
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(
            1,
            BlockHeader::Unknown(kaeso_kpp::RawItem::new(
                origin,
                raw(r#"{"type":"web_search_call"}"#),
            )),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            1,
            Delta::Unknown(kaeso_kpp::RawItem::new(origin, raw(r#"{"type":"x"}"#))),
        )),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStart(BlockStart::new(2, BlockHeader::Refusal)),
        StreamEvent::BlockDelta(BlockDelta::new(2, Delta::Refusal("No.".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(
            2,
            Delta::Citation(kaeso_kpp::Citation::new(
                origin,
                raw(r#"{"type":"url_citation"}"#),
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(kaeso_kpp::StopReason::ContentFilter),
            Some(kaeso_kpp::Usage::new(3, 4)),
        )),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let out = encode_stream(&events);
    assert_eq!(
        accumulate(&out),
        json!({
            "id": "resp_1", "type": "message", "role": "assistant", "model": "o4",
            "content": [
                {"type": "thinking", "thinking": "First.\n\nSecond.",
                    "signature": crate::continuity::wrap_reasoning(&carried)},
                {"type": "text", "text": "No."},
            ],
            "stop_reason": "refusal", "stop_sequence": null, "stop_details": null,
            "usage": {"input_tokens": 3, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0,
                "cache_creation": null, "output_tokens": 4},
        })
    );
}

#[test]
fn a_stream_without_response_start_gets_one() {
    let ctx = EncodeCtx::for_request(&Request::Generate(chat_request()));
    let mut enc = AnthropicMessages::stream_encoder(&ctx);
    let mut out = Vec::new();
    for e in [
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("x".into()))),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ] {
        out.extend(enc.encode(&e).unwrap());
    }
    let msg = accumulate(&out);
    assert_eq!(msg["model"], "claude-sonnet-4-6");
    assert_eq!(msg["content"], json!([{"type": "text", "text": "x"}]));
    assert_eq!(msg["stop_reason"], Value::Null);
    let mut enc = AnthropicMessages::stream_encoder(&ctx);
    assert!(
        enc.encode(&StreamEvent::BlockDelta(BlockDelta::new(
            4,
            Delta::Text("x".into())
        )))
        .is_err()
    );
}

#[test]
fn a_second_response_start_is_refused() {
    let ctx = EncodeCtx::for_request(&Request::Generate(chat_request()));
    let start = StreamEvent::ResponseStart(ResponseStart::new("msg_1", "claude-sonnet-4-6"));
    let mut enc = AnthropicMessages::stream_encoder(&ctx);
    let out = enc.encode(&start).unwrap();
    assert_eq!(
        String::from_utf8(out)
            .unwrap()
            .matches("message_start")
            .count(),
        2
    );
    let err = enc.encode(&start).unwrap_err();
    assert!(err.message.contains("started twice"), "{}", err.message);
    let mut enc = AnthropicMessages::stream_encoder(&ctx);
    enc.encode(&StreamEvent::BlockStart(BlockStart::new(
        0,
        BlockHeader::Text,
    )))
    .unwrap();
    assert!(enc.encode(&start).is_err());
}

mod stream_props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn random_splits_decode_to_the_same_events(
            pick in 0usize..14,
            cuts in proptest::collection::vec(any::<prop::sample::Index>(), 0..12),
        ) {
            let all = fixtures(DIALECT, "streams", "sse");
            let (name, bytes) = &all[pick % all.len()];
            let ctx = stream_ctx();
            let whole = decode_stream_chunked(bytes, 0, &ctx);
            let mut points: Vec<usize> = cuts.iter().map(|c| c.index(bytes.len() + 1)).collect();
            points.sort_unstable();
            let mut dec = AnthropicMessages::stream_decoder(&ctx);
            let mut events = Vec::new();
            let mut last = 0;
            for p in points {
                events.extend(dec.feed(&bytes[last..p]));
                last = p;
            }
            events.extend(dec.feed(&bytes[last..]));
            events.extend(dec.finish());
            prop_assert_eq!(&events, &whole, "{}", name);
        }
    }
}
