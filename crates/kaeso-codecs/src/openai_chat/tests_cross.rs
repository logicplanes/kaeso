use kaeso_kpp::{
    Block, CacheTtl, DialectId, DocumentBlock, FunctionTool, GenerateRequest, JsonSchemaSpec,
    Message, Opaque, OpaqueKind, Origin, OutputSpec, ReasoningBlock, ReasoningEffort, Reply,
    Request, RequestKind, Response, Role, Source, StopReason, StreamMode, TextBlock, ToolChoice,
    ToolDef, Usage,
};
use serde_json::json;

use super::OpenAiChat;
use super::tests::{decode, encode, encode_reply, raw};
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::test_support::fixtures;

fn anthropic() -> Origin {
    Origin::new(DialectId::AnthropicMessages)
}

#[test]
fn a_foreign_request_becomes_idiomatic_chat() {
    let mut req = GenerateRequest::new("gpt-5.5");
    let mut sys = TextBlock::new("Be brief.");
    sys.cache = Some(kaeso_kpp::CacheHint::new(Some(CacheTtl::OneHour)));
    req.system = vec![Block::Text(sys), Block::text("Use tools.")];
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("thinking".into());
    reasoning.opaque.push(Opaque::new(
        anthropic(),
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    req.messages = vec![
        Message::user("Weather in Paris?"),
        Message::new(
            Role::Assistant,
            vec![
                Block::Reasoning(reasoning),
                Block::text("Let me check."),
                Block::tool_call("toolu_01", "get_weather", raw(r#"{"city":"Paris"}"#)),
            ],
        ),
        Message::new(
            Role::User,
            vec![
                Block::tool_result("toolu_01", vec![Block::text("18C")], true),
                Block::text("Thanks!"),
            ],
        ),
    ];
    let mut tool = FunctionTool::new("get_weather", Some(raw(r#"{"type":"object"}"#)));
    tool.description = Some("Weather".into());
    tool.cache = Some(kaeso_kpp::CacheHint::default());
    req.tools.push(ToolDef::Function(tool));
    req.tool_choice = Some(ToolChoice::Tool("get_weather".into()));
    req.sampling.max_output_tokens = Some(1024);
    req.sampling.stop = vec!["END".into()];
    req.user = Some("user-42".into());
    req.stream = StreamMode::on(false);
    req.output = OutputSpec::JsonSchema(JsonSchemaSpec::new(raw(r#"{"type":"object"}"#)));
    let mut config = kaeso_kpp::ReasoningConfig::default();
    config.effort = Some(ReasoningEffort::High);
    config.mode = Some(kaeso_kpp::ReasoningMode::Adaptive);
    req.reasoning = Some(config);

    let (out, _) = encode(&req);
    assert_eq!(
        out,
        json!({
            "model": "gpt-5.5",
            "messages": [
                {"role": "system", "content": [
                    {"type": "text", "text": "Be brief."},
                    {"type": "text", "text": "Use tools."}
                ]},
                {"role": "user", "content": "Weather in Paris?"},
                {"role": "assistant", "content": "Let me check.", "tool_calls": [
                    {"id": "toolu_01", "type": "function",
                     "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "toolu_01", "content": "18C"},
                {"role": "user", "content": "Thanks!"}
            ],
            "max_completion_tokens": 1024,
            "stop": ["END"],
            "safety_identifier": "user-42",
            "stream": true,
            "stream_options": {"include_usage": true},
            "reasoning_effort": "high",
            "response_format": {"type": "json_schema", "json_schema": {
                "name": "response", "schema": {"type": "object"}}},
            "tools": [{"type": "function", "function": {
                "name": "get_weather", "description": "Weather",
                "parameters": {"type": "object"}}}],
            "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
        })
    );
}

#[test]
fn tool_names_chat_rejects_are_rewritten_and_mapped_back() {
    let long = format!("mcp.files/{}", "x".repeat(80));
    let req = GenerateRequest::new("m")
        .with_message(Message::user("go"))
        .with_tool(ToolDef::Function(FunctionTool::new(long.clone(), None)));
    let (out, ctx) = encode(&req);
    let sent = out["tools"][0]["function"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(super::convert::valid_tool_name(&sent), "{sent}");
    assert_eq!(ctx.tool_names.get(&sent), Some(&long));

    let body = json!({
        "id": "c", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": null,
            "tool_calls": [{"id": "call_1", "type": "function",
                "function": {"name": sent, "arguments": "{}"}}]},
            "logprobs": null, "finish_reason": "tool_calls"}]
    });
    let reply = OpenAiChat::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap();
    let Reply::Generate(resp) = reply else {
        panic!()
    };
    let Block::ToolCall(call) = &resp.blocks[0] else {
        panic!()
    };
    assert_eq!(call.name, long);
}

#[test]
fn chat_documents_encode_as_file_parts() {
    let mut doc = DocumentBlock::new(Source::base64("application/pdf", "JVBE").unwrap());
    doc.cache = Some(kaeso_kpp::CacheHint::default());
    let req = GenerateRequest::new("m").with_message(Message::new(
        Role::User,
        vec![
            Block::Document(doc),
            Block::image(Source::base64("image/png", "iVBO").unwrap()),
        ],
    ));
    let (out, _) = encode(&req);
    assert_eq!(
        out["messages"][0]["content"],
        json!([
            {"type": "file", "file": {"file_data": "data:application/pdf;base64,JVBE", "filename": "document.pdf"}},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBO"}}
        ])
    );
}

#[test]
fn a_foreign_reply_becomes_a_chat_completion() {
    let mut resp = Response::new("msg_01", "claude-x", Vec::new());
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("hmm".into());
    let mut cited = TextBlock::new(" world");
    cited.citations.push(kaeso_kpp::Citation::new(
        anthropic(),
        raw(r#"{"type":"char_location"}"#),
    ));
    resp.blocks = vec![
        Block::Reasoning(reasoning),
        Block::text("Hello"),
        Block::Text(cited),
        Block::tool_call("toolu_1", "f", raw(r#"{"a":1}"#)),
        Block::Unknown(kaeso_kpp::RawItem::new(
            anthropic(),
            raw(r#"{"type":"server_tool_use"}"#),
        )),
    ];
    resp.stop = StopReason::ToolUse;
    let mut usage = Usage::new(10, 5);
    usage.cache_read_tokens = 100;
    usage.cache_write_tokens = 3;
    resp.usage = usage;
    resp.extras = kaeso_kpp::Extras::new(anthropic()).with_field("stop_sequence", raw("null"));
    assert_eq!(
        encode_reply(&resp),
        json!({
            "id": "msg_01", "object": "chat.completion", "created": 1_700_000_000, "model": "claude-x",
            "choices": [{"index": 0, "message": {
                "role": "assistant", "content": "Hello world", "refusal": null, "annotations": [],
                "tool_calls": [{"id": "toolu_1", "type": "function",
                    "function": {"name": "f", "arguments": "{\"a\":1}"}}]},
                "logprobs": null, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 113, "completion_tokens": 5, "total_tokens": 118,
                "prompt_tokens_details": {"cached_tokens": 100, "cache_write_tokens": 3}}
        })
    );
    resp.stop = StopReason::StopSequence("END".into());
    assert_eq!(encode_reply(&resp)["choices"][0]["finish_reason"], "stop");
    resp.stop = StopReason::Refusal;
    assert_eq!(
        encode_reply(&resp)["choices"][0]["finish_reason"],
        "content_filter"
    );
}

#[test]
fn anthropic_fixture_requests_encode_or_name_the_feature() {
    use crate::anthropic_messages::AnthropicMessages;
    let (mut encoded, mut refused) = (Vec::new(), Vec::new());
    for folder in ["requests", "requests/count_tokens"] {
        let kind = if folder.ends_with("count_tokens") {
            RequestKind::CountTokens
        } else {
            RequestKind::Generate
        };
        for (name, body) in fixtures("anthropic_messages", folder, "json") {
            let Ok(req) = AnthropicMessages::decode_request(kind, &body, &HeaderList::new()) else {
                continue;
            };
            let mut ctx = EgressCtx::new(&req);
            match OpenAiChat::encode_request(&req, &mut ctx) {
                Ok(out) => {
                    let back = decode(&out.body).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                    assert!(matches!(back, Request::Generate(_)), "{name}");
                    encoded.push(name);
                }
                Err(err) => {
                    let feature = err.unsupported_feature().map(str::to_owned);
                    assert!(feature.is_some(), "{folder}/{name}: {err:?}");
                    refused.push((name, feature));
                }
            }
        }
    }
    assert!(
        encoded.len() >= 10,
        "encoded {encoded:?}, refused {refused:?}"
    );
    assert!(
        encoded.contains(&"tools-parallel".to_owned()),
        "{refused:?}"
    );
    assert!(
        refused
            .iter()
            .any(|(_, f)| f.as_deref() == Some("top_k") || f.as_deref() == Some("token counting"))
    );
}

#[test]
fn anthropic_fixture_responses_become_chat_completions() {
    use crate::anthropic_messages::AnthropicMessages;
    for (name, body) in fixtures("anthropic_messages", "responses", "json") {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        let Ok(reply) = AnthropicMessages::decode_response(200, &HeaderList::new(), &body, &ctx)
        else {
            continue;
        };
        let out = OpenAiChat::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back = OpenAiChat::decode_response(200, &HeaderList::new(), &out, &ctx);
        assert!(back.is_ok(), "{name}: {back:?}");
    }
}

#[test]
fn openrouter_reasoning_object_crosses_to_anthropic_gemini_responses_and_openai_chat() {
    let (_, body) = fixtures("openai_chat", "requests", "json")
        .into_iter()
        .find(|(n, _)| n == "openrouter-reasoning")
        .unwrap();
    let req = OpenAiChat::decode_request(RequestKind::Generate, &body, &HeaderList::new()).unwrap();
    let mut anthropic_req = req.clone();
    if let Request::Generate(g) = &mut anthropic_req {
        g.sampling.max_output_tokens = Some(16_384);
    }

    let mut ctx = EgressCtx::new(&anthropic_req);
    crate::dispatch::check(DialectId::AnthropicMessages, &anthropic_req, &ctx).unwrap();
    let enc =
        crate::dispatch::encode_request(DialectId::AnthropicMessages, &anthropic_req, &mut ctx)
            .unwrap();
    let anthropic: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        anthropic["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
    assert_eq!(anthropic["output_config"]["effort"], "max");
    assert_eq!(anthropic["stream"], true);
    assert_eq!(anthropic["tools"][0]["name"], "get_weather");

    let mut ctx = EgressCtx::new(&req);
    crate::dispatch::check(DialectId::Gemini, &req, &ctx).unwrap();
    let enc = crate::dispatch::encode_request(DialectId::Gemini, &req, &mut ctx).unwrap();
    let gemini: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        gemini["generationConfig"]["thinkingConfig"],
        json!({"thinkingLevel": "HIGH", "includeThoughts": true})
    );

    let mut ctx = EgressCtx::new(&req);
    crate::dispatch::check(DialectId::OpenAiResponses, &req, &ctx).unwrap();
    let enc = crate::dispatch::encode_request(DialectId::OpenAiResponses, &req, &mut ctx).unwrap();
    let responses: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        responses["reasoning"],
        json!({"effort": "max", "summary": "auto"})
    );

    let mut ctx = EgressCtx::new(&req);
    OpenAiChat::check(&req, &ctx).unwrap();
    let enc = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
    let chat: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(chat["reasoning_effort"], "max");
    assert!(chat.get("reasoning").is_none(), "{chat}");
}

#[test]
fn openrouter_reasoning_fields_translate_across_dialects() {
    let decode_chat = |body: serde_json::Value| {
        OpenAiChat::decode_request(
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
    };
    let encode_to = |dialect: DialectId, req: &Request, adaptive: Option<bool>| {
        let mut req = req.clone();
        if dialect == DialectId::AnthropicMessages
            && let Request::Generate(g) = &mut req
            && g.sampling.max_output_tokens.is_none()
        {
            g.sampling.max_output_tokens = Some(16_384);
        }
        let mut ctx = EgressCtx::new(&req);
        if let Some(a) = adaptive {
            ctx = ctx.with_adaptive_thinking(a);
        }
        crate::dispatch::check(dialect, &req, &ctx).unwrap();
        let enc = crate::dispatch::encode_request(dialect, &req, &mut ctx).unwrap();
        serde_json::from_slice::<serde_json::Value>(&enc.body).unwrap()
    };

    let req = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {"max_tokens": 4096}
    }))
    .unwrap();
    let anth = encode_to(DialectId::AnthropicMessages, &req, None);
    assert_eq!(
        anth["thinking"],
        json!({"type": "enabled", "budget_tokens": 4096, "display": "summarized"})
    );
    let gem = encode_to(DialectId::Gemini, &req, None);
    assert_eq!(
        gem["generationConfig"]["thinkingConfig"],
        json!({"thinkingBudget": 4096, "includeThoughts": true})
    );
    let resp = encode_to(DialectId::OpenAiResponses, &req, None);
    assert_eq!(
        resp["reasoning"],
        json!({"effort": "medium", "summary": "auto"})
    );

    let req_on = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {"enabled": true}
    }))
    .unwrap();
    let anth_on = encode_to(DialectId::AnthropicMessages, &req_on, None);
    assert_eq!(
        anth_on["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
    let gem_on = encode_to(DialectId::Gemini, &req_on, None);
    assert_eq!(
        gem_on["generationConfig"]["thinkingConfig"],
        json!({"thinkingBudget": -1, "includeThoughts": true})
    );

    let req_off = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {"enabled": false}
    }))
    .unwrap();
    let anth_off = encode_to(DialectId::AnthropicMessages, &req_off, None);
    assert_eq!(anth_off["thinking"], json!({"type": "disabled"}));
    let gem_off = encode_to(DialectId::Gemini, &req_off, None);
    assert_eq!(
        gem_off["generationConfig"]["thinkingConfig"],
        json!({"thinkingBudget": 0})
    );
    let resp_off = encode_to(DialectId::OpenAiResponses, &req_off, None);
    assert_eq!(resp_off["reasoning"], json!({"effort": "none"}));
    let chat_off = encode_to(DialectId::OpenAiChat, &req_off, None);
    assert_eq!(chat_off["reasoning_effort"], "none");

    let req_excl = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {"effort": "high", "exclude": true}
    }))
    .unwrap();
    let anth_excl = encode_to(DialectId::AnthropicMessages, &req_excl, Some(false));
    assert_eq!(
        anth_excl["thinking"],
        json!({"type": "enabled", "budget_tokens": 16_383})
    );
    assert!(anth_excl["thinking"].get("display").is_none());
    let gem_excl = encode_to(DialectId::Gemini, &req_excl, None);
    assert_eq!(
        gem_excl["generationConfig"]["thinkingConfig"],
        json!({"thinkingLevel": "HIGH"})
    );
    let resp_excl = encode_to(DialectId::OpenAiResponses, &req_excl, None);
    assert_eq!(resp_excl["reasoning"], json!({"effort": "high"}));

    let req_empty = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {}
    }))
    .unwrap();
    assert!(
        encode_to(DialectId::AnthropicMessages, &req_empty, None)
            .get("thinking")
            .is_none()
    );
    assert!(
        encode_to(DialectId::Gemini, &req_empty, None)
            .get("generationConfig")
            .is_none()
    );

    let req_minimal = decode_chat(json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}],
        "reasoning": {"effort": "minimal"}
    }))
    .unwrap();
    let mut anth_min = req_minimal.clone();
    if let Request::Generate(g) = &mut anth_min {
        g.sampling.max_output_tokens = Some(1024);
    }
    let err = crate::dispatch::check(
        DialectId::AnthropicMessages,
        &anth_min,
        &EgressCtx::new(&anth_min),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("reasoning.effort"));
    assert_eq!(
        encode_to(DialectId::Gemini, &req_minimal, None)["generationConfig"]["thinkingConfig"],
        json!({"thinkingLevel": "MINIMAL", "includeThoughts": true})
    );
}

#[test]
fn openrouter_reasoning_conflicts_and_malformed_fields_are_rejected() {
    let check_err = |body: serde_json::Value, param: &str| {
        let err = OpenAiChat::decode_request(
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap_err();
        assert_eq!(
            err.kind,
            kaeso_kpp::ErrorKind::InvalidRequest,
            "{body}: {err:?}"
        );
        assert_eq!(err.param.as_deref(), Some(param), "{body}: {err:?}");
    };
    let msg = json!([{"role": "user", "content": "hi"}]);
    check_err(
        json!({"model": "m", "messages": msg, "reasoning_effort": "low", "reasoning": {"effort": "high"}}),
        "reasoning.effort",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"effort": "high", "max_tokens": 2048}}),
        "reasoning",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning_effort": "high", "reasoning": {"max_tokens": 2048}}),
        "reasoning.max_tokens",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"enabled": false, "effort": "low"}}),
        "reasoning.enabled",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"enabled": false, "max_tokens": 2048}}),
        "reasoning.enabled",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning_effort": "high", "reasoning": {"enabled": false}}),
        "reasoning.enabled",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"enabled": true, "effort": "none"}}),
        "reasoning.enabled",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"max_tokens": 0}}),
        "reasoning.max_tokens",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": "max"}),
        "reasoning",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"effort": 42}}),
        "reasoning.effort",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"max_tokens": -5}}),
        "reasoning.max_tokens",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"enabled": "yes"}}),
        "reasoning.enabled",
    );
    check_err(
        json!({"model": "m", "messages": msg, "reasoning": {"exclude": 1}}),
        "reasoning.exclude",
    );

    let same = OpenAiChat::decode_request(
        RequestKind::Generate,
        json!({"model": "m", "messages": msg, "reasoning_effort": "high", "reasoning": {"effort": "high"}})
            .to_string()
            .as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(g) = same else { panic!() };
    assert_eq!(g.reasoning.unwrap().effort, Some(ReasoningEffort::High));

    let off_none = OpenAiChat::decode_request(
        RequestKind::Generate,
        json!({"model": "m", "messages": msg, "reasoning": {"enabled": false, "effort": "none"}})
            .to_string()
            .as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(g) = off_none else {
        panic!()
    };
    let r = g.reasoning.unwrap();
    assert_eq!(r.mode, Some(kaeso_kpp::ReasoningMode::Disabled));
    assert_eq!(r.effort, Some(ReasoningEffort::None));
}

#[test]
fn chat_replies_and_streams_include_or_exclude_reasoning_according_to_openrouter_request() {
    let req_with = |reasoning: serde_json::Value| {
        let mut body = json!({
            "model": "claude-sonnet-5-5",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        });
        body["reasoning"] = reasoning;
        OpenAiChat::decode_request(
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap()
    };

    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("Think first.".into());
    let mut resp = Response::new("msg_01", "claude-sonnet-5-5", Vec::new());
    resp.blocks = vec![
        Block::Reasoning(reasoning),
        Block::text("Answer."),
        Block::tool_call("toolu_1", "get_weather", raw(r#"{"city":"Berlin"}"#)),
    ];
    resp.stop = StopReason::ToolUse;
    let mut usage = Usage::new(10, 20);
    usage.reasoning_tokens = Some(12);
    resp.usage = usage;

    let enc = |req: Request| -> serde_json::Value {
        let ctx = EncodeCtx::for_request(req).with_created_at(1_700_000_000);
        let bytes = OpenAiChat::encode_response(&Reply::Generate(resp.clone()), &ctx).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let with_reasoning = enc(req_with(json!({"effort": "max"})));
    assert_eq!(
        with_reasoning["choices"][0]["message"]["reasoning"],
        "Think first."
    );
    assert_eq!(
        with_reasoning["choices"][0]["message"]["content"],
        "Answer."
    );
    assert_eq!(
        with_reasoning["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "get_weather"
    );

    let excluded = enc(req_with(json!({"effort": "max", "exclude": true})));
    assert!(
        excluded["choices"][0]["message"].get("reasoning").is_none(),
        "{excluded}"
    );
    assert_eq!(excluded["choices"][0]["message"]["content"], "Answer.");
    assert_eq!(
        excluded["usage"]["completion_tokens_details"]["reasoning_tokens"],
        12
    );

    let events = vec![
        kaeso_kpp::StreamEvent::ResponseStart(kaeso_kpp::ResponseStart::new(
            "msg_01",
            "claude-sonnet-5-5",
        )),
        kaeso_kpp::StreamEvent::BlockStart(kaeso_kpp::BlockStart::new(
            0,
            kaeso_kpp::BlockHeader::Reasoning(kaeso_kpp::ReasoningHeader::new(anthropic())),
        )),
        kaeso_kpp::StreamEvent::BlockDelta(kaeso_kpp::BlockDelta::new(
            0,
            kaeso_kpp::Delta::ReasoningText("Step 1. ".into()),
        )),
        kaeso_kpp::StreamEvent::BlockDelta(kaeso_kpp::BlockDelta::new(
            0,
            kaeso_kpp::Delta::ReasoningSummary(kaeso_kpp::SummaryDelta::new(0, "Step 2.")),
        )),
        kaeso_kpp::StreamEvent::BlockStop(kaeso_kpp::BlockStop::new(0)),
        kaeso_kpp::StreamEvent::BlockStart(kaeso_kpp::BlockStart::new(
            1,
            kaeso_kpp::BlockHeader::Text,
        )),
        kaeso_kpp::StreamEvent::BlockDelta(kaeso_kpp::BlockDelta::new(
            1,
            kaeso_kpp::Delta::Text("Done.".into()),
        )),
        kaeso_kpp::StreamEvent::BlockStop(kaeso_kpp::BlockStop::new(1)),
        kaeso_kpp::StreamEvent::ResponseDelta(kaeso_kpp::ResponseDelta::new(
            Some(StopReason::EndTurn),
            None,
        )),
        kaeso_kpp::StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::default()),
    ];
    let encode_sse = |req: Request| -> String {
        let ctx = EncodeCtx::for_request(req).with_created_at(1_700_000_000);
        let mut encoder = OpenAiChat::stream_encoder(&ctx);
        let mut out = Vec::new();
        for ev in &events {
            out.extend(encoder.encode(ev).unwrap());
        }
        out.extend(encoder.finish().unwrap());
        String::from_utf8(out).unwrap()
    };
    let streamed = encode_sse(req_with(json!({"effort": "max"})));
    assert!(streamed.contains(r#""reasoning":"Step 1. ""#), "{streamed}");
    assert!(streamed.contains(r#""reasoning":"Step 2.""#), "{streamed}");
    assert!(streamed.contains(r#""content":"Done.""#), "{streamed}");

    let streamed_excl = encode_sse(req_with(json!({"effort": "max", "exclude": true})));
    assert!(!streamed_excl.contains("Step 1"), "{streamed_excl}");
    assert!(!streamed_excl.contains("Step 2"), "{streamed_excl}");
    assert!(
        streamed_excl.contains(r#""content":"Done.""#),
        "{streamed_excl}"
    );

    let own_body = fixtures("openai_chat", "responses", "json")
        .into_iter()
        .find(|(n, _)| n == "providers/openrouter/recorded-free-reasoning")
        .unwrap()
        .1;
    let own_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_chat_profile(
        super::ChatProfile::compatible("OpenRouter").reasoning_fields(&["reasoning"]),
    );
    let own_reply =
        OpenAiChat::decode_response(200, &HeaderList::new(), &own_body, &own_ctx).unwrap();
    let own_excl_bytes = OpenAiChat::encode_response(
        &own_reply,
        &EncodeCtx::for_request(req_with(json!({"exclude": true}))),
    )
    .unwrap();
    let own_excl: serde_json::Value = serde_json::from_slice(&own_excl_bytes).unwrap();
    assert!(
        own_excl["choices"][0]["message"].get("reasoning").is_none(),
        "{own_excl}"
    );
    assert!(
        own_excl["choices"][0]["message"]
            .get("reasoning_details")
            .is_none(),
        "{own_excl}"
    );
    assert_eq!(
        own_excl["usage"]["completion_tokens_details"]["reasoning_tokens"],
        418
    );

    let own_sse = fixtures("openai_chat", "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == "providers/openrouter/recorded-free-reasoning-stream")
        .unwrap()
        .1;
    let own_stream_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true))
        .with_chat_profile(
            super::ChatProfile::compatible("OpenRouter").reasoning_fields(&["reasoning"]),
        );
    let mut dec = OpenAiChat::stream_decoder(&own_stream_ctx);
    let mut own_events = dec.feed(&own_sse);
    own_events.extend(dec.finish());
    let mut own_enc =
        OpenAiChat::stream_encoder(&EncodeCtx::for_request(req_with(json!({"exclude": true}))));
    let mut own_out = Vec::new();
    for ev in &own_events {
        own_out.extend(own_enc.encode(ev).unwrap());
    }
    own_out.extend(own_enc.finish().unwrap());
    let own_out = String::from_utf8(own_out).unwrap();
    assert!(!own_out.contains(r#""reasoning""#), "{own_out}");
    assert!(!own_out.contains(r#""reasoning_details""#), "{own_out}");
    assert!(own_out.contains("No"), "{own_out}");
}
