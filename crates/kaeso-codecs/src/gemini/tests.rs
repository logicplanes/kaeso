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
    let mut image =
        kaeso_kpp::ImageBlock::new(kaeso_kpp::Source::base64("image/png", "AAAA").unwrap());
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
    r.system = vec![Block::image(
        kaeso_kpp::Source::base64("image/png", "AAAA").unwrap(),
    )];
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
fn cached_image_and_audio_tokens_are_not_counted_again_as_uncached_input() {
    let usage = json!({
        "promptTokenCount": 4564,
        "cachedContentTokenCount": 4120,
        "candidatesTokenCount": 12,
        "totalTokenCount": 4576,
        "promptTokensDetails": [
            {"modality": "TEXT", "tokenCount": 74},
            {"modality": "IMAGE", "tokenCount": 1290},
            {"modality": "AUDIO", "tokenCount": 3200}
        ],
        "cacheTokensDetails": [
            {"modality": "TEXT", "tokenCount": 30},
            {"modality": "IMAGE", "tokenCount": 1290},
            {"modality": "AUDIO", "tokenCount": 2800}
        ]
    });
    let body = json!({
        "candidates": [{"content": {"parts": [{"text": "A fox."}], "role": "model"}, "finishReason": "STOP"}],
        "usageMetadata": usage,
        "modelVersion": MODEL,
        "responseId": "r1"
    });
    let check = |u: &kaeso_kpp::Usage| {
        assert_eq!(u.input_tokens, 444);
        assert_eq!(u.cache_read_tokens, 4120);
        assert_eq!(u.input_image_tokens, Some(0));
        assert_eq!(u.cache_read_image_tokens, Some(1290));
        assert_eq!(u.input_audio_tokens, Some(400));
    };
    let Reply::Generate(r) = Gemini::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &generate_ctx(),
    )
    .unwrap() else {
        panic!()
    };
    check(&r.usage);
    let sse = format!("data: {body}\n\n");
    let streamed = decode_stream(sse.as_bytes(), 0)
        .into_iter()
        .find_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage,
            _ => None,
        })
        .unwrap();
    check(&streamed);

    let mut uncached = body;
    uncached["usageMetadata"] = json!({
        "promptTokenCount": 1300,
        "candidatesTokenCount": 12,
        "promptTokensDetails": [
            {"modality": "TEXT", "tokenCount": 10},
            {"modality": "IMAGE", "tokenCount": 1290}
        ]
    });
    let Reply::Generate(r) = Gemini::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&uncached).unwrap(),
        &generate_ctx(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.usage.input_tokens, 1300);
    assert_eq!(r.usage.input_image_tokens, Some(1290));
    assert_eq!(r.usage.cache_read_image_tokens, None);
    assert_eq!(r.usage.input_audio_tokens, None);
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
    let depleted = err("billing-prepay-depleted");
    assert_eq!(depleted.kind, ErrorKind::Billing);
    assert!(
        depleted
            .message
            .starts_with("Your prepayment credits are depleted.")
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
    let file = err("file-permission-denied-published");
    assert_eq!(
        (file.kind, file.status),
        (ErrorKind::InvalidRequest, Some(403)),
        "a file the key can't read is the request's fault, not the key's"
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
    assert!(out.starts_with("\n\ndata: "), "{out}");
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
    assert_eq!(enc.keepalive(), b"\n\n");
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
    let depleted = b"data: {\"error\": {\"code\": 402, \"message\": \"Your prepayment credits are depleted.\", \"status\": \"RESOURCE_EXHAUSTED\"}}\n\n";
    let Some(StreamEvent::Error(e)) = decode_stream(depleted, 0).pop() else {
        panic!()
    };
    assert_eq!((e.kind, e.status), (ErrorKind::Billing, Some(402)));
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

#[test]
fn function_response_contents_decode_as_user_regardless_of_wire_role() {
    use kaeso_kpp::Role;
    let body = fixture("requests", "tool-result-role-model");
    let req = decode_fixture("requests", "tool-result-role-model", &body);
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.messages.len(), 3);
    assert_eq!(g.messages[1].role, Role::Assistant);
    assert_eq!(g.messages[2].role, Role::User);
    assert!(matches!(
        g.messages[2].blocks.as_slice(),
        [Block::ToolResult(r)] if r.call_id == "call_go_sdk_1"
    ));

    let back = Gemini::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_json_eq("tool-result-role-model", &body, &back.body);

    let anthropic: Value = serde_json::from_slice(
        &crate::dispatch::encode_request(
            DialectId::AnthropicMessages,
            &req,
            &mut EgressCtx::new(&req),
        )
        .unwrap()
        .body,
    )
    .unwrap();
    assert_eq!(anthropic["messages"][2]["role"], "user");
    assert_eq!(
        anthropic["messages"][2]["content"][0]["type"],
        "tool_result"
    );

    let chat: Value = serde_json::from_slice(
        &crate::dispatch::encode_request(DialectId::OpenAiChat, &req, &mut EgressCtx::new(&req))
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(chat["messages"][2]["role"], "tool");

    let responses: Value = serde_json::from_slice(
        &crate::dispatch::encode_request(
            DialectId::OpenAiResponses,
            &req,
            &mut EgressCtx::new(&req),
        )
        .unwrap()
        .body,
    )
    .unwrap();
    assert_eq!(responses["input"][2]["type"], "function_call_output");

    for wire_role in ["model", "function", "absent"] {
        let raw = serde_json::to_vec(&json!({
            "contents": [
                {"role": "user", "parts": [{"text": "run"}]},
                {"role": "model", "parts": [{"functionCall": {"id": "c1", "name": "f", "args": {}}}]},
                {"role": wire_role, "parts": [{"functionResponse": {"id": "c1", "name": "f", "response": {"output": "ok"}}}]}
            ]
        }))
        .unwrap();
        let req = crate::dispatch::decode_gemini_request(
            MODEL,
            GeminiAction::GenerateContent,
            &raw,
            &HeaderList::new(),
        )
        .unwrap();
        let Request::Generate(g) = &req else { panic!() };
        assert_eq!(g.messages[2].role, Role::User, "wire_role={wire_role}");
        let enc = Gemini::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
        assert_json_eq(wire_role, &raw, &enc.body);
    }

    let mixed = serde_json::to_vec(&json!({
        "contents": [
            {"role": "model", "parts": [{"text": "done"}, {"functionResponse": {"id": "c1", "name": "f", "response": {"output": "ok"}}}]}
        ]
    }))
    .unwrap();
    let Request::Generate(g) = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &mixed,
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(g.messages[0].role, Role::Assistant);

    // Multiple functionResponse parts in one role: "model" content decode as one User turn.
    let multi = serde_json::to_vec(&json!({
        "contents": [
            {"role": "user", "parts": [{"text": "run both"}]},
            {"role": "model", "parts": [
                {"functionCall": {"id": "c1", "name": "f", "args": {}}},
                {"functionCall": {"id": "c2", "name": "g", "args": {}}}
            ]},
            {"role": "model", "parts": [
                {"functionResponse": {"id": "c1", "name": "f", "response": {"output": "a"}}},
                {"functionResponse": {"id": "c2", "name": "g", "response": {"output": "b"}}}
            ]}
        ]
    }))
    .unwrap();
    let req = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &multi,
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(g) = &req else { panic!() };
    assert_eq!(g.messages[2].role, Role::User);
    assert_eq!(g.messages[2].blocks.len(), 2);
    let enc = Gemini::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_json_eq("multi functionResponse role:model", &multi, &enc.body);

    // A functionResponse with a thoughtSignature emits a Reasoning block too, so it is not
    // rewritten to Role::User; an unknown role with mixed parts stays raw Unknown.
    let signed = serde_json::to_vec(&json!({
        "contents": [
            {"role": "model", "parts": [{"functionResponse": {"id": "c1", "name": "f", "response": {"output": "ok"}}, "thoughtSignature": "c2ln"}]}
        ]
    }))
    .unwrap();
    let Request::Generate(g) = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &signed,
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(g.messages[0].role, Role::Assistant);

    let unknown_mixed = serde_json::to_vec(&json!({
        "contents": [
            {"role": "assistant", "parts": [{"text": "hi"}, {"functionResponse": {"id": "c1", "name": "f", "response": {"output": "ok"}}}]}
        ]
    }))
    .unwrap();
    let Request::Generate(g) = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &unknown_mixed,
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert!(matches!(
        g.messages[0].blocks.as_slice(),
        [Block::Unknown(_)]
    ));

    // Absent role on a functionResponse content respects `role_on_every_content`.
    let absent = serde_json::to_vec(&json!({
        "contents": [
            {"role": "user", "parts": [{"text": "run"}]},
            {"role": "model", "parts": [{"functionCall": {"id": "c1", "name": "f", "args": {}}}]},
            {"parts": [{"functionResponse": {"id": "c1", "name": "f", "response": {"output": "ok"}}}]}
        ]
    }))
    .unwrap();
    let req = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::GenerateContent,
        &absent,
        &HeaderList::new(),
    )
    .unwrap();
    let back_default = Gemini::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_json_eq("absent role default", &absent, &back_default.body);
    let mut agent_ctx = EgressCtx::new(&req).with_gemini_profile(GeminiProfile::AGENT_PLATFORM);
    let back_agent: Value =
        serde_json::from_slice(&Gemini::encode_request(&req, &mut agent_ctx).unwrap().body)
            .unwrap();
    assert_eq!(back_agent["contents"][2]["role"], "user");
}

// Mirrors google.golang.org/genai (`api_client.go`: `scan` + `iterateResponseStream`).
fn parse_go_genai_stream(mut bytes: &[u8]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let (token, rest) = if let Some(i) = bytes.windows(2).position(|w| w == b"\n\n") {
            (&bytes[..i], &bytes[i + 2..])
        } else if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            (&bytes[..i], &bytes[i + 4..])
        } else {
            (bytes, &bytes[bytes.len()..])
        };
        let token = token.strip_suffix(b"\r").unwrap_or(token);
        bytes = rest;
        if token.is_empty() {
            continue;
        }
        let colon = token.iter().position(|&b| b == b':');
        let (prefix, data) = match colon {
            Some(i) => (&token[..i], &token[i + 1..]),
            None => (token, &token[token.len()..]),
        };
        if prefix != b"data" {
            return Err(format!(
                "iterateResponseStream: invalid stream chunk: {}:{}",
                String::from_utf8_lossy(prefix),
                String::from_utf8_lossy(data)
            ));
        }
        let v: Value = serde_json::from_slice(data)
            .map_err(|e| format!("iterateResponseStream: error unmarshalling data: {e}"))?;
        out.push(v);
    }
    Ok(out)
}

// Mirrors google-genai (`google/genai/_api_client.py`: `_SseLineDecoder` + `_iter_response_stream`).
fn parse_python_genai_stream(bytes: &[u8]) -> Result<Vec<Value>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut data_buffer: Vec<&str> = Vec::new();
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if line.is_empty() {
            if !data_buffer.is_empty() {
                let joined = data_buffer.join("\n");
                let v: Value = serde_json::from_str(&joined).map_err(|e| e.to_string())?;
                out.push(v);
                data_buffer.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("data: ") {
            data_buffer.push(rest);
            continue;
        }
        return Err(format!(
            "non-data line treated as JSON error body: {line:?}"
        ));
    }
    if !data_buffer.is_empty() {
        let joined = data_buffer.join("\n");
        out.push(serde_json::from_str(&joined).map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// Mirrors @google/genai (`src/_api_client.ts`: `processStreamResponse`).
fn parse_js_genai_stream(bytes: &[u8], chunk_size: usize) -> Result<Vec<Value>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(chunk_size.max(1)) {
        buffer.push_str(std::str::from_utf8(chunk).map_err(|e| e.to_string())?);
        loop {
            let found = ["\n\n", "\r\r", "\r\n\r\n"]
                .iter()
                .filter_map(|d| buffer.find(d).map(|i| (i, d.len())))
                .min_by_key(|&(i, _)| i);
            let Some((idx, len)) = found else {
                break;
            };
            let event = buffer[..idx].trim().to_owned();
            buffer.drain(..idx + len);
            if let Some(rest) = event.strip_prefix("data:") {
                let v: Value = serde_json::from_str(rest.trim()).map_err(|e| e.to_string())?;
                out.push(v);
            }
        }
    }
    if !buffer.trim().is_empty() {
        return Err(format!("Incomplete JSON segment at the end: {buffer:?}"));
    }
    Ok(out)
}

#[test]
fn gemini_keepalives_follow_go_python_and_js_sdk_framing_rules() {
    use kaeso_kpp::{
        BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, RawItem, RawJson, ResponseDelta,
        ResponseStart, ResponseStop,
    };
    let ping = StreamEvent::Extra(RawItem::new(
        DialectId::AnthropicMessages,
        RawJson::from_string(r#"{"event":"ping","data":{"type":"ping"}}"#.into()).unwrap(),
    ));
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("msg_1", "claude")),
        ping.clone(),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("Hi".into()))),
        ping.clone(),
        ping.clone(),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("!".into()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(StopReason::EndTurn),
            Some(kaeso_kpp::Usage::new(2, 2)),
        )),
        StreamEvent::ResponseStop(ResponseStop::new()),
    ];
    let check_all_sdks = |label: &str, wire: &[u8], expected: usize| {
        let go = parse_go_genai_stream(wire)
            .unwrap_or_else(|e| panic!("{label} go: {e}\nwire={wire:?}"));
        let py = parse_python_genai_stream(wire)
            .unwrap_or_else(|e| panic!("{label} python: {e}\nwire={wire:?}"));
        let js_whole = parse_js_genai_stream(wire, wire.len())
            .unwrap_or_else(|e| panic!("{label} js whole: {e}\nwire={wire:?}"));
        let js_byte = parse_js_genai_stream(wire, 1)
            .unwrap_or_else(|e| panic!("{label} js 1B: {e}\nwire={wire:?}"));
        assert_eq!(go.len(), expected, "{label}");
        assert_eq!(go, py, "{label}");
        assert_eq!(go, js_whole, "{label}");
        assert_eq!(go, js_byte, "{label}");
    };

    // Single leading keepalive from a foreign ping, single mid-stream keepalive, and double keepalive.
    let mut enc =
        Gemini::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)));
    let mut wire = Vec::new();
    for ev in &events {
        wire.extend(enc.encode(ev).unwrap());
    }
    wire.extend(enc.finish().unwrap());
    check_all_sdks("single leading + double mid", &wire, 3);

    // Gateway-driven keepalive before the first event, single keepalive between chunks, and one before end.
    let mut enc =
        Gemini::stream_encoder(&EncodeCtx::new(RequestKind::Generate, StreamMode::on(true)));
    let mut driven = enc.keepalive();
    for ev in [
        StreamEvent::ResponseStart(ResponseStart::new("msg_2", "claude")),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("Hello".into()))),
    ] {
        driven.extend(enc.encode(&ev).unwrap());
    }
    driven.extend(enc.keepalive());
    for ev in [
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text(" world".into()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
        StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::EndTurn), None)),
    ] {
        driven.extend(enc.encode(&ev).unwrap());
    }
    driven.extend(enc.keepalive());
    driven.extend(
        enc.encode(&StreamEvent::ResponseStop(ResponseStop::new()))
            .unwrap(),
    );
    driven.extend(enc.finish().unwrap());
    check_all_sdks("gateway keepalives", &driven, 3);

    assert!(
        parse_go_genai_stream(b"\ndata: {\"candidates\":[]}\n\n").is_err(),
        "a single leading newline breaks google.golang.org/genai"
    );
    assert!(
        parse_python_genai_stream(b": keepalive\n\ndata: {\"candidates\":[]}\n\n").is_err(),
        "an SSE comment breaks google-genai"
    );
}

#[test]
fn gemini_clis_turn_reaches_other_formats_with_top_k_only_where_the_target_takes_it() {
    use crate::openai_chat::{ChatProfile, OpenAiChat};
    use crate::openai_responses::OpenAiResponses;
    let body = fixtures(DIR, "requests", "json")
        .into_iter()
        .find(|(name, _)| name == "gemini-cli-turn")
        .unwrap()
        .1;
    let req = crate::dispatch::decode_gemini_request(
        MODEL,
        GeminiAction::StreamGenerateContent,
        &body,
        &HeaderList::new(),
    )
    .unwrap();
    let sent = |out: EncodedRequest| -> Value { serde_json::from_slice(&out.body).unwrap() };

    let mut ctx = EgressCtx::new(&req);
    let chat = sent(OpenAiChat::encode_request(&req, &mut ctx).unwrap());
    assert!(chat.get("top_k").is_none(), "{chat}");
    assert_eq!(
        (&chat["temperature"], &chat["top_p"]),
        (&json!(1.0), &json!(0.95))
    );
    assert_eq!(ctx.left_out, ["generationConfig.topK"]);

    let mut ctx = EgressCtx::new(&req)
        .with_chat_profile(ChatProfile::compatible("Example Cloud").with_top_k());
    let chat = sent(OpenAiChat::encode_request(&req, &mut ctx).unwrap());
    assert_eq!(chat["top_k"], 64, "{chat}");
    assert!(ctx.left_out.is_empty());

    let mut ctx = EgressCtx::new(&req);
    let responses = sent(OpenAiResponses::encode_request(&req, &mut ctx).unwrap());
    assert!(responses.get("top_k").is_none(), "{responses}");
    assert_eq!(ctx.left_out, ["generationConfig.topK"]);
}

#[test]
fn a_wrapped_token_count_names_top_k_where_the_app_wrote_it() {
    use crate::openai_responses::OpenAiResponses;
    let body = json!({"generateContentRequest": {
        "model": "models/gpt-5.5",
        "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
        "generationConfig": {"topK": 64}
    }});
    let req = crate::dispatch::decode_gemini_request(
        "gpt-5.5",
        GeminiAction::CountTokens,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    OpenAiResponses::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(
        ctx.left_out,
        ["generateContentRequest.generationConfig.topK"]
    );
}
