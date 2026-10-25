use kaeso_kpp::{
    CacheHint, DocumentBlock, FunctionTool, ImageBlock, OpaqueKind, ReasoningBlock,
    ReasoningEffort, ReasoningMode, Role, Source, ToolChoice, ToolDef, ToolResultBlock,
};
use serde_json::json;

use super::*;

fn foreign_request() -> GenerateRequest {
    let a = DialectId::AnthropicMessages;
    let mut req = GenerateRequest::new("gpt-5.5");
    let mut sys = kaeso_kpp::TextBlock::new("Be brief.");
    sys.cache = Some(CacheHint::new(Some(kaeso_kpp::CacheTtl::OneHour)));
    req.system = vec![Block::Text(sys)];
    let mut thinking = ReasoningBlock::new(a);
    thinking.text.push("Let me check the weather.".into());
    thinking.opaque.push(kaeso_kpp::Opaque::new(
        a,
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    let mut doc = DocumentBlock::new(Source::base64("application/pdf", "JVBERi0="));
    doc.cache = Some(CacheHint::default());
    req.messages = vec![
        Message::new(
            Role::User,
            vec![Block::text("Weather in Paris?"), Block::Document(doc)],
        ),
        Message::new(
            Role::Assistant,
            vec![
                Block::Reasoning(thinking),
                Block::text("Checking."),
                Block::tool_call("toolu_01", "get_weather", raw(r#"{"city": "Paris"}"#)),
                Block::tool_call("toolu_02", "get_time", raw("{}")),
            ],
        ),
        Message::new(
            Role::User,
            vec![
                Block::tool_result("toolu_01", vec![Block::text("rain")], false),
                Block::Image(ImageBlock::new(Source::Url("https://x/y.png".into()))),
                Block::tool_result("toolu_02", vec![], false),
                Block::text("Thanks"),
            ],
        ),
    ];
    let mut tool = FunctionTool::new("get_weather", Some(raw(r#"{"type":"object"}"#)));
    tool.cache = Some(CacheHint::default());
    req.tools = vec![
        ToolDef::Function(tool),
        ToolDef::Function(FunctionTool::new("get_time", None)),
    ];
    req.tool_choice = Some(ToolChoice::Auto);
    let mut cfg = kaeso_kpp::ReasoningConfig::default();
    cfg.mode = Some(ReasoningMode::Adaptive);
    cfg.effort = Some(ReasoningEffort::Medium);
    req.reasoning = Some(cfg);
    req.sampling.max_output_tokens = Some(1024);
    req.user = Some("u-1".into());
    req.extras = Extras::new(a);
    req
}

#[test]
fn foreign_requests_become_valid_item_lists() {
    let body = body_of(foreign_request());
    assert_eq!(
        body,
        json!({
            "model": "gpt-5.5",
            "instructions": "Be brief.",
            "include": ["reasoning.encrypted_content"],
            "input": [
                {"role": "user", "content": [
                    {"type": "input_text", "text": "Weather in Paris?"},
                    {"type": "input_file", "filename": "document.pdf", "file_data": "data:application/pdf;base64,JVBERi0="}
                ]},
                {"role": "assistant", "content": "Checking."},
                {"type": "function_call", "call_id": "toolu_01", "name": "get_weather", "arguments": "{\"city\": \"Paris\"}"},
                {"type": "function_call", "call_id": "toolu_02", "name": "get_time", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "toolu_01", "output": "rain"},
                {"role": "user", "content": [{"type": "input_image", "image_url": "https://x/y.png"}]},
                {"type": "function_call_output", "call_id": "toolu_02", "output": ""},
                {"role": "user", "content": "Thanks"}
            ],
            "tools": [
                {"type": "function", "name": "get_weather", "parameters": {"type": "object"}},
                {"type": "function", "name": "get_time"}
            ],
            "tool_choice": "auto",
            "reasoning": {"effort": "medium"},
            "max_output_tokens": 1024,
            "safety_identifier": "u-1",
            "store": false
        })
    );
}

#[test]
fn multi_block_system_prompts_become_a_developer_item() {
    let mut req = GenerateRequest::new("m").with_message(Message::user("hi"));
    req.system = vec![Block::text("a"), Block::text("b")];
    let body = body_of(req);
    assert!(body.get("instructions").is_none());
    assert_eq!(
        body["input"][0],
        json!({"role": "developer", "content": [{"type": "input_text", "text": "a"}, {"type": "input_text", "text": "b"}]})
    );
    let mut req = GenerateRequest::new("m")
        .with_message(Message::new(Role::System, vec![Block::text("late")]));
    req.messages.insert(0, Message::user("hi"));
    assert_eq!(
        body_of(req)["input"][1],
        json!({"role": "developer", "content": "late"})
    );
}

#[test]
fn reasoning_items_replay_only_to_their_origin_and_provider() {
    let mut r = generate("reasoning-encrypted");
    let (enc, _) = encode_req(&Request::Generate(r.clone()));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["input"][1]["type"], "reasoning");
    for m in &mut r.messages {
        for b in &mut m.blocks {
            if let Block::Reasoning(rs) = b {
                rs.origin = Some(Origin::new(R).with_provider("azure"));
                for o in &mut rs.opaque {
                    o.origin = Origin::new(R).with_provider("azure");
                }
            }
        }
    }
    let (enc, _) = encode_req(&Request::Generate(r));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["input"][1]["type"], "function_call");
}

#[test]
fn rejected_tool_names_and_long_ids_are_rewritten_and_mapped_back() {
    let long_id = "toolu_".to_owned() + &"x".repeat(80);
    let mut req = GenerateRequest::new("m")
        .with_tool(ToolDef::Function(FunctionTool::new("weather.lookup", None)))
        .with_message(Message::user("hi"))
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::tool_call(
                long_id.clone(),
                "weather.lookup",
                raw("{}"),
            )],
        ))
        .with_message(Message::new(
            Role::User,
            vec![Block::tool_result(
                long_id.clone(),
                vec![Block::text("ok")],
                false,
            )],
        ));
    req.tool_choice = Some(ToolChoice::Tool("weather.lookup".into()));
    let (enc, ctx) = encode_req(&Request::Generate(req));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    let sent_name = body["tools"][0]["name"].as_str().unwrap().to_owned();
    assert!(sent_name.starts_with("weather_lookup_") && sent_name.len() <= 64);
    assert_eq!(body["tool_choice"]["name"], sent_name);
    assert_eq!(body["input"][1]["name"], sent_name);
    let sent_id = body["input"][1]["call_id"].as_str().unwrap().to_owned();
    assert!(sent_id.len() <= 64);
    assert_eq!(body["input"][2]["call_id"], sent_id);
    let reply = json!({"id": "resp_1", "object": "response", "status": "completed", "model": "m",
        "output": [{"id": "fc_1", "type": "function_call", "status": "completed", "call_id": sent_id, "name": sent_name, "arguments": "{}"}]});
    let Reply::Generate(resp) = OpenAiResponses::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&reply).unwrap(),
        &ctx,
    )
    .unwrap() else {
        panic!()
    };
    let Block::ToolCall(c) = &resp.blocks[0] else {
        panic!()
    };
    assert_eq!(c.name, "weather.lookup");
    assert_eq!(c.id, long_id);
    assert_eq!(resp.stop, kaeso_kpp::StopReason::ToolUse);
}

#[test]
fn non_json_arguments_survive() {
    let body = br#"{"model":"m","input":[{"type":"function_call","call_id":"c","name":"f","arguments":"{\"a\":"},{"type":"function_call_output","call_id":"c","output":"x"}]}"#;
    let req = decode_req(RequestKind::Generate, body);
    let (enc, _) = encode_req(&req);
    assert_json_eq("non-json args", body, &enc.body);
    let Request::Generate(r) = req else { panic!() };
    let Block::ToolCall(c) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert_eq!(c.arguments.get(), r#""{\"a\":""#);
}

#[test]
fn tool_result_contents_from_other_dialects() {
    let mut res = ToolResultBlock::new("c", vec![Block::text("a"), Block::text("b")]);
    res.extras = Extras::new(DialectId::AnthropicMessages);
    let req = GenerateRequest::new("m")
        .with_message(Message::new(
            Role::Assistant,
            vec![Block::tool_call("c", "f", raw("{}"))],
        ))
        .with_message(Message::new(Role::User, vec![Block::ToolResult(res)]));
    let body = body_of(req);
    assert_eq!(
        body["input"][1]["output"],
        json!([{"type": "input_text", "text": "a"}, {"type": "input_text", "text": "b"}])
    );
}

#[test]
fn requests_without_input_and_empty_objects_round_trip() {
    for body in [
        &br#"{"model":"m","prompt":{"id":"pmpt_1","variables":{"x":"y"}}}"#[..],
        br#"{"model":"m","input":[],"text":{},"reasoning":{},"tools":[],"include":[]}"#,
        br#"{"model":"m","input":[{"role":"user","content":[]}]}"#,
        br#"{"model":"m","input":"x","reasoning":null,"include":["reasoning.encrypted_content"]}"#,
    ] {
        let req = decode_req(RequestKind::Generate, body);
        let (enc, _) = encode_req(&req);
        assert_json_eq("edge", body, &enc.body);
    }
}

#[test]
fn decoded_fixtures_are_valid_kpp() {
    for (folder, kind) in [
        ("requests", RequestKind::Generate),
        ("requests/input_tokens", RequestKind::CountTokens),
    ] {
        for (name, body) in fixtures(DIR, folder, "json") {
            if REFUSED_REQUESTS.iter().any(|(n, _)| *n == name) {
                continue;
            }
            decode_req(kind, &body)
                .validate()
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}

#[test]
fn invalid_arguments_from_other_dialects_are_sent_as_their_text() {
    let mut call = kaeso_kpp::ToolCallBlock::new("c", "f", raw(r#""{\"a\":""#));
    call.extras = Extras::new(DialectId::OpenAiChat);
    let req = GenerateRequest::new("m")
        .with_message(Message::user("x"))
        .with_message(Message::new(Role::Assistant, vec![Block::ToolCall(call)]));
    assert_eq!(body_of(req)["input"][1]["arguments"], r#"{"a":"#);
}

#[test]
fn foreign_requests_are_not_stored_and_native_ones_keep_their_spelling() {
    assert_eq!(body_of(foreign_request())["store"], false);
    assert!(
        body_of(generate("text-input-string"))
            .get("store")
            .is_none()
    );
    assert_eq!(body_of(generate("reasoning-encrypted"))["store"], false);
}

#[test]
fn verbosity_medium_is_presentation_other_values_are_extras_that_cross_as_a_hint() {
    let anthropic = Origin::new(DialectId::AnthropicMessages);
    for (value, presentation) in [("medium", true), ("low", false), ("high", false)] {
        let body = format!(r#"{{"model":"m","input":"x","text":{{"verbosity":"{value}"}}}}"#);
        let req = decode_req(RequestKind::Generate, body.as_bytes());
        let (enc, _) = encode_req(&req);
        assert_json_eq(value, body.as_bytes(), &enc.body);
        let Request::Generate(r) = &req else { panic!() };
        assert_eq!(
            r.extras.fields.contains_key("text.verbosity"),
            !presentation,
            "{value}"
        );
        assert!(
            crate::cross::check_foreign(&req, &anthropic).is_ok(),
            "{value}"
        );
    }
}

#[test]
fn reasoning_items_with_a_kaeso_id_and_nothing_else_are_not_sent() {
    let body = br#"{"model":"m","input":[
        {"role":"user","content":"x"},
        {"type":"reasoning","id":"rs_kaeso_r_0","summary":[]},
        {"type":"reasoning","id":"rs_kaeso_r_1","summary":[],"encrypted_content":"gAAAAenc"},
        {"type":"reasoning","id":"rs_real","summary":[]},
        {"role":"assistant","content":"y"}]}"#;
    let req = decode_req(RequestKind::Generate, body);
    let (enc, _) = encode_req(&req);
    let v: Value = serde_json::from_slice(&enc.body).unwrap();
    let input = v["input"].as_array().unwrap();
    assert_eq!(input.len(), 4, "{input:?}");
    assert_eq!(
        input[1],
        json!({"type": "reasoning", "summary": [], "encrypted_content": "gAAAAenc"})
    );
    assert_eq!(input[2]["id"], "rs_real");
}

#[test]
fn foreign_reasoning_configs() {
    let with = |mode: Option<ReasoningMode>, effort: Option<ReasoningEffort>| {
        let mut r = GenerateRequest::new("m").with_message(Message::user("x"));
        let mut cfg = kaeso_kpp::ReasoningConfig::default();
        cfg.mode = mode;
        cfg.effort = effort;
        cfg.extras = Extras::new(DialectId::AnthropicMessages);
        r.reasoning = Some(cfg);
        r
    };
    assert!(
        body_of(with(Some(ReasoningMode::Adaptive), None))
            .get("reasoning")
            .is_none()
    );
    assert_eq!(
        body_of(with(
            Some(ReasoningMode::Adaptive),
            Some(ReasoningEffort::High)
        ))["reasoning"],
        json!({"effort": "high", "summary": "auto"})
    );
    assert_eq!(
        body_of(with(
            Some(ReasoningMode::Enabled),
            Some(ReasoningEffort::Low)
        ))["reasoning"],
        json!({"effort": "low", "summary": "auto"})
    );
    let err = OpenAiResponses::check(
        &Request::Generate(with(
            Some(ReasoningMode::Other("between_tools".into())),
            None,
        )),
        &crate::test_support::egress(),
    )
    .unwrap_err();
    assert_eq!(
        err.unsupported_feature(),
        Some("reasoning mode \"between_tools\"")
    );
    assert!(!err.message.contains("Other("), "{}", err.message);
}

#[test]
fn stateless_requests_replay_only_reasoning_with_encrypted_content() {
    let r = DialectId::OpenAiResponses;
    let with_id = |id: &str, encrypted: Option<&str>| {
        let mut b = ReasoningBlock::new(r);
        b.summary.push("thought".into());
        b.opaque.push(kaeso_kpp::Opaque::new(
            r,
            OpaqueKind::OpenAiReasoningItemId,
            id,
        ));
        if let Some(e) = encrypted {
            b.opaque.push(kaeso_kpp::Opaque::new(
                r,
                OpaqueKind::OpenAiEncryptedReasoning,
                e,
            ));
        }
        Block::Reasoning(b)
    };
    let mut req = GenerateRequest::new("gpt-5.5");
    req.messages = vec![
        Message::user("q"),
        Message::new(
            Role::Assistant,
            vec![
                with_id("rs_1", None),
                with_id("rs_2", Some("gAAAA")),
                Block::text("a"),
            ],
        ),
        Message::user("next"),
    ];
    req.extras = Extras::new(DialectId::AnthropicMessages);
    let body = body_of(req);
    let reasoning: Vec<&Value> = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "reasoning")
        .collect();
    assert_eq!(
        reasoning,
        [&json!({"id": "rs_2", "type": "reasoning", "summary": [
            {"type": "summary_text", "text": "thought"}], "encrypted_content": "gAAAA"})],
        "{body}"
    );
    assert_eq!(body["store"], false);
}

#[test]
fn output_limits_below_the_responses_minimum_are_refused_not_raised() {
    for low in [0, 15] {
        let mut req = foreign_request();
        req.sampling.max_output_tokens = Some(low);
        let err = OpenAiResponses::check(
            &Request::Generate(req.clone()),
            &crate::test_support::egress(),
        )
        .unwrap_err();
        assert_eq!(
            err.unsupported_feature(),
            Some(format!("max_tokens {low}").as_str())
        );
        assert_eq!(err.param.as_deref(), Some("max_tokens"));
        assert!(
            err.message.contains("at least 16 tokens"),
            "{}",
            err.message
        );
        assert!(
            OpenAiResponses::check(&Request::CountTokens(req), &crate::test_support::egress())
                .is_ok()
        );
    }
    let mut req = foreign_request();
    req.sampling.max_output_tokens = Some(16);
    assert!(
        OpenAiResponses::check(&Request::Generate(req), &crate::test_support::egress()).is_ok()
    );
}
