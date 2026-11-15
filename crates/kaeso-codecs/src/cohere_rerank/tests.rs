use kaeso_kpp::{
    DialectId, ErrorKind, Reply, Request, RequestKind, RerankDocument, RerankRequest,
    RerankResponse, RerankResult, StreamEvent, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "cohere_rerank";
const C: DialectId = DialectId::CohereRerank;

// Written by Kaeso to Cohere's v2 form, so not the body the app sent (see the folder's README).
const ACCEPTED_FORMS: &[&str] = &["object-documents"];

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn decode(body: &[u8]) -> RerankRequest {
    match CohereRerank::decode_request(RequestKind::Rerank, body, &HeaderList::new()).unwrap() {
        Request::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn rerank_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Rerank, StreamMode::Off).with_provider("cohere")
}

fn reply(body: &[u8]) -> RerankResponse {
    match CohereRerank::decode_response(200, &HeaderList::new(), body, &rerank_ctx()).unwrap() {
        Reply::Rerank(r) => r,
        other => panic!("{other:?}"),
    }
}

fn sent(req: &Request) -> Value {
    let mut ctx = EgressCtx::new(req);
    let enc = CohereRerank::encode_request(req, &mut ctx).unwrap();
    assert_eq!((enc.method, enc.path.as_str()), ("POST", RERANK_PATH));
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn every_request_fixture_round_trips() {
    let all = fixtures(DIR, "requests", "json");
    assert!(all.len() >= 4);
    for (name, body) in all {
        if ACCEPTED_FORMS.contains(&name.as_str()) {
            continue;
        }
        let req = Request::Rerank(decode(&body));
        let mut ctx = EgressCtx::new(&req);
        let enc = CohereRerank::encode_request(&req, &mut ctx).unwrap();
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let r = Reply::Rerank(reply(&body));
        let out = CohereRerank::encode_response(
            &r,
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
        let err = CohereRerank::decode_error(status, &headers, &body);
        assert!(err.origin.as_ref().is_some_and(|o| o.is(C)), "{name}");
        let out = CohereRerank::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &body, &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
    let kind = |name: &str, status| {
        CohereRerank::decode_error(status, &HeaderList::new(), &fixture("errors", name)).kind
    };
    assert_eq!(kind("invalid-api-key-401", 401), ErrorKind::Authentication);
    assert_eq!(kind("no-api-key-401", 401), ErrorKind::Authentication);
    assert_eq!(kind("invalid-request-400", 400), ErrorKind::InvalidRequest);
    assert_eq!(kind("rate-limit-429", 429), ErrorKind::RateLimited);
    assert_eq!(kind("unavailable-503", 503), ErrorKind::Overloaded);
}

#[test]
fn fields_and_billed_units_are_typed() {
    let r = decode(&fixture("requests", "context-focus"));
    assert_eq!(r.model, "rerank-v5.0-pro");
    assert_eq!(r.documents.len(), 5);
    assert_eq!(r.top_n, Some(3));
    assert_eq!(r.return_documents, None);
    assert!(r.extras.fields.contains_key("relevant_spans"));
    let r = decode(&fixture("requests", "max-tokens-per-doc"));
    assert_eq!(r.max_tokens_per_doc, Some(512));
    assert!(r.extras.fields.contains_key("priority"));

    let tokens = reply(&fixture("responses", "context-focus"));
    assert_eq!(tokens.usage.input_tokens, 192);
    assert_eq!(tokens.search_units, None);
    assert_eq!(
        tokens.id.as_deref(),
        Some("9f75d94c-5047-4f9f-ad14-4a42ef5d3ff3")
    );
    let first = &tokens.results[0];
    assert_eq!((first.index, first.relevance_score), (2, 0.8428898));
    assert!(first.extras.fields.contains_key("relevant_spans"));
    let searches = reply(&fixture("responses", "search-units"));
    assert_eq!(
        (searches.usage.input_tokens, searches.search_units),
        (0, Some(1))
    );
}

#[test]
fn object_documents_reach_cohere_as_strings_and_come_back_from_the_request() {
    let req = decode(&fixture("requests", "object-documents"));
    assert!(req.returns_documents());
    assert!(
        req.documents
            .iter()
            .all(|d| d.extras.form_flag(C, "object"))
    );
    let app = Request::Rerank(req);
    let body = sent(&app);
    assert!(
        body["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_string)
    );
    assert!(body.get("return_documents").is_none(), "{body}");

    // Cohere's v2 reply has no documents; the app's own come back as it wrote them.
    let cohere = json!({"results": [{"index": 1, "relevance_score": 0.9871}], "id": "x"});
    let r = Reply::Rerank(reply(cohere.to_string().as_bytes()));
    let out = CohereRerank::encode_response(&r, &EncodeCtx::for_request(app.clone())).unwrap();
    let out: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        out["results"][0]["document"],
        json!({"text": fixture_text(&app, 1)})
    );
    assert!(out.get("meta").is_none(), "{out}");

    // Without `return_documents` nothing is added.
    let Request::Rerank(mut plain) = app else {
        unreachable!()
    };
    plain.return_documents = Some(false);
    let out =
        CohereRerank::encode_response(&r, &EncodeCtx::for_request(Request::Rerank(plain))).unwrap();
    let out: Value = serde_json::from_slice(&out).unwrap();
    assert!(out["results"][0].get("document").is_none(), "{out}");
}

fn fixture_text(req: &Request, index: usize) -> String {
    match req {
        Request::Rerank(r) => r.documents[index].text.clone(),
        _ => unreachable!(),
    }
}

#[test]
fn a_document_with_fields_beside_text_is_refused_before_sending() {
    let body = json!({
        "model": "rerank-v5.0-fast",
        "query": "q",
        "documents": [{"text": "a"}, {"text": "b", "title": "B"}],
    });
    let req = Request::Rerank(decode(body.to_string().as_bytes()));
    let mut ctx = EgressCtx::new(&req);
    let err = CohereRerank::encode_request(&req, &mut ctx).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("document fields"));
    assert_eq!(err.param.as_deref(), Some("documents[1]"));
    assert_eq!(
        err.message,
        "Cohere's Rerank API takes each document as text only; remove `title` from `documents[1]`."
    );
}

#[test]
fn a_reply_from_another_format_gets_cohere_meta_and_object_documents() {
    let mut r = RerankResponse::new(
        "rerank-3",
        vec![RerankResult::new(1, 0.5).with_document(RerankDocument::new("b"))],
    );
    r.usage = Usage::new(40, 0);
    let out = CohereRerank::encode_response(
        &Reply::Rerank(r.clone()),
        &EncodeCtx::new(RequestKind::Rerank, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq(
        "foreign",
        json!({
            "results": [{"index": 1, "relevance_score": 0.5, "document": {"text": "b"}}],
            "meta": {"api_version": {"version": "2"}, "billed_units": {"input_tokens": 40}},
        })
        .to_string()
        .as_bytes(),
        &out,
    );
    r.usage = Usage::default();
    r.search_units = Some(2);
    let out = CohereRerank::encode_response(
        &Reply::Rerank(r),
        &EncodeCtx::new(RequestKind::Rerank, StreamMode::Off),
    )
    .unwrap();
    let out: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(out["meta"]["billed_units"], json!({"search_units": 2}));
}

#[test]
fn bodies_cohere_cant_read_are_refused_with_the_field() {
    let param = |body: Value| {
        CohereRerank::decode_request(
            RequestKind::Rerank,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap_err()
        .param
    };
    assert_eq!(
        param(json!({"query": "q", "documents": ["a"]})).as_deref(),
        Some("model")
    );
    assert_eq!(
        param(json!({"model": "m", "query": "q", "documents": []})).as_deref(),
        Some("documents")
    );
    assert_eq!(
        param(json!({"model": "m", "query": "q", "documents": ["a", 1]})).as_deref(),
        Some("documents[1]")
    );
    assert_eq!(
        param(json!({"model": "m", "query": "q", "documents": [{"title": "a"}]})).as_deref(),
        Some("documents[0]")
    );
    assert_eq!(
        param(json!({"model": "m", "query": "q", "documents": ["a"], "top_n": 0})).as_deref(),
        Some("top_n")
    );
    let many = vec!["d"; crate::limits::MAX_RERANK_DOCUMENTS + 1];
    let err = CohereRerank::decode_request(
        RequestKind::Rerank,
        json!({"model": "m", "query": "q", "documents": many})
            .to_string()
            .as_bytes(),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(
        (err.param.as_deref(), err.message.as_str()),
        (
            Some("documents"),
            "`documents` holds more than 10000 documents; Kaeso takes at most 10000."
        )
    );
}

#[test]
fn other_kinds_are_refused_both_ways() {
    let body = fixture("requests", "context-focus");
    let err =
        CohereRerank::decode_request(RequestKind::Embed, &body, &HeaderList::new()).unwrap_err();
    assert!(err.message.contains("Cohere Rerank"), "{}", err.message);
    let embed = Request::Embed(kaeso_kpp::EmbedRequest::new(
        "m",
        vec![kaeso_kpp::EmbedInput::Text("x".into())],
    ));
    assert!(CohereRerank::check(&embed, &rerank_ctx()).is_err());
    assert!(
        CohereRerank::encode_response(
            &Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![])),
            &EncodeCtx::new(RequestKind::Embed, StreamMode::Off),
        )
        .is_err()
    );
    let mut ctx = rerank_ctx();
    ctx.kind = RequestKind::Embed;
    assert!(CohereRerank::decode_response(200, &HeaderList::new(), &body, &ctx).is_err());

    // Every other codec names the rerank endpoint.
    let err = crate::dispatch::decode_request(
        DialectId::OpenAiEmbeddings,
        RequestKind::Rerank,
        b"{}",
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(
        err.message,
        "`rerank request` is not supported by OpenAI Embeddings. Send rerank requests to `/v1/rerank`."
    );
    let rerank = Request::Rerank(decode(&body));
    let mut ctx = EgressCtx::new(&rerank);
    assert!(crate::dispatch::encode_request(DialectId::OpenAiChat, &rerank, &mut ctx).is_err());
}

#[test]
fn streaming_is_refused_both_ways() {
    let mut enc =
        CohereRerank::stream_encoder(&EncodeCtx::new(RequestKind::Rerank, StreamMode::on(false)));
    assert!(
        enc.encode(&StreamEvent::Error(KppError::provider("x")))
            .is_err()
    );
    assert!(enc.finish().unwrap().is_empty());
    let mut dec = CohereRerank::stream_decoder(&rerank_ctx());
    assert!(matches!(dec.feed(b"x").as_slice(), [StreamEvent::Error(_)]));
    assert!(dec.finish().is_empty());
}

#[test]
fn an_error_for_an_app_carries_the_request_id_and_cohere_status() {
    let err = KppError::unsupported("x", "No.");
    let out = CohereRerank::encode_error_for_request(&err, "req_1");
    assert_eq!(out.status, 400);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.body).unwrap(),
        json!({"id": "req_1", "message": "No."})
    );
    let filtered = KppError::new(ErrorKind::ContentFiltered, "Blocked.");
    assert_eq!(CohereRerank::encode_error(&filtered).status, 498);
    let unread = CohereRerank::decode_error(502, &HeaderList::new(), b"<html>bad gateway</html>");
    assert_eq!(unread.kind, ErrorKind::ProviderError);
    assert!(unread.message.contains("bad gateway"), "{}", unread.message);
}
