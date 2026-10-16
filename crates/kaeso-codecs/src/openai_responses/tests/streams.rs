use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Citation, Delta, ErrorKind, Opaque, OpaqueKind,
    RawItem, ReasoningHeader, ResponseDelta, ResponseStart, ResponseStop, StopReason, SummaryDelta,
    ToolCallHeader, Usage,
};
use serde_json::json;

use crate::sse::{SseItem, parse_all};

use super::*;

pub(super) fn decode_stream_chunked(bytes: &[u8], size: usize) -> Vec<StreamEvent> {
    let mut dec = OpenAiResponses::stream_decoder(&gen_ctx());
    let mut events = Vec::new();
    for chunk in bytes.chunks(size.max(1)) {
        events.extend(dec.feed(chunk));
    }
    events.extend(dec.finish());
    events
}

pub(super) fn encode_stream(events: &[StreamEvent], ctx: &EncodeCtx) -> Vec<u8> {
    let mut enc = OpenAiResponses::stream_encoder(ctx);
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    out
}

#[test]
fn every_stream_fixture_round_trips_under_every_chunking() {
    for (name, bytes) in fixtures(DIR, "streams", "sse") {
        let whole = decode_stream_chunked(&bytes, bytes.len());
        for size in [1, 7, 64] {
            assert_eq!(
                decode_stream_chunked(&bytes, size),
                whole,
                "{name} at {size}"
            );
        }
        let ctx = EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::on(true));
        let out = encode_stream(&whole, &ctx);
        assert_sse_eq(&name, &bytes, &out);
    }
}

pub(super) fn stream(name: &str) -> Vec<StreamEvent> {
    decode_stream_chunked(&fixture("streams", name, "sse"), usize::MAX)
}

pub(super) fn sse_ctx() -> EncodeCtx {
    EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::on(true))
        .with_created_at(1_800_000_000)
}

pub(super) fn datas(bytes: &[u8]) -> Vec<Value> {
    parse_all(bytes)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            SseItem::Event(e) => {
                let v: Value = serde_json::from_str(&e.data).unwrap();
                assert_eq!(
                    e.event.as_deref(),
                    v["type"].as_str(),
                    "event name equals type"
                );
                Some(v)
            }
            SseItem::Comment(_) => None,
        })
        .collect()
}

pub(super) fn types(datas: &[Value]) -> Vec<&str> {
    datas.iter().map(|d| d["type"].as_str().unwrap()).collect()
}

pub(super) fn typed(events: &[StreamEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ResponseStart(s) => Some(format!("start {}", s.model)),
            StreamEvent::BlockStart(b) => Some(format!(
                "start {} {:?}",
                b.index,
                match &b.block {
                    BlockHeader::ToolCall(h) => format!("call {} {}", h.id, h.name),
                    BlockHeader::Text => "text".into(),
                    BlockHeader::Refusal => "refusal".into(),
                    BlockHeader::Reasoning(_) => "reasoning".into(),
                    other => format!("{other:?}"),
                }
            )),
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::Text(t)
                | Delta::ToolArguments(t)
                | Delta::Refusal(t)
                | Delta::ReasoningText(t)
                    if !t.is_empty() =>
                {
                    Some(format!("delta {} {t}", d.index))
                }
                Delta::ReasoningSummary(s) if !s.text.is_empty() => {
                    Some(format!("summary {} {}", d.index, s.text))
                }
                _ => None,
            },
            StreamEvent::BlockStop(s) => Some(format!("stop {}", s.index)),
            StreamEvent::ResponseDelta(d) => d.stop.as_ref().map(|s| format!("stop reason {s:?}")),
            StreamEvent::ResponseStop(_) => Some("end".into()),
            StreamEvent::Error(e) => Some(format!("error {:?}", e.kind)),
            _ => None,
        })
        .collect()
}

#[test]
fn stream_events_are_typed() {
    let ev = stream("text-basic-stream");
    assert!(
        matches!(&ev[0], StreamEvent::ResponseStart(s) if s.id.starts_with("resp_67c9") && s.model == "gpt-6-astra")
    );
    let text: String = ev
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(BlockDelta {
                delta: Delta::Text(t),
                ..
            }) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hi there! How can I assist you today?");
    assert!(matches!(ev.last(), Some(StreamEvent::ResponseStop(_))));
    let ev = stream("tools-parallel-stream");
    assert!(ev.iter().any(|e| matches!(e, StreamEvent::BlockStart(BlockStart { block: BlockHeader::ToolCall(h), .. }) if h.id == "call_UwyDtaR1hYwdz52RLMC9zTpI")));
    assert!(ev.iter().any(
        |e| matches!(e, StreamEvent::ResponseDelta(d) if d.stop == Some(StopReason::ToolUse))
    ));
    let ev = stream("reasoning-summary-stream");
    let summaries: Vec<&SummaryDelta> = ev
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(BlockDelta {
                delta: Delta::ReasoningSummary(s),
                ..
            }) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(
        summaries.iter().filter(|s| s.text.is_empty()).count(),
        2,
        "two summary parts"
    );
    assert!(ev.iter().any(|e| matches!(e, StreamEvent::BlockDelta(BlockDelta { delta: Delta::Opaque(o), .. }) if o.kind == OpaqueKind::OpenAiEncryptedReasoning)));
    let ev = stream("max-output-tokens-incomplete-stream");
    assert!(ev.iter().any(|e| matches!(e, StreamEvent::ResponseDelta(d) if d.stop == Some(StopReason::MaxTokens) && d.usage.as_ref().is_some_and(|u| u.output_tokens == 16))));
    let ev = stream("web-search-stream");
    assert!(ev.iter().any(|e| matches!(
        e,
        StreamEvent::BlockDelta(BlockDelta {
            delta: Delta::Citation(_),
            ..
        })
    )));
    assert!(ev.iter().any(|e| matches!(
        e,
        StreamEvent::BlockStart(BlockStart {
            block: BlockHeader::Unknown(_),
            ..
        })
    )));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, StreamEvent::Extra(_)))
            .count(),
        3
    );
    let ev = stream("reasoning-text-stream");
    let reasoning: String = ev
        .iter()
        .filter_map(|e| match e {
            StreamEvent::BlockDelta(BlockDelta {
                delta: Delta::ReasoningText(t),
                ..
            }) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert!(reasoning.starts_with("The user wants"));
    let ev = stream("refusal-stream");
    assert!(ev.iter().any(|e| matches!(
        e,
        StreamEvent::BlockStart(BlockStart {
            block: BlockHeader::Refusal,
            ..
        })
    )));
}

#[test]
fn provider_errors_in_the_stream() {
    let ev = stream("error-mid-stream");
    let Some(StreamEvent::Error(e)) = ev.last() else {
        panic!("{ev:?}")
    };
    assert_eq!(e.kind, ErrorKind::ProviderError);
    assert!(e.message.starts_with("An error occurred"));
    let ev = stream("failed-stream");
    let Some(StreamEvent::Error(e)) = ev.last() else {
        panic!()
    };
    assert_eq!(e.message, "The model failed to generate a response.");
    let ev = stream("recorded-refusal-stream");
    let Some(StreamEvent::Error(e)) = ev.last() else {
        panic!("{ev:?}")
    };
    assert_eq!(e.kind, ErrorKind::ContentFiltered);
    assert!(
        e.message
            .starts_with("This content was flagged for possible cybersecurity risk."),
        "{}",
        e.message
    );
    let mut enc = OpenAiResponses::stream_encoder(&sse_ctx());
    let out = enc
        .encode(&StreamEvent::Error(
            KppError::new(ErrorKind::Overloaded, "Overloaded")
                .with_origin(DialectId::AnthropicMessages),
        ))
        .unwrap();
    assert_eq!(
        datas(&out),
        vec![
            json!({"type": "error", "code": "server_is_overloaded", "message": "Overloaded", "param": null, "sequence_number": 0})
        ]
    );
}

#[test]
fn broken_streams_end_with_one_provider_error() {
    let created = b"event: response.created\ndata: {\"type\":\"response.created\",\"sequence_number\":0,\"response\":{\"id\":\"r\",\"model\":\"m\",\"status\":\"in_progress\",\"output\":[]}}\n\n";
    let ev = decode_stream_chunked(created, 5);
    assert!(matches!(&ev[0], StreamEvent::ResponseStart(_)));
    assert!(matches!(ev.last(), Some(StreamEvent::Error(e)) if e.kind == ErrorKind::ProviderError));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count(),
        1
    );
    let mut bad = created.to_vec();
    bad.extend_from_slice(b"data: {not json\n\n");
    let ev = decode_stream_chunked(&bad, 3);
    assert!(matches!(ev.last(), Some(StreamEvent::Error(_))));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count(),
        1
    );
    let mut cut = created.to_vec();
    cut.extend_from_slice(b"data: {\"type\":\"resp");
    let ev = decode_stream_chunked(&cut, 7);
    assert!(matches!(ev.last(), Some(StreamEvent::Error(_))));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count(),
        1
    );
}

#[test]
fn unusual_framing_and_orders_still_round_trip() {
    let sse = concat!(
        "data: {\"type\":\"response.created\",\"sequence_number\":0,\"response\":{\"id\":\"r\",\"model\":\"m\",\"status\":\"in_progress\",\"output\":[]}}\n\n",
        "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"sequence_number\":1,\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"status\":\"in_progress\",\"content\":[],\"role\":\"assistant\"}}\n\n",
        ": ping\n\n",
        "event: response.content_part.added\ndata: {\"type\":\"response.content_part.added\",\"sequence_number\":2,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"\"}}\n\n",
        "id: 9\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":3,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"Hi\",\"logprobs\":[]}\n\n",
        "event: lonely\n\n",
        "event: response.content_part.done\ndata: {\"type\":\"response.content_part.done\",\"sequence_number\":4,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"Hi\"}}\n\n",
        "event: response.output_text.done\ndata: {\"type\":\"response.output_text.done\",\"sequence_number\":5,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"text\":\"Hi\",\"logprobs\":[]}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"sequence_number\":7,\"response\":{\"id\":\"r\",\"model\":\"m\",\"status\":\"completed\",\"output\":[]}}\n\n",
    );
    let whole = decode_stream_chunked(sse.as_bytes(), usize::MAX);
    for size in [1, 2, 3, 5, 11] {
        assert_eq!(decode_stream_chunked(sse.as_bytes(), size), whole, "{size}");
    }
    let out = encode_stream(&whole, &sse_ctx());
    assert_sse_eq("unusual", sse.as_bytes(), &out);
    assert!(
        typed(&whole).contains(&"delta 0 Hi".to_owned()),
        "{whole:?}"
    );
}

#[test]
fn frames_with_id_retry_or_unknown_fields_are_read_and_written_back() {
    let sse = concat!(
        "retry: 3000\n\n",
        "id: 1\nretry: 500\nevent: response.created\ndata: {\"type\":\"response.created\",\"sequence_number\":0,\"response\":{\"id\":\"r\",\"model\":\"m\",\"status\":\"in_progress\",\"output\":[]}}\n\n",
        "id: 2\nevent: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"sequence_number\":1,\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"status\":\"in_progress\",\"content\":[],\"role\":\"assistant\"}}\n\n",
        "id: 3\nevent: response.content_part.added\ndata: {\"type\":\"response.content_part.added\",\"sequence_number\":2,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"\"}}\n\n",
        "id: 4\nx-trace: abc\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":3,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"Hi\",\"logprobs\":[]}\n\n",
        "id: 5\n\n",
        "event: response.output_text.done\ndata: {\"type\":\"response.output_text.done\",\"sequence_number\":4,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"text\":\"Hi\",\"logprobs\":[]}\n\n",
        "event: response.content_part.done\ndata: {\"type\":\"response.content_part.done\",\"sequence_number\":5,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"Hi\"}}\n\n",
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"sequence_number\":6,\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"Hi\"}],\"role\":\"assistant\"}}\n\n",
        "id: 9\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"sequence_number\":7,\"response\":{\"id\":\"r\",\"model\":\"m\",\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"annotations\":[],\"logprobs\":[],\"text\":\"Hi\"}],\"role\":\"assistant\"}]}}\n\n",
    );
    let whole = decode_stream_chunked(sse.as_bytes(), usize::MAX);
    for size in [1, 3, 7] {
        assert_eq!(decode_stream_chunked(sse.as_bytes(), size), whole, "{size}");
    }
    let kinds = typed(&whole);
    assert!(kinds.contains(&"start m".to_owned()), "{kinds:?}");
    assert!(kinds.contains(&"delta 0 Hi".to_owned()), "{kinds:?}");
    let extras = whole
        .iter()
        .filter(|e| matches!(e, StreamEvent::Extra(_)))
        .count();
    assert_eq!(extras, 2, "only the two frames without data are extras");
    let out = encode_stream(&whole, &sse_ctx());
    assert_sse_eq("framed", sse.as_bytes(), &out);
    assert!(
        String::from_utf8(out)
            .unwrap()
            .starts_with("retry: 3000\n\n")
    );
}

pub(super) fn start(id: &str, model: &str) -> StreamEvent {
    let mut s = ResponseStart::new(id, model);
    s.usage = Some(Usage::new(12, 0));
    StreamEvent::ResponseStart(s)
}

fn anthropic_like() -> Vec<StreamEvent> {
    let a = DialectId::AnthropicMessages;
    let mut usage = Usage::new(12, 34);
    usage.cache_read_tokens = 3;
    vec![
        start("msg_01XYZ", "claude-sonnet-4-6"),
        StreamEvent::Extra(RawItem::new(
            a,
            raw(r#"{"event":"ping","data":{"type":"ping"}}"#),
        )),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Reasoning(ReasoningHeader::new(a)),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ReasoningText("Think".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ReasoningText("ing.".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Opaque(Opaque::new(
                a,
                OpaqueKind::AnthropicThinkingSignature,
                "sig",
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("Hello".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(
            1,
            Delta::Citation(Citation::new(a, raw(r#"{"type":"char_location"}"#))),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text(" world".into()))),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStart(BlockStart::new(2, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(2, Delta::Text("!".into()))),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::BlockStart(BlockStart::new(
            3,
            BlockHeader::Unknown(RawItem::new(a, raw(r#"{"type":"server_tool_use"}"#))),
        )),
        StreamEvent::BlockStop(BlockStop::new(3)),
        StreamEvent::BlockStart(BlockStart::new(
            4,
            BlockHeader::ToolCall(ToolCallHeader::new("toolu_1", "get_weather")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            4,
            Delta::ToolArguments("{\"city\":".into()),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            4,
            Delta::ToolArguments("\"Paris\"}".into()),
        )),
        StreamEvent::BlockStop(BlockStop::new(4)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::ToolUse), Some(usage))),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]
}

#[test]
fn an_anthropic_like_stream_becomes_a_valid_responses_stream() {
    let out = encode_stream(&anthropic_like(), &sse_ctx());
    let d = datas(&out);
    assert_eq!(
        types(&d),
        [
            "response.created",
            "response.in_progress",
            "response.output_item.added",
            "response.reasoning_summary_part.added",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
            "response.reasoning_summary_part.done",
            "response.output_item.done",
            "response.output_item.added",
            "response.content_part.added",
            "response.output_text.delta",
            "response.output_text.delta",
            "response.output_text.done",
            "response.content_part.done",
            "response.content_part.added",
            "response.output_text.delta",
            "response.output_text.done",
            "response.content_part.done",
            "response.output_item.done",
            "response.output_item.added",
            "response.function_call_arguments.delta",
            "response.function_call_arguments.delta",
            "response.function_call_arguments.done",
            "response.output_item.done",
            "response.completed",
        ]
    );
    for (i, e) in d.iter().enumerate() {
        assert_eq!(e["sequence_number"], i, "{e}");
    }
    assert_eq!(d[0]["response"]["id"], "msg_01XYZ");
    assert_eq!(d[0]["response"]["usage"], Value::Null);
    assert_eq!(d[0]["response"]["created_at"], 1_800_000_000);
    let done = &d[25]["response"];
    assert_eq!(done["status"], "completed");
    let output = done["output"].as_array().unwrap();
    assert_eq!(output.len(), 3, "{output:?}");
    assert_eq!(
        output[0]["summary"],
        json!([{"type": "summary_text", "text": "Thinking."}])
    );
    assert!(output[0]["id"].as_str().unwrap().starts_with("rs_kaeso_"));
    let a = DialectId::AnthropicMessages;
    let mut thinking = kaeso_kpp::ReasoningBlock::new(a);
    thinking.text.push("Thinking.".into());
    thinking.opaque.push(Opaque::new(
        a,
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    assert_eq!(
        crate::continuity::unwrap_reasoning(output[0]["encrypted_content"].as_str().unwrap()),
        Some(thinking)
    );
    let output = &output[1..];
    assert_eq!(output[0]["content"][0]["text"], "Hello world");
    assert_eq!(output[0]["content"][1]["text"], "!");
    assert!(output[0]["id"].as_str().unwrap().starts_with("msg_kaeso_"));
    assert!(output[1]["id"].as_str().unwrap().starts_with("fc_kaeso_"));
    assert_eq!(
        output[0]["content"][0]["annotations"],
        json!([]),
        "foreign citations are left out"
    );
    assert_eq!(output[1]["arguments"], r#"{"city":"Paris"}"#);
    assert_eq!(output[1]["call_id"], "toolu_1");
    assert_eq!(done["usage"]["input_tokens"], 15);
    assert_eq!(done["usage"]["input_tokens_details"]["cached_tokens"], 3);
    assert_eq!(d[4]["item_id"], d[2]["item"]["id"]);
    assert_eq!(d[11]["item_id"], d[9]["item"]["id"]);
    assert_eq!(d[21]["item_id"], d[20]["item"]["id"]);

    let decoded = decode_stream_chunked(&out, 13);
    let got = typed(&decoded);
    let expect = [
        "start claude-sonnet-4-6",
        "start 0 \"reasoning\"",
        "summary 0 Think",
        "summary 0 ing.",
        "stop 0",
        "start 1 \"text\"",
        "delta 1 Hello",
        "delta 1  world",
        "stop 1",
        "start 2 \"text\"",
        "delta 2 !",
        "stop 2",
        "start 3 \"call toolu_1 get_weather\"",
        "delta 3 {\"city\":",
        "delta 3 \"Paris\"}",
        "stop 3",
        "stop reason ToolUse",
        "end",
    ];
    assert_eq!(got, expect);
    assert_sse_eq("re-encode", &out, &encode_stream(&decoded, &sse_ctx()));
}

#[test]
fn a_chat_like_stream_with_interleaved_tool_calls_and_late_usage() {
    let c = DialectId::OpenAiChat;
    let mut s = ResponseStart::new("chatcmpl-1", "gpt-4.1");
    s.extras = Extras::new(c).with_form("x", json!(1));
    let events = vec![
        StreamEvent::ResponseStart(s),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("Let me check.".into()))),
        StreamEvent::BlockStart(BlockStart::new(
            1,
            BlockHeader::ToolCall(ToolCallHeader::new("call_a", "f")),
        )),
        StreamEvent::BlockStart(BlockStart::new(
            2,
            BlockHeader::ToolCall(ToolCallHeader::new("call_b", "g")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::ToolArguments("{}".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(2, Delta::ToolArguments("{\"x\":1}".into()))),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::ToolUse), None)),
        StreamEvent::ResponseDelta(ResponseDelta::new(None, Some(Usage::new(5, 6)))),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let out = encode_stream(&events, &sse_ctx());
    let d = datas(&out);
    for (i, e) in d.iter().enumerate() {
        assert_eq!(e["sequence_number"], i);
    }
    let last = d.last().unwrap();
    assert_eq!(last["type"], "response.completed");
    let output = last["response"]["output"].as_array().unwrap();
    assert_eq!(output.len(), 3, "{output:?}");
    assert_eq!(output[0]["content"][0]["text"], "Let me check.");
    assert_eq!(output[1]["arguments"], "{}");
    assert_eq!(output[2]["arguments"], r#"{"x":1}"#);
    assert_eq!(last["response"]["usage"]["total_tokens"], 11);
    assert_eq!(
        types(&d)
            .iter()
            .filter(|t| **t == "response.output_item.done")
            .count(),
        3
    );
    assert_eq!(
        types(&d)
            .iter()
            .filter(|t| **t == "response.function_call_arguments.done")
            .count(),
        2
    );
    let decoded = decode_stream_chunked(&out, 9);
    assert!(decoded.iter().any(
        |e| matches!(e, StreamEvent::ResponseDelta(d) if d.stop == Some(StopReason::ToolUse))
    ));
}

#[test]
fn a_stream_without_response_start_gets_one_and_max_tokens_is_incomplete() {
    let events = vec![
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("cut".into()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::MaxTokens), None)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let d = datas(&encode_stream(&events, &sse_ctx()));
    assert_eq!(d[0]["type"], "response.created");
    let last = d.last().unwrap();
    assert_eq!(last["type"], "response.incomplete");
    assert_eq!(
        last["response"]["incomplete_details"]["reason"],
        "max_output_tokens"
    );
    assert_eq!(last["response"]["output"][0]["status"], "incomplete");
}

#[test]
fn summaries_refusals_and_hidden_foreign_reasoning() {
    let events = vec![
        start("r", "m"),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Reasoning(ReasoningHeader::new(R)),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningSummary(SummaryDelta::new(0, "a")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningSummary(SummaryDelta::new(1, "b")),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Refusal)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Refusal("no".into()))),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let out = encode_stream(&events, &sse_ctx());
    let d = datas(&out);
    assert_eq!(
        types(&d)[2..13],
        [
            "response.output_item.added",
            "response.reasoning_summary_part.added",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
            "response.reasoning_summary_part.done",
            "response.reasoning_summary_part.added",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
            "response.reasoning_summary_part.done",
            "response.output_item.done",
            "response.output_item.added",
        ]
    );
    let last = d.last().unwrap();
    assert_eq!(last["response"]["output"][0]["summary"][1]["text"], "b");
    assert_eq!(
        last["response"]["output"][1]["content"][0],
        json!({"type": "refusal", "refusal": "no"})
    );
    assert_eq!(last["response"]["usage"], Value::Null);
    let mut foreign = events.clone();
    foreign[1] = StreamEvent::BlockStart(BlockStart::new(
        0,
        BlockHeader::Reasoning(ReasoningHeader::new(DialectId::AnthropicMessages)),
    ));
    let fd = datas(&encode_stream(&foreign, &sse_ctx()));
    assert_eq!(types(&fd), types(&d));
    let item = &fd.last().unwrap()["response"]["output"][0];
    assert!(item["id"].as_str().unwrap().starts_with("rs_kaeso_"));
    assert_eq!(item["summary"][1]["text"], "b");
    let carried =
        crate::continuity::unwrap_reasoning(item["encrypted_content"].as_str().unwrap()).unwrap();
    assert_eq!(carried.summary, ["a", "b"]);
    assert_sse_eq(
        "summaries",
        &out,
        &encode_stream(&decode_stream_chunked(&out, 4), &sse_ctx()),
    );
}

#[test]
fn rewritten_tool_names_map_back_in_streams() {
    let mut ctx = gen_ctx();
    ctx.tool_names
        .insert("weather_lookup_1234abcd".into(), "weather.lookup".into());
    let mut events = anthropic_like();
    events[17] = StreamEvent::BlockStart(BlockStart::new(
        4,
        BlockHeader::ToolCall(ToolCallHeader::new("toolu_1", "weather_lookup_1234abcd")),
    ));
    let out = encode_stream(&events, &sse_ctx());
    let mut dec = OpenAiResponses::stream_decoder(&ctx);
    let mut got = dec.feed(&out);
    got.extend(dec.finish());
    assert!(got.iter().any(|e| matches!(e, StreamEvent::BlockStart(BlockStart { block: BlockHeader::ToolCall(h), .. }) if h.name == "weather.lookup")));
}

mod split {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        #[test]
        fn random_splits_decode_to_the_same_events(
            which in 0usize..12,
            cuts in proptest::collection::vec(any::<prop::sample::Index>(), 0..16),
        ) {
            let all = fixtures(DIR, "streams", "sse");
            let (_, bytes) = &all[which % all.len()];
            let whole = decode_stream_chunked(bytes, usize::MAX);
            let mut points: Vec<usize> = cuts.iter().map(|i| i.index(bytes.len() + 1)).collect();
            points.sort_unstable();
            let mut dec = OpenAiResponses::stream_decoder(&gen_ctx());
            let mut events = Vec::new();
            let mut last = 0;
            for p in points {
                events.extend(dec.feed(&bytes[last..p]));
                last = p;
            }
            events.extend(dec.feed(&bytes[last..]));
            events.extend(dec.finish());
            prop_assert_eq!(events, whole);
        }
    }
}
