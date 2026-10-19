use std::time::Duration;

use kaeso_kpp::{
    Block, DialectId, ErrorKind, GenerateRequest, KppError, Message, Origin, OutputSpec, RawJson,
    ReasoningEffort, Reply, Request, RequestKind, Response, Role, Source, StopReason, StreamMode,
    ToolChoice, ToolDef,
};
use serde_json::{Value, json};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const D: &str = "openai_chat";

pub(super) const REJECTED_REQUESTS: &[(&str, &str)] = &[
    ("input-audio-unsupported", "input_audio"),
    ("n-2", "n"),
    ("providers/groq/n-above-one", "n"),
];

const REJECTED_RESPONSES: &[(&str, &str)] = &[("n-2", "n")];

pub(super) fn raw(s: &str) -> RawJson {
    RawJson::from_string(s.to_owned()).unwrap()
}

pub(super) fn decode(body: &[u8]) -> Result<Request, KppError> {
    OpenAiChat::decode_request(RequestKind::Generate, body, &HeaderList::new())
}

fn decode_fixture(name: &str) -> GenerateRequest {
    let (_, body) = fixtures(D, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no request fixture {name}"));
    match decode(&body).unwrap() {
        Request::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

pub(super) fn encode(req: &GenerateRequest) -> (Value, EgressCtx) {
    let req = Request::Generate(req.clone());
    let mut ctx = EgressCtx::new(&req);
    let out = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(out.path, "/v1/chat/completions");
    assert_eq!(out.method, "POST");
    (serde_json::from_slice(&out.body).unwrap(), ctx)
}

fn decode_response_fixture(name: &str) -> Response {
    let (_, body) = fixtures(D, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    match OpenAiChat::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap() {
        Reply::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

pub(super) fn encode_reply(resp: &Response) -> Value {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1_700_000_000);
    let out = OpenAiChat::encode_response(&Reply::Generate(resp.clone()), &ctx).unwrap();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(D, "requests", "json") {
        if REJECTED_REQUESTS.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let req = decode(&body).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let mut ctx = EgressCtx::new(&req);
        let out =
            OpenAiChat::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_json_eq(&name, &body, &out.body);
        assert!(ctx.tool_names.is_empty(), "{name}: no name was rewritten");
    }
}

#[test]
fn rejected_request_fixtures_name_the_feature() {
    for (name, feature) in REJECTED_REQUESTS {
        let (_, body) = fixtures(D, "requests", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap();
        let err = decode(&body).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some(*feature), "{name}: {err:?}");
        assert!(err.param.is_some(), "{name}: {err:?}");
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(D, "responses", "json") {
        if REJECTED_RESPONSES.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        let reply = OpenAiChat::decode_response(200, &HeaderList::new(), &body, &ctx)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let out = OpenAiChat::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn rejected_response_fixtures_name_the_feature() {
    for (name, feature) in REJECTED_RESPONSES {
        let (_, body) = fixtures(D, "responses", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap();
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        let err = OpenAiChat::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some(*feature), "{name}");
    }
}

fn meta_headers(meta: &Value) -> HeaderList {
    meta.get("headers")
        .and_then(Value::as_object)
        .map(|h| {
            h.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    for (name, body) in fixtures(D, "errors", "json") {
        let meta = sidecar(D, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let headers = meta_headers(&meta);
        let err = OpenAiChat::decode_error(status, &headers, &body);
        assert_eq!(err.status, Some(status), "{name}");
        assert_eq!(
            err.origin,
            Some(Origin::new(DialectId::OpenAiChat)),
            "{name}"
        );
        let out = OpenAiChat::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &body, &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
}

#[test]
fn error_kinds_follow_the_error_codes_guide() {
    let expect = [
        ("context-length-exceeded", ErrorKind::ContextTooLong),
        ("credit-balance-exhausted", ErrorKind::Billing),
        ("insufficient-quota", ErrorKind::Billing),
        ("invalid-api-key", ErrorKind::Authentication),
        ("invalid-service-tier", ErrorKind::InvalidRequest),
        ("invalid-type-param", ErrorKind::InvalidRequest),
        ("model-not-found", ErrorKind::NotFound),
        ("rate-limit-slow-down", ErrorKind::RateLimited),
        ("rate-limit-tokens", ErrorKind::RateLimited),
        ("server-error", ErrorKind::ProviderError),
        ("server-overloaded", ErrorKind::Overloaded),
        ("unknown-error-field", ErrorKind::InvalidRequest),
        ("unsupported-country", ErrorKind::PermissionDenied),
        ("unsupported-parameter", ErrorKind::InvalidRequest),
    ];
    for (name, kind) in expect {
        let (_, body) = fixtures(D, "errors", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap();
        let meta = sidecar(D, "errors", name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = OpenAiChat::decode_error(status, &meta_headers(&meta), &body);
        assert_eq!(err.kind, kind, "{name}");
    }
    let err = OpenAiChat::decode_error(
        429,
        &HeaderList::new().with("retry-after", "20"),
        br#"{"error":{"message":"m","type":"rate_limit_error","param":null,"code":"slow_down"}}"#,
    );
    assert_eq!(err.retry_after, Some(Duration::from_secs(20)));
    let err = OpenAiChat::decode_error(400, &HeaderList::new(), br#"{"error":{"message":"m","type":"invalid_request_error","param":"messages","code":null}}"#);
    assert_eq!(err.param.as_deref(), Some("messages"));
}

#[test]
fn unknown_error_shapes_take_their_kind_from_the_status() {
    let err = OpenAiChat::decode_error(502, &HeaderList::new(), b"<html>Bad gateway</html>");
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert_eq!(err.status, Some(502));
    assert_eq!(err.raw, Some(json!("<html>Bad gateway</html>")));
    let err = OpenAiChat::decode_error(400, &HeaderList::new(), br#"{"detail":"x"}"#);
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.raw, Some(json!({"detail": "x"})));
    let out = OpenAiChat::encode_error(&err);
    assert_eq!(out.status, 400);

    let headers = HeaderList::new().with("retry-after", "7");
    let err = OpenAiChat::decode_error(
        429,
        &headers,
        b"<html><body>Too Many Requests</body></html>",
    );
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert_eq!(err.retry_after, Some(Duration::from_secs(7)));
    assert!(err.raw.is_some());
    let err = OpenAiChat::decode_error(503, &HeaderList::new(), b"upstream connect error");
    assert_eq!(err.kind, ErrorKind::Overloaded);
    let err = OpenAiChat::decode_error(429, &HeaderList::new(), br#"{"error":"rate limited"}"#);
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert_eq!(err.message, "rate limited");
    assert_eq!(err.raw, Some(json!({"error": "rate limited"})));
    let out = OpenAiChat::encode_error(&err);
    assert_eq!(out.status, 429);
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(body["error"]["type"], "rate_limit_error");
}

#[test]
fn retry_after_is_rounded_up_and_sent_in_seconds_too() {
    let out = OpenAiChat::encode_error(
        &KppError::new(ErrorKind::RateLimited, "slow")
            .with_retry_after(Duration::from_micros(1_000_400)),
    );
    assert_eq!(out.headers.get("retry-after"), Some("2"));
    assert_eq!(out.headers.get("retry-after-ms"), Some("1001"));
    let out = OpenAiChat::encode_error(
        &KppError::new(ErrorKind::RateLimited, "slow").with_retry_after(Duration::from_secs(3)),
    );
    assert_eq!(out.headers.get("retry-after"), Some("3"));
    assert_eq!(out.headers.get("retry-after-ms"), None);
}

#[test]
fn foreign_and_kaeso_errors_get_chat_shape_and_status() {
    let cases = [
        (
            KppError::unsupported_by("top_k", DialectId::OpenAiChat, "Remove it.")
                .with_param("top_k"),
            400,
            "invalid_request_error",
        ),
        (
            KppError::new(ErrorKind::Overloaded, "busy")
                .with_status(529)
                .with_origin(DialectId::AnthropicMessages)
                .with_raw(
                    json!({"type":"error","error":{"type":"overloaded_error","message":"busy"}}),
                ),
            503,
            "service_unavailable_error",
        ),
        (
            KppError::new(ErrorKind::Billing, "pay"),
            429,
            "insufficient_quota",
        ),
        (
            KppError::new(ErrorKind::Authentication, "who"),
            401,
            "invalid_request_error",
        ),
        (
            KppError::new(ErrorKind::RateLimited, "slow"),
            429,
            "rate_limit_error",
        ),
        (
            KppError::new(ErrorKind::Timeout, "late"),
            504,
            "server_error",
        ),
        (
            KppError::new(ErrorKind::ContextTooLong, "long"),
            400,
            "invalid_request_error",
        ),
        (
            KppError::provider("bad").with_status(502),
            502,
            "server_error",
        ),
    ];
    for (err, status, ty) in cases {
        let out =
            OpenAiChat::encode_error(&err.clone().with_retry_after(Duration::from_millis(1500)));
        assert_eq!(out.status, status, "{err:?}");
        let body: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(body["error"]["type"], ty, "{err:?}");
        assert_eq!(body["error"]["message"], err.message.as_str());
        assert_eq!(out.headers.get("retry-after-ms"), Some("1500"));
        assert_eq!(out.headers.get("retry-after"), Some("2"));
    }
    let out = OpenAiChat::encode_error(&KppError::new(ErrorKind::ContextTooLong, "long"));
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(body["error"]["code"], "context_length_exceeded");
    let out = OpenAiChat::encode_error(&KppError::invalid_request("bad").with_param("messages[0]"));
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(body["error"]["param"], "messages[0]");
}

#[test]
fn leading_system_and_developer_messages_become_the_system_prompt() {
    let r = decode_fixture("content-array-all-roles");
    let texts: Vec<_> = r
        .system
        .iter()
        .map(|b| match b {
            Block::Text(t) => t.text.as_str(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(texts.len(), 3);
    assert!(texts[2].starts_with("Customer plan"));
    let roles: Vec<Role> = r.messages.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [
            Role::User,
            Role::Assistant,
            Role::System,
            Role::System,
            Role::User
        ]
    );
    assert!(matches!(&r.messages[1].blocks[0], Block::Refusal(_)));
    assert!(matches!(&r.messages[1].blocks[1], Block::Text(_)));

    let r = decode_fixture("plain-developer-string");
    assert!(matches!(&r.system[..], [Block::Text(t)] if t.text == "You are a helpful assistant."));
    assert_eq!(r.messages, vec![Message::user("Hello!")]);
}

#[test]
fn system_message_name_travels_as_an_extra_of_its_first_block() {
    let r = decode_fixture("system-name-multiturn");
    let extras = r.system[0].extras().unwrap();
    assert_eq!(extras.fields["message.name"].get(), "\"house_rules\"");
    assert_eq!(r.messages[0].extras.fields["name"].get(), "\"alice\"");
    assert_eq!(r.sampling.temperature, Some(0.7));
    assert_eq!(r.sampling.top_p, Some(0.9));
    assert_eq!(
        r.reasoning.as_ref().unwrap().effort,
        Some(ReasoningEffort::None)
    );
}

#[test]
fn consecutive_tool_messages_merge_into_one_user_message() {
    let r = decode_fixture("tools-results-parallel-error");
    let last = r.messages.last().unwrap();
    assert_eq!(last.role, Role::User);
    let ids: Vec<_> = last
        .blocks
        .iter()
        .map(|b| match b {
            Block::ToolResult(t) => t.call_id.as_str(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        ids,
        [
            "call_kfCAiUZI9KHN37p4VH3N0E2a",
            "call_kfNMlUq01JIft0lr9lKKbGMM"
        ]
    );
    let assistant = &r.messages[r.messages.len() - 2];
    let Block::ToolCall(call) = &assistant.blocks[0] else {
        panic!()
    };
    assert_eq!(call.name, "get_weather");
    assert_eq!(
        call.arguments.get(),
        r#"{"location":"Paris, France","unit":null}"#
    );
}

#[test]
fn sampling_limits_and_spellings_are_typed() {
    let r = decode_fixture("stop-string-seed-penalties");
    assert_eq!(r.sampling.stop, ["END"]);
    assert_eq!(r.sampling.seed, Some(42));
    assert_eq!(r.sampling.frequency_penalty, Some(0.5));
    assert_eq!(r.sampling.presence_penalty, Some(-0.25));
    assert_eq!(r.sampling.max_output_tokens, Some(200));

    let r = decode_fixture("max-tokens-length");
    assert_eq!(r.sampling.max_output_tokens, Some(16));
    let (out, _) = encode(&r);
    assert_eq!(out["max_tokens"], 16);
    assert!(out.get("max_completion_tokens").is_none());

    let r = decode_fixture("stop-array");
    assert_eq!(r.sampling.stop, ["7", "\n\n", "</end>"]);
    assert_eq!(r.sampling.top_p, Some(0.5));

    let r = decode_fixture("all-params-sdk");
    assert_eq!(r.sampling.max_output_tokens, Some(0));
    assert_eq!(r.user.as_deref(), Some("safety-identifier-1234"));
    assert_eq!(
        r.extras.fields[super::request::KEY_USER_CONFLICT].get(),
        "\"user-1234\""
    );
    for field in super::wire::PASSTHROUGH_REQUEST_FIELDS {
        assert!(r.extras.fields.contains_key(*field), "{field} is an extra");
    }
}

#[test]
fn user_falls_back_to_the_deprecated_field() {
    let r = decode_fixture("identifiers-metadata-store");
    assert_eq!(r.user.as_deref(), Some("8f14e45fceea167a5a36dedd4bea2543"));
    let mut only_user =
        decode(br#"{"model":"m","messages":[{"role":"user","content":"x"}],"user":"u1"}"#).unwrap();
    let Request::Generate(r) = &mut only_user else {
        panic!()
    };
    assert_eq!(r.user.as_deref(), Some("u1"));
    let (out, _) = encode(r);
    assert_eq!(out["user"], "u1");
    assert!(out.get("safety_identifier").is_none());
}

#[test]
fn output_tools_and_choices_are_typed() {
    let r = decode_fixture("json-schema");
    let OutputSpec::JsonSchema(spec) = &r.output else {
        panic!()
    };
    assert_eq!(spec.name.as_deref(), Some("calendar_event"));
    assert_eq!(spec.description.as_deref(), Some("A calendar event"));
    assert_eq!(spec.strict, Some(true));
    assert!(spec.schema.get().contains("participants"));
    assert_eq!(decode_fixture("json-object").output, OutputSpec::JsonObject);
    assert_eq!(
        decode_fixture("response-format-text").output,
        OutputSpec::Text
    );

    let r = decode_fixture("tool-choice-function");
    assert_eq!(r.tool_choice, Some(ToolChoice::Tool("get_weather".into())));
    assert_eq!(r.parallel_tool_calls, Some(false));
    let ToolDef::Function(f) = &r.tools[0] else {
        panic!()
    };
    assert_eq!(f.strict, Some(true));
    assert_eq!(
        f.description.as_deref(),
        Some("Get the current weather for a location.")
    );
    assert_eq!(
        decode_fixture("tools-single").tool_choice,
        Some(ToolChoice::Required)
    );
    assert_eq!(
        decode_fixture("tool-choice-none").tool_choice,
        Some(ToolChoice::None)
    );
    assert!(matches!(
        decode_fixture("tool-choice-allowed-tools").tool_choice,
        Some(ToolChoice::Unknown(_))
    ));
    let r = decode_fixture("tools-custom-grammar");
    assert!(matches!(r.tools[0], ToolDef::Unknown(_)));
    assert!(matches!(r.tool_choice, Some(ToolChoice::Unknown(_))));
}

#[test]
fn stream_options_map_to_stream_mode() {
    assert_eq!(
        decode_fixture("stream-include-usage").stream,
        StreamMode::on(true)
    );
    assert_eq!(decode_fixture("stream-doc").stream, StreamMode::on(false));
    assert_eq!(decode_fixture("all-params-sdk").stream, StreamMode::Off);
}

#[test]
fn images_files_and_cache_breakpoints_are_typed() {
    let r = decode_fixture("image-base64-detail");
    let blocks = &r.messages[0].blocks;
    let Block::Image(img) = &blocks[1] else {
        panic!()
    };
    assert!(matches!(&img.source, Source::Base64(b) if b.media_type == "image/png"));
    assert_eq!(img.extras.fields["image_url.detail"].get(), "\"low\"");
    let Block::Image(img) = &blocks[2] else {
        panic!()
    };
    assert!(matches!(&img.source, Source::Url(u) if u.starts_with("https://")));

    let r = decode_fixture("file-pdf-data");
    let Block::Document(doc) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert!(
        matches!(&doc.source, Source::Base64(b) if b.media_type == "application/pdf" && b.data.starts_with("JVBERi"))
    );
    assert_eq!(doc.extras.fields["file.filename"].get(), "\"hello.pdf\"");

    let r = decode_fixture("file-pdf-file-id");
    let Block::Document(doc) = &r.messages[0].blocks[0] else {
        panic!()
    };
    assert_eq!(
        doc.source,
        Source::file_id(DialectId::OpenAiChat, "file-6F2ksmvXxt4VdoqmHRw6kL")
    );

    let r = decode_fixture("prompt-cache-breakpoint");
    let Block::Text(t) = &r.system[0] else {
        panic!()
    };
    let cache = t.cache.as_ref().unwrap();
    assert_eq!(cache.ttl, None);
    assert!(cache.extras.is_from(DialectId::OpenAiChat));
    assert!(r.extras.fields.contains_key("prompt_cache_options"));
}

#[test]
fn unknown_fields_and_parts_are_kept_bound_to_chat() {
    let r = decode_fixture("unknown-top-level-field");
    assert!(r.extras.fields.contains_key("x_future_field"));
    assert!(r.messages[0].extras.fields.contains_key("x_future_field"));
    let Block::Unknown(item) = &r.messages[0].blocks[1] else {
        panic!()
    };
    assert!(item.origin.is(DialectId::OpenAiChat));
}

#[test]
fn explicit_nulls_are_form_not_fields() {
    let r = decode_fixture("explicit-nulls");
    assert!(r.extras.fields.is_empty(), "{:?}", r.extras.fields);
    assert!(r.sampling.is_empty());
    assert_eq!(r.stream, StreamMode::Off);
}

#[test]
fn malformed_requests_are_invalid() {
    for body in [
        &br#"{"messages":[]}"#[..],
        br#"{"model":"m","messages":[{"role":"user","content":[]}]}"#,
        br#"{"model":"m","messages":[{"role":"wizard","content":"x"}]}"#,
        br#"{"model":"m","messages":[{"content":"x"}]}"#,
        br#"{"model":"m","messages":[{"role":"user","content":[{"type":"file","file":{}}]}]}"#,
        br#"{"model":"m","messages":[{"role":"user","content":"x"}],"n":0}"#,
        br#"{"model":"m","messages":[{"role":"user","content":"x"}],"response_format":{"type":"json_schema"}}"#,
        b"not json",
    ] {
        let err = decode(body).unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{}", String::from_utf8_lossy(body));
    }
    let err = OpenAiChat::decode_request(RequestKind::CountTokens, b"{}", &HeaderList::new())
        .unwrap_err();
    assert!(err.unsupported_feature().is_some());
}

#[test]
fn forwarded_headers_are_kept_and_sent_back() {
    let headers = HeaderList::new()
        .with("OpenAI-Beta", "assistants=v2")
        .with("authorization", "Bearer sk-secret");
    let req = OpenAiChat::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","messages":[{"role":"user","content":"x"}]}"#,
        &headers,
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    let out = OpenAiChat::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(out.headers.get("openai-beta"), Some("assistants=v2"));
    assert_eq!(out.headers.get("authorization"), None);
    assert_eq!(out.headers.get("content-type"), Some("application/json"));
    let body: Value = serde_json::from_slice(&out.body).unwrap();
    assert!(body.get("header:openai-beta").is_none());
}

#[test]
fn body_keys_never_become_headers_or_key_paths() {
    for body in [
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"header:authorization":"Bearer x"}"#,
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"response_format.zzz":1}"#,
        r#"{"model":"m","messages":[{"role":"user","content":"x","a.b":1}]}"#,
    ] {
        let err =
            OpenAiChat::decode_request(RequestKind::Generate, body.as_bytes(), &HeaderList::new())
                .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}");
        assert!(err.message.contains("cannot carry"), "{}", err.message);
    }
    let mut generate = GenerateRequest::new("m").with_message(Message::user("x"));
    generate.extras.insert_header("authorization", "Bearer x");
    generate.extras.insert_header("openai-beta", "v");
    generate.extras.origin = Some(Origin::new(DialectId::OpenAiChat));
    let req = Request::Generate(generate);
    let out = OpenAiChat::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_eq!(out.headers.get("authorization"), None);
    assert_eq!(out.headers.get("openai-beta"), Some("v"));
}

#[test]
fn usage_splits_cached_and_reasoning_tokens() {
    let r = decode_response_fixture("reasoning-cached-usage");
    assert_eq!(r.usage.input_tokens, 128);
    assert_eq!(r.usage.cache_read_tokens, 1920);
    assert_eq!(r.usage.cache_write_tokens, 0);
    assert_eq!(r.usage.output_tokens, 1186);
    assert_eq!(r.usage.reasoning_tokens, Some(1152));
    assert!(r.usage.raw.is_some());
    assert_eq!(r.stop, StopReason::EndTurn);
}

#[test]
fn response_blocks_and_stop_reasons_are_typed() {
    let r = decode_response_fixture("tools-parallel");
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(r.blocks.len(), 2);
    let r = decode_response_fixture("refusal");
    assert!(matches!(&r.blocks[0], Block::Refusal(x) if x.text.starts_with("I'm sorry")));
    assert_eq!(
        decode_response_fixture("max-tokens-length").stop,
        StopReason::MaxTokens
    );
    assert_eq!(
        decode_response_fixture("content-filter").stop,
        StopReason::ContentFilter
    );
    assert!(decode_response_fixture("content-filter").blocks.is_empty());
    let r = decode_response_fixture("web-search");
    let Block::Text(t) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(t.citations.len(), 2);
    let r = decode_response_fixture("tools-custom-grammar");
    assert!(matches!(r.blocks[0], Block::Unknown(_)));
    let r = decode_response_fixture("unknown-top-level-field");
    assert!(r.extras.fields.contains_key("x_future_field"));
    assert!(r.extras.fields.contains_key("choice.x_future_choice_field"));
    assert!(
        r.extras
            .fields
            .contains_key("choice.message.x_future_field")
    );
    assert_eq!(
        super::response::decode_finish("function_call"),
        StopReason::Other("function_call".into())
    );
}

#[test]
fn changed_usage_overrides_the_raw_object() {
    let mut r = decode_response_fixture("reasoning-cached-usage");
    r.usage.output_tokens = 2000;
    let out = encode_reply(&r);
    assert_eq!(out["usage"]["completion_tokens"], 2000);
    assert_eq!(out["usage"]["total_tokens"], 4048);
    assert_eq!(out["usage"]["completion_tokens_details"]["audio_tokens"], 0);
}

#[test]
fn other_reply_kinds_and_endpoints_are_refused() {
    let ctx = EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off);
    let reply = Reply::CountTokens(kaeso_kpp::CountTokensResponse::new(3));
    assert!(OpenAiChat::encode_response(&reply, &ctx).is_err());
    let ctx = EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off);
    assert!(OpenAiChat::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let err = OpenAiChat::decode_response(
        429,
        &HeaderList::new().with("retry-after", "3"),
        br#"{"error":{"message":"slow","type":"rate_limit_error","param":null,"code":null}}"#,
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert_eq!(err.retry_after, Some(Duration::from_secs(3)));
    let err = OpenAiChat::decode_response(200, &HeaderList::new(), b"{", &ctx).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
}

#[test]
fn provider_responses_get_the_provider_origin() {
    let (_, body) = fixtures(D, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == "tools-single")
        .unwrap();
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider("openai");
    let Reply::Generate(r) =
        OpenAiChat::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap()
    else {
        panic!()
    };
    let expected = Origin::new(DialectId::OpenAiChat).with_provider("openai");
    assert_eq!(r.extras.origin.as_ref(), Some(&expected));
    let Block::ToolCall(call) = &r.blocks[0] else {
        panic!()
    };
    assert_eq!(call.extras.origin.as_ref(), Some(&expected));
    assert_eq!(r.usage.raw.as_ref().unwrap().origin, expected);
}

#[test]
fn legacy_function_conversations_round_trip_and_stay_in_their_dialect() {
    let req = decode_fixture("legacy-functions");
    assert_eq!(req.messages.len(), 3);
    assert!(req.messages[1].blocks.is_empty());
    assert!(req.messages[1].extras.fields.contains_key("function_call"));
    assert!(matches!(
        req.messages[2].blocks.as_slice(),
        [Block::Unknown(_)]
    ));
    Request::Generate(req.clone()).validate().unwrap();
    let body = br#"{"model":"m","messages":[
        {"role":"user","content":"q"},
        {"role":"assistant","content":null,"function_call":{"name":"f","arguments":"{}"}},
        {"role":"function","name":"f","content":"1"},
        {"role":"assistant","content":"done","function_call":null},
        {"role":"function","name":"f","content":null},
        {"role":"user","content":"again"}]}"#;
    let Request::Generate(multi) = decode(body).unwrap() else {
        panic!()
    };
    assert_json_eq(
        "legacy",
        body,
        &serde_json::to_vec(&encode(&multi).0).unwrap(),
    );
    let target = Origin::new(DialectId::AnthropicMessages);
    let err = crate::cross::check_foreign(&Request::Generate(req), &target).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("functions"));
    let err = crate::cross::check_foreign(&Request::Generate(multi.clone()), &target).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("messages[1].function_call"));
    let only_result = br#"{"model":"m","messages":[{"role":"user","content":"q"},{"role":"function","name":"f","content":"1"}]}"#;
    let Request::Generate(only_result) = decode(only_result).unwrap() else {
        panic!()
    };
    let err = crate::cross::check_foreign(&Request::Generate(only_result), &target).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("`function` message"));
    assert_eq!(err.param.as_deref(), Some("messages[1].blocks[0]"));
    assert!(
        err.message
            .starts_with("`function` message comes from an OpenAI Chat Completions request"),
        "{}",
        err.message
    );
}
