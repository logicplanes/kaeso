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
        // Voyage AI reports only `total_tokens`; an OpenAI app gets it as `prompt_tokens` too.
        let mut expected: Value = serde_json::from_slice(&body).unwrap();
        if name.starts_with("providers/voyage/") {
            let usage = expected["usage"].as_object_mut().unwrap();
            assert!(!usage.contains_key("prompt_tokens"), "{name}");
            usage.insert("prompt_tokens".into(), usage["total_tokens"].clone());
        }
        assert_json_eq(&name, &serde_json::to_vec(&expected).unwrap(), &out);
    }
}

#[test]
fn error_bodies_without_an_error_object_reach_apps_as_openai_errors_with_their_message() {
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
        let original: Value = serde_json::from_slice(&body).unwrap();
        match original["message"].as_str() {
            Some(message) => assert_eq!(err.message, message, "{name}"),
            None => {
                let quoted: String = String::from_utf8_lossy(&body).chars().take(200).collect();
                assert!(
                    err.message.contains(quoted.trim()),
                    "{name}: {}",
                    err.message
                );
            }
        }
        let out = OpenAiEmbeddings::encode_error(&err);
        assert_eq!(out.status, 400, "{name}");
        let sent: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(sent["error"]["message"], err.message.as_str(), "{name}");
    }
}

#[test]
fn cohere_error_bodies_reach_apps_with_cohere_message() {
    for (name, status, message) in [
        (
            "providers/cohere/recorded-embed-dimensions-v3",
            400,
            "invalid request: output_dimension is not supported for this model",
        ),
        (
            "providers/cohere/recorded-embed-generation-model",
            400,
            "invalid request: model 'command-a-03-2025' is not supported by the embed API",
        ),
        (
            "providers/cohere/recorded-embed-token-ids",
            422,
            "invalid type: parameter '' is of type number but should be of type Object",
        ),
    ] {
        let body = fixture("errors", name);
        let meta = sidecar(DIR, "errors", name, "meta").unwrap();
        assert_eq!(meta["status"], status, "{name}");
        let err = OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(
            (&err.kind, err.message.as_str()),
            (&ErrorKind::InvalidRequest, message),
            "{name}"
        );
        let sent: Value =
            serde_json::from_slice(&OpenAiEmbeddings::encode_error(&err).body).unwrap();
        assert_eq!(sent["error"]["message"], message, "{name}");
    }
}

#[test]
fn a_rejected_key_in_fastapis_detail_is_an_authentication_error_with_its_text() {
    let err = OpenAiEmbeddings::decode_error(
        401,
        &HeaderList::new(),
        b"{\"detail\":\"Invalid API Key\"}\n",
    );
    assert_eq!(
        (&err.kind, err.message.as_str()),
        (&ErrorKind::Authentication, "Invalid API Key")
    );
    assert_eq!(OpenAiEmbeddings::encode_error(&err).status, 401);
}

#[test]
fn an_ovhcloud_key_refusal_with_a_bare_message_reaches_apps_in_openais_shape() {
    let name = "providers/ovhcloud/recorded-invalid-api-key";
    let body = fixture("errors", name);
    let err = OpenAiEmbeddings::decode_error(403, &HeaderList::new(), &body);
    let message = "Forbidden: authentication failed.  Please generate a new one at https://kepler.ai.cloud.ovh.net/v1/oauth/ovh/authorize?iam_action=publicCloudProject:ai:endpoints/call";
    assert_eq!(
        (&err.kind, err.status, err.message.as_str()),
        (&ErrorKind::PermissionDenied, Some(403), message)
    );
    let out = OpenAiEmbeddings::encode_error(&err);
    assert_eq!(out.status, 403);
    let sent: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(sent["error"]["message"], message);
    let body = fixture("errors", "providers/ovhcloud/recorded-bge-m3-dimensions");
    let err = OpenAiEmbeddings::decode_error(400, &HeaderList::new(), &body);
    assert_eq!(
        (&err.kind, err.message.as_str()),
        (
            &ErrorKind::InvalidRequest,
            "feature 'dimensions' is not currently supported"
        )
    );
}

#[test]
fn every_error_fixture_round_trips_with_status_and_retry_after() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        // NVIDIA answers with RFC 9457 problem documents, SiliconFlow with `{"code", "message", "data"}`,
        // Cloudflare with its `errors` envelope, IONOS with its `messages`, Novita AI with
        // `{"code", "reason", "message", "metadata"}`, OVHcloud with a bare `message`, Venice with
        // `{"error": "<text>"}` and Voyage AI and Jina AI with `{"detail": "<text>"}`; apps get them in OpenAI's
        // shape.
        if FOREIGN_ERROR_BODIES.contains(&name.as_str())
            || name.starts_with("providers/cloudflare-ai-gateway/")
            || name.starts_with("providers/ionos/")
            || name.starts_with("providers/cohere/")
            || name.starts_with("providers/novita-ai/")
            || name.starts_with("providers/nvidia/")
            || name.starts_with("providers/ovhcloud/")
            || name.starts_with("providers/siliconflow/")
            || name.starts_with("providers/venice/")
            || name.starts_with("providers/voyage/")
            || name.starts_with("providers/jina/")
        {
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
fn venices_string_errors_keep_their_text_and_get_the_kind_of_their_status() {
    for (name, status, kind, message) in [
        (
            "providers/venice/recorded-invalid-api-key",
            401,
            ErrorKind::Authentication,
            "Authentication failed",
        ),
        (
            "providers/venice/unknown-model-published",
            404,
            ErrorKind::NotFound,
            "Specified model not found: no-such-embedding-model. Did you mean: text-embedding-bge-m3, text-embedding-bge-en-icl, text-embedding-qwen3-8b?",
        ),
    ] {
        let err =
            OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &fixture("errors", name));
        assert_eq!((&err.kind, err.status), (&kind, Some(status)), "{name}");
        assert_eq!(err.message, message, "{name}");
        let out = OpenAiEmbeddings::encode_error(&err);
        let sent: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(
            (out.status, &sent["error"]["message"]),
            (status, &json!(message)),
            "{name}"
        );
    }
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

#[test]
fn an_nvidia_problem_document_reaches_embeddings_apps_as_a_key_that_isnt_allowed() {
    let body = fixture("errors", "providers/nvidia/recorded-invalid-api-key");
    let err = OpenAiEmbeddings::decode_error(403, &HeaderList::new(), &body);
    assert_eq!(
        (&err.kind, err.status),
        (&ErrorKind::PermissionDenied, Some(403))
    );
    assert!(
        err.message.contains("\"detail\": \"Authorization failed\""),
        "{}",
        err.message
    );
    let out = OpenAiEmbeddings::encode_error(&err);
    assert_eq!(out.status, 403);
    let sent: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(sent["error"]["message"], err.message.as_str());
}

#[test]
fn a_siliconflow_error_reaches_embeddings_apps_as_an_openai_error_with_its_message() {
    let body = fixture("errors", "providers/siliconflow/recorded-invalid-api-key");
    let err = OpenAiEmbeddings::decode_error(401, &HeaderList::new(), &body);
    assert_eq!(
        (&err.kind, err.status),
        (&ErrorKind::Authentication, Some(401))
    );
    assert!(err.message.contains("Token is invalid."), "{}", err.message);
    let out = OpenAiEmbeddings::encode_error(&err);
    assert_eq!(out.status, 401);
    let sent: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(sent["error"]["message"], err.message.as_str());
}

#[test]
fn an_account_scoped_provider_gets_embeddings_under_its_api_root_and_its_refused_dimensions() {
    let req = Request::Embed(decode(br#"{"model":"@cf/baai/bge-m3","input":"hi"}"#));
    let cloudflare = EmbeddingsProfile::compatible("Cloudflare AI Gateway").without_dimensions();
    let mut ctx = EgressCtx::new(&req)
        .with_api_root("/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1")
        .with_embeddings_profile(cloudflare);
    let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(
        enc.path,
        "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1/embeddings"
    );

    let sized = Request::Embed(decode(
        br#"{"model":"@cf/baai/bge-m3","input":"hi","dimensions":256}"#,
    ));
    let e = OpenAiEmbeddings::encode_request(
        &sized,
        &mut EgressCtx::new(&sized).with_embeddings_profile(cloudflare),
    )
    .unwrap_err();
    assert!(matches!(e.kind, ErrorKind::Unsupported { .. }), "{e:?}");
    assert_eq!(
        (e.param.as_deref(), e.message.as_str()),
        (
            Some("dimensions"),
            "`dimensions` is not supported by Cloudflare AI Gateway; remove it from the request."
        )
    );
    let mut openai = EgressCtx::new(&sized);
    assert!(OpenAiEmbeddings::encode_request(&sized, &mut openai).is_ok());
    assert_eq!(EmbeddingsProfile::default(), EmbeddingsProfile::OPENAI);
}

#[test]
fn cloudflare_envelopes_reach_embeddings_apps_with_cloudflares_message_and_kind() {
    for (name, kind, message) in [
        (
            "recorded-invalid-api-key",
            ErrorKind::Authentication,
            "Authentication error",
        ),
        (
            "recorded-unknown-model",
            ErrorKind::InvalidRequest,
            "AiError: No such model: No such model @cf/baai/bge-kaeso-none or task (00000000-0000-0000-0000-000000000000)",
        ),
    ] {
        let name = format!("providers/cloudflare-ai-gateway/{name}");
        let status = u16::try_from(
            sidecar(DIR, "errors", &name, "meta").unwrap()["status"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let err =
            OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &fixture("errors", &name));
        assert_eq!(
            (&err.kind, err.message.as_str(), err.status),
            (&kind, message, Some(status)),
            "{name}"
        );
        let sent: Value =
            serde_json::from_slice(&OpenAiEmbeddings::encode_error(&err).body).unwrap();
        assert_eq!(sent["error"]["message"], message, "{name}");
    }
}

#[test]
fn an_ionos_error_reaches_embeddings_apps_with_ionos_message() {
    let name = "providers/ionos/recorded-invalid-api-key";
    let err = OpenAiEmbeddings::decode_error(401, &HeaderList::new(), &fixture("errors", name));
    assert_eq!(
        (&err.kind, err.message.as_str()),
        (
            &ErrorKind::Authentication,
            "Unauthorized, invalid or no token provided for exchange"
        )
    );
    let sent: Value = serde_json::from_slice(&OpenAiEmbeddings::encode_error(&err).body).unwrap();
    assert_eq!(
        sent["error"]["message"],
        "Unauthorized, invalid or no token provided for exchange"
    );
}

#[test]
fn a_novita_ai_error_reaches_embeddings_apps_as_an_openai_error_with_its_message() {
    let body = fixture("errors", "providers/novita-ai/recorded-missing-api-key");
    let err = OpenAiEmbeddings::decode_error(403, &HeaderList::new(), &body);
    assert_eq!(
        (&err.kind, err.status, err.message.as_str()),
        (&ErrorKind::PermissionDenied, Some(403), "invalid API key")
    );
    let out = OpenAiEmbeddings::encode_error(&err);
    assert_eq!(out.status, 403);
    let sent: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(sent["error"]["message"], err.message.as_str());
}

#[test]
fn voyage_errors_reach_apps_in_openais_shape_with_voyages_text() {
    for (name, status, kind) in [
        (
            "providers/voyage/recorded-openai-fields-400",
            400,
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/voyage/recorded-bad-input-400",
            400,
            ErrorKind::InvalidRequest,
        ),
        (
            "providers/voyage/recorded-invalid-key-401",
            401,
            ErrorKind::Authentication,
        ),
        (
            "providers/voyage/recorded-no-key-401",
            401,
            ErrorKind::Authentication,
        ),
    ] {
        let body = fixture("errors", name);
        let meta = sidecar(DIR, "errors", name, "meta").unwrap();
        assert_eq!(meta["status"], status, "{name}");
        let detail: Value = serde_json::from_slice(&body).unwrap();
        let err = OpenAiEmbeddings::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(
            (&err.kind, err.message.as_str()),
            (&kind, detail["detail"].as_str().unwrap()),
            "{name}"
        );
        let out = OpenAiEmbeddings::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        let sent: Value = serde_json::from_slice(&out.body).unwrap();
        assert_eq!(sent["error"]["message"], detail["detail"], "{name}");
    }
}

#[test]
fn jina_gets_embedding_type_in_place_of_encoding_format_and_its_errors_reach_apps_with_their_text()
{
    let jina = EmbeddingsProfile::JINA;
    let send = |body: &[u8]| -> Result<Value, KppError> {
        let req = Request::Embed(decode(body));
        let mut ctx = EgressCtx::new(&req);
        ctx.embeddings_profile = jina;
        let enc = OpenAiEmbeddings::encode_request(&req, &mut ctx)?;
        Ok(serde_json::from_slice(&enc.body).unwrap())
    };
    let sent = send(
        br#"{"model":"jina-embeddings-v5-text-nano","input":["When is the conference call scheduled?"],"encoding_format":"base64","dimensions":256,"task":"retrieval.query"}"#,
    )
    .unwrap();
    assert_json_eq(
        "providers/jina/query-base64",
        &fixture("requests", "providers/jina/query-base64"),
        &serde_json::to_vec(&sent).unwrap(),
    );
    let floats = send(br#"{"model":"m","input":"a","encoding_format":"float"}"#).unwrap();
    assert_eq!(
        floats,
        json!({"model": "m", "input": "a", "embedding_type": "float"})
    );
    let null = send(br#"{"model":"m","input":"a","encoding_format":null}"#).unwrap();
    assert_eq!(
        null,
        json!({"model": "m", "input": "a", "embedding_type": null})
    );
    let plain = send(br#"{"model":"m","input":"a","embedding_type":"binary"}"#).unwrap();
    assert_eq!(plain["embedding_type"], "binary");
    let both =
        send(br#"{"model":"m","input":"a","encoding_format":"base64","embedding_type":"binary"}"#)
            .unwrap_err();
    assert_eq!(
        (both.param.as_deref(), both.message.as_str()),
        (
            Some("embedding_type"),
            "`encoding_format` reaches Jina AI as `embedding_type`, which the request sets too; send one of them."
        )
    );
    let user = send(br#"{"model":"m","input":"a","user":"u-1"}"#).unwrap_err();
    assert_eq!(
        user.message,
        "`user` is not supported by Jina AI; remove it from the request."
    );
    let tokens = send(br#"{"model":"m","input":[1,2]}"#).unwrap_err();
    assert_eq!(tokens.param.as_deref(), Some("input"));

    for (name, code) in [
        ("providers/jina/recorded-no-key-401", "AUTH_MISSING_API_KEY"),
        (
            "providers/jina/recorded-invalid-key-401",
            "AUTH_INVALID_API_KEY",
        ),
        (
            "providers/jina/recorded-basic-auth-401",
            "AUTH_INVALID_FORMAT",
        ),
    ] {
        let body = fixture("errors", name);
        let detail: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(detail["code"], code, "{name}");
        let err = OpenAiEmbeddings::decode_error(401, &HeaderList::new(), &body);
        assert_eq!(
            (&err.kind, err.message.as_str()),
            (
                &ErrorKind::Authentication,
                detail["detail"].as_str().unwrap()
            ),
            "{name}"
        );
        let sent: Value =
            serde_json::from_slice(&OpenAiEmbeddings::encode_error(&err).body).unwrap();
        assert_eq!(sent["error"]["message"], detail["detail"], "{name}");
    }
    let balance = OpenAiEmbeddings::decode_error(
        403,
        &HeaderList::new(),
        br#"{"detail":"Insufficient account balance.","code":"AUTHZ_INSUFFICIENT_BALANCE","request_id":"0"}"#,
    );
    assert_eq!(balance.kind, ErrorKind::Billing);
}
