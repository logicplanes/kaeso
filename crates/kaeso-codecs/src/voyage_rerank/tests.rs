use kaeso_kpp::{
    DialectId, ErrorKind, Reply, Request, RequestKind, RerankDocument, RerankRequest,
    RerankResponse, RerankResult, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "voyage_rerank";
const V: DialectId = DialectId::VoyageRerank;

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn decode(body: &[u8]) -> RerankRequest {
    match VoyageRerank::decode_request(RequestKind::Rerank, body, &HeaderList::new()).unwrap() {
        Request::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Rerank, StreamMode::Off).with_provider("voyage")
}

fn reply(body: &[u8]) -> RerankResponse {
    match VoyageRerank::decode_response(200, &HeaderList::new(), body, &ctx()).unwrap() {
        Reply::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn sent(req: RerankRequest) -> Result<Value, KppError> {
    let req = Request::Rerank(req);
    let mut ctx = EgressCtx::new(&req);
    let enc = VoyageRerank::encode_request(&req, &mut ctx)?;
    assert_eq!((enc.method, enc.path.as_str()), ("POST", RERANK_PATH));
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = Request::Rerank(decode(&body));
        let mut ctx = EgressCtx::new(&req);
        let enc = VoyageRerank::encode_request(&req, &mut ctx).unwrap();
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let out = VoyageRerank::encode_response(
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
        let err = VoyageRerank::decode_error(status, &headers, &body);
        assert!(err.origin.as_ref().is_some_and(|o| o.is(V)), "{name}");
        let out = VoyageRerank::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &body, &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
    let kind = |name: &str, status| {
        VoyageRerank::decode_error(status, &HeaderList::new(), &fixture("errors", name)).kind
    };
    assert_eq!(
        kind("recorded-invalid-key-401", 401),
        ErrorKind::Authentication
    );
    assert_eq!(
        kind("recorded-cohere-fields-400", 400),
        ErrorKind::InvalidRequest
    );
    assert_eq!(kind("recorded-no-models-404", 404), ErrorKind::NotFound);
    assert_eq!(kind("rate-limit-429", 429), ErrorKind::RateLimited);
}

#[test]
fn top_k_documents_and_total_tokens_are_typed() {
    let r = decode(&fixture("requests", "top-k-documents"));
    assert_eq!((r.top_n, r.return_documents), (Some(2), Some(true)));
    assert_eq!(r.documents.len(), 3);
    assert!(r.extras.fields.contains_key("truncation"));
    let answer = reply(&fixture("responses", "top-k-documents"));
    assert_eq!(answer.model, "rerank-3");
    assert_eq!(answer.usage.input_tokens, 94);
    assert_eq!(
        (answer.results[0].index, answer.results[0].relevance_score),
        (2, 0.9140625)
    );
    assert!(
        answer.results[0]
            .document
            .as_ref()
            .is_some_and(|d| d.text.starts_with("Apple's"))
    );
}

#[test]
fn a_cohere_request_reaches_voyage_with_top_k_and_text_documents() {
    let mut req = RerankRequest::new(
        "rerank-3-lite",
        "q",
        vec![RerankDocument::new("a"), RerankDocument::new("b")],
    )
    .with_top_n(1);
    // A `{"text"}` object the app wrote carries only a form marker, which goes nowhere.
    let mut object = kaeso_kpp::Extras::new(DialectId::CohereRerank);
    object.form.insert("object".into(), true.into());
    req.documents[1].extras = object;
    assert_eq!(
        sent(req.clone()).unwrap(),
        json!({"query": "q", "documents": ["a", "b"], "model": "rerank-3-lite", "top_k": 1})
    );
    assert_eq!(
        sent(req.clone().with_return_documents(false)).unwrap()["return_documents"],
        false
    );

    let err = sent(req.clone().with_max_tokens_per_doc(512)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("max_tokens_per_doc"));
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
        "Voyage AI takes each document as text only; remove `title` from `documents[0]`."
    );
}

#[test]
fn a_reply_from_another_format_gets_voyage_usage_and_string_documents() {
    let mut r = RerankResponse::new(
        "rerank-v5.0-fast",
        vec![RerankResult::new(1, 0.5).with_document(RerankDocument::new("b"))],
    );
    r.usage = Usage::new(40, 0);
    let out = VoyageRerank::encode_response(
        &Reply::Rerank(r),
        &EncodeCtx::new(RequestKind::Rerank, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq(
        "foreign",
        json!({
            "object": "list",
            "data": [{"index": 1, "relevance_score": 0.5, "document": "b"}],
            "model": "rerank-v5.0-fast",
            "usage": {"total_tokens": 40},
        })
        .to_string()
        .as_bytes(),
        &out,
    );
}

#[test]
fn object_documents_and_other_kinds_are_refused() {
    let err = VoyageRerank::decode_request(
        RequestKind::Rerank,
        br#"{"model":"rerank-3","query":"q","documents":[{"text":"a"}]}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    let err = VoyageRerank::decode_request(
        RequestKind::Embed,
        &fixture("requests", "basic-live"),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert!(err.message.contains("Voyage Rerank"), "{}", err.message);
    let mut c = ctx();
    c.kind = RequestKind::Embed;
    assert!(
        VoyageRerank::decode_response(
            200,
            &HeaderList::new(),
            &fixture("responses", "basic-live"),
            &c
        )
        .is_err()
    );
    let unread = VoyageRerank::decode_error(502, &HeaderList::new(), b"bad gateway");
    assert_eq!(unread.kind, ErrorKind::ProviderError);
    assert_eq!(
        VoyageRerank::encode_error(&KppError::unsupported("x", "No.")).body,
        br#"{"detail":"No."}"#
    );
}
