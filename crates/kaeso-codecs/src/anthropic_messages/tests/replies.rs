use super::*;

pub(super) fn generate_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider("anthropic")
}

pub(super) fn decode_reply(name: &str) -> kaeso_kpp::Response {
    let (_, body) = fixtures(DIALECT, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {name}"));
    match AnthropicMessages::decode_response(200, &HeaderList::new(), &body, &generate_ctx()) {
        Ok(kaeso_kpp::Reply::Generate(r)) => r,
        other => panic!("{other:?}"),
    }
}

pub(super) fn encode_reply(reply: &kaeso_kpp::Reply) -> Value {
    let ctx = EncodeCtx::new(reply.kind(), StreamMode::Off);
    serde_json::from_slice(&AnthropicMessages::encode_response(reply, &ctx).unwrap()).unwrap()
}

#[test]
fn every_response_fixture_round_trips() {
    let mut n = 0;
    for (name, body) in fixtures(DIALECT, "responses", "json") {
        let reply =
            AnthropicMessages::decode_response(200, &HeaderList::new(), &body, &generate_ctx())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
        let out = AnthropicMessages::encode_response(&reply, &ctx).unwrap();
        assert_json_eq(&name, &body, &out);
        n += 1;
    }
    assert!(n >= 28, "{n}");
}

#[test]
fn every_count_tokens_response_fixture_round_trips() {
    let ctx = EgressCtx::for_kind(RequestKind::CountTokens, StreamMode::Off);
    for (name, body) in fixtures(DIALECT, "responses/count_tokens", "json") {
        let reply = AnthropicMessages::decode_response(200, &HeaderList::new(), &body, &ctx)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let kaeso_kpp::Reply::CountTokens(c) = &reply else {
            panic!("{name}")
        };
        assert!(c.input_tokens > 0);
        let out = AnthropicMessages::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
    let reply = kaeso_kpp::Reply::CountTokens(kaeso_kpp::CountTokensResponse::new(7));
    assert_eq!(encode_reply(&reply), json!({"input_tokens": 7}));
}

#[test]
fn usage_follows_d1_with_cache_breakdown_and_raw() {
    let r = decode_reply("system-blocks-cache-ttl");
    let u = &r.usage;
    assert_eq!(
        (
            u.input_tokens,
            u.cache_read_tokens,
            u.cache_write_tokens,
            u.output_tokens
        ),
        (14, 0, 6283, 27)
    );
    let b = u.cache_write_breakdown.unwrap();
    assert_eq!((b.ephemeral_5m, b.ephemeral_1h), (2187, 4096));
    assert_eq!(u.total_input_tokens(), 14 + 6283);
    let raw = u.raw.as_ref().unwrap();
    assert_eq!(raw.origin, Origin::new(A).with_provider("anthropic"));
    assert_eq!(raw.value["service_tier"], "standard");

    let u = decode_reply("thinking-enabled-budget").usage;
    assert_eq!((u.output_tokens, u.reasoning_tokens), (612, Some(287)));
    let u = decode_reply("web-search-server-tool").usage;
    assert_eq!(
        u.raw_for(A).unwrap()["server_tool_use"]["web_search_requests"],
        1
    );
    let u = decode_reply("cache-prewarm").usage;
    assert!(u.raw_for(A).unwrap()["iterations"].is_array());
    assert_eq!(u.cache_write_tokens, 5120);
}

#[test]
fn stop_reasons_are_typed() {
    use kaeso_kpp::StopReason as S;
    for (name, stop) in [
        ("text-simple", S::EndTurn),
        ("max-tokens", S::MaxTokens),
        ("tools-single", S::ToolUse),
        ("pause-turn", S::PauseTurn),
        ("refusal", S::Refusal),
        ("sampling-stop-sequences", S::StopSequence("END".into())),
    ] {
        assert_eq!(decode_reply(name).stop, stop, "{name}");
    }
    let r = decode_reply("refusal");
    assert!(r.blocks.is_empty());
    assert!(r.extras.fields["stop_details"].get().contains("cyber"));

    let body = json!({"id":"msg_1","type":"message","role":"assistant","content":[],"model":"m",
        "stop_reason":"model_context_window_exceeded","stop_sequence":null,"stop_details":null,
        "usage":{"input_tokens":1,"output_tokens":0}});
    let bytes = serde_json::to_vec(&body).unwrap();
    let reply =
        AnthropicMessages::decode_response(200, &HeaderList::new(), &bytes, &generate_ctx())
            .unwrap();
    let kaeso_kpp::Reply::Generate(r) = &reply else {
        panic!()
    };
    assert_eq!(r.stop, S::MaxTokens);
    assert_eq!(encode_reply(&reply), body);
}

#[test]
fn unusual_stop_spellings_round_trip() {
    for body in [
        json!({"id":"m","content":[],"model":"x","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}),
        json!({"id":"m","content":[],"model":"x","usage":{"input_tokens":1,"output_tokens":1}}),
        json!({"id":"m","content":[],"model":"x","stop_reason":"stop_sequence","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}),
        json!({"id":"m","content":[],"model":"x","stop_reason":"end_turn","stop_sequence":"odd","usage":{"input_tokens":1,"output_tokens":1}}),
    ] {
        let bytes = serde_json::to_vec(&body).unwrap();
        let reply =
            AnthropicMessages::decode_response(200, &HeaderList::new(), &bytes, &generate_ctx())
                .unwrap();
        assert_eq!(encode_reply(&reply), body);
    }
}

#[test]
fn response_blocks_are_typed() {
    let r = decode_reply("thinking-redacted");
    assert!(matches!(&r.blocks[0], Block::Reasoning(b)
        if b.opaque_of(OpaqueKind::AnthropicThinkingSignature).is_some()
        && b.origin == Some(Origin::new(A).with_provider("anthropic"))));
    assert!(matches!(&r.blocks[1], Block::Reasoning(b)
        if b.opaque_of(OpaqueKind::AnthropicRedactedThinking).is_some()));
    assert!(matches!(&r.blocks[2], Block::ToolCall(c) if c.name == "get_weather"));

    let r = decode_reply("document-text-citations");
    let Block::Text(t) = &r.blocks[1] else {
        panic!()
    };
    assert_eq!(t.citations.len(), 1);
    assert!(t.citations[0].raw.get().contains("char_location"));

    let r = decode_reply("web-search-server-tool");
    assert!(matches!(&r.blocks[1], Block::Unknown(i) if i.raw.get().contains("server_tool_use")));
    assert!(matches!(&r.blocks[2], Block::Unknown(_)));

    let r = decode_reply("unknown-fields");
    assert!(matches!(&r.blocks[0], Block::Unknown(_)));
    assert!(r.extras.fields.contains_key("x_future_field"));
    assert!(r.usage.raw_for(A).unwrap()["x_future_usage"] == 7);

    let r = decode_reply("thinking-display-omitted");
    let Block::Reasoning(b) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(b.text, [""]);
}

#[test]
fn rewritten_tool_ids_and_names_map_back_in_responses() {
    let mut r = chat_request();
    r.tools.push(ToolDef::Function(FunctionTool::new(
        "files.read",
        Some(raw(r#"{"type":"object"}"#)),
    )));
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::tool_call("call:1", "files.read", raw("{}"))],
    ));
    r.messages.push(Message::new(
        Role::User,
        vec![Block::tool_result("call:1", vec![Block::text("x")], false)],
    ));
    let (_, ctx) = encode(&Request::Generate(r));
    let sent_name = ctx.tool_names.keys().next().unwrap().clone();
    let body = json!({"id":"msg_1","type":"message","role":"assistant","model":"m",
        "content":[{"type":"tool_use","id":"toolu_new","name": sent_name,"input":{}}],
        "stop_reason":"tool_use","stop_sequence":null,
        "usage":{"input_tokens":1,"output_tokens":1}});
    let reply = AnthropicMessages::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap();
    let kaeso_kpp::Reply::Generate(resp) = reply else {
        panic!()
    };
    assert!(
        matches!(&resp.blocks[0], Block::ToolCall(c) if c.name == "files.read" && c.id == "toolu_new")
    );
}

#[test]
fn a_foreign_response_encodes_with_message_defaults() {
    use kaeso_kpp::{Citation, RawItem, RefusalBlock, Response, StopReason, Usage};
    let mut text = kaeso_kpp::TextBlock::new("Paris");
    text.citations
        .push(Citation::new(CHAT, raw(r#"{"type":"url_citation"}"#)));
    text.extras = Extras::new(CHAT).with_field("annotations", raw("[]"));
    let mut reasoning = ReasoningBlock::new(DialectId::OpenAiResponses);
    reasoning.summary = vec!["Think".into(), "more".into()];
    reasoning.opaque.push(kaeso_kpp::Opaque::new(
        DialectId::OpenAiResponses,
        OpaqueKind::OpenAiEncryptedReasoning,
        "gAAA",
    ));
    let blob = crate::continuity::wrap_reasoning(&reasoning);
    let mut resp = Response::new(
        "chatcmpl-1",
        "gpt-5",
        vec![
            Block::Reasoning(reasoning),
            Block::Text(text),
            Block::Refusal(RefusalBlock::new("I can't")),
            Block::tool_call("call_1", "f", raw(r#"{"a":1}"#)),
            Block::Unknown(RawItem::new(CHAT, raw(r#"{"type":"audio"}"#))),
            Block::image(Source::Url("https://x/y.png".into())),
            Block::Document(DocumentBlock::new(Source::text("text/plain", "x"))),
            Block::tool_result("call_1", vec![Block::text("ok")], false),
        ],
    );
    resp.stop = StopReason::ContentFilter;
    let mut usage = Usage::new(10, 5);
    usage.cache_read_tokens = 90;
    usage.reasoning_tokens = Some(2);
    usage.raw = Some(kaeso_kpp::RawUsage::new(
        CHAT,
        json!({"prompt_tokens": 100}),
    ));
    resp.usage = usage;
    resp.extras = Extras::new(CHAT).with_field("system_fingerprint", raw("\"fp\""));
    assert_eq!(
        encode_reply(&kaeso_kpp::Reply::Generate(resp)),
        json!({
            "id": "chatcmpl-1", "type": "message", "role": "assistant", "model": "gpt-5",
            "content": [
                {"type": "thinking", "thinking": "Think\n\nmore", "signature": blob},
                {"type": "text", "text": "Paris"},
                {"type": "text", "text": "I can't"},
                {"type": "tool_use", "id": "call_1", "name": "f", "input": {"a": 1}},
            ],
            "stop_reason": "refusal", "stop_sequence": null, "stop_details": null,
            "usage": {"input_tokens": 10, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 90,
                "cache_creation": null, "output_tokens": 5, "output_tokens_details": {"thinking_tokens": 2}},
        })
    );
}

#[test]
fn non_2xx_responses_are_errors() {
    let (_, body) = fixtures(DIALECT, "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "overloaded")
        .unwrap();
    let e = AnthropicMessages::decode_response(529, &HeaderList::new(), &body, &generate_ctx())
        .unwrap_err();
    assert_eq!(e.kind, kaeso_kpp::ErrorKind::Overloaded);
    let e = AnthropicMessages::decode_response(200, &HeaderList::new(), b"{", &generate_ctx())
        .unwrap_err();
    assert_eq!(e.kind, kaeso_kpp::ErrorKind::ProviderError);
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    use kaeso_kpp::ErrorKind as K;
    let mut n = 0;
    for (name, body) in fixtures(DIALECT, "errors", "json") {
        let meta = sidecar(DIALECT, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let headers: HeaderList = meta
            .get("headers")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let err = AnthropicMessages::decode_error(status, &headers, &body);
        assert_eq!(err.status, Some(status), "{name}");
        assert!(err.origin.as_ref().unwrap().is(A));
        let expected = match status {
            400 => K::InvalidRequest,
            401 => K::Authentication,
            402 => K::Billing,
            403 => K::PermissionDenied,
            404 => K::NotFound,
            413 => K::RequestTooLarge,
            429 => K::RateLimited,
            500 => K::ProviderError,
            504 => K::Timeout,
            529 => K::Overloaded,
            other => panic!("{name}: unexpected status {other}"),
        };
        assert_eq!(err.kind, expected, "{name}");
        assert!(!err.message.is_empty());
        let enc = AnthropicMessages::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
        assert_eq!(
            enc.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
        n += 1;
    }
    assert!(n >= 13, "{n}");
}

#[test]
fn errors_from_elsewhere_get_anthropic_shapes_and_statuses() {
    use kaeso_kpp::{ErrorKind as K, KppError};
    let cases = [
        (
            KppError::unsupported_by("top_k", CHAT, ""),
            400,
            "invalid_request_error",
        ),
        (
            KppError::invalid_request("bad"),
            400,
            "invalid_request_error",
        ),
        (
            KppError::new(K::ContextTooLong, "long"),
            400,
            "invalid_request_error",
        ),
        (
            KppError::new(K::Authentication, "a"),
            401,
            "authentication_error",
        ),
        (KppError::new(K::Billing, "b"), 402, "billing_error"),
        (
            KppError::new(K::PermissionDenied, "p"),
            403,
            "permission_error",
        ),
        (KppError::new(K::NotFound, "n"), 404, "not_found_error"),
        (
            KppError::new(K::RequestTooLarge, "r"),
            413,
            "request_too_large",
        ),
        (KppError::new(K::RateLimited, "r"), 429, "rate_limit_error"),
        (KppError::new(K::ProviderError, "p"), 500, "api_error"),
        (KppError::provider("p").with_status(502), 502, "api_error"),
        (KppError::new(K::Timeout, "t"), 504, "timeout_error"),
        (
            KppError::new(K::Overloaded, "o").with_status(503),
            529,
            "overloaded_error",
        ),
    ];
    for (err, status, ty) in cases {
        let err = err
            .with_origin(CHAT)
            .with_raw(json!({"error": {"message": "openai"}}));
        let enc = AnthropicMessages::encode_error(&err);
        assert_eq!(enc.status, status, "{ty}");
        let body: Value = serde_json::from_slice(&enc.body).unwrap();
        assert_eq!(
            body,
            json!({"type": "error", "error": {"type": ty, "message": err.message}})
        );
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
    }
    let err = KppError::new(K::RateLimited, "slow")
        .with_retry_after(std::time::Duration::from_millis(1500));
    assert_eq!(
        AnthropicMessages::encode_error(&err)
            .headers
            .get("retry-after"),
        Some("2")
    );
}

#[test]
fn unknown_error_shapes_become_provider_errors_or_follow_the_status() {
    use kaeso_kpp::ErrorKind as K;
    let e = AnthropicMessages::decode_error(502, &HeaderList::new(), b"<html>Bad gateway</html>");
    assert_eq!((e.kind.clone(), e.status), (K::ProviderError, Some(502)));
    assert_eq!(e.raw, Some(json!("<html>Bad gateway</html>")));
    assert!(e.message.contains("502"));
    let enc = AnthropicMessages::encode_error(&e);
    assert_eq!(enc.status, 502);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["error"]["type"], "api_error");

    let body = br#"{"type":"error","error":{"type":"conflict_error","message":"busy"}}"#;
    let e = AnthropicMessages::decode_error(409, &HeaderList::new(), body);
    assert_eq!(e.kind, K::InvalidRequest);
    assert_eq!(AnthropicMessages::encode_error(&e).status, 409);

    let body = br#"{"type":"error","error":{"type":"future_error","message":"x"}}"#;
    let e = AnthropicMessages::decode_error(529, &HeaderList::new(), body);
    assert_eq!(e.kind, K::Overloaded);
    let e = AnthropicMessages::decode_error(418, &HeaderList::new(), body);
    assert_eq!(e.kind, K::ProviderError);
    assert_json_eq("future", body, &AnthropicMessages::encode_error(&e).body);
}

#[test]
fn embeddings_are_not_an_anthropic_reply() {
    let reply = kaeso_kpp::Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![]));
    let ctx = EncodeCtx::new(RequestKind::Embed, StreamMode::Off);
    assert!(AnthropicMessages::encode_response(&reply, &ctx).is_err());
    let ctx = EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off);
    assert!(AnthropicMessages::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
}
