use super::replies::{encode_reply, generate_ctx};
use super::streams::{accumulate, decode_stream_chunked, encode_stream, stream_ctx};
use super::*;
use kaeso_kpp::{
    BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ErrorKind, KppError, ResponseDelta,
    ResponseStart, ResponseStop, StopReason, StreamEvent,
};

fn reply_with_stop(stop: StopReason) -> Value {
    let mut r = kaeso_kpp::Response::new("id", "m", vec![Block::text("x")]);
    r.stop = stop;
    r.extras = Extras::new(CHAT);
    encode_reply(&kaeso_kpp::Reply::Generate(r))
}

#[test]
fn inputs_are_left_out_of_replies() {
    let mut r = kaeso_kpp::Response::new(
        "id",
        "m",
        vec![
            Block::image(Source::base64("image/png", "AAAA")),
            Block::Document(DocumentBlock::new(Source::base64(
                "application/pdf",
                "JVBE",
            ))),
            Block::tool_result("c", vec![Block::text("x")], false),
            Block::text("kept"),
        ],
    );
    r.extras = Extras::new(CHAT);
    let body = encode_reply(&kaeso_kpp::Reply::Generate(r));
    assert_eq!(body["content"], json!([{"type": "text", "text": "kept"}]));
}

#[test]
fn own_blocks_that_replies_rarely_carry_round_trip() {
    let body = json!({
        "id": "msg_1",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-6",
        "content": [
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}},
            {"type": "tool_use", "id": "toolu_1", "name": "f", "input": [1]},
            {"type": "text", "text": "kept"}
        ],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let reply =
        AnthropicMessages::decode_response(200, &HeaderList::new(), &bytes, &generate_ctx())
            .unwrap();
    assert_eq!(encode_reply(&reply), body);
}

#[test]
fn foreign_stop_reasons_become_documented_values() {
    let other = |s: &str| StopReason::Other(s.to_owned());
    assert_eq!(
        reply_with_stop(other("length"))["stop_reason"],
        "max_tokens"
    );
    assert_eq!(
        reply_with_stop(other("function_call"))["stop_reason"],
        "tool_use"
    );
    assert_eq!(reply_with_stop(other("weird"))["stop_reason"], "end_turn");
    assert_eq!(
        reply_with_stop(other("pause_turn"))["stop_reason"],
        "pause_turn"
    );
    let body = json!({"id":"m","type":"message","role":"assistant","content":[],"model":"x",
        "stop_reason":"future_reason","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}});
    let reply = AnthropicMessages::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &generate_ctx(),
    )
    .unwrap();
    assert_eq!(encode_reply(&reply), body);
    let mut delta = ResponseDelta::new(Some(other("function_call")), None);
    delta.extras = Extras::new(CHAT);
    let out = encode_stream(&[
        StreamEvent::ResponseStart(ResponseStart::new("x", "m")),
        StreamEvent::ResponseDelta(delta),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]);
    assert_eq!(accumulate(&out)["stop_reason"], "tool_use");
}

#[test]
fn unsigned_own_thinking_is_written_back_as_received() {
    let body = json!({"model":"m","max_tokens":5,"messages":[
        {"role":"user","content":"q"},
        {"role":"assistant","content":[
            {"type":"thinking","thinking":"shown","signature":""},
            {"type":"text","text":"a"}]},
        {"role":"user","content":"next"},
        {"role":"assistant","content":[{"type":"thinking","thinking":"only"}]},
        {"role":"user","content":"again"}]});
    let r = generate(body.clone());
    let Block::Reasoning(b) = &r.messages[1].blocks[0] else {
        panic!()
    };
    assert!(b.opaque.is_empty(), "an empty signature is not opaque data");
    assert_eq!(
        encode_value(r),
        body,
        "the turns and their thinking are kept"
    );
    let mut reasoning = ReasoningBlock::new(DialectId::OpenAiResponses);
    reasoning.summary.push("thought".into());
    let mut resp = kaeso_kpp::Response::new("id", "m", vec![Block::Reasoning(reasoning.clone())]);
    resp.extras = Extras::new(CHAT);
    let content = &encode_reply(&kaeso_kpp::Reply::Generate(resp))["content"];
    assert_eq!(content[0]["thinking"], "thought", "{content}");
    assert_eq!(
        crate::continuity::unwrap_reasoning(content[0]["signature"].as_str().unwrap()),
        Some(reasoning)
    );
    for content in [
        json!([{"type":"thinking","thinking":"","signature":""}]),
        json!([{"type":"thinking","thinking":"t"},{"type":"text","text":"a"}]),
    ] {
        let body = json!({"id":"m","type":"message","role":"assistant","model":"x",
            "content":content,
            "stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}});
        let reply = AnthropicMessages::decode_response(
            200,
            &HeaderList::new(),
            &serde_json::to_vec(&body).unwrap(),
            &generate_ctx(),
        )
        .unwrap();
        assert_eq!(encode_reply(&reply), body);
    }
}

#[test]
fn unsigned_reasoning_that_was_not_read_from_anthropic_is_dropped() {
    let mut b = ReasoningBlock::new(A);
    b.text.push("t".into());
    let mut req = GenerateRequest::new("m")
        .with_message(Message::user("q"))
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::Reasoning(b.clone()), Block::text("a")],
        ));
    req.sampling.max_output_tokens = Some(5);
    assert_eq!(
        encode_value(req)["messages"][1]["content"],
        json!([{"type": "text", "text": "a"}])
    );
    let mut resp = kaeso_kpp::Response::new("id", "m", vec![Block::Reasoning(b)]);
    resp.extras = Extras::new(A);
    assert_eq!(
        encode_reply(&kaeso_kpp::Reply::Generate(resp))["content"],
        json!([])
    );
}

const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"x\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n";
const TEXT_START: &str = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
const TEXT_DELTA: &str = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n";
const BLOCK_STOP: &str =
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
const STOP: &str = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

fn assert_fails_after(stream: &str, kept: usize, why: &str) {
    for size in [0, 1, 7] {
        let events = decode_stream_chunked(stream.as_bytes(), size, &stream_ctx());
        assert_eq!(events.len(), kept + 1, "{why}: {events:?}");
        let Some(StreamEvent::Error(e)) = events.last() else {
            panic!("{why}: {events:?}")
        };
        assert_eq!(e.kind, ErrorKind::ProviderError, "{why}");
        assert!(
            !events[..kept]
                .iter()
                .any(|e| matches!(e, StreamEvent::Error(_))),
            "{why}"
        );
    }
}

#[test]
fn stream_order_is_validated() {
    assert_fails_after(TEXT_START, 0, "block before message_start");
    assert_fails_after(&format!("{START}{START}"), 1, "second message_start");
    let tool_start = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"f\",\"input\":{}}}\n\n";
    assert_fails_after(
        &format!("{START}{tool_start}{TEXT_DELTA}"),
        2,
        "text delta on a tool block",
    );
    assert_fails_after(
        &format!("{START}{TEXT_START}{BLOCK_STOP}{STOP}{TEXT_START}"),
        4,
        "lifecycle event after message_stop",
    );
    assert_fails_after(
        &format!("{START}{TEXT_START}{TEXT_DELTA}{STOP}"),
        3,
        "message_stop with an open block",
    );
    assert_fails_after(
        &format!("{START}event: content_block_stop\ndata: {{\"index\":0}}\n\n"),
        1,
        "known event without type",
    );
    let ok = format!(
        "{START}{TEXT_START}{BLOCK_STOP}{STOP}event: ping\ndata: {{\"type\":\"ping\"}}\n\n: bye\n\nevent: x\ndata: {{\"type\":\"x\"}}\n\n"
    );
    let events = decode_stream_chunked(ok.as_bytes(), 0, &stream_ctx());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
    let open = format!(
        "{START}{TEXT_START}{TEXT_DELTA}event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"output_tokens\":1}}}}\n\n"
    );
    let events = decode_stream_chunked(open.as_bytes(), 0, &stream_ctx());
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!("{events:?}")
    };
    assert!(e.message.contains("still open"), "{}", e.message);
}

#[test]
fn odd_error_bodies_are_classified_by_status() {
    let html = b"<html><body>413 Request Entity Too Large</body></html>";
    let e = AnthropicMessages::decode_error(413, &HeaderList::new(), html);
    assert_eq!(e.kind, ErrorKind::RequestTooLarge);
    assert_eq!(e.raw, Some(json!(String::from_utf8_lossy(html))));
    let enc = AnthropicMessages::encode_error(&e);
    assert_eq!(enc.status, 413);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["error"]["type"], "request_too_large");
    for (status, kind) in [
        (429, ErrorKind::RateLimited),
        (529, ErrorKind::Overloaded),
        (418, ErrorKind::ProviderError),
    ] {
        let e = AnthropicMessages::decode_error(status, &HeaderList::new(), br#"{"oops":1}"#);
        assert_eq!(e.kind, kind, "{status}");
        assert_eq!(e.status, Some(status));
    }
}

#[test]
fn model_context_window_exceeded_is_max_tokens() {
    let body = json!({"id":"m","type":"message","role":"assistant","content":[],"model":"x",
        "stop_reason":"model_context_window_exceeded","stop_sequence":null,"stop_details":null,
        "usage":{"input_tokens":1,"output_tokens":0}});
    let reply = AnthropicMessages::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &generate_ctx(),
    )
    .unwrap();
    let kaeso_kpp::Reply::Generate(r) = &reply else {
        panic!()
    };
    assert_eq!(r.stop, StopReason::MaxTokens);
    assert_eq!(encode_reply(&reply), body);
    let stream = format!(
        "{START}event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"model_context_window_exceeded\",\"stop_sequence\":null}},\"usage\":{{\"output_tokens\":1}}}}\n\n{STOP}"
    );
    let events = decode_stream_chunked(stream.as_bytes(), 0, &stream_ctx());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::ResponseDelta(d)
        if d.stop == Some(StopReason::MaxTokens)))
    );
    crate::test_support::assert_sse_eq("s3", stream.as_bytes(), &encode_stream(&events));
}

#[test]
fn prompt_too_long_is_context_too_long() {
    let body = br#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 210000 tokens > 200000 maximum"},"request_id":"req_1"}"#;
    let e = AnthropicMessages::decode_error(400, &HeaderList::new(), body);
    assert_eq!(e.kind, ErrorKind::ContextTooLong);
    let enc = AnthropicMessages::encode_error(&e);
    assert_eq!(enc.status, 400);
    crate::test_support::assert_json_eq("s4", body, &enc.body);
    let enc = AnthropicMessages::encode_error(
        &KppError::new(ErrorKind::ContextTooLong, "too long").with_origin(CHAT),
    );
    assert_eq!(enc.status, 400);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[test]
fn temperature_range_tool_result_children_and_data_urls() {
    for t in [1.5, -0.1] {
        let mut r = chat_request();
        r.sampling.temperature = Some(t);
        assert_eq!(feature(r), format!("temperature {t}"));
    }
    let mut r = chat_request();
    r.sampling.temperature = Some(1.0);
    AnthropicMessages::check(&Request::Generate(r), &crate::test_support::egress()).unwrap();

    let mut r = chat_request();
    let mut result = kaeso_kpp::ToolResultBlock::new("c", vec![]);
    result
        .content
        .push(Block::Refusal(kaeso_kpp::RefusalBlock::new("no")));
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::tool_call("c", "f", raw("{}"))],
    ));
    r.messages
        .push(Message::new(Role::User, vec![Block::ToolResult(result)]));
    assert_eq!(feature(r), "refusal block in a tool result");

    let with_url = |url: &str, doc: bool| {
        let mut r = chat_request();
        let source = Source::Url(url.to_owned());
        r.messages[0].blocks.push(if doc {
            Block::Document(DocumentBlock::new(source))
        } else {
            Block::image(source)
        });
        r
    };
    let body = encode_value(with_url("data:image/png;base64,AAAA", false));
    assert_eq!(
        body["messages"][0]["content"][1],
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}})
    );
    let body = encode_value(with_url("data:application/pdf;base64,JVBE", true));
    assert_eq!(
        body["messages"][0]["content"][1]["source"],
        json!({"type": "base64", "media_type": "application/pdf", "data": "JVBE"})
    );
    assert_eq!(
        feature(with_url("data:image/png,raw", false)),
        "data URL without base64 encoding"
    );
    assert_eq!(
        feature(with_url("data:image/bmp;base64,AA", false)),
        "image media type `image/bmp`"
    );
    assert_eq!(
        feature(with_url("data:text/html;base64,AA", true)),
        "document media type `text/html`"
    );
}

#[test]
fn foreign_reasoning_becomes_thinking_with_a_blob_in_streams() {
    let origin = DialectId::OpenAiResponses;
    let mut header = kaeso_kpp::ReasoningHeader::new(origin);
    header.opaque.push(kaeso_kpp::Opaque::new(
        origin,
        OpaqueKind::OpenAiReasoningItemId,
        "rs_1",
    ));
    let out = encode_stream(&[
        StreamEvent::ResponseStart(ResponseStart::new("r", "m")),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Reasoning(header))),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::Opaque(kaeso_kpp::Opaque::new(
                origin,
                OpaqueKind::OpenAiEncryptedReasoning,
                "gAAA",
            )),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("x".into()))),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]);
    let content = &accumulate(&out)["content"];
    assert_eq!(content[0]["type"], "thinking", "{content}");
    assert_eq!(content[0]["thinking"], "");
    assert_eq!(content[1], json!({"type": "text", "text": "x"}));
    let block =
        crate::continuity::unwrap_reasoning(content[0]["signature"].as_str().unwrap()).unwrap();
    assert!(block.origin.as_ref().unwrap().is(origin));
    assert_eq!(
        block
            .opaque
            .iter()
            .map(|o| (o.kind, o.data.as_str()))
            .collect::<Vec<_>>(),
        [
            (OpaqueKind::OpenAiReasoningItemId, "rs_1"),
            (OpaqueKind::OpenAiEncryptedReasoning, "gAAA")
        ]
    );
}

#[test]
fn sampling_refusals_do_not_apply_to_token_counting() {
    let mut r = chat_request();
    r.sampling.seed = Some(1);
    r.sampling.presence_penalty = Some(0.5);
    r.sampling.temperature = Some(1.7);
    AnthropicMessages::check(
        &Request::CountTokens(r.clone()),
        &crate::test_support::egress(),
    )
    .unwrap();
    assert_eq!(feature(r), "seed");
}

#[test]
fn malformed_versions_are_not_newer() {
    use super::super::request::is_newer_version;
    assert!(is_newer_version("2027-01-15"));
    assert!(!is_newer_version("2027-13-01"));
    assert!(!is_newer_version("2027-00-10"));
    assert!(!is_newer_version("2027-01-32"));
    assert!(!is_newer_version("9999-99-99"));
    assert!(!is_newer_version("2023-01-01"));
}

#[test]
fn messages_name_things_the_user_knows() {
    let req = Request::Embed(kaeso_kpp::EmbedRequest::new(
        "m",
        vec![kaeso_kpp::EmbedInput::Text("x".into())],
    ));
    let e = AnthropicMessages::check(&req, &EgressCtx::new(&req)).unwrap_err();
    assert!(!e.message.contains("Embed request"), "{}", e.message);
    assert!(e.message.contains("embeddings request"), "{}", e.message);
    let e = AnthropicMessages::decode_request(RequestKind::Embed, b"{}", &HeaderList::new());
    assert!(e.is_err());
    let reply = kaeso_kpp::Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![]));
    let e = AnthropicMessages::encode_response(
        &reply,
        &EncodeCtx::new(RequestKind::Embed, StreamMode::Off),
    )
    .unwrap_err();
    assert!(e.message.contains("embeddings response"), "{}", e.message);
    assert!(!e.message.ends_with(". "), "{}", e.message);
}

#[test]
fn tool_arguments_that_are_not_an_object_are_refused_in_replies() {
    for arguments in [raw(r#""{\"city\": \"Par""#), raw("[1]")] {
        let mut r = kaeso_kpp::Response::new(
            "id",
            "m",
            vec![
                Block::tool_call("call_1", "get_weather", arguments),
                Block::tool_call("call_3", "g", raw(r#"{"ok":true}"#)),
            ],
        );
        r.stop = StopReason::MaxTokens;
        r.extras = Extras::new(CHAT);
        let err = AnthropicMessages::encode_response(
            &kaeso_kpp::Reply::Generate(r),
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
        )
        .unwrap_err();
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::ProviderError);
        assert!(err.message.contains("`get_weather`"), "{}", err.message);
    }
    let mut r = kaeso_kpp::Response::new(
        "id",
        "m",
        vec![Block::tool_call("call_3", "g", raw(r#"{"ok":true}"#))],
    );
    r.extras = Extras::new(CHAT);
    let body = encode_reply(&kaeso_kpp::Reply::Generate(r));
    assert_eq!(body["content"][0]["input"], json!({"ok": true}));
}

#[test]
fn streamed_tool_arguments_that_do_not_form_an_object_are_sent_as_they_came() {
    use kaeso_kpp::ToolCallHeader;
    let tool = |i: u32, id: &str| {
        StreamEvent::BlockStart(BlockStart::new(
            i,
            BlockHeader::ToolCall(ToolCallHeader::new(id, "get_weather")),
        ))
    };
    let args = |i: u32, s: &str| {
        StreamEvent::BlockDelta(BlockDelta::new(i, Delta::ToolArguments(s.into())))
    };
    let out = encode_stream(&[
        StreamEvent::ResponseStart(ResponseStart::new("r", "m")),
        tool(0, "call_a"),
        args(0, "{\"city\": "),
        args(0, "\"Par"),
        tool(1, "call_b"),
        args(1, "{\"city\":"),
        args(1, "\"Oslo\"}"),
        tool(2, "call_c"),
        args(2, ""),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::MaxTokens), None)),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ]);
    let partial: Vec<String> = crate::sse::parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            crate::sse::SseItem::Event(e) => {
                let v: Value = serde_json::from_str(&e.data).ok()?;
                Some((
                    v["index"].as_u64()?,
                    v["delta"]["partial_json"].as_str()?.to_owned(),
                ))
            }
            _ => None,
        })
        .map(|(i, p)| format!("{i}:{p}"))
        .collect();
    assert_eq!(partial, ["0:{\"city\": \"Par", "1:{\"city\":\"Oslo\"}"]);
    let own = format!(
        "{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"f\",\"input\":{{}}}}}}\n\n\
event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":\"{{\\\"a\\\": \"}}}}\n\n\
{BLOCK_STOP}event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"max_tokens\",\"stop_sequence\":null}},\"usage\":{{\"output_tokens\":5}}}}\n\n{STOP}"
    );
    let events = decode_stream_chunked(own.as_bytes(), 0, &stream_ctx());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
    crate::test_support::assert_sse_eq("own cut", own.as_bytes(), &encode_stream(&events));
}

#[test]
fn foreign_summary_asks_for_summarized_thinking_and_include_encrypted_is_a_hint() {
    let mut r = chat_request();
    let mut c = ReasoningConfig::default();
    c.effort = Some(ReasoningEffort::High);
    c.summary = Some(ReasoningSummary::Auto);
    c.include_encrypted = true;
    c.extras = Extras::new(DialectId::OpenAiResponses);
    r.reasoning = Some(c);
    let body = encode_value(r);
    assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
    assert_eq!(body["output_config"], json!({"effort": "high"}));
}

#[test]
fn foreign_effort_turns_on_adaptive_thinking() {
    let with = |e: ReasoningEffort| {
        let mut r = chat_request();
        let mut c = ReasoningConfig::default();
        c.effort = Some(e);
        c.extras = Extras::new(CHAT);
        r.reasoning = Some(c);
        r
    };
    for (e, wire) in [
        (ReasoningEffort::Low, "low"),
        (ReasoningEffort::Medium, "medium"),
        (ReasoningEffort::XHigh, "xhigh"),
    ] {
        let body = encode_value(with(e));
        assert_eq!(body["thinking"], json!({"type": "adaptive"}));
        assert_eq!(body["output_config"]["effort"], wire);
    }
    let body = encode_value(with(ReasoningEffort::None));
    assert!(body.get("thinking").is_none() && body.get("output_config").is_none());
    assert_eq!(
        feature(with(ReasoningEffort::Minimal)),
        "reasoning effort `minimal`"
    );
    let r = generate(
        json!({"model":"m","max_tokens":5,"output_config":{"effort":"low"},
        "messages":[{"role":"user","content":"x"}]}),
    );
    let body = encode_value(r);
    assert!(body.get("thinking").is_none());
}

#[test]
fn thinking_is_left_off_for_a_tool_turn_without_thinking() {
    let tool_loop = |assistant: Vec<Block>, reasoning: ReasoningConfig| {
        let mut r = chat_request();
        r.reasoning = Some(reasoning);
        r.messages.push(Message::new(Role::Assistant, assistant));
        r.messages.push(Message::new(
            Role::User,
            vec![Block::tool_result("call_1", vec![Block::text("ok")], false)],
        ));
        r
    };
    let foreign_effort = || {
        let mut c = ReasoningConfig::default();
        c.effort = Some(ReasoningEffort::High);
        c.extras = Extras::new(CHAT);
        c
    };
    let mut openai_reasoning = ReasoningBlock::new(DialectId::OpenAiResponses);
    openai_reasoning.summary.push("thought".into());
    let body = encode_value(tool_loop(
        vec![
            Block::Reasoning(openai_reasoning),
            Block::tool_call("call_1", "f", raw("{}")),
        ],
        foreign_effort(),
    ));
    assert_eq!(body["thinking"], json!({"type": "adaptive"}), "{body}");
    assert_eq!(body["output_config"]["effort"], "high", "{body}");
    let mut budget = ReasoningConfig::default();
    budget.budget_tokens = Some(2048);
    let body = encode_value(tool_loop(
        vec![Block::tool_call("call_1", "f", raw("{}"))],
        budget,
    ));
    assert!(body.get("thinking").is_none(), "{body}");
    let mut signed = ReasoningBlock::new(A);
    signed.text.push("t".into());
    signed.opaque.push(kaeso_kpp::Opaque::new(
        A,
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    let body = encode_value(tool_loop(
        vec![
            Block::Reasoning(signed),
            Block::tool_call("call_1", "f", raw("{}")),
        ],
        foreign_effort(),
    ));
    assert_eq!(body["thinking"], json!({"type": "adaptive"}));
    let mut r = tool_loop(
        vec![Block::tool_call("call_1", "f", raw("{}"))],
        foreign_effort(),
    );
    r.messages
        .push(Message::new(Role::Assistant, vec![Block::text("done")]));
    r.messages.push(Message::user("next"));
    assert_eq!(encode_value(r)["thinking"], json!({"type": "adaptive"}));
}

#[test]
fn budget_thinking_stays_on_when_an_app_carries_thinking_back() {
    let mut signed = ReasoningBlock::new(A);
    signed.text.push("Weather first.".into());
    signed.opaque.push(kaeso_kpp::Opaque::new(
        A,
        OpaqueKind::AnthropicThinkingSignature,
        "EqQBsig",
    ));
    let blob = crate::continuity::wrap_reasoning(&signed);
    let body = json!({"model": "claude-haiku-4-5", "max_output_tokens": 4096,
    "reasoning": {"effort": "medium"},
    "input": [
        {"role": "user", "content": "Weather?"},
        {"type": "reasoning", "id": "rs_kaeso_msg01_0", "summary": [
            {"type": "summary_text", "text": "Weather first."}], "encrypted_content": blob},
        {"type": "function_call", "id": "fc_kaeso_msg01_1", "call_id": "toolu_1",
            "name": "get_weather", "arguments": "{}", "status": "completed"},
        {"type": "function_call_output", "call_id": "toolu_1", "output": "sunny"}
    ]});
    let req = crate::openai_responses::OpenAiResponses::decode_request(
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req).with_adaptive_thinking(false);
    let enc = AnthropicMessages::encode_request(&req, &mut ctx).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["thinking"]["type"], "enabled", "{body}");
    assert_eq!(
        body["messages"][1]["content"][0],
        json!({"type": "thinking", "thinking": "Weather first.", "signature": "EqQBsig"})
    );
    let mut stripped = req.clone();
    if let Request::Generate(r) = &mut stripped {
        r.messages[1]
            .blocks
            .retain(|b| !matches!(b, Block::Reasoning(_)));
    }
    let mut ctx = EgressCtx::new(&stripped).with_adaptive_thinking(false);
    let enc = AnthropicMessages::encode_request(&stripped, &mut ctx).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert!(body.get("thinking").is_none(), "{body}");
}

#[test]
fn a_system_message_after_tool_results_still_counts_as_the_tool_loop_r24() {
    let r = generate(json!({"model":"m","max_tokens":4096,
    "thinking":{"type":"enabled","budget_tokens":2048},
    "messages":[
        {"role":"user","content":"x"},
        {"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"f","input":{}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}]},
        {"role":"system","content":"note"}
    ]}));
    let body = encode_value(r);
    assert!(body.get("thinking").is_none(), "{body}");
    assert_eq!(body["messages"][3]["role"], "system");
}

#[test]
fn count_tokens_skips_output_and_reasoning_refusals() {
    let mut r = chat_request();
    r.output = OutputSpec::JsonObject;
    let mut c = ReasoningConfig::default();
    c.effort = Some(ReasoningEffort::Minimal);
    c.extras = Extras::new(CHAT);
    r.reasoning = Some(c);
    let req = Request::CountTokens(r.clone());
    AnthropicMessages::check(&req, &EgressCtx::new(&req)).unwrap();
    let (enc, _) = encode(&req);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert!(body.get("output_config").is_none(), "{body}");
    assert!(body.get("thinking").is_none(), "{body}");
    assert_eq!(feature(r), "json_object output");
    let mut r = chat_request();
    r.messages[0]
        .blocks
        .push(Block::image(Source::base64("image/bmp", "AA")));
    assert!(
        AnthropicMessages::check(&Request::CountTokens(r), &crate::test_support::egress()).is_err()
    );
}

#[test]
fn proxy_statuses_408_and_503_are_classified() {
    let e = AnthropicMessages::decode_error(408, &HeaderList::new(), b"Request Timeout");
    assert_eq!(e.kind, ErrorKind::Timeout);
    let e = AnthropicMessages::decode_error(503, &HeaderList::new(), b"<html>busy</html>");
    assert_eq!(e.kind, ErrorKind::Overloaded);
    let body = br#"{"type":"error","error":{"type":"api_error","message":"x"}}"#;
    let e = AnthropicMessages::decode_error(503, &HeaderList::new(), body);
    assert_eq!(e.kind, ErrorKind::ProviderError);
}

#[test]
fn decode_errors_name_the_field_without_parser_positions() {
    let cases: [(&[u8], &str, Option<&str>); 7] = [
        (b"{not json", "not valid JSON", None),
        (b"[1]", "must be a JSON object", None),
        (br#"{"max_tokens":1,"messages":[]}"#, "`model` is required", Some("model")),
        (
            br#"{"model":"m","messages":[{"role":"user","content":"Hi"}]}"#,
            "`max_tokens` is required",
            Some("max_tokens"),
        ),
        (
            br#"{"model":"m","max_tokens":"many","messages":[]}"#,
            "`max_tokens` must be a non-negative integer",
            Some("max_tokens"),
        ),
        (
            br#"{"model":"m","max_tokens":1,"messages":[{"role":"user"}]}"#,
            "message 0 needs `content`",
            Some("messages[0]"),
        ),
        (
            br#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":[{"type":"text","text":5}]}]}"#,
            "invalid `text` block",
            Some("messages[0].content"),
        ),
    ];
    for (body, needle, param) in cases {
        let e = AnthropicMessages::decode_request(RequestKind::Generate, body, &HeaderList::new())
            .unwrap_err();
        assert!(e.message.contains(needle), "{}", e.message);
        assert!(!e.message.contains(" line "), "{}", e.message);
        assert!(!e.message.contains("column"), "{}", e.message);
        assert_eq!(e.param.as_deref(), param, "{}", e.message);
    }
    let e = AnthropicMessages::decode_response(
        200,
        &HeaderList::new(),
        br#"{"id":"m","model":"x","content":5,"usage":{}}"#,
        &generate_ctx(),
    )
    .unwrap_err();
    assert!(!e.message.contains("column"), "{}", e.message);
    let events = decode_stream_chunked(
        format!(
            "{START}event: content_block_start\ndata: {{\"type\":\"content_block_start\"}}\n\n"
        )
        .as_bytes(),
        0,
        &stream_ctx(),
    );
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!("{events:?}")
    };
    assert!(!e.message.contains("column"), "{}", e.message);
}

#[test]
fn foreign_effort_becomes_a_budget_on_models_without_adaptive_thinking() {
    let send = |effort: ReasoningEffort, max_tokens: u32, adaptive: Option<bool>| {
        let mut r = chat_request();
        r.sampling.max_output_tokens = Some(max_tokens);
        let mut c = ReasoningConfig::default();
        c.effort = Some(effort);
        c.extras = Extras::new(CHAT);
        r.reasoning = Some(c);
        let req = Request::Generate(r);
        let mut ctx = EgressCtx::new(&req);
        ctx.adaptive_thinking = adaptive;
        AnthropicMessages::encode_request(&req, &mut ctx)
            .map(|enc| serde_json::from_slice::<Value>(&enc.body).unwrap())
    };
    let thinking = |v: &Value| v["thinking"].clone();
    let b = send(ReasoningEffort::Low, 64_000, Some(false)).unwrap();
    assert_eq!(
        thinking(&b),
        json!({"type": "enabled", "budget_tokens": 2048})
    );
    assert!(b.get("output_config").is_none(), "{b}");
    let b = send(ReasoningEffort::Medium, 64_000, Some(false)).unwrap();
    assert_eq!(thinking(&b)["budget_tokens"], 8192);
    for e in [
        ReasoningEffort::High,
        ReasoningEffort::XHigh,
        ReasoningEffort::Max,
    ] {
        let b = send(e, 64_000, Some(false)).unwrap();
        assert_eq!(thinking(&b)["budget_tokens"], 24_576);
    }
    let b = send(ReasoningEffort::High, 16_000, Some(false)).unwrap();
    assert_eq!(thinking(&b)["budget_tokens"], 15_999);
    assert_eq!(b["max_tokens"], 16_000);
    let b = send(ReasoningEffort::Low, 1_024, Some(false)).unwrap();
    assert_eq!(thinking(&b)["budget_tokens"], 1_024);
    assert_eq!(b["max_tokens"], 2_048);
    for adaptive in [Some(true), None] {
        let b = send(ReasoningEffort::Medium, 64_000, adaptive).unwrap();
        assert_eq!(thinking(&b), json!({"type": "adaptive"}));
        assert_eq!(b["output_config"]["effort"], "medium");
    }
    let b = send(ReasoningEffort::None, 64_000, Some(false)).unwrap();
    assert!(b.get("thinking").is_none(), "{b}");
}

#[test]
fn direct_caller_on_tool_use_is_presentation_other_callers_are_extras() {
    let body = |caller: &str| {
        format!(
            r#"{{"model":"m","max_tokens":10,"messages":[{{"role":"user","content":"q"}},{{"role":"assistant","content":[{{"type":"tool_use","id":"toolu_1","name":"f","input":{{}},"caller":{caller}}}]}},{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}}]}}]}}"#
        )
    };
    let direct = body(r#"{"type":"direct"}"#);
    let req = AnthropicMessages::decode_request(
        RequestKind::Generate,
        direct.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let out = AnthropicMessages::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    crate::test_support::assert_json_eq("direct caller", direct.as_bytes(), &out.body);
    crate::dispatch::check(DialectId::OpenAiChat, &req, &EgressCtx::new(&req)).unwrap();
    let other = body(r#"{"type":"code_execution_20250825","tool_id":"srvtoolu_1"}"#);
    let req = AnthropicMessages::decode_request(
        RequestKind::Generate,
        other.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let e = crate::dispatch::check(DialectId::OpenAiChat, &req, &EgressCtx::new(&req)).unwrap_err();
    assert!(
        e.unsupported_feature().unwrap().ends_with("caller"),
        "{e:?}"
    );
}

#[test]
fn an_empty_text_from_another_dialect_is_not_sent() {
    let body = json!({"model": "claude-sonnet-4-6", "max_tokens": 100, "messages": [
        {"role": "user", "content": "weather?"},
        {"role": "assistant", "content": "", "tool_calls": [{"id": "toolu_1", "type": "function",
            "function": {"name": "get_weather", "arguments": "{}"}}]},
        {"role": "tool", "tool_call_id": "toolu_1", "content": "sunny"}
    ]});
    let req = crate::openai_chat::OpenAiChat::decode_request(
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(r) = req else {
        panic!("{req:?}")
    };
    let body = encode_value(r);
    assert_eq!(
        body["messages"][1]["content"],
        json!([{"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {}}]),
        "{body}"
    );
    let r = generate(json!({"model": "m", "max_tokens": 5, "messages": [
        {"role": "user", "content": [{"type": "text", "text": ""}]}
    ]}));
    assert_eq!(
        encode_value(r)["messages"][0]["content"],
        json!([{"type": "text", "text": ""}])
    );
}
