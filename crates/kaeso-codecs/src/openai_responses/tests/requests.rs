use kaeso_kpp::{
    FunctionTool, OpaqueKind, OutputSpec, ReasoningEffort, ReasoningSummary, Role, Source,
    StreamMode, ToolChoice, ToolDef,
};
use serde_json::json;

use super::*;

#[test]
fn every_request_fixture_round_trips() {
    for (folder, kind) in [
        ("requests", RequestKind::Generate),
        ("requests/input_tokens", RequestKind::CountTokens),
    ] {
        for (name, body) in fixtures(DIR, folder, "json") {
            if REFUSED_REQUESTS.iter().any(|(n, _)| *n == name) {
                continue;
            }
            let req = decode_req(kind, &body);
            assert_eq!(req.kind(), kind, "{name}");
            let (enc, _) = encode_req(&req);
            let path = if kind == RequestKind::Generate {
                RESPONSES_PATH
            } else {
                INPUT_TOKENS_PATH
            };
            assert_eq!(enc.path, path);
            assert_json_eq(&name, &body, &enc.body);
        }
    }
}

#[test]
fn stateful_fixtures_are_refused_with_their_feature() {
    for (name, feature) in REFUSED_REQUESTS {
        let err = OpenAiResponses::decode_request(
            RequestKind::Generate,
            &fixture("requests", name, "json"),
            &HeaderList::new(),
        )
        .unwrap_err();
        assert_eq!(err.unsupported_feature(), Some(*feature), "{name}: {err}");
    }
}

const TYPED_KEYS: &[&str] = &[
    "model",
    "input",
    "instructions",
    "text",
    "content",
    "call_id",
    "name",
    "arguments",
    "output",
    "image_url",
    "file_id",
    "file_data",
    "file_url",
    "temperature",
    "top_p",
    "max_output_tokens",
    "parallel_tool_calls",
    "tool_choice",
    "parameters",
    "strict",
    "description",
    "schema",
    "effort",
    "summary",
    "encrypted_content",
    "delta",
    "refusal",
    "annotation",
    "annotations",
    "safety_identifier",
    "stream",
];

fn assert_no_typed_overrides(name: &str, extras: &Extras) {
    for key in extras.fields.keys() {
        let last = key.rsplit('.').next().unwrap_or(key);
        assert!(
            !TYPED_KEYS.contains(&last),
            "{name}: typed key `{key}` kept as an extras field"
        );
    }
}

fn all_extras(req: &GenerateRequest) -> Vec<&Extras> {
    let mut out = vec![&req.extras];
    out.extend(req.messages.iter().map(|m| &m.extras));
    out.extend(req.all_blocks().filter_map(Block::extras));
    for t in &req.tools {
        if let ToolDef::Function(f) = t {
            out.push(&f.extras);
        }
    }
    if let OutputSpec::JsonSchema(s) = &req.output {
        out.push(&s.extras);
    }
    if let Some(r) = &req.reasoning {
        out.push(&r.extras);
    }
    out
}

#[test]
fn typed_data_never_hides_in_extras_fields() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        if REFUSED_REQUESTS.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let Request::Generate(req) = decode_req(RequestKind::Generate, &body) else {
            panic!()
        };
        for e in all_extras(&req) {
            assert_no_typed_overrides(&name, e);
            assert!(
                e.is_empty() || e.is_from(R),
                "{name}: extras without origin"
            );
        }
    }
}

#[test]
fn string_input_and_instructions() {
    let r = generate("text-basic-stream");
    assert_eq!(r.model, "gpt-6-astra");
    assert_eq!(r.system, vec![Block::text("You are a helpful assistant.")]);
    assert_eq!(r.messages, vec![Message::user("Hello!")]);
    assert_eq!(r.stream, StreamMode::on(true));
    let mut plain = r.clone();
    plain.extras = Extras::default();
    let body = body_of(plain);
    assert_eq!(body["instructions"], "You are a helpful assistant.");
    assert_eq!(
        body["input"],
        json!([{"role": "user", "content": "Hello!"}])
    );
}

#[test]
fn roles_system_items_and_non_leading_developer_messages() {
    let r = generate("multi-turn-roles");
    assert_eq!(
        r.system,
        vec![
            Block::text("You are a concise travel assistant."),
            Block::text("Answer in at most two sentences.")
        ]
    );
    let roles: Vec<Role> = r.messages.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [
            Role::User,
            Role::Assistant,
            Role::User,
            Role::Assistant,
            Role::System,
            Role::User
        ]
    );
    assert_eq!(
        r.extras.fields.get("text.verbosity").unwrap().get(),
        "\"low\""
    );
    let Block::Text(t) = &r.messages[3].blocks[0] else {
        panic!()
    };
    assert!(t.text.starts_with("Bring comfortable"));
    assert!(r.messages[3].extras.fields.is_empty());
}

#[test]
fn tool_calls_results_and_reasoning_group_into_turns() {
    let r = generate("reasoning-encrypted");
    assert_eq!(r.messages.len(), 3);
    assert_eq!(r.messages[1].role, Role::Assistant);
    let [Block::Reasoning(rs), Block::ToolCall(call)] = r.messages[1].blocks.as_slice() else {
        panic!("{:?}", r.messages[1].blocks)
    };
    assert_eq!(rs.summary.len(), 1);
    assert!(rs.replayable_to(R));
    assert!(
        rs.opaque_of(OpaqueKind::OpenAiReasoningItemId)
            .unwrap()
            .data
            .starts_with("rs_")
    );
    assert!(
        rs.opaque_of(OpaqueKind::OpenAiEncryptedReasoning)
            .unwrap()
            .data
            .starts_with("gAAAA")
    );
    assert_eq!(call.id, "call_5GXBuXmucGIkbFfAryp6wDbC");
    assert_eq!(call.name, "get_weather");
    assert_eq!(call.arguments.get(), r#"{"city":"Paris","unit":"celsius"}"#);
    let [Block::ToolResult(res)] = r.messages[2].blocks.as_slice() else {
        panic!()
    };
    assert_eq!(res.call_id, call.id);
    assert_eq!(r.messages[2].role, Role::User);
    let cfg = r.reasoning.as_ref().unwrap();
    assert_eq!(cfg.effort, Some(ReasoningEffort::High));
    assert_eq!(cfg.summary, Some(ReasoningSummary::Auto));
    assert!(cfg.include_encrypted);
    assert_eq!(r.tools.len(), 1);
    r.validate().unwrap();
}

#[test]
fn tool_outputs_with_parts_images_and_files() {
    let r = generate("function-call-output-array");
    let [Block::ToolResult(res)] = r.messages[2].blocks.as_slice() else {
        panic!()
    };
    assert!(matches!(&res.content[0], Block::Text(t) if t.text == "Chart rendered (1 image)."));
    assert!(
        matches!(&res.content[1], Block::Image(i) if matches!(&i.source, Source::Base64(b) if b.media_type == "image/png"))
    );
    assert!(
        matches!(&res.content[2], Block::Document(d) if matches!(&d.source, Source::Base64(b) if b.media_type == "application/pdf"))
    );
    let ToolDef::Function(f) = &r.tools[0] else {
        panic!()
    };
    assert_eq!(f.strict, Some(true));
    assert!(f.parameters.as_ref().unwrap().get().contains("Weekly") || f.parameters.is_some());
}

#[test]
fn image_and_file_sources() {
    let r = generate("image-base64-file-id");
    let b = &r.messages[0].blocks;
    assert!(matches!(&b[1], Block::Image(i) if matches!(&i.source, Source::Base64(_))));
    assert!(
        matches!(&b[2], Block::Image(i) if matches!(&i.source, Source::FileId(f) if f.id == "file-6F2ksmvXxt4VdoqmHRw6kL" && f.origin.is(R)))
    );
    assert!(
        matches!(&b[3], Block::Image(i) if matches!(&i.source, Source::Url(u) if u.starts_with("https://")))
    );
    let r = generate("file-input-url");
    assert!(
        matches!(&r.messages[0].blocks[1], Block::Document(d) if matches!(&d.source, Source::Url(_)))
    );
    let r = generate("file-input-base64-file-id");
    assert!(
        matches!(&r.messages[0].blocks[2], Block::Document(d) if matches!(&d.source, Source::FileId(_)))
    );
}

#[test]
fn output_formats_tools_and_choices() {
    let r = generate("structured-output-json-schema");
    let OutputSpec::JsonSchema(s) = &r.output else {
        panic!()
    };
    assert_eq!(s.name.as_deref(), Some("calendar_event"));
    assert_eq!(s.strict, Some(true));
    assert!(s.description.is_some());
    assert_eq!(
        r.system,
        vec![Block::text("Extract the event information.")]
    );
    assert_eq!(generate("json-object-mode").output, OutputSpec::JsonObject);
    assert_eq!(generate("json-object-mode").sampling.temperature, Some(0.0));
    assert_eq!(
        generate("tool-choice-none").tool_choice,
        Some(ToolChoice::None)
    );
    assert_eq!(
        generate("tools-parallel").tool_choice,
        Some(ToolChoice::Required)
    );
    assert_eq!(generate("tools-parallel").parallel_tool_calls, Some(true));
    assert_eq!(
        generate("function-call-stream").tool_choice,
        Some(ToolChoice::Tool("get_weather".into()))
    );
    let r = generate("tool-choice-allowed-tools");
    assert!(matches!(&r.tool_choice, Some(ToolChoice::Unknown(i)) if i.origin.is(R)));
    assert!(matches!(&r.tools[2], ToolDef::Unknown(_)));
    assert_eq!(r.parallel_tool_calls, Some(false));
    let r = generate("tools-function");
    let ToolDef::Function(f) = &r.tools[0] else {
        panic!()
    };
    assert_eq!(f.strict, None);
}

#[test]
fn sampling_user_and_extras() {
    let r = generate("sampling-and-ids");
    assert_eq!(r.sampling.temperature, Some(0.2));
    assert_eq!(r.sampling.top_p, Some(0.9));
    assert_eq!(r.sampling.max_output_tokens, Some(64));
    assert_eq!(r.user.as_deref(), Some("5f2b8c1e9a7d4b3c8e6f1a2d3c4b5a69"));
    for key in [
        "user",
        "metadata",
        "service_tier",
        "truncation",
        "prompt_cache_key",
        "top_logprobs",
        "include",
    ] {
        assert!(r.extras.fields.contains_key(key), "{key}");
    }
    let r = generate("unknown-top-level-field");
    assert!(r.extras.fields.contains_key("x_future_field"));
    let r = generate("explicit-nulls");
    assert!(
        r.extras.fields.is_empty(),
        "nulls and defaults are presentation"
    );
    assert_eq!(r.tool_choice, Some(ToolChoice::Auto));
}

#[test]
fn unknown_items_references_breakpoints_and_phase() {
    let r = generate("unknown-item-type");
    assert_eq!(r.messages.len(), 2);
    assert!(matches!(&r.messages[1].blocks[..], [Block::Unknown(i)] if i.origin.is(R)));
    let r = generate("item-reference-breakpoint");
    assert!(matches!(&r.messages[0].blocks[..], [Block::Unknown(_)]));
    assert_eq!(r.messages[0].role, Role::User);
    let Block::Text(t) = &r.messages[1].blocks[0] else {
        panic!()
    };
    assert!(t.cache.is_some(), "explicit breakpoint is a cache hint");
    assert!(r.messages[2].extras.fields.contains_key("phase"));
    assert!(r.extras.fields.contains_key("prompt_cache_options"));
}

#[test]
fn input_tokens_requests_decode_as_count_tokens() {
    let body = fixture("requests/input_tokens", "tools-and-instructions", "json");
    let Request::CountTokens(r) = decode_req(RequestKind::CountTokens, &body) else {
        panic!()
    };
    assert_eq!(r.system.len(), 1);
    assert_eq!(r.reasoning.unwrap().effort, Some(ReasoningEffort::Low));
    assert!(r.extras.fields.contains_key("text.verbosity"));
    assert!(!r.extras.fields.contains_key("truncation"));
    assert_eq!(r.extras.form["set"]["truncation"], "disabled");
}

#[test]
fn allowlisted_headers_are_kept_and_forwarded() {
    let headers = HeaderList::new()
        .with("OpenAI-Beta", "responses=v1")
        .with("Authorization", "Bearer sk-secret");
    let req = OpenAiResponses::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","input":"hi"}"#,
        &headers,
    )
    .unwrap();
    let (enc, _) = encode_req(&req);
    assert_eq!(enc.headers.get("openai-beta"), Some("responses=v1"));
    assert_eq!(enc.headers.get("authorization"), None);
    assert_eq!(enc.headers.get("content-type"), Some("application/json"));
}

#[test]
fn body_keys_shaped_like_headers_or_scopes_stay_in_the_body() {
    let body = br#"{"model":"m","input":"hi","header:authorization":"Bearer x","header:openai-beta":"v","@x":1,"~event":2}"#;
    let req = decode_req(RequestKind::Generate, body);
    let (enc, _) = encode_req(&req);
    assert_eq!(enc.headers.get("authorization"), None);
    assert_eq!(enc.headers.get("openai-beta"), None);
    assert_json_eq("header-shaped keys", body, &enc.body);
}

#[test]
fn invalid_bodies_are_invalid_requests() {
    for body in [
        &b"[1]"[..],
        b"{",
        br#"{"input":"x"}"#,
        br#"{"model":"m","max_output_tokens":"many"}"#,
    ] {
        let err = OpenAiResponses::decode_request(RequestKind::Generate, body, &HeaderList::new())
            .unwrap_err();
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::InvalidRequest, "{err}");
    }
    let err =
        OpenAiResponses::decode_request(RequestKind::Embed, b"{}", &HeaderList::new()).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("embeddings"));
    let ok = br#"{"model":"m","input":"x","store":false,"background":false,"previous_response_id":null}"#;
    OpenAiResponses::decode_request(RequestKind::Generate, ok, &HeaderList::new()).unwrap();
    let err = OpenAiResponses::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","input":"x","store":true}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("store"));
}

#[test]
fn typed_fields_changed_after_decoding_win_over_recorded_spellings() {
    let mut r = generate("explicit-nulls");
    r.system = vec![Block::text("Be nice.")];
    r.messages.push(Message::assistant("Hi."));
    r.sampling.max_output_tokens = Some(100);
    r.sampling.temperature = Some(0.5);
    r.sampling.top_p = Some(0.8);
    r.parallel_tool_calls = Some(false);
    r.tools
        .push(ToolDef::Function(FunctionTool::new("f", None)));
    r.tool_choice = Some(ToolChoice::Required);
    r.output = OutputSpec::JsonObject;
    let mut cfg = kaeso_kpp::ReasoningConfig::default();
    cfg.effort = Some(ReasoningEffort::Low);
    cfg.include_encrypted = true;
    r.reasoning = Some(cfg);
    r.user = Some("u".into());
    r.stream = StreamMode::on(true);
    let body = body_of(r);
    assert_eq!(body["instructions"], "Be nice.");
    assert_eq!(
        body["input"][1],
        json!({"role": "assistant", "content": "Hi."})
    );
    assert_eq!(body["max_output_tokens"], 100);
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["top_p"], 0.8);
    assert_eq!(body["parallel_tool_calls"], false);
    assert_eq!(body["tools"], json!([{"type": "function", "name": "f"}]));
    assert_eq!(body["tool_choice"], "required");
    assert_eq!(body["text"]["format"], json!({"type": "json_object"}));
    assert_eq!(body["reasoning"], json!({"effort": "low"}));
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(body["safety_identifier"], "u");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], Value::Null);
    assert_eq!(body["conversation"], Value::Null);

    let mut r = match decode_req(
        RequestKind::Generate,
        br#"{"model":"m","prompt":{"id":"p"}}"#,
    ) {
        Request::Generate(r) => r,
        _ => panic!(),
    };
    r.messages.push(Message::user("hi"));
    assert_eq!(
        body_of(r)["input"],
        json!([{"role": "user", "content": "hi"}])
    );

    for spelling in [r#"{"effort":null}"#, "null"] {
        let body = format!(r#"{{"model":"m","input":"x","reasoning":{spelling}}}"#);
        let mut r = match decode_req(RequestKind::Generate, body.as_bytes()) {
            Request::Generate(r) => r,
            _ => panic!(),
        };
        let mut cfg = r.reasoning.clone().unwrap_or_default();
        cfg.effort = Some(ReasoningEffort::High);
        r.reasoning = Some(cfg);
        assert_eq!(body_of(r)["reasoning"]["effort"], "high", "{spelling}");
    }
}

#[test]
fn count_tokens_requests_omit_create_only_fields() {
    let body = br#"{"input":"x","temperature":0.5,"stream":true}"#;
    let req = decode_req(RequestKind::CountTokens, body);
    let Request::CountTokens(r) = &req else {
        panic!()
    };
    assert_eq!(r.model, "");
    assert!(
        r.sampling.temperature.is_none(),
        "create-only fields stay extras"
    );
    let (enc, _) = encode_req(&req);
    assert_json_eq("count", body, &enc.body);
    let mut g = GenerateRequest::new("gpt-5.5").with_message(Message::user("hi"));
    g.sampling.max_output_tokens = Some(10);
    g.sampling.temperature = Some(1.0);
    g.user = Some("u".into());
    g.stream = StreamMode::on(true);
    let (enc, _) = encode_req(&Request::CountTokens(g));
    let v: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        v,
        json!({"model": "gpt-5.5", "input": [{"role": "user", "content": "hi"}]})
    );
    assert_eq!(enc.path, INPUT_TOKENS_PATH);
    let err = OpenAiResponses::decode_request(
        RequestKind::Generate,
        br#"{"input":"x"}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("model"));
}

#[test]
fn request_errors_name_the_field_without_parser_positions() {
    let err = OpenAiResponses::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","input":"x","max_output_tokens":"many"}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_output_tokens"));
    assert!(
        err.message
            .contains("`max_output_tokens` must be a non-negative integer"),
        "{}",
        err.message
    );
    assert!(!err.message.contains("line"), "{}", err.message);
    let err = OpenAiResponses::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","input":"x","model":"n"}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert!(err.message.contains("duplicate"), "{}", err.message);
}

#[test]
fn max_output_tokens_below_the_documented_minimum_is_refused() {
    for low in [0, 15] {
        let body = format!(r#"{{"model":"m","input":"x","max_output_tokens":{low}}}"#);
        let err = OpenAiResponses::decode_request(
            RequestKind::Generate,
            body.as_bytes(),
            &HeaderList::new(),
        )
        .unwrap_err();
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::InvalidRequest);
        assert_eq!(err.param.as_deref(), Some("max_output_tokens"));
        assert_eq!(
            err.message,
            "The request body is not a valid OpenAI Responses request: `max_output_tokens` must be at least 16."
        );
    }
    let req = decode_req(
        RequestKind::Generate,
        br#"{"model":"m","input":"x","max_output_tokens":16}"#,
    );
    let Request::Generate(r) = &req else { panic!() };
    assert_eq!(r.sampling.max_output_tokens, Some(16));
    let count = decode_req(
        RequestKind::CountTokens,
        br#"{"model":"m","input":"x","max_output_tokens":0}"#,
    );
    let (enc, _) = encode_req(&count);
    assert_json_eq(
        "count",
        br#"{"model":"m","input":"x","max_output_tokens":0}"#,
        &enc.body,
    );
}
