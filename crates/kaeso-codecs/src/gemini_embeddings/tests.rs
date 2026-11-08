use kaeso_kpp::{
    DialectId, EmbedEncoding, EmbedInput, EmbedRequest, EmbedResponse, Embedding, EmbeddingVector,
    ErrorKind, Extras, Reply, Request, RequestKind, StreamEvent, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "gemini_embeddings";

fn call_of(name: &str) -> (String, GeminiAction) {
    let meta = sidecar(DIR, "requests", name, "meta").unwrap_or(Value::Null);
    (
        meta["model"]
            .as_str()
            .unwrap_or("gemini-embedding-2")
            .to_owned(),
        meta["action"]
            .as_str()
            .map_or(GeminiAction::EmbedContent, |a| {
                GeminiAction::from_wire(a).unwrap()
            }),
    )
}

fn embed_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off)
}

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

#[test]
fn every_request_fixture_round_trips_and_names_its_path() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let (model, action) = call_of(&name);
        let req = crate::dispatch::decode_gemini_request(&model, action, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut ctx = EgressCtx::new(&req);
        let enc = GeminiEmbeddings::encode_request(&req, &mut ctx).unwrap();
        assert_eq!(
            enc.path,
            crate::gemini::GeminiProfile::GOOGLE_AI_STUDIO
                .path(&model, action)
                .unwrap(),
            "{name}"
        );
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let reply = GeminiEmbeddings::decode_response(200, &HeaderList::new(), &body, &embed_ctx())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = GeminiEmbeddings::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Embed, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn requests_keep_their_inputs_dimensions_and_task_types() {
    let (model, action) = call_of("batch");
    let Request::Embed(r) = crate::dispatch::decode_gemini_request(
        &model,
        action,
        &fixture("requests", "batch"),
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        r.input,
        [
            EmbedInput::Text("First sentence.".into()),
            EmbedInput::Text("Second sentence.".into())
        ]
    );
    assert_eq!(r.dimensions, Some(8));
    assert!(r.extras.fields.is_empty(), "{:?}", r.extras.fields);
    let (model, action) = call_of("embed-task-title");
    let Request::Embed(r) = crate::dispatch::decode_gemini_request(
        &model,
        action,
        &fixture("requests", "embed-task-title"),
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.model, "gemini-embedding-001");
    assert_eq!(
        r.extras.fields.keys().collect::<Vec<_>>(),
        ["taskType", "title"]
    );
    let to_openai = Request::Embed(r);
    let e = crate::dispatch::check(
        DialectId::OpenAiEmbeddings,
        &to_openai,
        &EgressCtx::new(&to_openai),
    )
    .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("taskType"));
    let (model, action) = call_of("embed-config");
    let Request::Embed(r) = crate::dispatch::decode_gemini_request(
        &model,
        action,
        &fixture("requests", "embed-config"),
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.dimensions, Some(8));
    assert!(r.extras.fields.contains_key("embedContentConfig.taskType"));
}

#[test]
fn openai_embeddings_requests_become_batches() {
    let mut r = EmbedRequest::new(
        "gemini-embedding-2",
        vec![EmbedInput::Text("a".into()), EmbedInput::Text("b".into())],
    );
    r.dimensions = Some(8);
    r.encoding = EmbedEncoding::Base64;
    r.user = Some("u-1".into());
    r.extras = Extras::new(DialectId::OpenAiEmbeddings);
    let req = Request::Embed(r.clone());
    let enc = GeminiEmbeddings::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_eq!(
        enc.path,
        "/v1beta/models/gemini-embedding-2:batchEmbedContents"
    );
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body,
        json!({"requests": [
            {"model": "models/gemini-embedding-2", "content": {"parts": [{"text": "a"}]}, "outputDimensionality": 8},
            {"model": "models/gemini-embedding-2", "content": {"parts": [{"text": "b"}]}, "outputDimensionality": 8}
        ]})
    );
    r.input = vec![EmbedInput::Tokens(vec![1, 2])];
    let req = Request::Embed(r);
    let e = GeminiEmbeddings::check(&req, &EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("token-id inputs"));
    let generate = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
    assert!(GeminiEmbeddings::encode_request(&generate, &mut EgressCtx::new(&generate)).is_err());
}

#[test]
fn replies_reach_apps_of_both_embeddings_formats() {
    let reply = GeminiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "recorded-batch"),
        &embed_ctx(),
    )
    .unwrap();
    let Reply::Embed(r) = &reply else { panic!() };
    assert_eq!(r.data.len(), 2);
    assert_eq!(r.usage.input_tokens, 6);
    let mut req = EmbedRequest::new("text-embedding-3-small", vec![EmbedInput::Text("a".into())]);
    req.encoding = EmbedEncoding::Base64;
    let openai = crate::dispatch::encode_response(
        DialectId::OpenAiEmbeddings,
        &reply,
        &EncodeCtx::for_request(Request::Embed(req)),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&openai).unwrap();
    assert!(v["data"][0]["embedding"].is_string());
    assert_eq!(v["usage"]["prompt_tokens"], 6);

    let mut foreign = EmbedResponse::new(
        "text-embedding-3-small",
        vec![Embedding::new(
            0,
            EmbeddingVector::Base64("AACAPwAAAEA=".into()),
        )],
    );
    foreign.usage = Usage::new(3, 0);
    let (model, action) = call_of("embed-single");
    let single = crate::dispatch::decode_gemini_request(
        &model,
        action,
        &fixture("requests", "embed-single"),
        &HeaderList::new(),
    )
    .unwrap();
    let out = GeminiEmbeddings::encode_response(
        &Reply::Embed(foreign.clone()),
        &EncodeCtx::for_request(single),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v,
        json!({"embedding": {"values": [1.0, 2.0]}, "usageMetadata": {"promptTokenCount": 3}})
    );
    let out = GeminiEmbeddings::encode_response(
        &Reply::Embed(foreign),
        &EncodeCtx::new(RequestKind::Embed, StreamMode::Off),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["embeddings"][0]["values"], json!([1.0, 2.0]));
    let generate = Reply::Generate(kaeso_kpp::Response::new("r", "m", vec![]));
    assert!(
        GeminiEmbeddings::encode_response(
            &generate,
            &EncodeCtx::new(RequestKind::Embed, StreamMode::Off)
        )
        .is_err()
    );
    let e = GeminiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        b"{}",
        &EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("generation response"));
    let e = GeminiEmbeddings::decode_response(
        429,
        &HeaderList::new(),
        br#"{"error": {"code": 429, "message": "Slow.", "status": "RESOURCE_EXHAUSTED"}}"#,
        &embed_ctx(),
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::RateLimited);
    assert_eq!(
        e.origin.as_ref().map(|o| o.dialect),
        Some(DialectId::GeminiEmbeddings)
    );
}

#[test]
fn embeddings_never_stream() {
    let mut dec = GeminiEmbeddings::stream_decoder(&embed_ctx());
    assert!(matches!(
        dec.feed(b"data: {}\n\n").as_slice(),
        [StreamEvent::Error(_)]
    ));
    assert!(dec.finish().is_empty());
    let mut enc =
        GeminiEmbeddings::stream_encoder(&EncodeCtx::new(RequestKind::Embed, StreamMode::Off));
    assert!(
        enc.encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
            .is_err()
    );
    assert!(enc.finish().unwrap().is_empty());
    let e = GeminiEmbeddings::decode_request(RequestKind::Generate, b"{}", &HeaderList::new())
        .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("generation request"));
    let r = GeminiEmbeddings::decode_request(
        RequestKind::Embed,
        &fixture("requests", "batch"),
        &HeaderList::new(),
    )
    .unwrap();
    assert_eq!(r.model(), "");
    let out =
        GeminiEmbeddings::encode_error(&kaeso_kpp::KppError::new(ErrorKind::NotFound, "No model."));
    assert_eq!(out.status, 404);
    let models = GeminiEmbeddings::decode_models(
        200,
        &HeaderList::new(),
        br#"{"models": [{"name": "models/gemini-embedding-2"}]}"#,
    )
    .unwrap();
    assert_eq!(models.models[0].id, "gemini-embedding-2");
}
