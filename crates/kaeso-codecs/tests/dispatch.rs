use kaeso_codecs::anthropic_messages::AnthropicMessages;
use kaeso_codecs::openai_chat::OpenAiChat;
use kaeso_codecs::openai_embeddings::OpenAiEmbeddings;
use kaeso_codecs::openai_responses::OpenAiResponses;
use kaeso_codecs::{Codec, EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, DocumentBlock, ErrorKind, GenerateRequest, Message, Origin, Request,
    RequestKind, Role, Source, StreamMode,
};

const DIALECTS: [DialectId; 4] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::OpenAiEmbeddings,
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
        ],
        DIALECTS
    );
}
