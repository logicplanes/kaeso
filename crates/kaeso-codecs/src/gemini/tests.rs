use kaeso_kpp::{
    Block, DialectId, ErrorKind, Extras, GenerateRequest, Message, OpaqueKind, Reply, Request,
    RequestKind, StopReason, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq, encoded_error, fixtures, sidecar};

const DIR: &str = "gemini";
const MODEL: &str = "gemini-3.5-flash";

fn call_of(folder: &str, name: &str) -> (String, GeminiAction) {
    let meta = sidecar(DIR, folder, name, "meta").unwrap_or(Value::Null);
    let model = meta["model"].as_str().unwrap_or(MODEL).to_owned();
    let default = if folder.ends_with("count_tokens") {
        GeminiAction::CountTokens
    } else {
        GeminiAction::GenerateContent
    };
    let action = meta["action"]
        .as_str()
        .map_or(default, |a| GeminiAction::from_wire(a).unwrap());
    (model, action)
}

fn decode_fixture(folder: &str, name: &str, body: &[u8]) -> Request {
    let (model, action) = call_of(folder, name);
    crate::dispatch::decode_gemini_request(&model, action, body, &HeaderList::new())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn generate_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off)
}

fn stream_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true))
}

fn decode_stream(bytes: &[u8], size: usize) -> Vec<StreamEvent> {
    let mut dec = Gemini::stream_decoder(&stream_ctx());
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

fn encode_stream(events: &[StreamEvent]) -> Vec<u8> {
    let mut enc =
        Gemini::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)));
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    out
}

#[test]
fn every_request_fixture_round_trips_and_names_its_path() {
    for folder in ["requests", "requests/count_tokens"] {
        for (name, body) in fixtures(DIR, folder, "json") {
            let req = decode_fixture(folder, &name, &body);
            let (model, action) = call_of(folder, &name);
            let mut ctx = EgressCtx::new(&req);
            let enc =
                Gemini::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                enc.path,
                GeminiProfile::GOOGLE_AI_STUDIO
                    .path(&model, action)
                    .unwrap(),
                "{name}"
            );
            assert_json_eq(&name, &body, &enc.body);
            assert!(ctx.adapted.is_empty(), "{name}: {:?}", ctx.adapted);
        }
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (folder, kind) in [
        ("responses", RequestKind::Generate),
        ("responses/count_tokens", RequestKind::CountTokens),
    ] {
        for (name, body) in fixtures(DIR, folder, "json") {
            let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
            let reply = Gemini::decode_response(200, &HeaderList::new(), &body, &ctx)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let out = Gemini::encode_response(&reply, &EncodeCtx::new(kind, StreamMode::Off))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_json_eq(&name, &body, &out);
        }
    }
}

#[test]
fn every_stream_fixture_round_trips_under_every_chunking() {
    for (name, bytes) in fixtures(DIR, "streams", "sse") {
        let whole = decode_stream(&bytes, 0);
        for size in [1, 7, 64] {
            assert_eq!(
                decode_stream(&bytes, size),
                whole,
                "{name}: chunks of {size}"
            );
        }
        assert!(
            !whole.iter().any(|e| matches!(e, StreamEvent::Error(_))),
            "{name}: {whole:?}"
        );
        assert_sse_eq(&name, &bytes, &encode_stream(&whole));
    }
}

#[test]
fn every_error_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = Gemini::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.status, Some(status), "{name}");
        let out = Gemini::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &encoded_error(&meta, &body), &out.body);
    }
}

#[test]
fn every_model_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "models", "json") {
        let v: Value = serde_json::from_slice(&body).unwrap();
        let out = if v.get("models").is_some() {
            let list = Gemini::decode_models(200, &HeaderList::new(), &body).unwrap();
            Gemini::encode_models(&list).unwrap()
        } else {
            let model = Gemini::decode_model(200, &HeaderList::new(), &body).unwrap();
            Gemini::encode_model(&model).unwrap()
        };
        assert_json_eq(&name, &body, &out);
    }
}

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(
        DIR,
        folder,
        if folder == "streams" { "sse" } else { "json" },
    )
    .into_iter()
    .find(|(n, _)| n == name)
    .unwrap_or_else(|| panic!("no fixture {name}"))
    .1
}

fn request(name: &str) -> GenerateRequest {
    match decode_fixture("requests", name, &fixture("requests", name)) {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn reply(name: &str) -> kaeso_kpp::Response {
    match Gemini::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", name),
        &generate_ctx(),
    )
    .unwrap()
    {
        Reply::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode_value(req: &Request) -> (Value, EgressCtx) {
    let mut ctx = EgressCtx::new(req);
    let enc = Gemini::encode_request(req, &mut ctx).unwrap_or_else(|e| panic!("{e}"));
    (serde_json::from_slice(&enc.body).unwrap(), ctx)
}

fn from(dialect: DialectId, mut r: GenerateRequest) -> Request {
    r.extras = Extras::new(dialect);
    Request::Generate(r)
}

fn refused(req: &Request) -> kaeso_kpp::KppError {
    Gemini::check(req, &EgressCtx::new(req)).unwrap_err()
}

#[test]
fn thought_signatures_ride_on_their_own_reasoning_blocks() {
    let r = request("tool-result-signature");
    let model = &r.messages[1];
    let kinds: Vec<&str> = model.blocks.iter().map(Block::kind).collect();
    assert_eq!(kinds, ["reasoning", "reasoning", "tool_call", "tool_call"]);
    let Block::Reasoning(thought) = &model.blocks[0] else {
        panic!()
    };
    assert!(thought.opaque.is_empty() && !thought.text.is_empty());
    let Block::Reasoning(carrier) = &model.blocks[1] else {
        panic!()
    };
    assert!(carrier.text.is_empty());
    assert_eq!(carrier.opaque[0].kind, OpaqueKind::GeminiThoughtSignature);
    let Block::ToolResult(result) = &r.messages[2].blocks[0] else {
        panic!()
    };
    assert_eq!(
        result.content,
        vec![Block::text("18 degrees Celsius and sunny")]
    );
    assert!(!result.is_error);
    let reply = reply("recorded-text-simple");
    assert_eq!(reply.blocks.len(), 2);
    assert!(matches!(&reply.blocks[1], Block::Text(t) if t.text == "Hello, how are you today?"));
    assert_eq!(reply.stop, StopReason::EndTurn);
}

#[test]
fn a_chat_app_gets_its_gemini_signatures_back_through_the_gateway() {
    let gemini = reply("recorded-tools-parallel");
    assert_eq!(gemini.stop, StopReason::ToolUse);
    let kept = crate::continuity::reasoning_for_tool_calls(&gemini.blocks);
    assert_eq!(kept.len(), 1, "only the first call carries a signature");
    let calls: Vec<_> = gemini
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(kept[0].tool_call_id, calls[0].id);
    let mut assistant = Message::new(
        kaeso_kpp::Role::Assistant,
        calls.iter().cloned().map(Block::ToolCall).collect(),
    );
    assistant.extras = Extras::new(DialectId::OpenAiChat);
    let mut r = GenerateRequest::new(MODEL)
        .with_message(Message::user(
            "Check the weather in Paris and in London, both at once.",
        ))
        .with_message(assistant)
        .with_message(Message::new(
            kaeso_kpp::Role::User,
            calls
                .iter()
                .map(|c| Block::tool_result(c.id.clone(), vec![Block::text("mild")], false))
                .collect(),
        ));
    assert_eq!(crate::continuity::tool_calls_missing_reasoning(&r).len(), 2);
    assert_eq!(crate::continuity::restore_reasoning(&mut r, &kept), 1);
    let (body, ctx) = encode_value(&from(DialectId::OpenAiChat, r));
    let parts = &body["contents"][1]["parts"];
    assert_eq!(
        parts[0]["thoughtSignature"].as_str(),
        kept[0].blocks[0].opaque[0].data.as_str().into()
    );
    assert!(parts[1].get("thoughtSignature").is_none());
    assert_eq!(parts[0]["functionCall"]["id"], calls[0].id.as_str());
    assert!(ctx.adapted.is_empty(), "{:?}", ctx.adapted);
    let results = &body["contents"][2]["parts"];
    assert_eq!(results[1]["functionResponse"]["name"], "get_weather");
}

#[test]
fn an_anthropic_app_carries_gemini_reasoning_in_its_thinking_blocks() {
    let gemini = Reply::Generate(reply("recorded-tools-parallel"));
    let out = crate::dispatch::encode_response(
        DialectId::AnthropicMessages,
        &gemini,
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let message: Value = serde_json::from_slice(&out).unwrap();
    let content = message["content"].as_array().unwrap().clone();
    let next = json!({
        "model": MODEL,
        "max_tokens": 1024,
        "messages": [
            {"role": "user", "content": "Check the weather in Paris and in London, both at once."},
            {"role": "assistant", "content": content},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": message["content"][2]["id"], "content": "18 °C"},
                {"type": "tool_result", "tool_use_id": message["content"][3]["id"], "content": "12 °C"}
            ]}
        ]
    });
    let req = crate::dispatch::decode_request(
        DialectId::AnthropicMessages,
        RequestKind::Generate,
        &serde_json::to_vec(&next).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let (body, ctx) = encode_value(&req);
    let parts = body["contents"][1]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["thought"], true);
    let original: Value =
        serde_json::from_slice(&fixture("responses", "recorded-tools-parallel")).unwrap();
    assert_eq!(
        parts[1]["thoughtSignature"],
        original["candidates"][0]["content"]["parts"][1]["thoughtSignature"]
    );
    assert!(parts[2].get("thoughtSignature").is_none());
    assert!(ctx.adapted.is_empty(), "{:?}", ctx.adapted);
}

#[test]
fn calls_from_other_models_get_the_documented_dummy_signature_in_the_current_turn_only() {
    let call = |id: &str| {
        Block::tool_call(
            id,
            "get_weather",
            kaeso_kpp::RawJson::from_string(r#"{"location":"Paris"}"#.into()).unwrap(),
        )
    };
    let result = |id: &str| Block::tool_result(id, vec![Block::text("sunny")], false);
    let r = GenerateRequest::new(MODEL)
        .with_message(Message::user("Earlier question"))
        .with_message(Message::new(
            kaeso_kpp::Role::Assistant,
            vec![call("toolu_old")],
        ))
        .with_message(Message::new(
            kaeso_kpp::Role::User,
            vec![result("toolu_old")],
        ))
        .with_message(Message::user("Weather in Paris and London?"))
        .with_message(Message::new(
            kaeso_kpp::Role::Assistant,
            vec![Block::text("Checking."), call("toolu_a"), call("toolu_b")],
        ))
        .with_message(Message::new(kaeso_kpp::Role::User, vec![result("toolu_a")]))
        .with_message(Message::new(kaeso_kpp::Role::User, vec![result("toolu_b")]))
        .with_message(Message::new(
            kaeso_kpp::Role::Assistant,
            vec![call("toolu_c")],
        ))
        .with_message(Message::new(kaeso_kpp::Role::User, vec![result("toolu_c")]));
    let (body, ctx) = encode_value(&from(DialectId::AnthropicMessages, r));
    let contents = body["contents"].as_array().unwrap();
    assert!(
        contents[1]["parts"][0].get("thoughtSignature").is_none(),
        "an earlier turn"
    );
    assert_eq!(
        contents[4]["parts"][1]["thoughtSignature"],
        DUMMY_SIGNATURE_VALUE
    );
    assert!(contents[4]["parts"][2].get("thoughtSignature").is_none());
    assert_eq!(
        contents[5]["parts"].as_array().unwrap().len(),
        2,
        "results of one step share a content"
    );
    assert_eq!(
        contents[6]["parts"][0]["thoughtSignature"],
        DUMMY_SIGNATURE_VALUE
    );
    assert_eq!(
        ctx.adapted,
        [
            "thought signature of tool call toolu_a",
            "thought signature of tool call toolu_c"
        ]
    );
}

const DUMMY_SIGNATURE_VALUE: &str = "skip_thought_signature_validator";

#[test]
fn calls_without_ids_get_kaeso_ids_that_never_reach_gemini() {
    let r = request("history-without-ids");
    let ids: Vec<String> = r.messages[1]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall(c) => Some(c.id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(
        ids.iter()
            .all(|id| id.starts_with(parts::GENERATED_ID_PREFIX))
    );
    assert_ne!(ids[0], ids[1]);
    let results: Vec<(String, Block)> = r.messages[2]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolResult(t) => Some((t.call_id.clone(), t.content[0].clone())),
            _ => None,
        })
        .collect();
    assert_eq!(results[0].0, ids[0]);
    assert_eq!(results[1].0, ids[1]);
    assert_eq!(
        results[1].1,
        Block::text(r#"{"temperature":12,"unit":"celsius"}"#)
    );
    let mut foreign = r.clone();
    foreign.extras = Extras::new(DialectId::OpenAiChat);
    for m in &mut foreign.messages {
        for b in &mut m.blocks {
            match b {
                Block::ToolCall(c) => c.extras = Extras::default(),
                Block::ToolResult(t) => t.extras = Extras::default(),
                _ => {}
            }
        }
    }
    foreign.tools.clear();
    let (body, _) = encode_value(&Request::Generate(foreign));
    let call = &body["contents"][1]["parts"][0]["functionCall"];
    assert!(call.get("id").is_none(), "{call}");
    let response = &body["contents"][2]["parts"][1]["functionResponse"];
    assert!(response.get("id").is_none());
    assert_eq!(
        response["response"],
        json!({"output": r#"{"temperature":12,"unit":"celsius"}"#}),
        "another format's result is text"
    );
}

#[test]
fn tool_results_from_other_formats_take_the_documented_shapes() {
    let call = Block::tool_call(
        "call_1",
        "lookup",
        kaeso_kpp::RawJson::from_string("{}".into()).unwrap(),
    );
    let mut image = kaeso_kpp::ImageBlock::new(kaeso_kpp::Source::base64("image/png", "AAAA"));
    image.extras = Extras::new(DialectId::AnthropicMessages);
    let r = GenerateRequest::new(MODEL)
        .with_message(Message::user("Go"))
        .with_message(Message::new(kaeso_kpp::Role::Assistant, vec![call]))
        .with_message(Message::new(
            kaeso_kpp::Role::User,
            vec![
                Block::tool_result(
                    "call_1",
                    vec![Block::text("a "), Block::text("b"), Block::Image(image)],
                    true,
                ),
                Block::text("Next"),
            ],
        ));
    let (body, _) = encode_value(&from(DialectId::AnthropicMessages, r.clone()));
    let response = &body["contents"][2]["parts"][0]["functionResponse"];
    assert_eq!(response["name"], "lookup");
    assert_eq!(response["response"], json!({"error": "a b"}));
    assert_eq!(
        response["parts"],
        json!([{"inlineData": {"mimeType": "image/png", "data": "AAAA"}}])
    );
    let mut orphan = r;
    orphan.messages.remove(1);
    let mut ctx = EgressCtx::new(&Request::Generate(orphan.clone()));
    let e =
        Gemini::encode_request(&from(DialectId::AnthropicMessages, orphan), &mut ctx).unwrap_err();
    assert_eq!(e.kind, ErrorKind::InvalidRequest);
    assert!(e.message.contains("`call_1`"), "{}", e.message);
}

#[test]
fn reasoning_settings_become_thinking_levels_or_budgets() {
    use kaeso_kpp::{ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary};
    let thinking = |c: ReasoningConfig, origin: DialectId| {
        let mut r = GenerateRequest::new(MODEL).with_message(Message::user("hi"));
        let mut c = c;
        c.extras = Extras::new(origin);
        r.reasoning = Some(c);
        encode_value(&from(origin, r)).0["generationConfig"]["thinkingConfig"].clone()
    };
    let cfg = |f: &dyn Fn(&mut ReasoningConfig)| {
        let mut c = ReasoningConfig::default();
        f(&mut c);
        c
    };
    let chat = DialectId::OpenAiChat;
    let anthropic = DialectId::AnthropicMessages;
    for (effort, level) in [
        (ReasoningEffort::Minimal, "MINIMAL"),
        (ReasoningEffort::Low, "LOW"),
        (ReasoningEffort::Medium, "MEDIUM"),
        (ReasoningEffort::High, "HIGH"),
        (ReasoningEffort::XHigh, "HIGH"),
        (ReasoningEffort::Max, "HIGH"),
    ] {
        let e = effort.clone();
        assert_eq!(
            thinking(cfg(&|c| c.effort = Some(e.clone())), chat),
            json!({"thinkingLevel": level})
        );
    }
    assert_eq!(
        thinking(cfg(&|c| c.effort = Some(ReasoningEffort::None)), chat),
        json!({"thinkingBudget": 0})
    );
    assert_eq!(
        thinking(
            cfg(&|c| {
                c.mode = Some(ReasoningMode::Enabled);
                c.budget_tokens = Some(4000);
            }),
            anthropic
        ),
        json!({"thinkingBudget": 4000, "includeThoughts": true})
    );
    assert_eq!(
        thinking(cfg(&|c| c.mode = Some(ReasoningMode::Disabled)), anthropic),
        json!({"thinkingBudget": 0})
    );
    assert_eq!(
        thinking(cfg(&|c| c.mode = Some(ReasoningMode::Adaptive)), anthropic),
        json!({"thinkingBudget": -1, "includeThoughts": true})
    );
    assert_eq!(
        thinking(
            cfg(&|c| {
                c.effort = Some(ReasoningEffort::Low);
                c.summary = Some(ReasoningSummary::Auto);
            }),
            DialectId::OpenAiResponses
        ),
        json!({"thinkingLevel": "LOW", "includeThoughts": true})
    );
    let mut r = GenerateRequest::new(MODEL).with_message(Message::user("hi"));
    let mut c = cfg(&|c| c.effort = Some(ReasoningEffort::Other("ultra".into())));
    c.extras = Extras::new(chat);
    r.reasoning = Some(c);
    let e = refused(&from(chat, r));
    assert_eq!(e.unsupported_feature(), Some("reasoning effort `ultra`"));
    assert_eq!(e.param.as_deref(), Some("reasoning_effort"));
}

#[test]
fn output_formats_tools_and_sampling_from_other_formats() {
    use kaeso_kpp::{FunctionTool, JsonSchemaSpec, OutputSpec, RawJson, ToolChoice, ToolDef};
    let schema = RawJson::from_string(r#"{"type":"object"}"#.into()).unwrap();
    let base = || GenerateRequest::new(MODEL).with_message(Message::user("hi"));
    let mut r = base();
    r.output = OutputSpec::JsonObject;
    let body = encode_value(&from(DialectId::OpenAiChat, r)).0;
    assert_eq!(
        body["generationConfig"],
        json!({"responseMimeType": "application/json"})
    );
    let mut r = base();
    let mut spec = JsonSchemaSpec::new(schema.clone());
    spec.name = Some("answer".into());
    r.output = OutputSpec::JsonSchema(spec.clone());
    let body = encode_value(&from(DialectId::OpenAiChat, r)).0;
    assert_eq!(
        body["generationConfig"],
        json!({"responseMimeType": "application/json", "responseJsonSchema": {"type": "object"}})
    );
    let mut r = base();
    spec.description = Some("A reply".into());
    r.output = OutputSpec::JsonSchema(spec.clone());
    assert_eq!(
        refused(&from(DialectId::OpenAiChat, r)).unsupported_feature(),
        Some("output schema description")
    );
    let mut r = base();
    spec.description = None;
    spec.strict = Some(false);
    r.output = OutputSpec::JsonSchema(spec);
    assert_eq!(
        refused(&from(DialectId::OpenAiChat, r)).unsupported_feature(),
        Some("non-strict output schema")
    );

    let mut strict = FunctionTool::new("f", Some(schema.clone()));
    strict.strict = Some(true);
    let mut r = base().with_tool(ToolDef::Function(strict));
    let body = encode_value(&from(DialectId::OpenAiResponses, r.clone())).0;
    assert_eq!(
        body["toolConfig"],
        json!({"functionCallingConfig": {"mode": "VALIDATED"}})
    );
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"],
        json!({"type": "object"})
    );
    for (choice, config) in [
        (ToolChoice::Auto, json!({"mode": "VALIDATED"})),
        (ToolChoice::None, json!({"mode": "NONE"})),
        (ToolChoice::Required, json!({"mode": "ANY"})),
        (
            ToolChoice::Tool("f".into()),
            json!({"mode": "ANY", "allowedFunctionNames": ["f"]}),
        ),
    ] {
        r.tool_choice = Some(choice);
        let body = encode_value(&from(DialectId::OpenAiResponses, r.clone())).0;
        assert_eq!(body["toolConfig"]["functionCallingConfig"], config);
    }
    r.parallel_tool_calls = Some(false);
    let e = refused(&from(DialectId::OpenAiChat, r.clone()));
    assert_eq!(e.param.as_deref(), Some("parallel_tool_calls"));
    r.parallel_tool_calls = Some(true);
    assert!(
        Gemini::check(
            &from(DialectId::OpenAiChat, r.clone()),
            &EgressCtx::new(&from(DialectId::OpenAiChat, r))
        )
        .is_ok()
    );

    let mut r = base();
    r.sampling.stop = (0..6).map(|i| i.to_string()).collect();
    let e = refused(&from(DialectId::OpenAiChat, r));
    assert_eq!(e.param.as_deref(), Some("stop"));
    let mut r = base();
    r.sampling.temperature = Some(1.0);
    r.sampling.top_k = Some(3);
    r.sampling.seed = Some(7);
    r.sampling.presence_penalty = Some(0.5);
    let body = encode_value(&from(DialectId::OpenAiChat, r)).0;
    assert_eq!(
        body["generationConfig"],
        json!({"temperature": 1.0, "topK": 3, "seed": 7, "presencePenalty": 0.5})
    );
}

#[test]
fn content_gemini_cannot_take_is_refused_before_sending() {
    let mut doc =
        kaeso_kpp::DocumentBlock::new(kaeso_kpp::Source::Url("https://example.com/a.pdf".into()));
    doc.title = Some("A".into());
    let r = GenerateRequest::new(MODEL).with_message(Message::new(
        kaeso_kpp::Role::User,
        vec![kaeso_kpp::Block::Document(doc)],
    ));
    assert_eq!(
        refused(&from(DialectId::AnthropicMessages, r)).unsupported_feature(),
        Some("document title, context or citations")
    );
    let r = GenerateRequest::new(MODEL).with_message(Message::new(
        kaeso_kpp::Role::Assistant,
        vec![Block::Refusal(kaeso_kpp::RefusalBlock::new("No."))],
    ));
    assert_eq!(
        refused(&from(DialectId::OpenAiChat, r)).unsupported_feature(),
        Some("refusal block")
    );
    let mut r = GenerateRequest::new(MODEL).with_message(Message::user("hi"));
    r.system = vec![Block::image(kaeso_kpp::Source::base64("image/png", "AAAA"))];
    assert_eq!(
        refused(&from(DialectId::AnthropicMessages, r)).unsupported_feature(),
        Some("image block in the system prompt")
    );
    let r = GenerateRequest::new(MODEL).with_message(Message::new(
        kaeso_kpp::Role::User,
        vec![Block::image(kaeso_kpp::Source::Url(
            "https://example.com/image".into(),
        ))],
    ));
    let mut ctx = EgressCtx::new(&from(DialectId::OpenAiChat, r.clone()));
    let e = Gemini::encode_request(&from(DialectId::OpenAiChat, r), &mut ctx).unwrap_err();
    assert_eq!(
        e.unsupported_feature(),
        Some("image URL without a known file extension")
    );
    let r = GenerateRequest::new(MODEL)
        .with_message(Message::user("hi"))
        .with_message(Message::new(
            kaeso_kpp::Role::System,
            vec![Block::text("Be brief.")],
        ))
        .with_message(Message::user("again"));
    let body = encode_value(&from(DialectId::OpenAiChat, r)).0;
    assert_eq!(
        body["contents"][1]["parts"],
        json!([{"text": "Be brief."}, {"text": "again"}])
    );
}

#[test]
fn replies_map_finish_reasons_usage_and_blocked_prompts() {
    let r = reply("unknown-parts");
    assert_eq!(r.stop, StopReason::Other("MALFORMED_FUNCTION_CALL".into()));
    assert_eq!(r.usage.input_tokens, 15);
    assert_eq!(r.usage.cache_read_tokens, 30);
    assert_eq!(r.usage.output_tokens, 15);
    assert_eq!(r.usage.reasoning_tokens, Some(3));
    assert!(matches!(&r.blocks[0], Block::Unknown(_)));
    let blocked = reply("prompt-blocked");
    assert!(blocked.blocks.is_empty());
    assert_eq!(blocked.stop, StopReason::ContentFilter);
    let image = reply("image-output");
    assert!(matches!(image.blocks.last(), Some(Block::Image(_))));
    assert!(
        matches!(&image.blocks[1], Block::Reasoning(_)),
        "the image's signature comes first"
    );
    for (wire, stop) in [
        ("SAFETY", StopReason::ContentFilter),
        ("PROHIBITED_CONTENT", StopReason::ContentFilter),
        ("MAX_TOKENS", StopReason::MaxTokens),
        ("LANGUAGE", StopReason::Other("LANGUAGE".into())),
    ] {
        let body = json!({"candidates": [{"content": {"parts": [{"text": "x"}], "role": "model"}, "finishReason": wire}]});
        let decoded = Gemini::decode_response(
            200,
            &HeaderList::new(),
            &serde_json::to_vec(&body).unwrap(),
            &generate_ctx(),
        )
        .unwrap();
        let Reply::Generate(d) = &decoded else {
            panic!()
        };
        assert_eq!(d.stop, stop, "{wire}");
        let back: Value = serde_json::from_slice(
            &Gemini::encode_response(
                &decoded,
                &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(back, body);
    }
    let mut foreign = kaeso_kpp::Response::new("msg_1", "claude", vec![Block::text("hi")]);
    for (stop, wire) in [
        (StopReason::Refusal, "SAFETY"),
        (StopReason::PauseTurn, "OTHER"),
        (StopReason::StopSequence("END".into()), "STOP"),
        (StopReason::Other("function_call".into()), "OTHER"),
        (StopReason::Other("RECITATION".into()), "RECITATION"),
    ] {
        foreign.stop = stop;
        let mut usage = kaeso_kpp::Usage::new(5, 8);
        usage.cache_read_tokens = 10;
        usage.reasoning_tokens = Some(3);
        foreign.usage = usage;
        let out: Value = serde_json::from_slice(
            &Gemini::encode_response(
                &Reply::Generate(foreign.clone()),
                &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(out["candidates"][0]["finishReason"], wire);
        assert_eq!(
            out["usageMetadata"],
            json!({"promptTokenCount": 15, "cachedContentTokenCount": 10, "candidatesTokenCount": 5, "thoughtsTokenCount": 3, "totalTokenCount": 23})
        );
        assert_eq!(out["responseId"], "msg_1");
    }
    let two = json!({"candidates": [{"content": {"parts": []}}, {"content": {"parts": []}}]});
    let e = Gemini::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&two).unwrap(),
        &generate_ctx(),
    )
    .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("more than one candidate"));
    let e = Gemini::decode_response(200, &HeaderList::new(), b"[]", &generate_ctx()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::ProviderError);
}

#[test]
fn errors_name_their_kind_and_retry_delay() {
    let err = |name: &str| {
        let meta = sidecar(DIR, "errors", name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        Gemini::decode_error(status, &HeaderList::new(), &fixture("errors", name))
    };
    assert_eq!(
        err("recorded-error-invalid-key").kind,
        ErrorKind::Authentication
    );
    assert_eq!(
        err("recorded-error-unknown-model").kind,
        ErrorKind::NotFound
    );
    let invalid = err("recorded-error-invalid-argument");
    assert_eq!(invalid.kind, ErrorKind::InvalidRequest);
    assert_eq!(
        invalid.param.as_deref(),
        Some("generation_config.thinking_config.thinking_level")
    );
    let limited = err("recorded-image-output");
    assert_eq!(limited.kind, ErrorKind::RateLimited);
    assert!(limited.retry_after.is_some());
    assert_eq!(
        err("recorded-stream-thinking-high").kind,
        ErrorKind::Overloaded
    );
    assert_eq!(
        err("image-generation-region").kind,
        ErrorKind::InvalidRequest
    );
    let foreign = kaeso_kpp::KppError::new(ErrorKind::RateLimited, "Slow down.")
        .with_origin(DialectId::OpenAiChat)
        .with_raw(json!({"error": {"message": "openai"}}));
    let out: Value = serde_json::from_slice(&Gemini::encode_error(&foreign).body).unwrap();
    assert_eq!(
        out,
        json!({"error": {"code": 429, "message": "Slow down.", "status": "RESOURCE_EXHAUSTED"}})
    );
}

#[test]
fn streams_from_other_formats_become_complete_gemini_streams() {
    use kaeso_kpp::{
        BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ResponseDelta, ResponseStart,
        ResponseStop, ToolCallHeader,
    };
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("msg_1", "claude")),
        StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Reasoning(kaeso_kpp::ReasoningHeader::new(
                DialectId::AnthropicMessages,
            )),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(
            0,
            Delta::ReasoningText("hidden".repeat(300)),
        )),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("Hi".into()))),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockStart(BlockStart::new(
            2,
            BlockHeader::ToolCall(ToolCallHeader::new("toolu_1", "f")),
        )),
        StreamEvent::BlockDelta(BlockDelta::new(2, Delta::ToolArguments("{\"a\":".into()))),
        StreamEvent::BlockDelta(BlockDelta::new(2, Delta::ToolArguments("1}".into()))),
        StreamEvent::BlockStop(BlockStop::new(2)),
        StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(StopReason::ToolUse),
            Some(kaeso_kpp::Usage::new(3, 4)),
        )),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let out = String::from_utf8(encode_stream(&events)).unwrap();
    assert!(out.starts_with("\ndata: "), "{out}");
    let back = decode_stream(out.as_bytes(), 0);
    assert!(
        back.iter().any(
            |e| matches!(e, StreamEvent::BlockDelta(d) if d.delta == Delta::Text("Hi".into()))
        )
    );
    assert!(back.iter().any(|e| matches!(e, StreamEvent::BlockDelta(d) if d.delta == Delta::ToolArguments("{\"a\":1}".into()))));
    assert!(back.iter().any(
        |e| matches!(e, StreamEvent::ResponseDelta(d) if d.stop == Some(StopReason::ToolUse))
    ));
    assert!(matches!(back.last(), Some(StreamEvent::ResponseStop(_))));
    assert!(!out.contains("hidden"));
    let mut enc =
        Gemini::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)));
    assert_eq!(enc.keepalive(), b"\n");
    let err = enc
        .encode(&StreamEvent::Error(kaeso_kpp::KppError::new(
            ErrorKind::Overloaded,
            "Busy.",
        )))
        .unwrap();
    let text = String::from_utf8(err).unwrap();
    assert!(text.starts_with("data: {\"error\""), "{text}");
    assert!(
        enc.encode(&StreamEvent::ResponseStop(ResponseStop::new()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn stream_decoding_reports_errors_and_keeps_frames_it_cannot_read() {
    let error = b"data: {\"error\": {\"code\": 503, \"message\": \"Busy.\", \"status\": \"UNAVAILABLE\"}}\n\n";
    let events = decode_stream(error, 0);
    let Some(StreamEvent::Error(e)) = events.last() else {
        panic!("{events:?}")
    };
    assert_eq!(
        (e.kind.clone(), e.status),
        (ErrorKind::Overloaded, Some(503))
    );
    let cut = b"data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hi\"}], \"role\": \"model\"}}]}\n\n";
    let events = decode_stream(cut, 0);
    assert!(
        matches!(events.last(), Some(StreamEvent::Error(e)) if e.message.contains("finish reason"))
    );
    let odd = b": comment\n\nevent: ping\ndata: {}\n\ndata: not json\n\ndata: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hi\"}]}, \"finishReason\": \"STOP\"}]}\n\n";
    let events = decode_stream(odd, 0);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, StreamEvent::Extra(_)))
            .count(),
        3
    );
    assert_sse_eq("odd frames", odd, &encode_stream(&events));
    let two = b"data: {\"candidates\": [{}, {}]}\n\n";
    assert!(
        matches!(decode_stream(two, 0).last(), Some(StreamEvent::Error(e)) if e.unsupported_feature() == Some("more than one candidate"))
    );
    let mut dec = Gemini::stream_decoder(&stream_ctx());
    dec.feed(b"data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Hi\"}]}}]}\n\n");
    assert!(matches!(
        dec.flush().as_slice(),
        [StreamEvent::BlockStop(_)]
    ));
}

#[test]
fn model_listings_page_with_tokens() {
    let q = Gemini::decode_models_query("pageSize=5&pageToken=gemini-x&view=FULL").unwrap();
    assert_eq!(q.limit, Some(5));
    assert_eq!(q.after_id.as_deref(), Some("gemini-x"));
    assert!(q.extras.fields.contains_key("query.view"));
    let e = Gemini::decode_models_query("pageSize=many").unwrap_err();
    assert_eq!(e.param.as_deref(), Some("pageSize"));
    let mut query = kaeso_kpp::ModelsQuery::default();
    query.limit = Some(5);
    query.after_id = Some("Ch1h/b+c".into());
    let ctx = generate_ctx()
        .with_gemini_profile(GeminiProfile::GOOGLE_AI_STUDIO.with_models_path("/v1/x/models"));
    let req = crate::dispatch::encode_models_request_with(DialectId::Gemini, &query, &ctx).unwrap();
    assert_eq!(req.path, "/v1/x/models?pageSize=5&pageToken=Ch1h%2Fb%2Bc");
    query.before_id = Some("a".into());
    let e = Gemini::encode_models_request(&query).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("before_id"));
    let one = crate::dispatch::encode_model_request_with(
        DialectId::Gemini,
        "models/gemini-3.5-flash",
        &ctx,
    )
    .unwrap();
    assert_eq!(one.path, "/v1/x/models/gemini-3.5-flash");
    assert_eq!(
        Gemini::encode_model_request("gemini-x").path,
        "/v1beta/models/gemini-x"
    );
    let mut list = kaeso_kpp::ModelList::new(vec![
        kaeso_kpp::ModelInfo::new("a"),
        kaeso_kpp::ModelInfo::new("b"),
    ]);
    list.has_more = true;
    let out: Value = serde_json::from_slice(&Gemini::encode_models(&list).unwrap()).unwrap();
    assert_eq!(
        out,
        json!({"models": [{"name": "models/a"}, {"name": "models/b"}], "nextPageToken": "b"})
    );
    let page = Gemini::decode_models(
        200,
        &HeaderList::new(),
        &fixture("models", "recorded-list-page-size-5"),
    )
    .unwrap();
    assert!(page.has_more);
    assert_eq!(page.models.len(), 5);
    let model = Gemini::decode_model(
        200,
        &HeaderList::new(),
        &fixture("models", "recorded-get-gemini-3.5-flash"),
    )
    .unwrap();
    assert_eq!(
        (
            model.id.as_str(),
            model.context_window,
            model.max_output_tokens
        ),
        ("gemini-3.5-flash", Some(1_048_576), Some(65_536))
    );
    let e = Gemini::decode_model(
        404,
        &HeaderList::new(),
        br#"{"error": {"code": 404, "message": "No.", "status": "NOT_FOUND"}}"#,
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
}

#[test]
fn requests_decode_only_with_their_path() {
    let body = br#"{"contents": [{"role": "user", "parts": [{"text": "hi"}]}]}"#;
    let req = crate::dispatch::decode_gemini_request(
        "models/gemini-x",
        GeminiAction::StreamGenerateContent,
        body,
        &HeaderList::new(),
    )
    .unwrap();
    assert_eq!(req.model(), "gemini-x");
    assert_eq!(req.stream_mode(), StreamMode::on(true));
    let checked = crate::limits::Checked::new(DialectId::Gemini, &body[..]).unwrap();
    assert_eq!(checked.shape().items, 1);
    assert_eq!(checked.shape().blocks, 1);
    let req = crate::dispatch::decode_checked_gemini_request(
        "gemini-x",
        GeminiAction::CountTokens,
        &checked,
        &HeaderList::new(),
    )
    .unwrap();
    assert!(matches!(req, Request::CountTokens(_)));
    let e = crate::dispatch::decode_checked_gemini_request(
        "gemini-x",
        GeminiAction::EmbedContent,
        &checked,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::InvalidRequest);
    let e = crate::dispatch::decode_checked_request(
        RequestKind::Generate,
        &checked,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert!(e.message.contains("decode_gemini_request"), "{}", e.message);
    let r = Gemini::decode_request(RequestKind::Generate, body, &HeaderList::new()).unwrap();
    assert_eq!(r.model(), "");
    for (bad, param) in [
        (&b"[]"[..], None),
        (&b"{\"contents\": 3}"[..], None),
        (&b"not json"[..], None),
    ] {
        let e = crate::dispatch::decode_gemini_request(
            "m",
            GeminiAction::GenerateContent,
            bad,
            &HeaderList::new(),
        )
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidRequest);
        assert_eq!(e.param.as_deref(), param);
    }
    let both = br#"{"contents": [{"parts": [{"text": "a"}]}], "generateContentRequest": {"contents": []}}"#;
    let e = crate::dispatch::decode_gemini_request(
        "m",
        GeminiAction::CountTokens,
        both,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(e.param.as_deref(), Some("contents"));
    let two = br#"{"contents": [], "generationConfig": {"candidateCount": 2}}"#;
    let e = crate::dispatch::decode_gemini_request(
        "m",
        GeminiAction::GenerateContent,
        two,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(e.param.as_deref(), Some("generationConfig.candidateCount"));
    let e = crate::dispatch::decode_gemini_request(
        "m",
        GeminiAction::GenerateContent,
        br#"{"contents": [{"parts": [1]}], "tools": [{"functionDeclarations": []}]}"#,
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(g) = e else { panic!() };
    assert!(matches!(g.messages[0].blocks[0], Block::Unknown(_)));
    assert!(matches!(g.tools[0], kaeso_kpp::ToolDef::Unknown(_)));
}

#[test]
fn count_tokens_requests_from_other_formats_wrap_what_needs_it() {
    let plain = Request::CountTokens(GenerateRequest::new(MODEL).with_message(Message::user("hi")));
    let mut plain = plain;
    if let Request::CountTokens(r) = &mut plain {
        r.extras = Extras::new(DialectId::AnthropicMessages);
    }
    let mut ctx = EgressCtx::new(&plain);
    let enc = Gemini::encode_request(&plain, &mut ctx).unwrap();
    assert_eq!(enc.path, "/v1beta/models/gemini-3.5-flash:countTokens");
    let v: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        v,
        json!({"contents": [{"role": "user", "parts": [{"text": "hi"}]}]})
    );
    let mut with_system = GenerateRequest::new(MODEL)
        .with_system("Be brief.")
        .with_message(Message::user("hi"));
    with_system.extras = Extras::new(DialectId::AnthropicMessages);
    with_system.sampling.max_output_tokens = Some(10);
    let req = Request::CountTokens(with_system);
    let mut ctx = EgressCtx::new(&req);
    let v: Value =
        serde_json::from_slice(&Gemini::encode_request(&req, &mut ctx).unwrap().body).unwrap();
    assert_eq!(
        v["generateContentRequest"]["model"],
        "models/gemini-3.5-flash"
    );
    assert_eq!(
        v["generateContentRequest"]["systemInstruction"],
        json!({"parts": [{"text": "Be brief."}]})
    );
    assert!(
        v["generateContentRequest"]
            .get("generationConfig")
            .is_none()
    );
    let reply = Reply::CountTokens(kaeso_kpp::CountTokensResponse::new(12));
    let out: Value = serde_json::from_slice(
        &Gemini::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(out, json!({"totalTokens": 12}));
    let embed = Request::Embed(kaeso_kpp::EmbedRequest::new("m", vec![]));
    assert!(Gemini::check(&embed, &EgressCtx::new(&embed)).is_err());
    assert!(Gemini::encode_request(&embed, &mut EgressCtx::new(&embed)).is_err());
}

#[test]
fn google_genais_snake_case_keys_and_float_top_k_reach_every_format_and_gemini_as_written() {
    let body = fixture("requests", "sdk-tools-snake-case");
    let req = decode_fixture("requests", "sdk-tools-snake-case", &body);
    let Request::Generate(g) = &req else { panic!() };
    let tool = match &g.tools[0] {
        kaeso_kpp::ToolDef::Function(f) => f,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        serde_json::from_str::<Value>(tool.parameters.as_ref().unwrap().get()).unwrap()["required"],
        json!(["location"])
    );
    assert_eq!(
        g.reasoning.as_ref().unwrap().effort,
        Some(kaeso_kpp::ReasoningEffort::Low)
    );
    for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
        let mut ctx = EgressCtx::new(&req);
        crate::dispatch::encode_request(target, &req, &mut ctx)
            .unwrap_or_else(|e| panic!("{target}: {e}"));
    }

    let float_k =
        json!({"contents": [{"parts": [{"text": "hi"}]}], "generationConfig": {"topK": 20.0}});
    let bytes = serde_json::to_vec(&float_k).unwrap();
    let req = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &bytes,
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.sampling.top_k, Some(20));
    let enc = Gemini::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_json_eq("float topK", &bytes, &enc.body);
    let e = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        br#"{"contents": [], "generationConfig": {"topK": 2.5}}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(e.param.as_deref(), Some("generationConfig.topK"));
}
