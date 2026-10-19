use kaeso_kpp::{
    DialectId, EmbedEncoding, EmbedInput, EmbedRequest, EmbedResponse, Embedding, EmbeddingVector,
    ErrorKind, Extras, GenerateRequest, Message, RawJson, Reply, Request, RequestKind, StreamEvent,
    StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "openai_embeddings";
const E: DialectId = DialectId::OpenAiEmbeddings;

const FOREIGN_ERROR_BODIES: &[&str] = &[
    "providers/mistral/recorded-embed-output-dimension",
    "providers/mistral/recorded-string-input",
    "providers/mistral/recorded-token-ids",
];

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn decode(body: &[u8]) -> EmbedRequest {
    match OpenAiEmbeddings::decode_request(RequestKind::Embed, body, &HeaderList::new()).unwrap() {
        Request::Embed(r) => r,
        other => panic!("{other:?}"),
    }
}

fn embed_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Embed, StreamMode::Off).with_provider("openai")
}

fn encode_ctx() -> EncodeCtx {
    EncodeCtx::new(RequestKind::Embed, StreamMode::Off)
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = Request::Embed(decode(&body));
        let mut ctx = EgressCtx::new(&req);
        let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx).unwrap();
        assert_eq!(enc.path, EMBEDDINGS_PATH);
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let reply = OpenAiEmbeddings::decode_response(200, &HeaderList::new(), &body, &embed_ctx())
            .unwrap();
        let out = OpenAiEmbeddings::encode_response(&reply, &encode_ctx()).unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn error_bodies_without_an_error_object_reach_apps_as_openai_errors_that_quote_them() {
    for name in FOREIGN_ERROR_BODIES {
        let body = fixture("errors", name);
        let status = u16::try_from(
            sidecar(DIR, "errors", name, "meta").unwrap()["status"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let err = OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(
            (&err.kind, err.status),
            (&ErrorKind::InvalidRequest, Some(status)),
            "{name}"
        );
        let quoted: String = String::from_utf8_lossy(&body).chars().take(200).collect();
        assert!(
            err.message.contains(quoted.trim()),
            "{name}: {}",
            err.message
        );
        let out = OpenAiEmbeddings::encode_error(&err);
        assert_eq!(out.status, 400, "{name}");
        let sent: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(sent["error"]["message"], err.message.as_str(), "{name}");
    }
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        if FOREIGN_ERROR_BODIES.contains(&name.as_str()) {
            continue;
        }
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
        let err = OpenAiEmbeddings::decode_error(status, &headers, &body);
        assert!(err.origin.as_ref().is_some_and(|o| o.is(E)));
        let out = OpenAiEmbeddings::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &body, &out.body);
        assert_eq!(
            out.headers.get("retry-after"),
            headers.get("retry-after"),
            "{name}"
        );
    }
    let kind = |name: &str, status| {
        OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &fixture("errors", name)).kind
    };
    assert_eq!(kind("rate-limit-429", 429), ErrorKind::RateLimited);
    assert_eq!(kind("overloaded-503", 503), ErrorKind::Overloaded);
    assert_eq!(kind("invalid-api-key-401", 401), ErrorKind::Authentication);
    assert_eq!(kind("invalid-input-400", 400), ErrorKind::InvalidRequest);
}

#[test]
fn inputs_encodings_and_options_are_typed() {
    let r = decode(&fixture("requests", "string-input"));
    assert_eq!(r.input, vec![EmbedInput::Text("Hello world".into())]);
    assert_eq!(r.encoding, EmbedEncoding::Float);
    assert_eq!(r.dimensions, Some(3));
    assert_eq!(r.extras.form["encoding_format"], "float");
    let r = decode(&fixture("requests", "string-array"));
    assert_eq!(r.input.len(), 2);
    assert!(r.extras.form.get("encoding_format").is_none());
    let r = decode(&fixture("requests", "token-ids"));
    assert_eq!(
        r.input,
        vec![EmbedInput::Tokens(vec![1212, 318, 257, 1332, 13])]
    );
    let r = decode(&fixture("requests", "token-arrays"));
    assert_eq!(
        r.input,
        vec![
            EmbedInput::Tokens(vec![1212, 318, 257, 1332, 13]),
            EmbedInput::Tokens(vec![9906, 1917])
        ]
    );
    let r = decode(&fixture("requests", "base64-batch"));
    assert_eq!(r.encoding, EmbedEncoding::Base64);
    assert_eq!(r.model, "text-embedding-3-large");
    let r = decode(&fixture("requests", "user-id"));
    assert_eq!(r.user.as_deref(), Some("user-1234"));
    let r = decode(&fixture("requests", "unknown-top-level-field"));
    assert!(r.extras.fields.contains_key("x_future_field"));
    assert!(r.extras.is_from(E));
}

#[test]
fn single_element_arrays_and_nulls_keep_their_spelling() {
    for body in [
        &br#"{"model":"m","input":["one"]}"#[..],
        br#"{"model":"m","input":[[1,2]]}"#,
        br#"{"model":"m","input":[],"dimensions":null,"user":null,"encoding_format":null}"#,
    ] {
        let req = Request::Embed(decode(body));
        let mut ctx = EgressCtx::new(&req);
        let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx).unwrap();
        assert_json_eq("spelling", body, &enc.body);
    }
}

#[test]
fn invalid_requests() {
    for body in [
        &br#"{"model":"m","input":5}"#[..],
        br#"{"model":"m","input":[1,"a"]}"#,
        br#"{"model":"m","input":[-1]}"#,
        br#"{"model":"m","input":"x","encoding_format":"int8"}"#,
        br#"{"input":"x"}"#,
    ] {
        let err = OpenAiEmbeddings::decode_request(RequestKind::Embed, body, &HeaderList::new())
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{err}");
    }
    let err = OpenAiEmbeddings::decode_request(RequestKind::Generate, b"{}", &HeaderList::new())
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("generation request"));
}

#[test]
fn responses_are_typed() {
    let Reply::Embed(r) = OpenAiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "string-input"),
        &embed_ctx(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.model, "text-embedding-3-small");
    assert_eq!(r.usage.input_tokens, 2);
    assert_eq!(r.data[0].index, 0);
    assert_eq!(
        r.data[0].vector,
        EmbeddingVector::Float(vec![0.26726124, 0.53452248, 0.80178373])
    );
    let Reply::Embed(r) = OpenAiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "base64-batch"),
        &embed_ctx(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.data.len(), 3);
    assert!(matches!(&r.data[2].vector, EmbeddingVector::Base64(s) if s.starts_with("ZaIt")));
    let err = OpenAiEmbeddings::decode_response(
        429,
        &HeaderList::new(),
        &fixture("errors", "rate-limit-429"),
        &embed_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::RateLimited);
    let err = OpenAiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        br#"{"object":"list","data":[{"object":"embedding","index":0,"embedding":5}],"model":"m"}"#,
        &embed_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
}

#[test]
fn replies_built_elsewhere_get_the_canonical_shape() {
    let mut resp = EmbedResponse::new(
        "m",
        vec![Embedding::new(0, EmbeddingVector::Float(vec![0.5, -1.0]))],
    );
    resp.usage = Usage::new(7, 0);
    let out = OpenAiEmbeddings::encode_response(&Reply::Embed(resp), &encode_ctx()).unwrap();
    assert_json_eq(
        "canonical",
        &serde_json::to_vec(&json!({"object": "list", "data": [{"object": "embedding", "index": 0, "embedding": [0.5, -1.0]}],
            "model": "m", "usage": {"prompt_tokens": 7, "total_tokens": 7}}))
        .unwrap(),
        &out,
    );
    let err = OpenAiEmbeddings::encode_response(
        &Reply::Generate(kaeso_kpp::Response::new("r", "m", vec![])),
        &encode_ctx(),
    )
    .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("generation response"));
}

#[test]
fn check_refuses_generation_mixed_inputs_and_foreign_extras() {
    let gen_req = Request::Generate(GenerateRequest::new("m").with_message(Message::user("x")));
    assert_eq!(
        OpenAiEmbeddings::check(&gen_req, &EgressCtx::new(&gen_req))
            .unwrap_err()
            .unsupported_feature(),
        Some("generation request")
    );
    let mixed = EmbedRequest::new(
        "m",
        vec![EmbedInput::Text("a".into()), EmbedInput::Tokens(vec![1])],
    );
    assert_eq!(
        OpenAiEmbeddings::check(&Request::Embed(mixed), &crate::test_support::egress())
            .unwrap_err()
            .unsupported_feature(),
        Some("mixed text and token inputs")
    );
    let mut foreign = EmbedRequest::new("m", vec![EmbedInput::Text("a".into())]);
    foreign.extras = Extras::new(DialectId::OpenAiChat)
        .with_field("x", RawJson::from_string("1".into()).unwrap());
    assert_eq!(
        OpenAiEmbeddings::check(&Request::Embed(foreign), &crate::test_support::egress())
            .unwrap_err()
            .unsupported_feature(),
        Some("x")
    );
    let ok = EmbedRequest::new("m", vec![EmbedInput::Text("a".into())]);
    let req = Request::Embed(ok);
    OpenAiEmbeddings::check(&req, &EgressCtx::new(&req)).unwrap();
    let mut ctx = EgressCtx::new(&req);
    let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx).unwrap();
    assert_json_eq("min", br#"{"model":"m","input":"a"}"#, &enc.body);
}

#[test]
fn headers_are_forwarded_within_the_dialect() {
    let req = OpenAiEmbeddings::decode_request(
        RequestKind::Embed,
        br#"{"model":"m","input":"a"}"#,
        &HeaderList::new()
            .with("OpenAI-Beta", "x")
            .with("Authorization", "Bearer k"),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(enc.headers.get("openai-beta"), Some("x"));
    assert_eq!(enc.headers.get("authorization"), None);
}

#[test]
fn streaming_is_unsupported() {
    let mut enc = OpenAiEmbeddings::stream_encoder(&encode_ctx());
    let err = enc
        .encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("streaming"));
    assert!(enc.finish().unwrap().is_empty());
    let mut dec = OpenAiEmbeddings::stream_decoder(&embed_ctx());
    let events = dec.feed(b"data: {}\n\n");
    assert!(
        matches!(&events[..], [StreamEvent::Error(e)] if e.unsupported_feature() == Some("streaming"))
    );
    assert!(dec.feed(b"more").is_empty());
    assert!(dec.finish().is_empty());
}

#[test]
fn vectors_follow_the_apps_encoding_and_absent_usage_stays_absent() {
    let Reply::Embed(float) = OpenAiEmbeddings::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "base64-encoding"),
        &embed_ctx(),
    )
    .unwrap() else {
        panic!()
    };
    let req = Request::Embed(EmbedRequest::new("m", vec![EmbedInput::Text("x".into())]));
    let out = OpenAiEmbeddings::encode_response(
        &Reply::Embed(float.clone()),
        &EncodeCtx::for_request(req),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let floats = v["data"][0]["embedding"].as_array().unwrap();
    assert_eq!(floats.len(), 8);
    let mut b64 = EmbedRequest::new("m", vec![EmbedInput::Text("x".into())]);
    b64.encoding = EmbedEncoding::Base64;
    let Reply::Embed(as_float) =
        OpenAiEmbeddings::decode_response(200, &HeaderList::new(), &out, &embed_ctx()).unwrap()
    else {
        panic!()
    };
    let back = OpenAiEmbeddings::encode_response(
        &Reply::Embed(as_float),
        &EncodeCtx::for_request(Request::Embed(b64)),
    )
    .unwrap();
    assert_json_eq(
        "base64 again",
        &fixture("responses", "base64-encoding"),
        &back,
    );
    let body = br#"{"object":"list","data":[{"object":"embedding","index":0,"embedding":[0.5]}],"model":"m"}"#;
    let reply =
        OpenAiEmbeddings::decode_response(200, &HeaderList::new(), body, &embed_ctx()).unwrap();
    assert_json_eq(
        "no usage",
        body,
        &OpenAiEmbeddings::encode_response(&reply, &encode_ctx()).unwrap(),
    );
}
