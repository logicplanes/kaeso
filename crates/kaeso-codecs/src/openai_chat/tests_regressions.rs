use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, BlockStart, Delta, DialectId, ErrorKind, GenerateRequest,
    Message, Origin, RawJson, ReasoningBlock, ReasoningConfig, ReasoningEffort, Request,
    RequestKind, ResponseStart, Role, Source, StreamEvent, StreamMode, ToolCallBlock,
};
use serde_json::{Value, json};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::test_support::{assert_json_eq, fixtures};
use crate::testing::{ComparableItem, sse_items};

fn round_trip(body: &Value) -> Request {
    let bytes = serde_json::to_vec(body).unwrap();
    let req =
        OpenAiChat::decode_request(RequestKind::Generate, &bytes, &HeaderList::new()).unwrap();
    let mut ctx = EgressCtx::new(&req);
    let out = OpenAiChat::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{e:?}"));
    assert_json_eq("round trip", &bytes, &out.body);
    req
}

fn encode_foreign(req: GenerateRequest) -> Value {
    let req = Request::Generate(req);
    let mut ctx = EgressCtx::new(&req);
    let out = OpenAiChat::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{e:?}"));
    serde_json::from_slice(&out.body).unwrap()
}

fn anthropic() -> Origin {
    Origin::new(DialectId::AnthropicMessages)
}

fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

#[test]
fn custom_tool_call_and_its_result_round_trip() {
    let req = round_trip(&json!({
        "model": "gpt-5.6-terra",
        "messages": [
            {"role": "user", "content": "Query the orders."},
            {"role": "assistant", "content": null, "tool_calls": [
                {"id": "call_c1", "type": "custom",
                 "custom": {"name": "sql_query", "input": "SELECT id FROM orders"}}
            ]},
            {"role": "tool", "tool_call_id": "call_c1", "content": "[1, 2, 3]"}
        ],
        "tools": [{"type": "custom", "custom": {"name": "sql_query"}}]
    }));
    let Request::Generate(r) = req else { panic!() };
    assert!(matches!(r.messages[1].blocks[0], Block::Unknown(_)));
    assert!(matches!(r.messages[2].blocks[0], Block::ToolResult(_)));
}

#[test]
fn audio_only_assistant_turn_round_trips() {
    let req = round_trip(&json!({
        "model": "gpt-audio-mini",
        "modalities": ["text", "audio"],
        "audio": {"voice": "marin", "format": "wav"},
        "messages": [
            {"role": "user", "content": "Say hello."},
            {"role": "assistant", "audio": {"id": "audio_abc123"}},
            {"role": "user", "content": "Again, slower."}
        ]
    }));
    let Request::Generate(r) = req else { panic!() };
    assert!(r.messages[1].blocks.is_empty());
    assert!(r.messages[1].extras.fields.contains_key("audio"));
}

fn stream_fixture(name: &str) -> Vec<u8> {
    fixtures("openai_chat", "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn decode_stream(bytes: &[u8]) -> Vec<StreamEvent> {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
    let mut decoder = OpenAiChat::stream_decoder(&ctx);
    let mut events = decoder.feed(bytes);
    events.extend(decoder.finish());
    events
}

fn encode_stream(events: &[StreamEvent], include_usage: bool) -> Vec<Value> {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(include_usage));
    let mut encoder = OpenAiChat::stream_encoder(&ctx);
    let mut out = Vec::new();
    for event in events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    sse_items(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            ComparableItem::Event { data, .. } => data,
            ComparableItem::Comment(_) => None,
        })
        .collect()
}

#[test]
fn a_dropped_event_never_puts_a_chunk_after_done() {
    let events = decode_stream(&stream_fixture("tools-parallel"));
    for skip in 0..events.len() {
        let mut fewer = events.clone();
        fewer.remove(skip);
        let frames = encode_stream(&fewer, true);
        if let Some(done) = frames.iter().position(|f| f == "[DONE]") {
            assert_eq!(done, frames.len() - 1, "dropping event {skip}: {frames:?}");
        }
    }
}

#[test]
fn later_chunks_follow_the_typed_model_and_id() {
    let mut events = decode_stream(&stream_fixture("stream-include-usage"));
    let StreamEvent::ResponseStart(start) = &mut events[0] else {
        panic!()
    };
    start.model = "alias-model".into();
    start.id = "chatcmpl-gateway".into();
    let frames = encode_stream(&events, true);
    for frame in frames.iter().filter(|f| f.is_object()) {
        assert_eq!(frame["model"], "alias-model", "{frame}");
        assert_eq!(frame["id"], "chatcmpl-gateway", "{frame}");
    }
    let bytes = String::from_utf8(stream_fixture("stream-include-usage"))
        .unwrap()
        .replacen("\"system_fingerprint\":\"fp_38b8254589\",", "", 1);
    let bytes = bytes.replacen(
        "\"system_fingerprint\":\"fp_38b8254589\"",
        "\"system_fingerprint\":\"fp_other\"",
        2,
    );
    let events = decode_stream(bytes.as_bytes());
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
    let mut encoder = OpenAiChat::stream_encoder(&ctx);
    let mut out = Vec::new();
    for event in &events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    crate::test_support::assert_sse_eq("env", bytes.as_bytes(), &out);
}

#[test]
fn data_urls_with_parameters_are_base64_and_keep_their_spelling() {
    let req = round_trip(&json!({
        "model": "m",
        "messages": [{"role": "user", "content": [
            {"type": "image_url", "image_url": {"url": "data:image/png;name=a.png;base64,AAAA"}},
            {"type": "image_url", "image_url": {"url": "data:image/png;BASE64,BBBB"}},
            {"type": "file", "file": {"filename": "x.pdf", "file_data": "data:application/pdf;name=x.pdf;base64,JVBE"}}
        ]}]
    }));
    let Request::Generate(r) = req else { panic!() };
    let sources: Vec<_> = r.messages[0]
        .blocks
        .iter()
        .map(|b| match b {
            Block::Image(i) => i.source.clone(),
            Block::Document(d) => d.source.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            Source::base64("image/png", "AAAA"),
            Source::base64("image/png", "BBBB"),
            Source::base64("application/pdf", "JVBE")
        ]
    );
}

#[test]
fn non_json_arguments_from_other_dialects_are_sent_as_text() {
    let mut text_args =
        ToolCallBlock::new("toolu_1", "run", RawJson::from_serialize("ls -la").unwrap());
    text_args.extras = kaeso_kpp::Extras::new(anthropic());
    let list_args = ToolCallBlock::new("toolu_2", "sum", raw("[1,2]"));
    let req = GenerateRequest::new("m")
        .with_message(Message::user("go"))
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::ToolCall(text_args), Block::ToolCall(list_args)],
        ));
    let out = encode_foreign(req);
    let calls = &out["messages"][1]["tool_calls"];
    assert_eq!(calls[0]["function"]["arguments"], "ls -la");
    assert_eq!(calls[1]["function"]["arguments"], "[1,2]");
    round_trip(&json!({
        "model": "m",
        "messages": [
            {"role": "user", "content": "x"},
            {"role": "assistant", "content": null, "tool_calls": [{"id": "c", "type": "function",
                "function": {"name": "f", "arguments": "\"just a string\""}}]},
            {"role": "tool", "tool_call_id": "c", "content": "ok"}
        ]
    }));
}

#[test]
fn a_reasoning_only_foreign_turn_keeps_valid_content() {
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("thinking".into());
    let req = GenerateRequest::new("m")
        .with_message(Message::user("hi"))
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::Reasoning(reasoning)],
        ))
        .with_message(Message::user("again"));
    let out = encode_foreign(req);
    assert_eq!(
        out["messages"][1],
        json!({"role": "assistant", "content": ""})
    );
}

#[test]
fn foreign_effort_values_outside_the_spec_are_refused() {
    let mut config = ReasoningConfig::default();
    config.effort = Some(ReasoningEffort::Other("ultra".into()));
    let mut req = GenerateRequest::new("m").with_message(Message::user("x"));
    req.reasoning = Some(config);
    let err =
        OpenAiChat::check(&Request::Generate(req), &crate::test_support::egress()).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("reasoning.effort"));
    round_trip(
        &json!({"model": "m", "messages": [{"role": "user", "content": "x"}], "reasoning_effort": "ultra"}),
    );
}

#[test]
fn named_error_events_are_errors() {
    let events = decode_stream(
        b"data: {\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"},\"finish_reason\":null}]}\n\nevent: error\ndata: {\"error\":{\"message\":\"overloaded\",\"type\":\"service_unavailable_error\",\"param\":null,\"code\":null}}\n\n",
    );
    let Some(StreamEvent::Error(err)) = events.last() else {
        panic!("{events:?}")
    };
    assert_eq!(err.kind, ErrorKind::Overloaded);
    assert_eq!(err.message, "overloaded");
}

#[test]
fn conflicting_fields_say_why_they_block_other_dialects() {
    let req = round_trip(&json!({
        "model": "m",
        "messages": [{"role": "user", "content": "x"}],
        "max_completion_tokens": 100,
        "max_tokens": 50,
        "user": "u1",
        "safety_identifier": "s1"
    }));
    let err = crate::anthropic_messages::AnthropicMessages::check(&req, &EgressCtx::new(&req))
        .unwrap_err();
    let feature = err.unsupported_feature().unwrap();
    assert!(
        feature.contains("differs from max_completion_tokens")
            || feature.contains("alongside safety_identifier"),
        "{feature}"
    );
    let err = OpenAiChat::check(
        &Request::CountTokens(GenerateRequest::new("m")),
        &crate::test_support::egress(),
    )
    .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("token counting"));
    assert!(err.message.contains("Anthropic"), "{}", err.message);
    let embed = Request::Embed(kaeso_kpp::EmbedRequest::new(
        "e",
        vec![kaeso_kpp::EmbedInput::Text("x".into())],
    ));
    let err = OpenAiChat::check(&embed, &EgressCtx::new(&embed)).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("embeddings"));
    assert!(err.message.contains("OpenAI Embeddings"), "{}", err.message);
}

#[test]
fn and_foreign_streams_skip_what_chat_cannot_show() {
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("x", "m")),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Unknown(kaeso_kpp::RawItem::new(
                anthropic(),
                raw(r#"{"type":"server_tool_use"}"#),
            )),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ToolArguments("{\"q\":1}".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("ok".into()))),
    ];
    let frames = encode_stream(&events, false);
    assert_eq!(frames.len(), 3, "{frames:?}");
    assert_eq!(frames[0]["choices"][0]["delta"]["role"], "assistant");
    assert_eq!(frames[1]["choices"][0]["delta"]["content"], "ok");
}

#[test]
fn assistant_annotations_sent_back_by_the_sdk() {
    let req = round_trip(&json!({
        "model": "gpt-5.5",
        "max_completion_tokens": 256,
        "messages": [
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": "Hello!", "refusal": null, "annotations": []},
            {"role": "user", "content": "more"}
        ]
    }));
    let Request::Generate(r) = &req else { panic!() };
    assert!(
        r.messages[1].extras.fields.is_empty(),
        "{:?}",
        r.messages[1].extras
    );
    crate::anthropic_messages::AnthropicMessages::check(&req, &EgressCtx::new(&req)).unwrap();
    crate::openai_responses::OpenAiResponses::check(&req, &EgressCtx::new(&req)).unwrap();

    let req = round_trip(&json!({
        "model": "gpt-5-search-api",
        "max_completion_tokens": 256,
        "messages": [
            {"role": "user", "content": "news?"},
            {"role": "assistant", "content": "A garden opened ([example.com](https://example.com/a)).",
             "annotations": [{"type": "url_citation", "url_citation": {
                 "start_index": 17, "end_index": 54, "title": "Garden", "url": "https://example.com/a"}}]},
            {"role": "user", "content": "more"}
        ]
    }));
    let Request::Generate(r) = &req else { panic!() };
    let Block::Text(text) = &r.messages[1].blocks[0] else {
        panic!()
    };
    assert_eq!(text.citations.len(), 1);
    assert!(text.citations[0].origin.is(DialectId::OpenAiChat));
    let err = crate::anthropic_messages::AnthropicMessages::check(&req, &EgressCtx::new(&req))
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("citations"));
}

#[test]
fn error_codes_match_the_responses_codec() {
    use kaeso_kpp::KppError;
    for kind in [
        ErrorKind::InvalidRequest,
        ErrorKind::Authentication,
        ErrorKind::PermissionDenied,
        ErrorKind::NotFound,
        ErrorKind::RequestTooLarge,
        ErrorKind::RateLimited,
        ErrorKind::Overloaded,
        ErrorKind::ContextTooLong,
        ErrorKind::ContentFiltered,
        ErrorKind::Timeout,
        ErrorKind::Billing,
        ErrorKind::ProviderError,
    ] {
        let err = KppError::new(kind.clone(), "m").with_origin(DialectId::AnthropicMessages);
        let chat = OpenAiChat::encode_error(&err);
        let responses = crate::openai_responses::OpenAiResponses::encode_error(&err);
        assert_eq!(chat.status, responses.status, "{kind:?}");
        let a: Value = serde_json::from_slice(&chat.body).unwrap();
        let b: Value = serde_json::from_slice(&responses.body).unwrap();
        assert_eq!(a["error"]["type"], b["error"]["type"], "{kind:?}");
        assert_eq!(a["error"]["code"], b["error"]["code"], "{kind:?}");
        let back = OpenAiChat::decode_error(chat.status, &HeaderList::new(), &chat.body);
        assert_eq!(back.kind, kind, "{kind:?}");
    }
    let rate = OpenAiChat::encode_error(&KppError::new(ErrorKind::RateLimited, "slow"));
    let body: Value = serde_json::from_slice(&rate.body).unwrap();
    assert_eq!(body["error"]["code"], "rate_limit_exceeded");
}

#[test]
fn decode_errors_name_the_field_without_parser_positions() {
    let decode = |body: &str| {
        OpenAiChat::decode_request(RequestKind::Generate, body.as_bytes(), &HeaderList::new())
            .unwrap_err()
    };
    let err = decode(
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"max_completion_tokens":"many"}"#,
    );
    assert_eq!(err.param.as_deref(), Some("max_completion_tokens"));
    assert!(
        err.message
            .contains("`max_completion_tokens` must be an integer from 0 to 4294967295"),
        "{}",
        err.message
    );
    let err = decode(
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"max_tokens":5000000000}"#,
    );
    assert_eq!(err.param.as_deref(), Some("max_tokens"));
    let err = decode(r#"{"messages":[]}"#);
    assert_eq!(err.param.as_deref(), Some("model"));
    assert!(
        err.message.contains("`model` is required"),
        "{}",
        err.message
    );
    let err = decode(r#"{"model":"m","messages":[{"role":"user","content":"x"}],"stop":[1]}"#);
    assert_eq!(err.param.as_deref(), Some("stop"));
    let err = decode(r#"{"model":"m","model":"n","messages":[]}"#);
    assert!(err.message.contains("duplicate"), "{}", err.message);
    let err = decode(r#"{"model":"m","messages":[{"role":"tool","content":"x"}]}"#);
    assert!(err.message.contains("tool_call_id"), "{}", err.message);
    for err in [
        decode(r#"{"model":"m","messages":[{"role":"tool","content":"x"}]}"#),
        decode(r#"{"model":"m","messages":[],"n":"two"}"#),
        decode("{"),
    ] {
        assert!(!err.message.contains(" line "), "{}", err.message);
        assert!(!err.message.contains("column"), "{}", err.message);
    }
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let err =
        OpenAiChat::decode_response(200, &HeaderList::new(), br#"{"id":1}"#, &ctx).unwrap_err();
    assert!(!err.message.contains("column"), "{}", err.message);
}

#[test]
fn a_402_is_a_billing_error_and_a_plain_string_error_keeps_its_message() {
    let fixture = |name: &str| {
        fixtures("openai_chat", "errors", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no fixture {name}"))
            .1
    };
    let credits = fixture("providers/huggingface/recorded-plain-developer-string");
    let err = OpenAiChat::decode_error(402, &HeaderList::new(), &credits);
    assert_eq!(err.kind, ErrorKind::Billing);
    let message = "You have no remaining credits. Purchase pre-paid credits to continue using Inference Providers. Alternatively, subscribe to PRO to get monthly included credits.";
    assert_eq!(err.message, message);
    assert_eq!(err.raw, Some(serde_json::from_slice(&credits).unwrap()));
    let out = OpenAiChat::encode_error(&err);
    assert_eq!(out.status, 402);
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(
        body,
        json!({"error": {"message": message, "type": "insufficient_quota", "param": null, "code": "insufficient_quota"}})
    );
    let anthropic = crate::dispatch::encode_error(DialectId::AnthropicMessages, &err);
    let body: Value = serde_json::from_slice(&anthropic.body).unwrap();
    assert_eq!(anthropic.status, 402);
    assert_eq!(body["error"]["type"], "billing_error");
    assert_eq!(body["error"]["message"], message);
    let token = fixture("providers/huggingface/recorded-invalid-token");
    let err = OpenAiChat::decode_error(401, &HeaderList::new(), &token);
    assert_eq!(err.kind, ErrorKind::Authentication);
    assert_eq!(err.message, "Invalid username or password.");
    let backend = fixture("providers/huggingface/sdk-tgi-template-error");
    let err = OpenAiChat::decode_error(422, &HeaderList::new(), &backend);
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.message, "Template error: template not found");
    assert_eq!(
        err.raw.as_ref().map(|r| &r["error_type"]),
        Some(&json!("template_error"))
    );
}
