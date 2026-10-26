use super::*;
use crate::EgressCtx;

#[test]
fn check_refuses_foreign_extras_blocks_and_headers() {
    let mut r = chat_request();
    r.extras = Extras::new(CHAT).with_field("logit_bias", raw("{}"));
    assert_eq!(feature(r), "logit_bias");

    let mut r = chat_request();
    r.messages[0]
        .blocks
        .push(Block::Unknown(kaeso_kpp::RawItem::new(
            CHAT,
            raw(r#"{"type":"input_audio"}"#),
        )));
    assert_eq!(feature(r), "input_audio content block");

    let mut r = chat_request();
    r.messages[0]
        .blocks
        .push(Block::image(Source::file_id(CHAT, "file-abc")));
    assert_eq!(feature(r), "file id");
}

#[test]
fn check_refuses_embeddings() {
    let req = Request::Embed(kaeso_kpp::EmbedRequest::new(
        "m",
        vec![kaeso_kpp::EmbedInput::Text("x".into())],
    ));
    assert_eq!(
        AnthropicMessages::check(&req, &EgressCtx::new(&req))
            .unwrap_err()
            .unsupported_feature(),
        Some("embeddings request")
    );
}

#[test]
fn check_requires_max_tokens_for_generation_only() {
    let mut r = chat_request();
    r.sampling.max_output_tokens = None;
    let e = AnthropicMessages::check(
        &Request::Generate(r.clone()),
        &crate::test_support::egress(),
    )
    .unwrap_err();
    assert_eq!(e.kind, kaeso_kpp::ErrorKind::InvalidRequest);
    assert_eq!(e.param.as_deref(), Some("max_completion_tokens"));
    assert_eq!(
        e.message,
        "Anthropic Messages requires an output limit. Set `max_completion_tokens` for the request."
    );
    AnthropicMessages::check(&Request::CountTokens(r), &crate::test_support::egress()).unwrap();
}

#[test]
fn foreign_system_messages_join_a_user_turn() {
    let mut r = chat_request();
    r.messages
        .push(Message::new(Role::System, vec![Block::text("late rule")]));
    let req = Request::Generate(r);
    let enc = AnthropicMessages::encode_request(&req, &mut crate::test_support::egress()).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["messages"],
        json!([{"role": "user", "content": [
            {"type": "text", "text": "hi"},
            {"type": "text", "text": "late rule", "cache_control": {"type": "ephemeral"}}
        ]}])
    );
}

#[test]
fn own_mid_conversation_system_messages_are_sent_unchanged() {
    for body in [
        json!({"model":"m","max_tokens":5,"messages":[
            {"role":"user","content":"hi"},
            {"role":"system","content":[{"type":"text","text":"env","cache_control":{"type":"ephemeral"}}]}
        ]}),
        json!({"model":"m","max_tokens":5,"messages":[{"role":"system","content":"x"}]}),
    ] {
        let r = generate(body.clone());
        assert!(r.messages.iter().any(|m| m.role == Role::System));
        let req = Request::Generate(r);
        AnthropicMessages::check(&req, &EgressCtx::new(&req)).unwrap();
        let enc = AnthropicMessages::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
        assert_json_eq(
            "system message",
            &serde_json::to_vec(&body).unwrap(),
            &enc.body,
        );
    }
    let count = AnthropicMessages::decode_request(
        RequestKind::CountTokens,
        br#"{"model":"m","messages":[{"role":"user","content":"hi"},{"role":"system","content":"x"}]}"#,
        &HeaderList::new(),
    )
    .unwrap();
    AnthropicMessages::check(&count, &EgressCtx::new(&count)).unwrap();
}

#[test]
fn check_refuses_json_mode_and_unexpressible_schemas() {
    let mut r = chat_request();
    r.output = OutputSpec::JsonObject;
    assert_eq!(feature(r.clone()), "json_object output");
    let mut spec = JsonSchemaSpec::new(raw("{}"));
    spec.description = Some("d".into());
    r.output = OutputSpec::JsonSchema(spec.clone());
    assert_eq!(feature(r.clone()), "output schema description");
    spec.description = None;
    spec.strict = Some(false);
    r.output = OutputSpec::JsonSchema(spec.clone());
    assert_eq!(feature(r.clone()), "non-strict output schema");
    spec.strict = Some(true);
    spec.name = Some("dropped".into());
    r.output = OutputSpec::JsonSchema(spec);
    AnthropicMessages::check(&Request::Generate(r), &crate::test_support::egress()).unwrap();
}

#[test]
fn check_refuses_openai_sampling() {
    for (name, set) in [
        (
            "seed",
            (|r: &mut GenerateRequest| r.sampling.seed = Some(1)) as fn(&mut _),
        ),
        ("frequency_penalty", |r| {
            r.sampling.frequency_penalty = Some(0.5)
        }),
        ("presence_penalty", |r| {
            r.sampling.presence_penalty = Some(0.5)
        }),
    ] {
        let mut r = chat_request();
        set(&mut r);
        assert_eq!(feature(r), name);
    }
}

#[test]
fn check_refuses_reasoning_config_conversions() {
    let with = |c: ReasoningConfig| {
        let mut r = chat_request();
        r.reasoning = Some(c);
        r
    };
    let own = |f: fn(&mut ReasoningConfig)| {
        let mut r = chat_request();
        let mut c = ReasoningConfig::default();
        f(&mut c);
        c.extras = Extras::new(A);
        r.reasoning = Some(c);
        r
    };
    assert_eq!(
        feature(own(|c| c.include_encrypted = true)),
        "reasoning.include_encrypted"
    );
    assert_eq!(
        feature(own(|c| c.summary = Some(ReasoningSummary::Auto))),
        "reasoning.summary"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.effort = Some(ReasoningEffort::Minimal);
        }))),
        "reasoning effort `minimal`"
    );
    let off = with(rc(|c| {
        c.effort = Some(ReasoningEffort::None);
    }));
    let req = Request::Generate(off);
    AnthropicMessages::check(&req, &EgressCtx::new(&req)).unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        &AnthropicMessages::encode_request(&req, &mut EgressCtx::new(&req))
            .unwrap()
            .body,
    )
    .unwrap();
    assert!(body.get("thinking").is_none() && body.get("output_config").is_none());
    assert_eq!(
        feature(with(rc(|c| {
            c.effort = Some(ReasoningEffort::None);
            c.budget_tokens = Some(1024);
        }))),
        "reasoning effort `none`"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.mode = Some(ReasoningMode::Disabled);
            c.budget_tokens = Some(2048);
        }))),
        "reasoning.budget_tokens"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.effort = Some(ReasoningEffort::Other("ultra".into()));
        }))),
        "reasoning effort `ultra`"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.mode = Some(ReasoningMode::Enabled);
            c.effort = Some(ReasoningEffort::High);
        }))),
        "reasoning without budget_tokens"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.mode = Some(ReasoningMode::Adaptive);
            c.budget_tokens = Some(2048);
        }))),
        "reasoning.budget_tokens"
    );
    assert_eq!(
        feature(with(rc(|c| {
            c.mode = Some(ReasoningMode::Other("turbo".into()));
        }))),
        "reasoning mode `turbo`"
    );
    let body = encode_value(with(rc(|c| {
        c.effort = Some(ReasoningEffort::Max);
    })));
    assert_eq!(body["output_config"], json!({"effort": "max"}));
    assert_eq!(body["thinking"], json!({"type": "adaptive"}));
    let body = encode_value(with(rc(|c| {
        c.budget_tokens = Some(2048);
    })));
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 2048})
    );
}

#[test]
fn check_refuses_content_anthropic_cannot_carry() {
    let push = |b: Block| {
        let mut r = chat_request();
        r.messages[0].blocks.push(b);
        r
    };
    assert_eq!(
        feature(push(Block::Refusal(kaeso_kpp::RefusalBlock::new("no")))),
        "refusal block"
    );
    assert_eq!(
        feature(push(Block::image(Source::base64("image/bmp", "AA")))),
        "image media type `image/bmp`"
    );
    assert_eq!(
        feature(push(Block::image(Source::text("text/plain", "x")))),
        "text image source"
    );
    assert_eq!(
        feature(push(Block::Document(DocumentBlock::new(Source::base64(
            "application/msword",
            "AA"
        ))))),
        "document media type `application/msword`"
    );
    assert_eq!(
        feature(push(Block::Document(DocumentBlock::new(Source::text(
            "text/markdown",
            "# x"
        ))))),
        "text document media type `text/markdown`"
    );
    let mut r = chat_request();
    r.messages.push(Message::new(
        Role::Assistant,
        vec![Block::tool_call("c", "f", raw("[1]"))],
    ));
    assert_eq!(feature(r), "tool call arguments that are not a JSON object");
    let mut r = chat_request();
    r.messages.push(Message::new(
        Role::User,
        vec![Block::tool_result(
            "c",
            vec![Block::image(Source::base64("image/tiff", "AA"))],
            false,
        )],
    ));
    assert_eq!(feature(r), "image media type `image/tiff`");
    let mut r = chat_request();
    r.system = vec![Block::image(Source::Url("https://x/y.png".into()))];
    assert_eq!(feature(r), "image block in the system prompt");
    let mut r = chat_request();
    r.tools
        .push(ToolDef::Function(FunctionTool::new("f", Some(raw("true")))));
    assert_eq!(
        feature(r),
        "tool parameters that are not a JSON object schema"
    );
}

#[test]
fn check_accepts_every_request_fixture_except_documented_ones() {
    for (name, body) in fixtures(DIALECT, "requests", "json") {
        let req = decode(RequestKind::Generate, &body);
        AnthropicMessages::check(&req, &EgressCtx::new(&req))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn encode_request_checks_with_the_provider() {
    let r = fixture_request("image-url-and-file");
    let mut img = r.clone();
    if let Block::Image(i) = &mut img.messages[0].blocks[1] {
        i.source = Source::file_id(Origin::new(A).with_provider("anthropic"), "file_x");
    }
    let req = Request::Generate(img);
    let mut ctx = EgressCtx::new(&req).with_provider("bedrock");
    let e = AnthropicMessages::encode_request(&req, &mut ctx).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("file id"));
    assert!(e.message.contains("file_x"));
}
