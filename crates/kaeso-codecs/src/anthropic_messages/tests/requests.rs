use super::*;

fn round_trip_requests(folder: &str, kind: RequestKind, path: &str) -> usize {
    let mut n = 0;
    for (name, body) in fixtures(DIALECT, folder, "json") {
        let headers = headers_for(folder, &name);
        let req = AnthropicMessages::decode_request(kind, &body, &headers)
            .unwrap_or_else(|e| panic!("{name}: decode failed: {e}"));
        let mut ctx = EgressCtx::new(&req).with_provider("anthropic");
        let enc = AnthropicMessages::encode_request(&req, &mut ctx)
            .unwrap_or_else(|e| panic!("{name}: encode failed: {e}"));
        assert_json_eq(&name, &body, &enc.body);
        assert_eq!(enc.path, path, "{name}");
        assert_eq!(enc.method, "POST");
        assert_eq!(
            enc.headers.get("anthropic-version"),
            Some(ANTHROPIC_VERSION)
        );
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
        for (h, v) in headers.iter() {
            assert_eq!(enc.headers.get(h), Some(v), "{name}: header {h}");
        }
        assert!(
            ctx.tool_ids.is_empty() && ctx.tool_names.is_empty(),
            "{name}"
        );
        let again = AnthropicMessages::decode_request(kind, &enc.body, &headers).unwrap();
        assert_eq!(again, req, "{name}");
        n += 1;
    }
    n
}

#[test]
fn every_request_fixture_round_trips() {
    let n = round_trip_requests("requests", RequestKind::Generate, MESSAGES_PATH);
    assert!(n >= 40, "{n}");
}

#[test]
fn every_count_tokens_request_fixture_round_trips() {
    let n = round_trip_requests(
        "requests/count_tokens",
        RequestKind::CountTokens,
        COUNT_TOKENS_PATH,
    );
    assert_eq!(n, 5);
}

#[test]
fn count_tokens_requests_decode_as_count_tokens() {
    let (_, body) = fixtures(DIALECT, "requests/count_tokens", "json")
        .into_iter()
        .find(|(n, _)| n == "full-params")
        .unwrap();
    let Request::CountTokens(r) = decode(RequestKind::CountTokens, &body) else {
        panic!()
    };
    assert!(matches!(r.output, OutputSpec::JsonSchema(_)));
    let c = r.reasoning.unwrap();
    assert_eq!(c.mode, Some(ReasoningMode::Adaptive));
    assert_eq!(c.effort, Some(ReasoningEffort::Low));
    assert_eq!(r.parallel_tool_calls, Some(false));
    assert!(r.extras.fields.contains_key("cache_control"));
    assert!(
        AnthropicMessages::decode_request(RequestKind::Embed, &body, &HeaderList::new()).is_err()
    );
}

#[test]
fn system_and_cache_hints_are_typed() {
    let r = fixture_request("system-blocks-cache-ttl");
    assert_eq!(r.system.len(), 2);
    let ttls: Vec<_> = r
        .system
        .iter()
        .map(|b| b.cache().unwrap().ttl.clone())
        .collect();
    assert_eq!(ttls, [Some(CacheTtl::OneHour), Some(CacheTtl::FiveMinutes)]);
    let ToolDef::Function(f) = &r.tools[0] else {
        panic!()
    };
    assert_eq!(f.cache.as_ref().unwrap().ttl, Some(CacheTtl::OneHour));
    assert!(f.parameters.as_ref().unwrap().get().contains("location"));
    assert_eq!(
        r.messages[0].blocks[0].cache().unwrap().ttl,
        None,
        "cache_control without ttl"
    );

    let r = fixture_request("system-string-multi-turn");
    assert!(matches!(&r.system[..], [Block::Text(t)] if t.text.starts_with("You are")));
    assert_eq!(r.messages.len(), 3);
    assert_eq!(r.messages[1].role, Role::Assistant);
}

#[test]
fn sampling_metadata_and_stream_are_typed() {
    let r = fixture_request("sampling-stop-sequences");
    assert_eq!(r.sampling.max_output_tokens, Some(1024));
    assert_eq!(r.sampling.temperature, Some(0.2));
    assert_eq!(r.sampling.top_k, Some(40));
    assert_eq!(r.sampling.stop, ["END", "STOP"]);
    assert_eq!(fixture_request("sampling-top-p").sampling.top_p, Some(0.9));

    let r = fixture_request("metadata-service-tier");
    assert_eq!(
        r.user.as_deref(),
        Some("13803d75-b4b5-4c3e-b2a2-6f21399b021b")
    );
    assert!(r.extras.fields.contains_key("service_tier"));
    assert!(r.extras.fields.contains_key("inference_geo"));

    let r = fixture_request("thinking-display-omitted");
    assert_eq!(r.stream, StreamMode::on(true));
}

#[test]
fn reasoning_config_is_typed() {
    let c = fixture_request("thinking-enabled-budget")
        .reasoning
        .unwrap();
    assert_eq!(c.mode, Some(ReasoningMode::Enabled));
    assert_eq!(c.budget_tokens, Some(10000));

    let c = fixture_request("thinking-adaptive-effort")
        .reasoning
        .unwrap();
    assert_eq!(c.mode, Some(ReasoningMode::Adaptive));
    assert_eq!(c.effort, Some(ReasoningEffort::High));
    assert!(c.extras.fields.contains_key("display"));

    let c = fixture_request("thinking-disabled").reasoning.unwrap();
    assert_eq!(c.mode, Some(ReasoningMode::Disabled));

    let c = fixture_request("thinking-between-tools").reasoning.unwrap();
    assert_eq!(c.mode, Some(ReasoningMode::Other("between_tools".into())));
    assert_eq!(c.effort, Some(ReasoningEffort::Medium));
}

#[test]
fn tool_choices_are_typed() {
    let r = fixture_request("tool-choice-any-no-parallel");
    assert_eq!(r.tool_choice, Some(ToolChoice::Required));
    assert_eq!(r.parallel_tool_calls, Some(false));
    let r = fixture_request("tool-choice-tool");
    assert_eq!(r.tool_choice, Some(ToolChoice::Tool("get_weather".into())));
    assert_eq!(r.parallel_tool_calls, Some(true));
    assert_eq!(
        fixture_request("tool-choice-none").tool_choice,
        Some(ToolChoice::None)
    );
    let r = fixture_request("tools-single");
    assert_eq!(r.tool_choice, Some(ToolChoice::Auto));
    assert_eq!(r.parallel_tool_calls, None);
    let r = fixture_request("web-search-server-tool");
    assert!(matches!(&r.tools[0], ToolDef::Unknown(i) if i.origin.is(A)));
}

#[test]
fn structured_output_is_typed() {
    let r = fixture_request("structured-output-json-schema");
    let OutputSpec::JsonSchema(spec) = &r.output else {
        panic!("{:?}", r.output)
    };
    assert!(spec.schema.get().contains("plan_interest"));
    assert_eq!(spec.name, None);
}

#[test]
fn content_blocks_are_typed() {
    let r = fixture_request("image-url-and-file");
    let blocks = &r.messages[0].blocks;
    assert!(
        matches!(&blocks[0], Block::Image(i) if matches!(&i.source, Source::Url(u) if u.ends_with(".jpg")))
    );
    assert!(
        matches!(&blocks[1], Block::Image(i) if matches!(&i.source, Source::FileId(f) if f.origin.is(A) && f.id == "file_011CNha8iCJcU1wXNR6q4V8w"))
    );

    let r = fixture_request("document-file-and-text");
    let Block::Document(pdf) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert!(matches!(pdf.source, Source::FileId(_)));
    assert_eq!(pdf.title.as_deref(), Some("Q3 Report"));
    assert!(pdf.citations);
    let Block::Document(text) = &r.messages[0].blocks[1] else {
        panic!()
    };
    assert!(matches!(&text.source, Source::Text(t) if t.media_type == "text/plain"));

    let r = fixture_request("document-pdf-base64");
    let Block::Document(d) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert!(matches!(&d.source, Source::Base64(b) if b.media_type == "application/pdf"));
    assert!(d.context.is_some() && d.citations);

    let r = fixture_request("document-custom-content");
    assert!(matches!(&r.messages[0].blocks[0], Block::Unknown(i) if i.origin.is(A)));

    let r = fixture_request("tool-result-multi-turn");
    let Block::ToolCall(call) = &r.messages[1].blocks[1] else {
        panic!()
    };
    assert_eq!(call.name, "get_weather");
    assert!(call.arguments.get().contains("Paris"));
    let results: Vec<_> = r.messages[2]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolResult(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 4);
    assert!(matches!(
        &results[1].content[..],
        [Block::Text(_), Block::Image(_)]
    ));
    assert!(results[2].is_error);
    assert!(!results[0].is_error);
}

#[test]
fn thinking_blocks_carry_their_opaque_data() {
    let r = fixture_request("thinking-redacted-replay");
    let blocks = &r.messages[1].blocks;
    let Block::Reasoning(thinking) = &blocks[0] else {
        panic!()
    };
    assert!(thinking.text[0].starts_with("Aurora"));
    let sig = thinking
        .opaque_of(OpaqueKind::AnthropicThinkingSignature)
        .unwrap();
    assert!(sig.origin.is(A) && !sig.data.is_empty());
    let Block::Reasoning(redacted) = &blocks[1] else {
        panic!()
    };
    assert!(redacted.text.is_empty());
    assert!(
        redacted
            .opaque_of(OpaqueKind::AnthropicRedactedThinking)
            .is_some()
    );
}

#[test]
fn unknown_fields_and_blocks_are_kept_bound_to_the_dialect() {
    let r = fixture_request("unknown-fields");
    assert!(r.extras.is_from(A));
    assert!(r.extras.fields.contains_key("x_future_field"));
    let Block::Text(t) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert!(t.extras.fields.contains_key("x_future_block_field"));
    assert!(matches!(&r.messages[0].blocks[1], Block::Unknown(_)));

    let r = fixture_request("web-search-result-replay");
    let Block::Text(cited) = &r.messages[1].blocks[4] else {
        panic!()
    };
    assert_eq!(cited.citations.len(), 1);
    assert!(cited.citations[0].origin.is(A));
}

#[test]
fn explicit_nulls_are_presentation_not_semantics() {
    let r = fixture_request("explicit-nulls");
    assert_eq!(r.user, None);
    let Block::Document(d) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert_eq!(
        (d.title.as_ref(), d.context.as_ref(), d.citations),
        (None, None, false)
    );
    assert!(d.extras.fields.is_empty());
    assert!(!d.extras.form.is_empty());
    assert!(r.extras.fields.contains_key("container"));
}

#[test]
fn anthropic_version_is_pinned_unless_a_newer_one_is_forwarded() {
    let body = br#"{"model":"m","max_tokens":5,"messages":[{"role":"user","content":"x"}]}"#;
    let send = |headers: HeaderList| {
        let req = AnthropicMessages::decode_request(RequestKind::Generate, body, &headers).unwrap();
        let (enc, _) = encode(&req);
        (req, enc)
    };
    let (req, enc) = send(HeaderList::new().with("Anthropic-Version", ANTHROPIC_VERSION));
    assert!(
        req.extras_of().fields.is_empty(),
        "the pinned version adds nothing"
    );
    assert_eq!(
        enc.headers.get("anthropic-version"),
        Some(ANTHROPIC_VERSION)
    );

    let (_, enc) = send(HeaderList::new().with("anthropic-version", "2023-01-01"));
    assert_eq!(
        enc.headers.get("anthropic-version"),
        Some(ANTHROPIC_VERSION)
    );
    let (_, enc) = send(HeaderList::new().with("anthropic-version", "2027-01-15"));
    assert_eq!(enc.headers.get("anthropic-version"), Some("2027-01-15"));
    let (_, enc) = send(HeaderList::new().with("anthropic-version", "latest"));
    assert_eq!(
        enc.headers.get("anthropic-version"),
        Some(ANTHROPIC_VERSION)
    );

    let (req, enc) = send(
        HeaderList::new()
            .with("anthropic-beta", "a")
            .with("anthropic-beta", "b")
            .with("authorization", "Bearer secret")
            .with("x-api-key", "secret"),
    );
    assert_eq!(enc.headers.get("anthropic-beta"), Some("a, b"));
    assert_eq!(enc.headers.get("x-api-key"), None);
    assert_eq!(enc.headers.get("authorization"), None);
    assert_eq!(req.extras_of().fields.len(), 1);
    assert_eq!(enc.headers.len(), 3);
}

trait ExtrasOf {
    fn extras_of(&self) -> &Extras;
}

impl ExtrasOf for Request {
    fn extras_of(&self) -> &Extras {
        match self {
            Request::Generate(r) | Request::CountTokens(r) => &r.extras,
            _ => unreachable!(),
        }
    }
}

#[test]
fn a_foreign_request_encodes_from_typed_fields() {
    let mut r = chat_request();
    r.system = vec![Block::text("Be brief.")];
    r.user = Some("user-42".into());
    r.sampling.temperature = Some(0.5);
    r.sampling.stop = vec!["\n\n".into()];
    r.stream = StreamMode::on(false);
    r.reasoning = Some(rc(|c| {
        c.effort = Some(ReasoningEffort::High);
    }));
    let mut f = FunctionTool::new("get.weather", None);
    f.description = Some("Weather".into());
    r.tools.push(ToolDef::Function(f));
    r.tool_choice = Some(ToolChoice::Tool("get.weather".into()));
    r.parallel_tool_calls = Some(false);
    let mut schema = JsonSchemaSpec::new(raw(r#"{"type":"object"}"#));
    schema.name = Some("answer".into());
    schema.strict = Some(true);
    r.output = OutputSpec::JsonSchema(schema);
    r.messages[0].extras = Extras::new(CHAT).with_form("content", json!("string"));

    let (enc, ctx) = encode(&Request::Generate(r));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    let name = ctx.tool_names.keys().next().unwrap().clone();
    assert_eq!(ctx.tool_names[&name], "get.weather");
    assert_eq!(
        body,
        json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 100,
            "system": [{"type": "text", "text": "Be brief.", "cache_control": {"type": "ephemeral"}}],
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi", "cache_control": {"type": "ephemeral"}}]}],
            "stop_sequences": ["\n\n"],
            "stream": true,
            "metadata": {"user_id": "user-42"},
            "output_config": {"effort": "high", "format": {"type": "json_schema", "schema": {"type": "object"}}},
            "thinking": {"type": "adaptive"},
            "tool_choice": {"type": "tool", "name": name, "disable_parallel_tool_use": true},
            "tools": [{"name": name, "description": "Weather", "input_schema": {"type": "object", "properties": {}},
                "cache_control": {"type": "ephemeral"}}],
        })
    );
}

#[test]
fn foreign_tool_ids_and_names_are_rewritten_and_recorded() {
    let mut r = chat_request();
    r.tools.push(ToolDef::Function(FunctionTool::new(
        "lookup:v2",
        Some(raw(r#"{"type":"object"}"#)),
    )));
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::tool_call(
            "call.abc/1",
            "lookup:v2",
            raw(r#"{"q":1}"#),
        )],
    ));
    r.messages.push(Message::new(
        Role::User,
        vec![Block::tool_result(
            "call.abc/1",
            vec![Block::text("ok")],
            false,
        )],
    ));
    let (enc, ctx) = encode(&Request::Generate(r));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    let call = &body["messages"][1]["content"][0];
    let result = &body["messages"][2]["content"][0];
    let sent_id = call["id"].as_str().unwrap();
    assert!(super::common::valid_id(sent_id));
    assert_eq!(result["tool_use_id"], call["id"]);
    assert_eq!(ctx.original_tool_id(sent_id), "call.abc/1");
    let sent_name = call["name"].as_str().unwrap();
    assert_eq!(body["tools"][0]["name"], sent_name);
    assert_eq!(ctx.original_tool_name(sent_name), "lookup:v2");
    assert_eq!(
        result["content"],
        json!([{"type": "text", "text": "ok"}]),
        "foreign content is always an array"
    );
}

#[test]
fn foreign_reasoning_is_dropped_and_own_reasoning_replayed() {
    let mut foreign = ReasoningBlock::new(DialectId::OpenAiResponses);
    foreign.summary.push("thought".into());
    let mut r = chat_request();
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::Reasoning(foreign.clone()), Block::text("answer")],
    ));
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::Reasoning(foreign)],
    ));
    r.messages.push(Message::user("next"));
    let body = encode_value(r);
    assert_eq!(
        body["messages"],
        json!([
            {"role": "user", "content": [{"type": "text", "text": "hi"}]},
            {"role": "assistant", "content": [{"type": "text", "text": "answer"}]},
            {"role": "user", "content": [{"type": "text", "text": "next", "cache_control": {"type": "ephemeral"}}]},
        ])
    );

    let r = fixture_request("thinking-redacted-replay");
    let req = Request::Generate(r);
    let mut ctx = EgressCtx::new(&req).with_provider("bedrock");
    let enc = AnthropicMessages::encode_request(&req, &mut ctx).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert!(
        body["messages"][1]["content"][0]["type"] == "thinking",
        "no provider recorded on app data"
    );
}

#[test]
fn foreign_cache_hints_are_kept_as_anthropic_cache_control() {
    let mut r = chat_request();
    let mut t = kaeso_kpp::TextBlock::new("long context");
    t.cache = Some(kaeso_kpp::CacheHint::new(Some(CacheTtl::OneHour)));
    t.cache.as_mut().unwrap().extras = Extras::new(CHAT).with_field("x", raw("1"));
    r.messages[0].blocks.push(Block::Text(t));
    let body = encode_value(r);
    assert_eq!(
        body["messages"][0]["content"][1],
        json!({"type": "text", "text": "long context", "cache_control": {"type": "ephemeral", "ttl": "1h"}})
    );
}

#[test]
fn foreign_parallel_tool_calls_without_choice_becomes_auto() {
    let mut r = chat_request();
    r.tools
        .push(ToolDef::Function(FunctionTool::new("f", Some(raw("{}")))));
    r.parallel_tool_calls = Some(false);
    assert_eq!(
        encode_value(r.clone())["tool_choice"],
        json!({"type": "auto", "disable_parallel_tool_use": true})
    );
    r.parallel_tool_calls = Some(true);
    assert!(encode_value(r.clone()).get("tool_choice").is_none());
    r.tool_choice = Some(ToolChoice::None);
    r.parallel_tool_calls = Some(false);
    assert_eq!(encode_value(r)["tool_choice"], json!({"type": "none"}));
}

#[test]
fn foreign_count_tokens_drops_generation_only_fields() {
    let mut r = chat_request();
    r.user = Some("u".into());
    r.sampling.temperature = Some(1.0);
    let (enc, _) = encode(&Request::CountTokens(r));
    assert_eq!(enc.path, COUNT_TOKENS_PATH);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body,
        json!({"model": "claude-sonnet-4-6", "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]})
    );
}

#[test]
fn foreign_documents_and_images_map_to_anthropic_sources() {
    let mut r = chat_request();
    let mut doc = DocumentBlock::new(Source::text("text/plain", "notes"));
    doc.title = Some("Notes".into());
    doc.citations = true;
    r.messages[0].blocks.push(Block::Document(doc));
    r.messages[0]
        .blocks
        .push(Block::image(Source::base64("image/png", "AAAA")));
    let body = encode_value(r);
    assert_eq!(
        body["messages"][0]["content"][1],
        json!({"type": "document", "source": {"type": "text", "media_type": "text/plain", "data": "notes"}, "title": "Notes", "citations": {"enabled": true}})
    );
    assert_eq!(
        body["messages"][0]["content"][2],
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}, "cache_control": {"type": "ephemeral"}})
    );
}
