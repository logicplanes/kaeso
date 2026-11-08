#![cfg(test)]

use kaeso_codecs::anthropic_messages::{MessagesProfile, RAW_PREDICT_VERSION};
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, ErrorKind, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const MODELS: &str = "/v1/projects/kaeso-example/locations/global/publishers/anthropic/models";

fn decode(dialect: DialectId, kind: RequestKind, body: &Value) -> Request {
    dispatch::decode_request(
        dialect,
        kind,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn raw_predict(req: &Request) -> (String, Value, HeaderList) {
    let mut ctx = EgressCtx::new(req)
        .with_messages_profile(
            MessagesProfile::compatible("Gemini Enterprise Agent Platform").raw_predict(),
        )
        .with_base_path(format!("{MODELS}/"));
    let enc = dispatch::encode_request(A, req, &mut ctx).unwrap();
    (
        enc.path,
        serde_json::from_slice(&enc.body).unwrap(),
        enc.headers,
    )
}

#[test]
fn a_claude_generation_goes_to_raw_predict_without_its_model_and_with_the_platforms_version() {
    let body = json!({
        "model": "claude-opus-4-5@20251101",
        "max_tokens": 64,
        "system": "Be brief.",
        "messages": [{ "role": "user", "content": "Hello" }]
    });
    let req = decode(A, RequestKind::Generate, &body);
    let (path, sent, headers) = raw_predict(&req);
    assert_eq!(
        path,
        format!("{MODELS}/claude-opus-4-5@20251101:rawPredict")
    );
    assert!(sent.get("model").is_none(), "{sent}");
    assert_eq!(sent["anthropic_version"], RAW_PREDICT_VERSION);
    assert_eq!(sent["anthropic_version"], "vertex-2023-10-16");
    assert_eq!(sent["max_tokens"], 64);
    assert_eq!(sent["messages"], body["messages"]);
    assert_eq!(headers.get("anthropic-version"), Some("2023-06-01"));

    let mut streamed = body.clone();
    streamed["stream"] = json!(true);
    let (path, sent, _) = raw_predict(&decode(A, RequestKind::Generate, &streamed));
    assert_eq!(
        path,
        format!("{MODELS}/claude-opus-4-5@20251101:streamRawPredict")
    );
    assert_eq!(sent["stream"], true);
}

#[test]
fn a_chat_apps_request_reaches_raw_predict_the_same_way_and_odd_model_ids_are_encoded() {
    let req = decode(
        C,
        RequestKind::Generate,
        &json!({ "model": "claude-sonnet-4-6", "max_completion_tokens": 32, "messages": [{ "role": "user", "content": "Hi" }] }),
    );
    let (path, sent, _) = raw_predict(&req);
    assert_eq!(path, format!("{MODELS}/claude-sonnet-4-6:rawPredict"));
    assert!(sent.get("model").is_none());
    assert_eq!(sent["anthropic_version"], "vertex-2023-10-16");

    let mut odd = req.clone();
    if let Request::Generate(g) = &mut odd {
        g.model = "a/b:c".into();
    }
    assert_eq!(
        raw_predict(&odd).0,
        format!("{MODELS}/a%2Fb%3Ac:rawPredict")
    );
}

#[test]
fn a_token_count_goes_to_count_tokens_raw_predict_with_its_model() {
    let req = decode(
        A,
        RequestKind::CountTokens,
        &json!({ "model": "claude-haiku-4-5@20251001", "messages": [{ "role": "user", "content": "Hi" }] }),
    );
    let (path, sent, _) = raw_predict(&req);
    assert_eq!(path, format!("{MODELS}/count-tokens:rawPredict"));
    assert_eq!(sent["model"], "claude-haiku-4-5@20251001");
    assert_eq!(sent["anthropic_version"], "vertex-2023-10-16");
}

#[test]
fn an_app_that_names_its_own_platform_version_keeps_it() {
    let req = decode(
        A,
        RequestKind::Generate,
        &json!({
            "model": "claude-opus-5-5",
            "max_tokens": 8,
            "anthropic_version": "2026-10-21",
            "messages": [{ "role": "user", "content": "Hi" }]
        }),
    );
    assert_eq!(raw_predict(&req).1["anthropic_version"], "2026-10-21");
}

#[test]
fn without_raw_predict_the_messages_api_is_unchanged_and_raw_predict_needs_a_path() {
    let req = decode(
        A,
        RequestKind::Generate,
        &json!({ "model": "claude-opus-5-5", "max_tokens": 8, "messages": [{ "role": "user", "content": "Hi" }] }),
    );
    let mut ctx = EgressCtx::new(&req).with_base_path(MODELS);
    let enc = dispatch::encode_request(A, &req, &mut ctx).unwrap();
    assert_eq!(enc.path, "/v1/messages");
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(sent["model"], "claude-opus-5-5");
    assert!(sent.get("anthropic_version").is_none());

    let mut ctx = EgressCtx::new(&req).with_messages_profile(
        MessagesProfile::compatible("Gemini Enterprise Agent Platform").raw_predict(),
    );
    assert!(dispatch::encode_request(A, &req, &mut ctx).is_err());
}

#[test]
fn chat_completions_go_under_the_base_path() {
    let req = decode(
        C,
        RequestKind::Generate,
        &json!({ "model": "meta/llama-4-maverick-17b-128e-instruct-maas", "messages": [{ "role": "user", "content": "Hi" }] }),
    );
    let mut ctx = EgressCtx::new(&req)
        .with_base_path("/v1/projects/kaeso-example/locations/us-east5/endpoints/openapi");
    let enc = dispatch::encode_request(C, &req, &mut ctx).unwrap();
    assert_eq!(
        enc.path,
        "/v1/projects/kaeso-example/locations/us-east5/endpoints/openapi/chat/completions"
    );
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        sent["model"],
        "meta/llama-4-maverick-17b-128e-instruct-maas"
    );
    let mut plain = EgressCtx::new(&req);
    assert_eq!(
        dispatch::encode_request(C, &req, &mut plain).unwrap().path,
        "/v1/chat/completions"
    );
}

// The platform answers its own failures in Google's shape in front of the partner models'
// formats; the documented example (partner-models/claude/web-search) and a quota refusal.
#[test]
fn googles_error_shape_in_front_of_claude_and_chat_models_reaches_every_app_in_its_own() {
    let precondition = json!({
        "error": {
            "code": 400,
            "message": "Web search is not enabled for this project.",
            "status": "FAILED_PRECONDITION"
        }
    });
    let quota = json!({
        "error": { "code": 429, "message": "Resource exhausted, please try again later.", "status": "RESOURCE_EXHAUSTED" }
    });
    for dialect in [A, C] {
        let body = serde_json::to_vec(&precondition).unwrap();
        let err = dispatch::decode_error(dialect, 400, &HeaderList::new(), &body);
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{dialect}");
        assert_eq!(err.message, "Web search is not enabled for this project.");
        let body = serde_json::to_vec(&quota).unwrap();
        let err = dispatch::decode_error(dialect, 429, &HeaderList::new(), &body);
        assert_eq!(err.kind, ErrorKind::RateLimited, "{dialect}");

        let to_anthropic = dispatch::encode_error(A, &err);
        let anthropic: Value = serde_json::from_slice(&to_anthropic.body).unwrap();
        assert_eq!(anthropic["type"], "error");
        assert_eq!(anthropic["error"]["type"], "rate_limit_error");
        assert_eq!(to_anthropic.status, 429);
        let to_chat = dispatch::encode_error(C, &err);
        let chat: Value = serde_json::from_slice(&to_chat.body).unwrap();
        assert_eq!(
            chat["error"]["message"],
            "Resource exhausted, please try again later."
        );
        assert!(chat["error"].get("status").is_none(), "{chat}");
        let to_gemini = dispatch::encode_error(DialectId::Gemini, &err);
        let gemini: Value = serde_json::from_slice(&to_gemini.body).unwrap();
        assert_eq!(gemini, quota);
    }
    let anthropic_own = json!({ "type": "error", "error": { "type": "overloaded_error", "message": "Overloaded" } });
    let err = dispatch::decode_error(
        A,
        529,
        &HeaderList::new(),
        &serde_json::to_vec(&anthropic_own).unwrap(),
    );
    assert_eq!(err.kind, ErrorKind::Overloaded);
}
