use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, ErrorKind, RequestKind, StreamMode};

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
