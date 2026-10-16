#![cfg(test)]

use kaeso_codecs::testing::json_bytes_diff;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Reply, Request, RequestKind, StreamMode};

fn round_trip(dialect: DialectId, body: &str) -> Vec<u8> {
    let req = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        body.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    dispatch::encode_request(dialect, &req, &mut ctx)
        .unwrap()
        .body
}

fn with_sampling(dialect: DialectId, body: &str, f: impl Fn(&mut kaeso_kpp::Sampling)) -> Vec<u8> {
    let mut req = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        body.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(generate) = &mut req else {
        panic!()
    };
    f(&mut generate.sampling);
    let mut ctx = EgressCtx::new(&req);
    dispatch::encode_request(dialect, &req, &mut ctx)
        .unwrap()
        .body
}

const BODIES: [(DialectId, &str); 3] = [
    (
        DialectId::OpenAiChat,
        r#"{"model":"m","messages":[{"role":"user","content":"q"}],"temperature":1,"top_p":0.5,"frequency_penalty":0,"presence_penalty":-1}"#,
    ),
    (
        DialectId::AnthropicMessages,
        r#"{"model":"m","max_tokens":5,"messages":[{"role":"user","content":"q"}],"temperature":1,"top_p":1.0}"#,
    ),
    (
        DialectId::OpenAiResponses,
        r#"{"model":"m","input":"q","temperature":1,"top_p":1.0}"#,
    ),
];

#[test]
fn integer_and_fraction_spellings_survive_a_round_trip() {
    for (dialect, body) in BODIES {
        let out = round_trip(dialect, body);
        assert_eq!(
            json_bytes_diff(body.as_bytes(), &out),
            None,
            "{dialect:?}: {}",
            String::from_utf8_lossy(&out)
        );
    }
}

#[test]
fn a_changed_number_is_written_as_the_new_value() {
    for (dialect, body) in BODIES {
        let out = with_sampling(dialect, body, |s| s.temperature = Some(0.25));
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["temperature"], serde_json::json!(0.25), "{dialect:?}");
    }
}

#[test]
fn numbers_from_another_dialect_use_the_target_spelling() {
    let body = r#"{"model":"m","max_completion_tokens":5,"messages":[{"role":"user","content":"q"}],"temperature":1}"#;
    let req = dispatch::decode_request(
        DialectId::OpenAiChat,
        RequestKind::Generate,
        body.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    let out = dispatch::encode_request(DialectId::AnthropicMessages, &req, &mut ctx).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(value["temperature"].to_string(), "1.0");
}

#[test]
fn responses_replies_keep_the_spelling_of_their_echoed_numbers() {
    let body = r#"{"id":"resp_1","object":"response","created_at":1,"status":"completed","model":"m","output":[],"temperature":1,"top_p":1.0,"parallel_tool_calls":true,"tool_choice":"auto","tools":[]}"#;
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    let reply = dispatch::decode_response(
        DialectId::OpenAiResponses,
        200,
        &HeaderList::new(),
        body.as_bytes(),
        &ctx,
    )
    .unwrap();
    assert!(matches!(reply, Reply::Generate(_)));
    let out = dispatch::encode_response(
        DialectId::OpenAiResponses,
        &reply,
        &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
    )
    .unwrap();
    let out: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(out["temperature"].to_string(), "1");
    assert_eq!(out["top_p"].to_string(), "1.0");
}
