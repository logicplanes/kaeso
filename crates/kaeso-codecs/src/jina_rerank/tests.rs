use kaeso_kpp::{
    DialectId, ErrorKind, Reply, Request, RequestKind, RerankDocument, RerankRequest,
    RerankResponse, RerankResult, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "jina_rerank";
const J: DialectId = DialectId::JinaRerank;

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn decode(body: &[u8]) -> RerankRequest {
    match JinaRerank::decode_request(RequestKind::Rerank, body, &HeaderList::new()).unwrap() {
        Request::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Rerank, StreamMode::Off).with_provider("jina")
}

fn reply(body: &[u8]) -> RerankResponse {
    match JinaRerank::decode_response(200, &HeaderList::new(), body, &ctx()).unwrap() {
        Reply::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn sent(req: RerankRequest) -> Result<Value, KppError> {
    let req = Request::Rerank(req);
    let mut ctx = EgressCtx::new(&req);
    let enc = JinaRerank::encode_request(&req, &mut ctx)?;
    assert_eq!((enc.method, enc.path.as_str()), ("POST", RERANK_PATH));
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = Request::Rerank(decode(&body));
        let mut ctx = EgressCtx::new(&req);
        let enc = JinaRerank::encode_request(&req, &mut ctx).unwrap();
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let out = JinaRerank::encode_response(
            &Reply::Rerank(reply(&body)),
            &EncodeCtx::new(RequestKind::Rerank, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let headers: HeaderList = meta
            .get("headers")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let err = JinaRerank::decode_error(status, &headers, &body);
        assert!(err.origin.as_ref().is_some_and(|o| o.is(J)), "{name}");
        let out = JinaRerank::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &body, &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
    let kind = |name: &str, status| {
        JinaRerank::decode_error(status, &HeaderList::new(), &fixture("errors", name)).kind
    };
    assert_eq!(kind("recorded-no-key-401", 401), ErrorKind::Authentication);
    assert_eq!(
        kind("recorded-invalid-key-401", 401),
        ErrorKind::Authentication
    );
    assert_eq!(kind("insufficient-balance-403", 403), ErrorKind::Billing);
    assert_eq!(kind("validation-422", 422), ErrorKind::InvalidRequest);
    assert_eq!(kind("rate-limit-429", 429), ErrorKind::RateLimited);
    let err = JinaRerank::decode_error(
        400,
        &HeaderList::new(),
        br#"{"detail":"Input too long.","code":"INPUT_TOKEN_LIMIT_EXCEEDED"}"#,
    );
    assert_eq!(err.kind, ErrorKind::ContextTooLong);
    let err = JinaRerank::decode_error(
        400,
        &HeaderList::new(),
        br#"{"detail":"No such model.","code":"INPUT_MODEL_NOT_FOUND"}"#,
    );
    assert_eq!(err.kind, ErrorKind::NotFound);
}

#[test]
fn documents_objects_max_doc_length_and_total_tokens_are_typed() {
    let r = decode(&fixture("requests", "text-objects"));
    assert_eq!(
        (r.top_n, r.return_documents, r.max_tokens_per_doc),
        (Some(2), Some(true), Some(1024))
    );
    assert_eq!(
        r.documents[0].text,
        "The Starter plan includes email support."
    );
    let v3 = decode(&fixture("requests", "v3-doc"));
    assert_eq!(v3.return_documents, Some(false));
    assert!(v3.extras.fields.contains_key("return_embeddings"));
    let answer = reply(&fixture("responses", "text-objects"));
    assert_eq!(answer.model, "jina-reranker-v3.5");
    assert_eq!(answer.usage.input_tokens, 38);
    assert_eq!(
        answer.results[1].document.as_ref().map(|d| d.text.as_str()),
        Some("The Starter plan includes email support.")
    );
}

#[test]
fn a_request_that_leaves_return_documents_out_asks_for_them_as_jina_does() {
    let r = decode(br#"{"model":"jina-reranker-v3","query":"q","documents":["a"]}"#);
    assert!(r.returns_documents());
}

#[test]
fn a_cohere_request_reaches_jina_with_return_documents_always_written() {
    let mut req = RerankRequest::new(
        "jina-reranker-v2-base-multilingual",
        "q",
        vec![RerankDocument::new("a"), RerankDocument::new("b")],
    )
    .with_top_n(1);
    // A `{"text"}` object a Cohere app wrote carries only Cohere's form marker: it goes as text.
    let mut object = kaeso_kpp::Extras::new(DialectId::CohereRerank);
    object.form.insert("object".into(), true.into());
    req.documents[1].extras = object;
    assert_eq!(
        sent(req.clone()).unwrap(),
        json!({
            "model": "jina-reranker-v2-base-multilingual",
            "query": "q",
            "documents": ["a", "b"],
            "top_n": 1,
            "return_documents": false,
        })
    );
    assert_eq!(
        sent(req.clone().with_return_documents(true)).unwrap()["return_documents"],
        true
    );

    let err = sent(req.clone().with_max_tokens_per_doc(512)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_tokens_per_doc"));
    assert_eq!(
        err.message,
        "`max_tokens_per_doc` is supported by Jina AI's `jina-reranker-v3` and `jina-reranker-v3.5` only, not `jina-reranker-v2-base-multilingual`; remove it."
    );
    let mut v3 = req.clone().with_max_tokens_per_doc(512);
    v3.model = "jina-reranker-v3.5".into();
    assert_eq!(sent(v3).unwrap()["max_doc_length"], 512);

    let mut spans = req.clone();
    spans.extras = kaeso_kpp::Extras::new(DialectId::CohereRerank).with_field(
        "relevant_spans",
        kaeso_kpp::RawJson::from_string(r#"{"mode":"enabled"}"#.into()).unwrap(),
    );
    let err = sent(spans).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("relevant_spans"));
    let mut titled = req;
    titled.documents[0].extras = kaeso_kpp::Extras::new(DialectId::CohereRerank).with_field(
        "title",
        kaeso_kpp::RawJson::from_string(r#""A""#.into()).unwrap(),
    );
    let err = sent(titled).unwrap_err();
    assert_eq!(
        err.message,
        "Jina AI takes each document as text only; remove `title` from `documents[0]`."
    );
}

#[test]
fn a_reply_from_another_format_gets_jina_usage_and_string_documents() {
    let mut r = RerankResponse::new(
        "rerank-3",
        vec![RerankResult::new(1, 0.5).with_document(RerankDocument::new("b"))],
    );
    r.usage = Usage::new(40, 0);
    let out = JinaRerank::encode_response(
        &Reply::Rerank(r),
        &EncodeCtx::new(RequestKind::Rerank, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq(
        "foreign",
        json!({
            "model": "rerank-3",
            "object": "list",
            "usage": {"total_tokens": 40},
            "results": [{"index": 1, "relevance_score": 0.5, "document": "b"}],
        })
        .to_string()
        .as_bytes(),
        &out,
    );
}

#[test]
fn other_kinds_streams_and_unread_errors_are_refused() {
    let err = JinaRerank::decode_request(
        RequestKind::Embed,
        &fixture("requests", "v2-doc"),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert!(err.message.contains("Jina Rerank"), "{}", err.message);
    let err = JinaRerank::decode_request(
        RequestKind::Rerank,
        br#"{"model":"m","query":"q","documents":[{"image":"https://example.com/a.png"}]}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("documents[0]"));
    let mut c = ctx();
    c.kind = RequestKind::Embed;
    assert!(
        JinaRerank::decode_response(
            200,
            &HeaderList::new(),
            &fixture("responses", "v3-scores"),
            &c
        )
        .is_err()
    );
    let image = br#"{"model":"jina-reranker-m0","results":[{"index":0,"relevance_score":0.5,"document":{"image":"x"}}]}"#;
    assert!(JinaRerank::decode_response(200, &HeaderList::new(), image, &ctx()).is_err());
    let unread = JinaRerank::decode_error(502, &HeaderList::new(), b"bad gateway");
    assert_eq!(unread.kind, ErrorKind::ProviderError);
    assert_eq!(
        JinaRerank::encode_error(&KppError::unsupported("x", "No.")).body,
        br#"{"detail":"No."}"#
    );
    let mut enc = JinaRerank::stream_encoder(&EncodeCtx::new(RequestKind::Rerank, StreamMode::Off));
    assert!(enc.finish().unwrap().is_empty());
    let mut dec = JinaRerank::stream_decoder(&ctx());
    assert_eq!(dec.feed(b"x").len(), 1);
    assert!(dec.finish().is_empty());
}
