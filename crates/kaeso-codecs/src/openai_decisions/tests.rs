use kaeso_kpp::{
    Answer, AnswerValue, Asset, Block, Choice, ChoiceAnswer, ChoiceOption, ChoiceValue,
    DecideRequest, DecideResponse, DecisionInput, Description, DialectId, ErrorKind, Extras,
    GenerateRequest, ImageBlock, Instructions, KppError, Level, Message, Origin, Predicate,
    PredicateAnswer, Question, QuestionKind, RawJson, Refusal, Reply, Request, RequestKind, Role,
    Score, ScoreAnswer, Source, StreamEvent, StreamMode, ToolResultBlock,
};
use serde_json::{Value, json};

use super::*;
use crate::dispatch;
use crate::system_one::{SystemOne, SystemOneProfile};
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "openai_decisions";

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no {folder}/{name}"))
        .1
}

fn system_one_fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures("system_one", folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no system_one {folder}/{name}"))
        .1
}

fn decode_with(body: &[u8], headers: &HeaderList) -> DecideRequest {
    match OpenAiDecisions::decode_request(RequestKind::Decide, body, headers) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn decode(body: &[u8]) -> DecideRequest {
    decode_with(body, &HeaderList::new())
}

fn decode_system_one(body: &[u8]) -> DecideRequest {
    match SystemOne::decode_request(RequestKind::Decide, body, &HeaderList::new()) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn json_body(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

fn body_of(enc: &EncodedRequest) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn encode_with(
    req: &DecideRequest,
    profile: DecisionsProfile,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&req)
        .with_provider("openai")
        .with_decisions_profile(profile);
    let enc = OpenAiDecisions::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn encode(req: &DecideRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    encode_with(req, DecisionsProfile::OPENAI)
}

fn to_system_one(
    req: &DecideRequest,
    profile: SystemOneProfile,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&req)
        .with_provider("typesafe")
        .with_system_one_profile(profile);
    let enc = SystemOne::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn reply_to(app: DialectId, reply: &Reply, req: &DecideRequest) -> Value {
    let out = dispatch::encode_response(
        app,
        reply,
        &EncodeCtx::for_request(Request::Decide(req.clone())),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    serde_json::from_slice(&out).unwrap()
}

fn decided(reply: Result<Reply, KppError>) -> DecideResponse {
    match reply {
        Ok(Reply::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn close(a: &Value, b: f64) {
    let a = a.as_f64().unwrap_or_else(|| panic!("{a} is no number"));
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    b.extend(width.to_be_bytes());
    b.extend(height.to_be_bytes());
    b.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    b
}

fn predicate(name: Option<&str>, instructions: &str) -> Question {
    Question::new(
        name.map(str::to_owned),
        QuestionKind::Predicate(Predicate::default()),
    )
    .with_instructions(Instructions::Text(instructions.to_owned()))
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = decode(&body);
        let (enc, c) = encode(&req).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &body, &enc.body);
        assert_eq!(enc.path, "/v1/decisions", "{name}");
        assert_eq!(enc.method, "POST");
        assert!(c.adapted.is_empty(), "{name}: {:?}", c.adapted);
    }
}

#[test]
fn every_response_fixture_round_trips_through_its_request() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let req = decode(&fixture("requests", &name));
        let (_, c) = encode(&req).unwrap();
        let reply = OpenAiDecisions::decode_response(200, &HeaderList::new(), &body, &c)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let out =
            OpenAiDecisions::encode_response(&reply, &EncodeCtx::for_request(Request::Decide(req)))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn requests_read_as_their_documented_values() {
    let req = decode(&fixture("requests", "cache"));
    let cache = req.cache.as_ref().unwrap();
    assert_eq!(cache.key.as_deref(), Some("refund-policy-v3"));
    assert_eq!(cache.mode.as_deref(), Some("explicit"));
    assert_eq!(cache.ttl, Some(kaeso_kpp::CacheTtl::Other("30m".into())));
    assert_eq!(req.user.as_deref(), Some("user-7f3a"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    assert_eq!(m[0].role, Role::System);
    assert!(m[0].blocks[0].cache().is_some());
    assert!(
        matches!(&m[1].blocks[0], Block::Image(b) if matches!(&b.source, Source::Base64(s) if s.media_type == "image/png"))
    );

    let req = decode(&fixture("requests", "booleans"));
    let QuestionKind::Choice(c) = &req.questions[0].kind else {
        panic!()
    };
    assert_eq!(c.options[0].value, ChoiceValue::Bool(true));
    assert_eq!(
        c.options[0].description,
        Some(Description::Text("The product works.".into()))
    );

    let req = decode(&fixture("requests", "null-names"));
    assert!(req.questions.iter().all(|q| q.name.is_none()));
    let req = decode(&fixture("requests", "repeated-names"));
    req.validate().unwrap();

    let req = decode(&fixture("requests", "conversation"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    assert_eq!(m.len(), 9);
    assert!(
        matches!(&m[3].blocks[0], Block::ToolCall(c) if c.name == "lookup_order" && c.arguments.get() == r#""{\"order_id\":\"4411\"}""#)
    );
    assert!(matches!(&m[4].blocks[0], Block::ToolResult(r) if r.content.len() == 2));
    assert!(matches!(&m[5].blocks[1], Block::Refusal(_)));

    let req = decode(&fixture("requests", "instructions-parts"));
    let Some(Instructions::Blocks(blocks)) = &req.questions[0].instructions else {
        panic!()
    };
    assert!(matches!(
        blocks.as_slice(),
        [Block::Text(_), Block::Image(_), Block::Json(_)]
    ));
}

#[test]
fn responses_read_as_their_documented_values() {
    let req = decode(&fixture("requests", "cache"));
    let (_, c) = encode(&req).unwrap();
    let r = decided(OpenAiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "cache"),
        &c,
    ));
    assert_eq!(
        (
            r.usage.input_tokens,
            r.usage.cache_read_tokens,
            r.usage.cache_write_tokens
        ),
        (94, 1152, 64)
    );
    assert_eq!(r.usage.total_input_tokens(), 1310);
    assert_eq!(r.usage.reasoning_tokens, Some(0));

    let req = decode(&fixture("requests", "booleans"));
    let (_, c) = encode(&req).unwrap();
    let r = decided(OpenAiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "booleans"),
        &c,
    ));
    assert_eq!(
        r.answers[0].value,
        AnswerValue::Choice(ChoiceAnswer::new(0, vec![Some(0.91), Some(0.09)]))
    );
    assert_eq!(
        r.answers[1].value,
        AnswerValue::Choice(ChoiceAnswer::new(
            2,
            vec![Some(0.05), Some(0.03), Some(0.92)]
        ))
    );

    let req = decode(&fixture("requests", "refusal"));
    let (_, c) = encode(&req).unwrap();
    let r = decided(OpenAiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "refusal"),
        &c,
    ));
    assert_eq!(r.answers[0].value, AnswerValue::Refusal(Refusal::new()));
}

#[test]
fn error_fixtures_read_as_their_kinds_and_reach_apps_as_sent() {
    let expected: &[(&str, ErrorKind)] = &[
        ("credit-balance-exhausted-429", ErrorKind::Billing),
        ("invalid-api-key-401", ErrorKind::Authentication),
        ("overloaded-503", ErrorKind::Overloaded),
        ("server-error-500", ErrorKind::ProviderError),
        ("slow-down-429", ErrorKind::RateLimited),
        ("too-many-questions-400", ErrorKind::InvalidRequest),
    ];
    let all = fixtures(DIR, "errors", "json");
    assert_eq!(all.len(), expected.len());
    for (name, kind) in expected {
        let body = &all.iter().find(|(n, _)| n == name).unwrap().1;
        let meta = sidecar(DIR, "errors", name, "meta").unwrap();
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
        let err = OpenAiDecisions::decode_error(status, &headers, body);
        assert_eq!(&err.kind, kind, "{name}");
        let out = OpenAiDecisions::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(name, body, &out.body);
        if let Some(retry) = headers.get("retry-after") {
            assert_eq!(out.headers.get("retry-after"), Some(retry), "{name}");
        }
    }
    let err = OpenAiDecisions::decode_error(
        400,
        &HeaderList::new(),
        &fixture("errors", "too-many-questions-400"),
    );
    assert_eq!(err.param.as_deref(), Some("questions"));
    let to_system_one = SystemOne::encode_error(&err);
    assert_eq!(to_system_one.status, 400);
}

#[test]
fn a_system_one_body_is_sent_to_its_own_endpoint() {
    let body = system_one_fixture("requests", "noul");
    let err = OpenAiDecisions::decode_request(RequestKind::Decide, &body, &HeaderList::new())
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(err.message.contains("/v1/systemone"), "{}", err.message);
    assert_eq!(err.param.as_deref(), Some("state"));
    let out = OpenAiDecisions::encode_error(&err);
    assert_eq!(out.status, 400);
}

#[test]
fn decoding_refuses_what_the_decisions_api_refuses() {
    let q = json!([{"type": "predicate", "instructions": "?"}]);
    let cases = [
        (json!({"model": "m", "input": "x"}), Some("questions")),
        (
            json!({"model": "m", "input": 5, "questions": q}),
            Some("input"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": {}}),
            Some("questions"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "box"}]}),
            Some("questions[0].type"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "choice", "instructions": "?"}]}),
            Some("questions[0].choices"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "score", "instructions": "?"}]}),
            Some("questions[0].levels"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "choices": []}]}),
            Some("questions[0].choices"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "levels": []}]}),
            Some("questions[0].levels"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "choice", "levels": []}]}),
            Some("questions[0].levels"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "score", "choices": []}]}),
            Some("questions[0].choices"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "choice", "choices": [{"value": 1}]}]}),
            Some("questions[0].choices[0].value"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "choice", "choices": [{"description": "?"}]}]}),
            Some("questions[0].choices[0]"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "choice", "choices": {}}]}),
            Some("questions[0].choices"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "score", "levels": [{"description": "?"}]}]}),
            Some("questions[0].levels[0]"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "instructions": 5}]}),
            Some("questions[0].instructions"),
        ),
        (
            json!({"model": "m", "input": "x", "questions": [5]}),
            Some("questions[0]"),
        ),
        (
            json!({"model": "m", "input": [5], "questions": q}),
            Some("input[0]"),
        ),
        (
            json!({"model": "m", "input": [{"type": "item_reference", "id": "x"}], "questions": q}),
            Some("input[0].type"),
        ),
        (
            json!({"model": "m", "input": [{"role": "tool", "content": "x"}], "questions": q}),
            Some("input[0].role"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user", "content": 5}], "questions": q}),
            Some("input[0].content"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user"}], "questions": q}),
            Some("input[0]"),
        ),
        (
            json!({"model": "m", "input": [{"type": "function_call", "call_id": "c"}], "questions": q}),
            Some("input[0]"),
        ),
        (
            json!({"model": "m", "input": [{"type": "function_call_output", "call_id": "c"}], "questions": q}),
            Some("input[0]"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_text"}]}], "questions": q}),
            Some("input[0].content[0]"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_image"}]}], "questions": q}),
            Some("input[0].content[0]"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_json"}]}], "questions": q}),
            Some("input[0].content[0]"),
        ),
        (
            json!({"model": "m", "input": [{"role": "assistant", "content": [{"type": "refusal"}]}], "questions": q}),
            Some("input[0].content[0]"),
        ),
        (
            json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_image", "image_url": "data:image/png;base64,!!"}]}], "questions": q}),
            Some("input[0].content[0].image_url"),
        ),
    ];
    for (body, param) in cases {
        let err = OpenAiDecisions::decode_request(
            RequestKind::Decide,
            &json_body(&body),
            &HeaderList::new(),
        )
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}");
        assert_eq!(err.param.as_deref(), param, "{body}: {}", err.message);
    }
    assert!(
        OpenAiDecisions::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err()
    );
}

#[test]
fn odd_forms_round_trip_too() {
    let png64 = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR42mM4oaEBRAwQCgAhLgRh9uHCTQAAAABJRU5ErkJggg==";
    let bodies = [
        json!({"model": "m", "input": [{"role": "user", "content": []}, {"role": "assistant", "content": "ok"}, {"role": "user", "content": [{"type": "x_future_part", "x": 1}]}], "questions": [{"type": "predicate"}]}),
        json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_image", "image_url": format!("data:image/png;name=a.png;base64,{png64}"), "detail": "low", "prompt_cache_breakpoint": null}]}], "questions": [{"type": "predicate", "instructions": "?", "name": "a"}]}),
        json!({"model": "m", "input": [{"role": "user", "content": [{"type": "output_text", "text": "x"}]}, {"role": "assistant", "content": [{"type": "input_text", "text": "y"}]}], "questions": [{"type": "predicate", "instructions": "?"}], "prompt_cache_key": null, "safety_identifier": null, "prompt_cache_options": null}),
        json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "instructions": "?"}], "prompt_cache_options": {}}),
        json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "instructions": "?"}], "prompt_cache_options": {"mode": "implicit", "x_future": 1}}),
        json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "instructions": "?"}], "prompt_cache_options": {"ttl": "1h"}}),
        json!({"model": "m", "input": "x", "questions": [{"type": "predicate", "instructions": "?"}], "prompt_cache_options": {"ttl": "5m"}}),
        json!({"model": "m", "input": [{"type": "custom_tool_call_output", "call_id": "c", "output": [{"type": "input_text", "text": "done"}]}, {"type": "function_call_output", "call_id": "d", "output": "plain"}], "questions": [{"type": "choice", "instructions": "?", "choices": [{"value": "a", "description": null}, {"value": false}]}, {"type": "score", "instructions": "?", "levels": [{"label": "a", "x_level": 1}, {"label": "b", "description": "B"}]}]}),
    ];
    for body in bodies {
        let req = decode(&json_body(&body));
        let (enc, _) = encode(&req).unwrap_or_else(|e| panic!("{body}: {e}"));
        assert_json_eq(&body.to_string(), &json_body(&body), &enc.body);
    }
}

#[test]
fn odd_answers_round_trip_too() {
    let req = decode(&json_body(
        &json!({"model": "m", "input": "x", "questions": [
            {"type": "choice", "instructions": "?", "name": "c", "choices": [{"value": "a"}, {"value": true}, {"value": "b"}]},
            {"type": "score", "instructions": "?", "levels": [{"label": "lo"}, {"label": "mid"}, {"label": "hi"}]},
            {"type": "predicate", "instructions": "?", "name": "p"},
            {"type": "predicate", "instructions": "?", "name": "q"}
        ]}),
    ));
    let (_, c) = encode(&req).unwrap();
    let body = json!({"model": "m", "answers": [
        {"type": "choice", "name": "renamed", "choice": true, "probabilities": [{"value": "b", "probability": 0}, {"value": true, "probability": 1, "x_rank": 1}]},
        {"type": "score", "score": 1, "probabilities": [{"value": 2, "label": "High!", "probability": 0.5}, {"value": 0, "label": "lo", "probability": 0.5}], "confidence": 0},
        {"type": "predicate", "name": null, "probability": 1},
        {"type": "refusal"}
    ], "x_trace": 1});
    let reply =
        OpenAiDecisions::decode_response(200, &HeaderList::new(), &json_body(&body), &c).unwrap();
    let Reply::Decide(r) = &reply else { panic!() };
    assert_eq!(
        r.answers[1].value,
        AnswerValue::Score(ScoreAnswer::new(1.0, vec![Some(0.5), None, Some(0.5)]))
    );
    let out =
        OpenAiDecisions::encode_response(&reply, &EncodeCtx::for_request(Request::Decide(req)))
            .unwrap();
    assert_json_eq("odd answers", &json_body(&body), &out);
}

#[test]
fn replies_the_decisions_api_never_sends_are_refused() {
    let req = decode(&fixture("requests", "booleans"));
    let (_, c) = encode(&req).unwrap();
    let read =
        |v: Value| OpenAiDecisions::decode_response(200, &HeaderList::new(), &json_body(&v), &c);
    let p = |v: Value, pr: f64| json!({"value": v, "probability": pr});
    let ok_second = json!({"type": "choice", "name": "packaging", "choice": "yes", "probabilities": [], "confidence": 1});
    for bad in [
        json!({"model": "m", "answers": {}}),
        json!({"model": "m", "answers": [ok_second]}),
        json!({"model": "m", "answers": [{"type": "choice", "choice": "true", "probabilities": []}, ok_second]}),
        json!({"model": "m", "answers": [{"type": "choice", "choice": true, "probabilities": [p(json!("maybe"), 0.1)]}, ok_second]}),
        json!({"model": "m", "answers": [{"type": "choice", "choice": true, "probabilities": [p(json!(true), 0.1), p(json!(true), 0.1)]}, ok_second]}),
        json!({"model": "m", "answers": [{"type": "predicate", "probability": 0.5}, ok_second]}),
        json!({"model": "m", "answers": [{"type": "choice", "choice": true}, ok_second]}),
        json!({"model": "m", "answers": [{"type": "choice", "choice": true, "probabilities": []}, ok_second], "usage": {"output_tokens": 1}}),
        json!([]),
    ] {
        assert!(read(bad.clone()).is_err(), "{bad}");
    }
    let score_req = decode(&fixture("requests", "score"));
    let (_, c) = encode(&score_req).unwrap();
    for bad in [
        json!({"model": "m", "answers": [{"type": "score", "score": 1, "probabilities": [{"value": 3, "label": "x", "probability": 1}]}]}),
        json!({"model": "m", "answers": [{"type": "score", "score": 1, "probabilities": [{"value": 0, "label": "x", "probability": 1}, {"value": 0, "label": "x", "probability": 1}]}]}),
        json!({"model": "m", "answers": [{"type": "score", "score": "high", "probabilities": []}]}),
    ] {
        assert!(
            OpenAiDecisions::decode_response(200, &HeaderList::new(), &json_body(&bad), &c)
                .is_err(),
            "{bad}"
        );
    }
    let err = OpenAiDecisions::decode_response(
        429,
        &HeaderList::new().with("retry-after", "1"),
        &fixture("errors", "slow-down-429"),
        &c,
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::RateLimited);
    let fresh = EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off);
    assert!(
        OpenAiDecisions::decode_response(
            200,
            &HeaderList::new(),
            &fixture("responses", "score"),
            &fresh
        )
        .is_err()
    );
    let other = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(OpenAiDecisions::decode_response(200, &HeaderList::new(), b"{}", &other).is_err());
}

#[test]
fn replies_without_their_request_or_with_other_answers_are_refused() {
    let req = decode(&fixture("requests", "choice"));
    let reply = Reply::Decide(DecideResponse::new(
        "m",
        vec![Answer::new(AnswerValue::Predicate(PredicateAnswer::new(
            0.5,
        )))],
    ));
    let ctx = EncodeCtx::for_request(Request::Decide(req.clone()));
    assert!(OpenAiDecisions::encode_response(&reply, &ctx).is_err());
    assert!(
        OpenAiDecisions::encode_response(&Reply::Decide(DecideResponse::new("m", vec![])), &ctx)
            .is_err()
    );
    assert!(
        OpenAiDecisions::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Decide, StreamMode::Off)
        )
        .is_err()
    );
    let far = Reply::Decide(DecideResponse::new(
        "m",
        vec![Answer::new(AnswerValue::Choice(ChoiceAnswer::new(
            9,
            vec![],
        )))],
    ));
    assert!(OpenAiDecisions::encode_response(&far, &ctx).is_err());
    let nan = Reply::Decide(DecideResponse::new(
        "m",
        vec![Answer::new(AnswerValue::Choice(ChoiceAnswer::new(
            0,
            vec![Some(f64::NAN)],
        )))],
    ));
    assert!(OpenAiDecisions::encode_response(&nan, &ctx).is_err());
    let chat = Request::Generate(GenerateRequest::new("m").with_message(Message::user("hi")));
    let generate = Reply::Generate(kaeso_kpp::Response::new("r", "m", vec![]));
    assert!(
        OpenAiDecisions::encode_response(&generate, &EncodeCtx::for_request(chat.clone())).is_err()
    );
    assert!(OpenAiDecisions::check(&chat, &EgressCtx::new(&chat)).is_err());
    let mut c = EgressCtx::new(&chat);
    assert!(OpenAiDecisions::encode_request(&chat, &mut c).is_err());
}

#[test]
fn decision_requests_are_never_streamed() {
    let mut enc =
        OpenAiDecisions::stream_encoder(&EncodeCtx::new(RequestKind::Decide, StreamMode::Off));
    let err = enc
        .encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
        .unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("streaming"));
    assert!(enc.finish().unwrap().is_empty());
    let mut dec =
        OpenAiDecisions::stream_decoder(&EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off));
    assert!(matches!(
        dec.feed(b"data: {}\n\n").as_slice(),
        [StreamEvent::Error(_)]
    ));
    assert!(dec.finish().is_empty());
}

#[test]
fn listings_are_openais() {
    let list = OpenAiDecisions::decode_models(200, &HeaderList::new(), br#"{"object":"list","data":[{"id":"gpt-6-luna","object":"model","created":1,"owned_by":"openai"}]}"#).unwrap();
    assert_eq!(list.models[0].id, "gpt-6-luna");
    let one = OpenAiDecisions::decode_model(
        200,
        &HeaderList::new(),
        br#"{"id":"gpt-6-luna","object":"model","created":1,"owned_by":"openai"}"#,
    )
    .unwrap();
    assert_eq!(one.id, "gpt-6-luna");
    assert_eq!(
        OpenAiDecisions::encode_models_request(&kaeso_kpp::ModelsQuery::default())
            .unwrap()
            .path,
        "/v1/models"
    );
    assert_eq!(
        OpenAiDecisions::encode_model_request("gpt-6-luna").path,
        "/v1/models/gpt-6-luna"
    );
    assert!(OpenAiDecisions::encode_models(&list).is_ok());
    assert!(OpenAiDecisions::encode_model(&one).is_ok());
    assert!(
        OpenAiDecisions::decode_models_query("limit=x").is_ok()
            || OpenAiDecisions::decode_models_query("limit=x").is_err()
    );
    let unauthorized = fixture("errors", "invalid-api-key-401");
    assert!(OpenAiDecisions::decode_models(401, &HeaderList::new(), &unauthorized).is_err());
    assert!(OpenAiDecisions::decode_model(401, &HeaderList::new(), &unauthorized).is_err());
}

#[test]
fn request_limits_count_questions_options_and_items() {
    let questions: Vec<Value> = (0..1001)
        .map(|_| json!({"type": "predicate", "instructions": "?"}))
        .collect();
    let err = OpenAiDecisions::decode_request(
        RequestKind::Decide,
        &json_body(&json!({"model": "m", "input": "x", "questions": questions})),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("questions"));
    let choices: Vec<Value> = (0..256)
        .map(|i| json!({"value": format!("o{i}")}))
        .collect();
    let err = OpenAiDecisions::decode_request(RequestKind::Decide, &json_body(&json!({"model": "m", "input": "x", "questions": [{"type": "choice", "instructions": "?", "choices": choices}]})), &HeaderList::new()).unwrap_err();
    assert!(err.message.contains("255"), "{}", err.message);
    let fine: Vec<Value> = (0..255)
        .map(|i| json!({"value": format!("o{i}")}))
        .collect();
    let body = json_body(
        &json!({"model": "m", "input": [{"role": "user", "content": [{"type": "input_text", "text": "a"}, {"type": "input_text", "text": "b"}]}], "questions": [{"type": "choice", "instructions": "?", "choices": fine}, {"type": "choice", "instructions": "?", "choices": fine}]}),
    );
    let shape = crate::limits::request_shape(DialectId::OpenAiDecisions, &body).unwrap();
    assert_eq!((shape.questions, shape.items, shape.blocks), (2, 1, 2));
    let checked = crate::limits::Checked::new(DialectId::OpenAiDecisions, body.as_slice()).unwrap();
    assert!(
        dispatch::decode_checked_request(RequestKind::Decide, &checked, &HeaderList::new()).is_ok()
    );
}

#[test]
fn headers_and_paths_follow_the_context() {
    let headers = HeaderList::new().with("OpenAI-Beta", "decisions=v1");
    let req = decode_with(&fixture("requests", "predicate"), &headers);
    let (enc, _) = encode(&req).unwrap();
    assert_eq!(enc.headers.get("openai-beta"), Some("decisions=v1"));
    assert_eq!(enc.headers.get("content-type"), Some("application/json"));
    let request = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&request).with_api_root("/v1/ai-gateway");
    assert_eq!(
        OpenAiDecisions::encode_request(&request, &mut c)
            .unwrap()
            .path,
        "/v1/ai-gateway/decisions"
    );
    let mut c = EgressCtx::new(&request).with_base_path("/custom/decide/");
    assert_eq!(
        OpenAiDecisions::encode_request(&request, &mut c)
            .unwrap()
            .path,
        "/custom/decide"
    );
    let mut c = EgressCtx::new(&request)
        .with_decisions_profile(DecisionsProfile::GENERIC.with_path("/decide"));
    assert_eq!(
        OpenAiDecisions::encode_request(&request, &mut c)
            .unwrap()
            .path,
        "/decide"
    );
    let to_system_one = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap().0;
    assert_eq!(to_system_one.headers.get("openai-beta"), None);
}

// Crossing from OpenAI apps to System One providers.

#[test]
fn unnamed_and_repeated_names_become_positions_and_answers_map_back() {
    for name in ["null-names", "repeated-names"] {
        let req = decode(&fixture("requests", name));
        let (enc, c) = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap();
        let sent = body_of(&enc);
        let keys: Vec<&String> = sent["questions"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["q0", "q1"], "{name}");
        assert_eq!(c.adapted, ["names"], "{name}");
        let answers = match name {
            "null-names" => {
                json!({"q1": {"type": "score", "score": 1.65, "confidence": 0.5, "legend": {}, "probabilities": {"0": 0.05, "1": 0.25, "2": 0.7}}, "q0": {"type": "noul", "noul": 0.83}})
            }
            _ => {
                json!({"q0": {"type": "noul", "noul": 0.98}, "q1": {"type": "noul", "noul": 0.96}})
            }
        };
        let reply = json!({"model": "jev-latest", "answers": answers, "usage": {"input_tokens": 40, "output_tokens": 3}});
        let reply =
            SystemOne::decode_response(200, &HeaderList::new(), &json_body(&reply), &c).unwrap();
        let out = reply_to(DialectId::OpenAiDecisions, &reply, &req);
        let expected_names: Vec<Value> = req.questions.iter().map(|q| json!(q.name)).collect();
        let names: Vec<Value> = out["answers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].clone())
            .collect();
        assert_eq!(names, expected_names, "{name}");
        assert_eq!(out["usage"]["input_tokens"], 40);
        assert_eq!(out["usage"]["output_tokens"], 3);
        assert_eq!(out["usage"]["total_tokens"], 43);
    }
    let req = decode(&fixture("requests", "null-names"));
    let (enc, _) = encode(&req).unwrap();
    assert!(body_of(&enc)["questions"][0].get("name").is_none());
}

#[test]
fn boolean_values_cross_as_strings_and_come_back_typed() {
    let req = decode(&fixture("requests", "booleans"));
    let (enc, c) = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        sent["questions"]["works"]["criteria"],
        json!({"true": "The product works.", "false": "The product is faulty."})
    );
    assert_eq!(
        sent["questions"]["packaging"]["criteria"],
        json!({"unsure": null, "false": null, "yes": null})
    );
    assert_eq!(c.adapted, ["booleans"]);
    let reply = json!({"model": "jev-latest", "answers": {
        "works": {"type": "choice", "choice": "true", "confidence": 0.82, "probabilities": {"true": 0.91, "false": 0.09}},
        "packaging": {"type": "choice", "choice": "false", "confidence": 0.1, "probabilities": {"unsure": 0.2, "false": 0.5, "yes": 0.3}}
    }, "usage": {"input_tokens": 20, "output_tokens": 2}});
    let reply =
        SystemOne::decode_response(200, &HeaderList::new(), &json_body(&reply), &c).unwrap();
    let out = reply_to(DialectId::OpenAiDecisions, &reply, &req);
    assert_eq!(out["answers"][0]["choice"], json!(true));
    assert_eq!(
        out["answers"][0]["probabilities"][1],
        json!({"value": false, "probability": 0.09})
    );
    assert_eq!(out["answers"][1]["choice"], json!(false));
    close(&out["answers"][1]["confidence"], 0.25);

    let mut clash = req.clone();
    clash.questions[1].kind = QuestionKind::Choice(Choice::new(vec![
        ChoiceOption::new(ChoiceValue::Bool(true), None),
        ChoiceOption::new(ChoiceValue::Text("true".into()), None),
    ]));
    encode(&clash).unwrap();
    assert_eq!(
        to_system_one(&clash, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("repeated choice value")
    );
}

#[test]
fn score_labels_are_rebuilt_from_the_request_and_confidence_recomputed() {
    let req = decode(&fixture("requests", "score"));
    let (enc, c) = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(
        body_of(&enc)["questions"]["severity"]["criteria"][0],
        "Cosmetic: Appearance only; no lost functionality."
    );
    assert_eq!(c.adapted, ["levels"]);
    let reply = json!({"model": "jev-latest", "answers": {"severity": {"type": "score", "score": 1.1, "confidence": 0.99,
        "legend": {"0": "Cosmetic: Appearance only; no lost functionality.", "1": "x", "2": "y"},
        "probabilities": {"0": 0.1, "1": 0.7, "2": 0.2}}}, "usage": {"input_tokens": 20, "output_tokens": 2}});
    let reply =
        SystemOne::decode_response(200, &HeaderList::new(), &json_body(&reply), &c).unwrap();
    let out = reply_to(DialectId::OpenAiDecisions, &reply, &req);
    let answer = &out["answers"][0];
    let labels: Vec<&str> = answer["probabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        ["Cosmetic", "Workaround available", "Fully blocked"]
    );
    close(&answer["confidence"], 0.55);
    close(&answer["score"], 1.1);
}

#[test]
fn confidence_is_kept_in_its_own_format_and_recomputed_across() {
    let req = decode(&fixture("requests", "choice"));
    let (_, c) = encode(&req).unwrap();
    let reply = OpenAiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "choice"),
        &c,
    )
    .unwrap();
    let same = reply_to(DialectId::OpenAiDecisions, &reply, &req);
    assert_eq!(same["answers"][0]["confidence"], json!(0.93));
    let Reply::Decide(mut foreign) = reply.clone() else {
        panic!()
    };
    foreign.answers[0].extras = Extras::new(DialectId::SystemOne);
    let across = reply_to(DialectId::OpenAiDecisions, &Reply::Decide(foreign), &req);
    close(&across["answers"][0]["confidence"], (0.95 - 0.25) / 0.75);
}

#[test]
fn refusals_reach_system_one_apps_as_refusal_answers() {
    let s1 = decode_system_one(
        br#"{"model": "gpt-6-luna", "state": "Pick the lock.", "questions": {
            "how_to": {"type": "noul", "instructions": "Would the answer explain the steps?"},
            "topic": {"type": "choice", "instructions": "Which topic is this?", "criteria": {"security": null, "home": null, "other": null}}
        }}"#,
    );
    let (enc, c) = encode(&s1).unwrap();
    let sent = body_of(&enc);
    assert_eq!(sent["questions"][0]["name"], "how_to");
    assert_eq!(
        sent["questions"][1]["choices"],
        json!([{"value": "security"}, {"value": "home"}, {"value": "other"}])
    );
    let reply = OpenAiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "refusal"),
        &c,
    )
    .unwrap();
    let out = reply_to(DialectId::SystemOne, &reply, &s1);
    assert_eq!(out["answers"]["how_to"], json!({"type": "refusal"}));
    assert_eq!(out["answers"]["topic"]["choice"], "security");
    close(
        &out["answers"]["topic"]["confidence"],
        (0.86 - 1.0 / 3.0) / (2.0 / 3.0),
    );
    assert_eq!(
        out["usage"],
        json!({"input_tokens": 51, "output_tokens": 0})
    );
}

#[test]
fn conversations_and_image_urls_dont_cross_to_system_one() {
    let req = decode(&fixture("requests", "conversation"));
    assert_eq!(
        to_system_one(&req, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("conversation input")
    );
    let req = decode(&fixture("requests", "instructions-images"));
    let err = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image URL"));
    assert!(err.message.contains("data URL"), "{}", err.message);
}

#[test]
fn one_user_message_with_json_and_cache_hints_crosses_to_system_one() {
    let req = decode(&fixture("requests", "input-json"));
    let (enc, _) = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap();
    assert_eq!(
        body_of(&enc)["state"],
        json!(["Evaluate the ticket using the order evidence.", {"ticket": {"body": "Please refund the duplicate charge."}, "order": {"charges": [49, 49]}}])
    );
    let mut cached: Value = serde_json::from_slice(&fixture("requests", "image")).unwrap();
    cached["prompt_cache_key"] = json!("k");
    cached["prompt_cache_options"] = json!({"mode": "explicit", "x_future": true});
    cached["safety_identifier"] = json!("u");
    cached["input"][0]["content"][1]["detail"] = json!("high");
    cached["input"][0]["content"][1]["image_url"] = json!(format!(
        "data:image/png;base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png(300, 300))
    ));
    let req = decode(&json_body(&cached));
    assert!(req.extras.fields.contains_key("prompt_cache_options"));
    let (enc, c) = to_system_one(&req, SystemOneProfile::TYPESAFE).unwrap();
    let sent = body_of(&enc);
    assert!(sent.get("prompt_cache_key").is_none() && sent.get("safety_identifier").is_none());
    assert_eq!(sent["state"][1], "kaeso-image-0-ref");
    assert_eq!(c.left_out, ["cache", "user"]);
    let mut foreign_field = req.clone();
    foreign_field
        .extras
        .fields
        .insert("x_future_field".into(), RawJson::null());
    assert_eq!(
        to_system_one(&foreign_field, SystemOneProfile::TYPESAFE)
            .unwrap_err()
            .unsupported_feature(),
        Some("x_future_field")
    );
}

// Crossing from System One apps to OpenAI.

#[test]
fn structured_state_is_input_json_within_the_limits_and_text_beyond() {
    let req = decode_system_one(&system_one_fixture("requests", "structured-state"));
    let (enc, c) = encode(&req).unwrap();
    let sent = body_of(&enc);
    let part = &sent["input"][0]["content"][0];
    assert_eq!(sent["input"][0]["role"], "user");
    assert_eq!(part["type"], "input_json");
    assert!(part["data"].is_object());
    assert!(!c.adapted.contains(&"state".to_owned()));

    let deep = format!("{}1{}", "{\"a\":".repeat(40), "}".repeat(40));
    let mut req = req.clone();
    req.input = DecisionInput::Json(RawJson::from_string(deep.clone()).unwrap());
    let (enc, c) = encode(&req).unwrap();
    let part = &body_of(&enc)["input"][0]["content"][0];
    assert_eq!(part["type"], "input_text");
    assert_eq!(part["text"], deep);
    assert!(c.adapted.contains(&"state".to_owned()));
    for scalar in ["[1, 2]", "12", "null"] {
        req.input = DecisionInput::Json(RawJson::from_string(scalar.into()).unwrap());
        let (enc, c) = encode(&req).unwrap();
        assert_eq!(body_of(&enc)["input"][0]["content"][0]["text"], scalar);
        assert!(c.adapted.contains(&"state".to_owned()));
    }
    req.input = DecisionInput::Text("Plain".into());
    assert_eq!(body_of(&encode(&req).unwrap().0)["input"], "Plain");
}

#[test]
fn criteria_are_folded_into_instructions_with_one_template() {
    let req = decode_system_one(&system_one_fixture("requests", "noul-criteria"));
    let (enc, c) = encode(&req).unwrap();
    let q = &body_of(&enc)["questions"];
    assert_eq!(
        q[0]["instructions"],
        "Is this message spam?\n\nIt is true when: Unsolicited advertising\nIt is false when: A legitimate conversation"
    );
    assert_eq!(
        q[1]["instructions"],
        "This message contains unsolicited advertising."
    );
    let folded = q[2]["instructions"].as_str().unwrap();
    let (instructions, criteria) = folded.split_once("\n\nIt is true when: ").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(instructions).unwrap(),
        json!({"task": "Identify unsolicited advertising."})
    );
    assert_eq!(
        serde_json::from_str::<Value>(criteria).unwrap(),
        json!({"signals": ["prizes", "urgency"]})
    );
    assert_eq!(
        q[4]["instructions"],
        "It is true when: The message asks the reader to reply."
    );
    assert_eq!(q[0]["name"], "spam");
    assert!(c.adapted.contains(&"criteria".to_owned()));
    assert!(c.adapted.contains(&"instructions".to_owned()));
    let mut parts = req.clone();
    parts.questions[0].instructions = Some(Instructions::Blocks(vec![Block::text("Spam?")]));
    let (enc, _) = encode(&parts).unwrap();
    assert_eq!(
        body_of(&enc)["questions"][0]["instructions"],
        json!([{"type": "input_text", "text": "Spam?"}, {"type": "input_text", "text": "It is true when: Unsolicited advertising\nIt is false when: A legitimate conversation"}])
    );
}

#[test]
fn what_openai_cant_take_is_refused_before_sending() {
    let req = decode_system_one(&system_one_fixture("requests", "instructions-forms"));
    let err = encode(&req).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("options"));
    assert_eq!(err.param.as_deref(), Some("questions[3]"));
    let mut one_level = req.clone();
    one_level.questions.remove(3);
    let err = encode(&one_level).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("levels"));
    let mut fits = one_level.clone();
    fits.questions.pop();
    let (enc, c) = encode(&fits).unwrap();
    let sent = body_of(&enc);
    let as_json = |v: &Value| serde_json::from_str::<Value>(v.as_str().unwrap()).unwrap();
    assert_eq!(
        as_json(&sent["questions"][1]["levels"][0]["label"]),
        json!({"level": "low", "means": "Can wait"})
    );
    assert_eq!(
        sent["questions"][1]["levels"][2]["label"],
        "Needs attention today"
    );
    assert_eq!(
        as_json(&sent["questions"][2]["instructions"]),
        json!([
            "Which product area is affected?",
            "Use the last customer message."
        ])
    );
    assert_eq!(
        sent["questions"][0]["instructions"],
        "It is true when: The customer reports broken behavior.\nIt is false when: The customer asks a question."
    );
    assert!(c.adapted.contains(&"criteria".to_owned()));

    let eleven = DecideRequest::new("gpt-6-luna", DecisionInput::Text("x".into())).with_question(
        Question::new(
            Some("s".into()),
            QuestionKind::Score(Score::new(
                (0..11)
                    .map(|i| Level::new(Some(i.to_string()), None))
                    .collect(),
            )),
        ),
    );
    assert_eq!(
        encode(&eleven).unwrap_err().unsupported_feature(),
        Some("levels")
    );
    let wide = DecideRequest::new("gpt-6-luna", DecisionInput::Text("x".into())).with_question(
        Question::new(
            None,
            QuestionKind::Choice(Choice::new(
                (0..256)
                    .map(|i| ChoiceOption::new(ChoiceValue::Text(format!("o{i}")), None))
                    .collect(),
            )),
        ),
    );
    assert_eq!(
        encode(&wide).unwrap_err().unsupported_feature(),
        Some("options")
    );

    let boxed = decode_system_one(&system_one_fixture("requests", "box"));
    let err = encode(&boxed).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("box question"));

    let many = (0..401).fold(
        DecideRequest::new("gpt-6-luna", DecisionInput::Text("x".into())),
        |r, i| r.with_question(predicate(Some(&format!("q{i}")), "?")),
    );
    let err = encode(&many).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("questions"));
    assert!(err.message.contains("400"), "{}", err.message);
    encode_with(&many, DecisionsProfile::GENERIC).unwrap();
    assert_eq!(OpenAiDecisions::encode_error(&err).status, 400);
}

#[test]
fn images_stand_where_their_references_stood() {
    let image = png(300, 300);
    let asset = || Asset::new(Source::bytes("image/png", image.clone()));
    let state =
        RawJson::from_string(r#"["Two photos:", "img-1", {"seats": 12}, "and img-2 here"]"#.into())
            .unwrap();
    let req = DecideRequest::new("gpt-6-luna", DecisionInput::Json(state))
        .with_question(predicate(Some("same"), "Is img-1 like img-2?"))
        .with_asset("img-1", asset())
        .with_asset(
            "img-2",
            Asset::new(Source::Url("https://example.com/b.png".into())),
        );
    let (enc, _) = encode(&req).unwrap();
    let sent = body_of(&enc);
    let content = sent["input"][0]["content"].as_array().unwrap();
    let types: Vec<&str> = content
        .iter()
        .map(|p| p["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "input_text",
            "input_image",
            "input_json",
            "input_text",
            "input_image",
            "input_text"
        ]
    );
    assert!(
        content[1]["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR")
    );
    assert_eq!(content[4]["image_url"], "https://example.com/b.png");
    let instructions = sent["questions"][0]["instructions"].as_array().unwrap();
    let types: Vec<&str> = instructions
        .iter()
        .map(|p| p["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "input_text",
            "input_image",
            "input_text",
            "input_image",
            "input_text"
        ]
    );

    let no_images = encode_with(
        &req,
        DecisionsProfile::GENERIC.with_instruction_images(false),
    )
    .unwrap_err();
    assert_eq!(
        no_images.unsupported_feature(),
        Some("image in question instructions")
    );

    let s1 = decode_system_one(&system_one_fixture("requests", "assets"));
    let err = encode(&s1).unwrap_err();
    assert_eq!(err.unsupported_feature(), Some("image reference"));

    let mut text_state = req.clone();
    text_state.input = DecisionInput::Text("Look: img-1".into());
    text_state.questions[0].instructions = Some(Instructions::Json(
        RawJson::from_string(r#"["Compare", "img-2"]"#.into()).unwrap(),
    ));
    let (enc, _) = encode(&text_state).unwrap();
    let sent = body_of(&enc);
    assert_eq!(sent["input"][0]["content"][1]["type"], "input_image");
    assert_eq!(
        sent["questions"][0]["instructions"][1]["type"],
        "input_image"
    );

    let mut nested = req.clone();
    nested.input = DecisionInput::Json(
        RawJson::from_string(r#"[{"photo": "img-1"}, "img-2"]"#.into()).unwrap(),
    );
    assert_eq!(
        encode(&nested).unwrap_err().unsupported_feature(),
        Some("image reference")
    );
    let mut in_object = req.clone();
    in_object.questions[0].instructions = Some(Instructions::Json(
        RawJson::from_string(r#"{"see": "img-1"}"#.into()).unwrap(),
    ));
    assert_eq!(
        encode(&in_object).unwrap_err().unsupported_feature(),
        Some("image reference")
    );
    let mut in_criteria = req.clone();
    in_criteria.questions.push(Question::new(
        Some("pick".into()),
        QuestionKind::Choice(Choice::new(vec![
            ChoiceOption::new(
                ChoiceValue::Text("a".into()),
                Some(Description::Text("like img-1".into())),
            ),
            ChoiceOption::new(ChoiceValue::Text("b".into()), None),
        ])),
    ));
    assert_eq!(
        encode(&in_criteria).unwrap_err().unsupported_feature(),
        Some("image in criteria")
    );

    let mut ftp = req.clone();
    ftp.assets.insert(
        "img-2".into(),
        Asset::new(Source::Url("ftp://example.com/b.png".into())),
    );
    assert_eq!(
        encode(&ftp).unwrap_err().unsupported_feature(),
        Some("image URL")
    );
    let mut file = req.clone();
    file.assets.insert(
        "img-2".into(),
        Asset::new(Source::file_id(
            Origin::new(DialectId::OpenAiResponses),
            "file-1",
        )),
    );
    let err = encode(&file).unwrap_err();
    assert!(err.message.contains("data URL"), "{}", err.message);
}

#[test]
fn system_one_keys_become_names_and_answers_go_back_under_them() {
    let s1 = decode_system_one(&system_one_fixture("requests", "score"));
    let (enc, c) = encode(&s1).unwrap();
    let sent = body_of(&enc);
    let q = &sent["questions"][0];
    let key = s1.questions[0].name.clone().unwrap();
    assert_eq!(q["name"], key.as_str());
    assert!(q["levels"][0]["label"].is_string());
    assert!(q["levels"][0].get("description").is_none());
    let labels: Vec<Value> = q["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["label"].clone())
        .collect();
    let ps = [0.1, 0.1, 0.8];
    let probabilities: Vec<Value> = labels
        .iter()
        .zip(ps)
        .enumerate()
        .map(|(i, (l, p))| json!({"value": i, "label": l, "probability": p}))
        .collect();
    let reply = json!({"model": "gpt-6-luna", "answers": [{"type": "score", "name": key, "score": 1.7,
        "probabilities": probabilities,
        "confidence": 0.7}], "usage": {"input_tokens": 300, "input_tokens_details": {"cached_tokens": 200, "cache_write_tokens": 0}, "output_tokens": 0, "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 300}});
    let reply =
        OpenAiDecisions::decode_response(200, &HeaderList::new(), &json_body(&reply), &c).unwrap();
    let out = reply_to(DialectId::SystemOne, &reply, &s1);
    let answer = &out["answers"][key.as_str()];
    assert_eq!(answer["type"], "score");
    assert_eq!(answer["legend"].as_object().unwrap().len(), 3);
    close(&answer["confidence"], 0.55);
    assert_eq!(out["usage"]["input_tokens"], 300);
}

#[test]
fn messages_of_other_formats_follow_the_part_rules() {
    let image = ImageBlock::new(Source::bytes("image/png", png(2, 2)));
    let ok = DecideRequest::new(
        "gpt-6-luna",
        DecisionInput::Messages(vec![
            Message::new(Role::System, vec![Block::text("Rules")]),
            Message::new(
                Role::User,
                vec![
                    Block::text("Look"),
                    Block::Image(image.clone()),
                    Block::json(RawJson::from_string("[1]".into()).unwrap()),
                ],
            ),
            Message::new(
                Role::Assistant,
                vec![
                    Block::text("Ok"),
                    Block::Refusal(kaeso_kpp::RefusalBlock::new("No")),
                ],
            ),
            Message::new(
                Role::Assistant,
                vec![
                    Block::text("Calling"),
                    Block::tool_call(
                        "c1",
                        "f",
                        RawJson::from_string(r#"{"a":1}"#.into()).unwrap(),
                    ),
                ],
            ),
            Message::new(
                Role::User,
                vec![Block::Unknown(kaeso_kpp::RawItem::new(
                    Origin::new(DialectId::OpenAiDecisions),
                    RawJson::from_string(r#"{"type":"x_part"}"#.into()).unwrap(),
                ))],
            ),
        ]),
    )
    .with_question(predicate(None, "?"));
    let (enc, c) = encode(&ok).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        sent["input"][0],
        json!({"role": "system", "content": [{"type": "input_text", "text": "Rules"}]})
    );
    assert_eq!(
        sent["input"][1]["content"][2],
        json!({"type": "input_text", "text": "[1]"})
    );
    assert_eq!(
        sent["input"][2]["content"][1],
        json!({"type": "refusal", "refusal": "No"})
    );
    assert_eq!(
        sent["input"][4],
        json!({"type": "function_call", "call_id": "c1", "name": "f", "arguments": "{\"a\":1}"})
    );
    assert!(c.adapted.contains(&"state".to_owned()));
    let refused = [
        (Role::System, Block::Image(image.clone()), "image part"),
        (Role::Assistant, Block::Image(image.clone()), "image part"),
        (
            Role::User,
            Block::audio(Source::bytes("audio/wav", vec![0u8; 4])),
            "audio part",
        ),
        (
            Role::User,
            Block::Refusal(kaeso_kpp::RefusalBlock::new("x")),
            "refusal part",
        ),
        (
            Role::User,
            Block::Unknown(kaeso_kpp::RawItem::new(
                Origin::new(DialectId::OpenAiChat),
                RawJson::null(),
            )),
            "unknown content block",
        ),
    ];
    for (role, block, feature) in refused {
        let req = DecideRequest::new(
            "gpt-6-luna",
            DecisionInput::Messages(vec![Message::new(role, vec![block])]),
        )
        .with_question(predicate(None, "?"));
        assert_eq!(
            encode(&req).unwrap_err().unsupported_feature(),
            Some(feature)
        );
    }
    let mut error_result = ToolResultBlock::new("c1", vec![Block::text("boom")]);
    error_result.is_error = true;
    let req = DecideRequest::new(
        "gpt-6-luna",
        DecisionInput::Messages(vec![Message::new(
            Role::User,
            vec![Block::ToolResult(error_result)],
        )]),
    )
    .with_question(predicate(None, "?"));
    assert_eq!(
        encode(&req).unwrap_err().unsupported_feature(),
        Some("tool error")
    );
    let mut file = ok.clone();
    file.input = DecisionInput::Messages(vec![Message::new(
        Role::User,
        vec![Block::Image(ImageBlock::new(Source::file_id(
            Origin::new(DialectId::OpenAiResponses),
            "file-1",
        )))],
    )]);
    assert_eq!(
        encode(&file).unwrap_err().unsupported_feature(),
        Some("image file id")
    );
}

#[test]
fn decision_requests_are_refused_by_codecs_of_other_kinds() {
    let req = Request::Decide(decode(&fixture("requests", "predicate")));
    for d in [
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
        DialectId::Gemini,
        DialectId::OpenAiEmbeddings,
    ] {
        let err = dispatch::check(d, &req, &EgressCtx::new(&req)).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some("decision request"), "{d}");
        assert!(err.message.contains("/v1/decisions"), "{d}");
    }
    let mut c = EgressCtx::new(&req).with_system_one_profile(SystemOneProfile::GENERIC);
    dispatch::encode_request(DialectId::SystemOne, &req, &mut c).unwrap();
    let mut c = EgressCtx::new(&req);
    dispatch::encode_request(DialectId::OpenAiDecisions, &req, &mut c).unwrap();
}

#[test]
fn json_parts_of_another_format_fit_or_become_text() {
    let mut writer_adapted = Vec::new();
    let mut w = parts::PartWriter {
        dry: false,
        adapted: &mut writer_adapted,
        images: true,
        name: "OpenAI",
    };
    let foreign = Block::json(RawJson::from_string(r#"{"a":1}"#.into()).unwrap());
    let part: Value = w
        .part(&foreign, parts::Place::User, "x")
        .unwrap()
        .to_value()
        .unwrap();
    assert_eq!(part, json!({"type": "input_json", "data": {"a": 1}}));
}
