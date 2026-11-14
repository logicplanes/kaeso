use kaeso_codecs::anthropic_messages::AnthropicMessages;
use kaeso_codecs::elevenlabs::ElevenLabsCall;
use kaeso_codecs::gemini::Gemini;
use kaeso_codecs::gemini_embeddings::GeminiEmbeddings;
use kaeso_codecs::limits::Checked;
use kaeso_codecs::openai_chat::OpenAiChat;
use kaeso_codecs::openai_embeddings::OpenAiEmbeddings;
use kaeso_codecs::openai_responses::OpenAiResponses;
use kaeso_codecs::{Codec, EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, DocumentBlock, ErrorKind, GenerateRequest, ImageOperation, Message, Origin,
    RawItem, RawJson, Request, RequestKind, Role, Source, StreamMode, VideoOperation, VideoRequest,
};

const DIALECTS: [DialectId; 6] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::OpenAiEmbeddings,
    DialectId::Gemini,
    DialectId::GeminiEmbeddings,
];

#[test]
fn every_dialect_decodes_provider_errors() {
    let headers = HeaderList::new().with("retry-after", "7");
    for dialect in DIALECTS {
        let err = dispatch::decode_error(
            dialect,
            429,
            &headers,
            b"{\"error\":{\"message\":\"Slow down.\"}}",
        );
        assert_eq!(err.kind, ErrorKind::RateLimited, "{dialect:?}");
        assert_eq!(err.status, Some(429), "{dialect:?}");
        let garbage = dispatch::decode_error(
            dialect,
            502,
            &HeaderList::new(),
            b"<html>bad gateway</html>",
        );
        assert_eq!(garbage.status, Some(502), "{dialect:?}");
    }
}

#[test]
fn every_dialect_encodes_errors_with_a_status() {
    let err = kaeso_kpp::KppError::new(ErrorKind::NotFound, "No such model.").with_status(404);
    for dialect in DIALECTS {
        let encoded = dispatch::encode_error(dialect, &err);
        assert_eq!(encoded.status, 404, "{dialect:?}");
        assert!(!encoded.body.is_empty(), "{dialect:?}");
    }
}

#[test]
fn only_anthropic_error_bodies_carry_a_request_id() {
    let err = kaeso_kpp::KppError::new(ErrorKind::NotFound, "No such model.").with_status(404);
    for dialect in DIALECTS {
        let plain = dispatch::encode_error(dialect, &err);
        let tagged = dispatch::encode_error_for_request(dialect, &err, "req_kaeso_1");
        let body: serde_json::Value = serde_json::from_slice(&tagged.body).unwrap();
        assert_eq!(tagged.status, plain.status, "{dialect:?}");
        if dialect == DialectId::AnthropicMessages {
            assert_eq!(body["request_id"], "req_kaeso_1");
        } else {
            assert_eq!(tagged.body, plain.body, "{dialect:?}");
            assert!(!String::from_utf8_lossy(&tagged.body).contains("req_kaeso_1"));
        }
    }
}

#[test]
fn an_embeddings_codec_does_not_read_generation_replies() {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let err = dispatch::decode_response(
        DialectId::OpenAiEmbeddings,
        200,
        &HeaderList::new(),
        b"{\"object\":\"list\",\"data\":[]}",
        &ctx,
    )
    .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
}

#[test]
fn base64_vectors_that_are_not_whole_floats_stay_base64() {
    let ctx = EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off);
    let body = br#"{"object":"list","data":[{"object":"embedding","index":0,"embedding":"AAA="}],"model":"text-embedding-3-small","usage":{"prompt_tokens":1,"total_tokens":1}}"#;
    let reply = dispatch::decode_response(
        DialectId::OpenAiEmbeddings,
        200,
        &HeaderList::new(),
        body,
        &ctx,
    )
    .unwrap();
    let encode_ctx = kaeso_codecs::EncodeCtx::new(RequestKind::Embed, StreamMode::Off);
    let out = dispatch::encode_response(DialectId::OpenAiEmbeddings, &reply, &encode_ctx).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["data"][0]["embedding"], "AAA=");
}

#[test]
fn both_openai_dialects_write_the_same_retry_headers() {
    let err = kaeso_kpp::KppError::new(ErrorKind::RateLimited, "Slow down.")
        .with_retry_after(std::time::Duration::from_micros(1_000_500));
    let headers = |dialect| {
        dispatch::encode_error(dialect, &err)
            .headers
            .iter()
            .filter(|(k, _)| k.starts_with("retry-after"))
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        headers(DialectId::OpenAiChat),
        ["retry-after=2", "retry-after-ms=1001"]
    );
    assert_eq!(
        headers(DialectId::OpenAiResponses),
        headers(DialectId::OpenAiChat)
    );
    assert_eq!(headers(DialectId::AnthropicMessages), ["retry-after=2"]);
}

#[test]
fn check_refuses_a_file_id_of_another_provider_like_encode_does() {
    let issued = Origin::new(DialectId::AnthropicMessages).with_provider("anthropic");
    let doc = DocumentBlock::new(Source::file_id(issued, "file_011"));
    let mut generate = GenerateRequest::new("m")
        .with_message(Message::new(Role::User, vec![Block::Document(doc)]));
    generate.sampling.max_output_tokens = Some(256);
    let req = Request::Generate(generate);
    let same = EgressCtx::new(&req).with_provider("anthropic");
    dispatch::check(DialectId::AnthropicMessages, &req, &same).unwrap();
    let mut other = EgressCtx::new(&req).with_provider("bedrock");
    let checked = dispatch::check(DialectId::AnthropicMessages, &req, &other).unwrap_err();
    let encoded =
        dispatch::encode_request(DialectId::AnthropicMessages, &req, &mut other).unwrap_err();
    assert_eq!(checked, encoded);
    assert_eq!(checked.unsupported_feature(), Some("file id"));
    assert!(
        checked.message.contains("`file_011`"),
        "{}",
        checked.message
    );
}

#[test]
fn every_codec_type_has_the_same_traits() {
    fn dialect<C>() -> DialectId
    where
        C: Codec + Clone + Copy + std::fmt::Debug + Default + Eq + std::hash::Hash,
    {
        assert_eq!(C::default(), C::default());
        C::DIALECT
    }
    assert_eq!(
        [
            dialect::<AnthropicMessages>(),
            dialect::<OpenAiChat>(),
            dialect::<OpenAiResponses>(),
            dialect::<OpenAiEmbeddings>(),
            dialect::<Gemini>(),
            dialect::<GeminiEmbeddings>(),
        ],
        DIALECTS
    );
}

fn with(mut body: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    if let (Some(body), serde_json::Value::Object(extra)) = (body.as_object_mut(), extra) {
        body.extend(extra);
    }
    body
}

#[test]
fn cross_format_refusals_name_the_field_the_app_sent() {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C, OpenAiResponses as R};
    use serde_json::json;
    let refusal = |app: DialectId, body: serde_json::Value, target: DialectId| {
        let body = serde_json::to_vec(&body).unwrap();
        let req = dispatch::decode_request(app, RequestKind::Generate, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{app:?} cannot decode the request: {e}"));
        dispatch::check(target, &req, &EgressCtx::new(&req)).unwrap_err()
    };
    let chat = |extra| {
        with(
            json!({"model": "m", "messages": [{"role": "user", "content": "hi"}]}),
            extra,
        )
    };
    let anthropic = |extra| {
        with(
            json!({"model": "m", "max_tokens": 1024, "messages": [{"role": "user", "content": "hi"}]}),
            extra,
        )
    };
    let responses = |extra| with(json!({"model": "m", "input": "hi"}), extra);
    let cases = [
        (
            C,
            chat(json!({"max_completion_tokens": 10})),
            R,
            "max_completion_tokens",
        ),
        (C, chat(json!({"max_tokens": 10})), R, "max_tokens"),
        (
            C,
            chat(json!({"max_tokens": 10, "max_completion_tokens": 10})),
            R,
            "max_completion_tokens",
        ),
        (A, anthropic(json!({"max_tokens": 10})), R, "max_tokens"),
        (C, chat(json!({})), A, "max_completion_tokens"),
        (C, chat(json!({"max_tokens": null})), A, "max_tokens"),
        (
            C,
            chat(json!({"max_completion_tokens": 10, "max_tokens": null})),
            R,
            "max_completion_tokens",
        ),
        (
            C,
            chat(json!({"max_completion_tokens": null})),
            A,
            "max_completion_tokens",
        ),
        (
            C,
            chat(json!({"max_tokens": null, "max_completion_tokens": null})),
            A,
            "max_completion_tokens",
        ),
        (R, responses(json!({})), A, "max_output_tokens"),
        (
            A,
            anthropic(json!({"stop_sequences": ["a", "b", "c", "d", "e"]})),
            C,
            "stop_sequences",
        ),
        (
            A,
            anthropic(json!({"stop_sequences": ["a"]})),
            R,
            "stop_sequences",
        ),
        (
            C,
            chat(json!({"max_tokens": 64, "response_format": {"type": "json_object"}})),
            A,
            "response_format",
        ),
        (
            R,
            responses(
                json!({"max_output_tokens": 64, "text": {"format": {"type": "json_object"}}}),
            ),
            A,
            "text.format",
        ),
        (
            C,
            chat(json!({"max_tokens": 64, "reasoning_effort": "minimal"})),
            A,
            "reasoning_effort",
        ),
        (
            R,
            responses(json!({"max_output_tokens": 64, "reasoning": {"effort": "minimal"}})),
            A,
            "reasoning.effort",
        ),
    ];
    for (app, body, target, field) in cases {
        let err = refusal(app, body, target);
        assert_eq!(
            err.param.as_deref(),
            Some(field),
            "{app:?} to {target:?}: {err:?}"
        );
        if app != A {
            let sent: serde_json::Value =
                serde_json::from_slice(&dispatch::encode_error(app, &err).body).unwrap();
            assert_eq!(
                sent["error"]["param"], field,
                "{app:?} to {target:?}: {sent}"
            );
        }
    }
    assert_eq!(
        refusal(C, chat(json!({"max_completion_tokens": 10})), R).message,
        "`max_completion_tokens 10` is not supported by OpenAI Responses. OpenAI Responses needs an output limit of at least 16 tokens; raise it or route the request to another model."
    );
    assert_eq!(
        refusal(C, chat(json!({})), A).message,
        "Anthropic Messages requires an output limit. Set `max_completion_tokens` for the request."
    );
}

#[test]
fn refusals_of_requests_built_in_code_name_the_target_field() {
    let mut generate = GenerateRequest::new("m").with_message(Message::user("hi"));
    generate.sampling.max_output_tokens = Some(10);
    let req = Request::Generate(generate.clone());
    let err = dispatch::check(DialectId::OpenAiResponses, &req, &EgressCtx::new(&req)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_output_tokens"));
    generate.sampling.max_output_tokens = None;
    let req = Request::Generate(generate);
    let err =
        dispatch::check(DialectId::AnthropicMessages, &req, &EgressCtx::new(&req)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_tokens"));
}

#[test]
fn a_chat_fixture_with_a_null_max_tokens_is_refused_for_anthropic_under_that_name() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/openai_chat/requests/max-tokens-null.json");
    let body = std::fs::read(path).unwrap();
    let req = dispatch::decode_request(
        DialectId::OpenAiChat,
        RequestKind::Generate,
        &body,
        &HeaderList::new(),
    )
    .unwrap();
    let err =
        dispatch::check(DialectId::AnthropicMessages, &req, &EgressCtx::new(&req)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_tokens"));
    assert_eq!(
        err.message,
        "Anthropic Messages requires an output limit. Set `max_tokens` for the request."
    );
}

#[test]
fn only_a_responses_stream_folds_an_item_it_does_not_model_into_the_item_it_ended_as() {
    let item = |d: DialectId, v: &str| RawItem::new(d, RawJson::from_string(v.to_owned()).unwrap());
    let r = DialectId::OpenAiResponses;
    let start = item(r, r#"{"type":"web_search_call","status":"in_progress"}"#);
    let done = item(r, r#"{"type":"web_search_call","status":"completed"}"#);
    assert_eq!(
        dispatch::finished_item(r, &start, std::slice::from_ref(&done)),
        Some(done)
    );
    assert_eq!(dispatch::finished_item(r, &start, &[]), Some(start.clone()));
    let a = DialectId::AnthropicMessages;
    assert_eq!(dispatch::finished_item(a, &start, &[]), None);
    let progress = item(
        r,
        r#"{"event":"response.web_search_call.searching","data":{"type":"response.web_search_call.searching","item_id":"ws_1","output_index":1}}"#,
    );
    let other = item(
        r,
        r#"{"event":"response.queued","data":{"type":"response.queued"}}"#,
    );
    assert!(dispatch::reports_item_progress(r, &progress));
    assert!(!dispatch::reports_item_progress(r, &other));
    assert!(!dispatch::reports_item_progress(a, &progress));
}

#[test]
fn a_checked_body_of_every_json_dialect_decodes_as_the_dialect_decodes_it() {
    let json = HeaderList::new().with("content-type", "application/json");
    let mut compared = 0;
    for dialect in DialectId::ALL {
        if matches!(
            dialect,
            DialectId::Gemini
                | DialectId::GeminiEmbeddings
                | DialectId::OpenAiImages
                | DialectId::OpenAiTranscriptions
        ) {
            continue;
        }
        let kinds: &[RequestKind] = if dialect == DialectId::OpenAiVideos {
            &[RequestKind::Video, RequestKind::Decide]
        } else {
            &[
                RequestKind::Generate,
                RequestKind::Image,
                RequestKind::Speech,
                RequestKind::Video,
                RequestKind::Decide,
            ]
        };
        for &kind in kinds {
            let body = br#"{"model":"m","prompt":"A fox","text":"Hi","model_id":"m"}"#;
            let direct = dispatch::decode_request(dialect, kind, body, &json);
            let checked = Checked::new(dialect, &body[..])
                .and_then(|c| dispatch::decode_checked_request(kind, &c, &json));
            assert_eq!(
                format!("{checked:?}"),
                format!("{direct:?}"),
                "{dialect} {kind:?}"
            );
            compared += usize::from(checked.is_ok());
        }
    }
    assert!(compared >= 10, "only {compared} decoded");
}

#[test]
fn a_checked_gemini_body_is_sent_to_the_path_decoder() {
    for dialect in [DialectId::Gemini, DialectId::GeminiEmbeddings] {
        let checked = Checked::new(dialect, &b"{}"[..]).unwrap();
        let err =
            dispatch::decode_checked_request(RequestKind::Generate, &checked, &HeaderList::new())
                .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest);
        assert_eq!(
            err.message,
            format!(
                "A {} request names its model and method in the path; decode it with `decode_gemini_request`.",
                dialect.display_name()
            )
        );
    }
}

#[test]
fn an_images_api_body_decodes_for_the_operation_its_path_names() {
    let headers = HeaderList::new().with("content-type", "application/json");
    let req = dispatch::decode_image_request(
        ImageOperation::Generate,
        br#"{"model":"gpt-image-1","prompt":"A fox"}"#,
        &headers,
    )
    .unwrap();
    let Request::Image(r) = req else {
        panic!("{req:?}")
    };
    assert_eq!(
        (r.model.as_str(), r.prompt.as_deref(), r.operation),
        ("gpt-image-1", Some("A fox"), ImageOperation::Generate)
    );
    let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
    let err = dispatch::decode_image_request(ImageOperation::Generate, deep.as_bytes(), &headers)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
}

#[test]
fn a_checked_elevenlabs_body_decodes_as_the_call_decodes_it() {
    let call = ElevenLabsCall::from_path("/v1/sound-generation", "").unwrap();
    let headers = HeaderList::new().with("content-type", "application/json");
    let body = br#"{"text":"Rain on a tin roof"}"#;
    let direct = dispatch::decode_elevenlabs_request(&call, body, &headers).unwrap();
    let checked = Checked::new(DialectId::ElevenLabs, body.to_vec()).unwrap();
    let again = dispatch::decode_checked_elevenlabs_request(&call, &checked, &headers).unwrap();
    assert_eq!(again, direct);
    assert!(matches!(&again, Request::Speech(_)), "{again:?}");
}

#[test]
fn a_remix_of_a_real_video_id_is_sent_and_only_elevenlabs_waits_between_task_polls() {
    let mut remix = VideoRequest::new("sora-2", VideoOperation::Remix).with_prompt("Add snow");
    remix.remix_of = Some("video_123".into());
    let req = Request::Video(remix);
    let enc =
        dispatch::encode_request(DialectId::OpenAiVideos, &req, &mut EgressCtx::new(&req)).unwrap();
    assert!(enc.path.contains("video_123"), "{}", enc.path);
    for dialect in DialectId::ALL {
        let gap = dispatch::task_poll_gap(dialect);
        if dialect == DialectId::ElevenLabs {
            assert_eq!(gap, std::time::Duration::from_secs(2));
        } else {
            assert_eq!(gap, std::time::Duration::ZERO, "{dialect}");
        }
    }
}
