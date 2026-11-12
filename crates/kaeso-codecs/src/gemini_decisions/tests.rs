use kaeso_kpp::{
    Answer, AnswerValue, Asset, Block, Choice, ChoiceAnswer, ChoiceOption, ChoiceValue,
    DecideRequest, DecideResponse, DecisionInput, Description, DialectId, ErrorKind, Extras,
    ImageBlock, Instructions, KppError, Level, Message, Predicate, PredicateAnswer, Question,
    QuestionKind, RawJson, Refusal, Reply, Request, RequestKind, Role, Score, ScoreAnswer, Source,
    StreamEvent, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::request::{is_question_key, option_key_problem};
use super::*;
use crate::dispatch;
use crate::openai_decisions::{DecisionsProfile, OpenAiDecisions};
use crate::system_one::{SystemOne, SystemOneProfile};
use crate::test_support::{assert_json_eq, encoded_error, fixtures, sidecar};

const DIR: &str = "gemini_decisions";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR42mM4oaEBRAwQCgAhLgRh9uHCTQAAAABJRU5ErkJggg==";

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no {folder}/{name}"))
        .1
}

fn decode(body: &[u8]) -> DecideRequest {
    match GeminiDecisions::decode_request(RequestKind::Decide, body, &HeaderList::new()) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn decode_err(v: &Value) -> KppError {
    GeminiDecisions::decode_request(
        RequestKind::Decide,
        &serde_json::to_vec(v).unwrap(),
        &HeaderList::new(),
    )
    .unwrap_err()
}

fn body_of(enc: &EncodedRequest) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn encode(req: &DecideRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&req).with_provider("google");
    let enc = GeminiDecisions::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn encode_err(req: &DecideRequest) -> KppError {
    encode(req).unwrap_err()
}

fn to_openai(req: &DecideRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&req)
        .with_provider("openai")
        .with_decisions_profile(DecisionsProfile::OPENAI);
    let enc = OpenAiDecisions::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn to_system_one(req: &DecideRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Decide(req.clone());
    let mut c = EgressCtx::new(&req)
        .with_provider("typesafe")
        .with_system_one_profile(SystemOneProfile::GENERIC);
    let enc = SystemOne::encode_request(&req, &mut c)?;
    Ok((enc, c))
}

fn reply_to(app: DialectId, reply: &Reply, req: &DecideRequest) -> Result<Value, KppError> {
    dispatch::encode_response(
        app,
        reply,
        &EncodeCtx::for_request(Request::Decide(req.clone())),
    )
    .map(|out| serde_json::from_slice(&out).unwrap())
}

fn decided(reply: Result<Reply, KppError>) -> DecideResponse {
    match reply {
        Ok(Reply::Decide(r)) => r,
        other => panic!("{other:?}"),
    }
}

fn reply_of(name: &str) -> (DecideRequest, DecideResponse) {
    let req = decode(&fixture("requests", name));
    let (_, c) = encode(&req).unwrap();
    let reply =
        GeminiDecisions::decode_response(200, &HeaderList::new(), &fixture("responses", name), &c);
    (req, decided(reply))
}

fn close(a: &Value, b: f64) {
    let a = a.as_f64().unwrap_or_else(|| panic!("{a} is no number"));
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn text_q(name: Option<&str>, kind: QuestionKind) -> Question {
    Question::new(name.map(str::to_owned), kind).with_instructions(Instructions::Text("Q?".into()))
}

fn noul(name: Option<&str>) -> Question {
    text_q(name, QuestionKind::Predicate(Predicate::default()))
}

fn choice_of(values: &[&str]) -> QuestionKind {
    QuestionKind::Choice(Choice::new(
        values
            .iter()
            .map(|v| ChoiceOption::new(ChoiceValue::Text((*v).to_owned()), None))
            .collect(),
    ))
}

fn score_of(n: usize) -> QuestionKind {
    QuestionKind::Score(Score::new(
        (0..n)
            .map(|i| Level::new(None, Some(Description::Text(format!("level {i}")))))
            .collect(),
    ))
}

fn text_req(questions: Vec<Question>) -> DecideRequest {
    let mut r = DecideRequest::new("gemini-decision-1", DecisionInput::Text("Hi.".into()));
    r.questions = questions;
    r
}

fn user(blocks: Vec<Block>) -> DecisionInput {
    DecisionInput::Messages(vec![Message::new(Role::User, blocks)])
}

fn feature(e: &KppError) -> Option<&str> {
    e.unsupported_feature()
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = decode(&body);
        let (enc, c) = encode(&req).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &body, &enc.body);
        assert_eq!(enc.path, "/v1beta/decisions", "{name}");
        assert_eq!(enc.method, "POST");
        assert!(c.adapted.is_empty(), "{name}: {:?}", c.adapted);
        assert!(c.left_out.is_empty(), "{name}: {:?}", c.left_out);
        assert_eq!(c.decision_keys().count(), req.questions.len(), "{name}");
    }
}

#[test]
fn every_response_fixture_round_trips_through_its_request() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let req = decode(&fixture("requests", &name));
        let (_, c) = encode(&req).unwrap();
        let reply = GeminiDecisions::decode_response(200, &HeaderList::new(), &body, &c);
        if name == "blocked" {
            // The reply answers nothing: every app gets the block as an error (D10).
            let e = reply.unwrap_err();
            assert_eq!(e.kind, ErrorKind::ContentFiltered, "{name}");
            continue;
        }
        let reply = reply.unwrap_or_else(|e| panic!("{name}: {e}"));
        let out =
            GeminiDecisions::encode_response(&reply, &EncodeCtx::for_request(Request::Decide(req)))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn requests_read_as_their_documented_values() {
    let req = decode(&fixture("requests", "noul-criteria"));
    assert_eq!(req.model, "gemini-decision-1");
    assert_eq!(req.questions[0].name.as_deref(), Some("is_defect"));
    let QuestionKind::Predicate(p) = &req.questions[0].kind else {
        panic!()
    };
    assert_eq!(
        p.when_true,
        Some(Description::Text(
            "The product itself is broken or faulty.".into()
        ))
    );
    assert!(p.when_false.is_some());

    let req = decode(&fixture("requests", "multiple-questions"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    assert_eq!((m.len(), m[0].role, m[0].blocks.len()), (1, Role::User, 2));
    let QuestionKind::Choice(c) = &req.questions[1].kind else {
        panic!()
    };
    assert_eq!(c.options.len(), 4);
    assert_eq!(
        c.options[3].value,
        ChoiceValue::Text("human_escalation".into())
    );
    assert_eq!(
        c.options[3].description,
        Some(Description::Text(String::new()))
    );
    let QuestionKind::Score(s) = &req.questions[2].kind else {
        panic!()
    };
    assert_eq!(s.levels.len(), 4);
    assert_eq!(s.levels[0].label, None);

    let req = decode(&fixture("requests", "image"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    let Block::Image(img) = &m[0].blocks[1] else {
        panic!()
    };
    assert!(matches!(&img.source, Source::Base64(s) if s.media_type == "image/png"));
    assert_eq!(
        img.extras.fields_for(DIALECT).unwrap()["resolution"].get(),
        r#""low""#
    );
    assert!(req.required_capabilities().images);

    let req = decode(&fixture("requests", "audio-uri"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    assert!(
        matches!(&m[0].blocks[0], Block::Audio(a) if matches!(&a.source, Source::Url(u) if u.ends_with(".mp3")))
    );
    assert!(req.required_capabilities().audio_input);

    let req = decode(&fixture("requests", "video-uri"));
    assert!(req.required_capabilities().video_input);
    let req = decode(&fixture("requests", "document"));
    assert!(req.required_capabilities().pdf);

    let req = decode(&fixture("requests", "files-api"));
    let DecisionInput::Messages(m) = &req.input else {
        panic!()
    };
    assert!(
        matches!(&m[0].blocks[0], Block::Image(i) if matches!(&i.source, Source::FileId(f) if f.id.ends_with("/files/a1b2c3d4e5")))
    );

    let req = decode(&fixture("requests", "input-object"));
    assert!(matches!(&req.input, DecisionInput::Messages(m) if m[0].blocks.len() == 1));

    let req = decode(&fixture("requests", "instructions-blocks"));
    assert!(
        matches!(&req.questions[0].instructions, Some(Instructions::Blocks(b)) if b.len() == 1)
    );
    assert!(
        matches!(&req.questions[1].instructions, Some(Instructions::Blocks(b)) if b.len() == 2)
    );
}

#[test]
fn responses_read_as_their_documented_values() {
    let (_, r) = reply_of("multiple-questions");
    assert_eq!(r.model, "gemini-decision-1");
    assert_eq!(r.usage.input_tokens, 324);
    assert_eq!(r.usage.output_tokens, 0);
    assert!(
        matches!(&r.answers[0].value, AnswerValue::Predicate(p) if (p.probability - 0.94).abs() < 1e-9)
    );
    assert_eq!(r.answers[0].confidence, Some(0.94));
    let AnswerValue::Choice(c) = &r.answers[1].value else {
        panic!()
    };
    assert_eq!(c.option, 3);
    assert_eq!(
        c.probabilities,
        vec![Some(0.02), Some(0.18), Some(0.04), Some(0.76)]
    );
    let AnswerValue::Score(s) = &r.answers[2].value else {
        panic!()
    };
    assert_eq!(
        s.probabilities,
        vec![Some(0.0), Some(0.0), Some(0.01), Some(0.99)]
    );
    assert!(r.answers.iter().all(|a| a.extras.is_from(DIALECT)));

    let (_, r) = reply_of("video-uri");
    assert_eq!(r.usage.input_tokens, 17 + 1290 + 480);
    assert_eq!(r.usage.input_image_tokens, Some(1290));
    assert_eq!(r.usage.input_audio_tokens, Some(480));
    let (_, r) = reply_of("document");
    assert_eq!(r.usage.input_image_tokens, Some(258));
    assert_eq!(r.usage.input_audio_tokens, None);
    let (_, r) = reply_of("noul-criteria");
    assert!(
        matches!(&r.answers[0].value, AnswerValue::Predicate(p) if (p.probability - 0.03).abs() < 1e-9)
    );
}

#[test]
fn error_fixtures_read_as_their_kinds_and_reach_apps_as_sent() {
    let kinds = [
        ("safety-400", ErrorKind::ContentFiltered),
        ("image-safety-400", ErrorKind::ContentFiltered),
        ("too-many-questions-400", ErrorKind::InvalidRequest),
        ("invalid-option-key-400", ErrorKind::InvalidRequest),
        ("invalid-api-key-400", ErrorKind::Authentication),
        ("rate-limit-429", ErrorKind::RateLimited),
        ("overloaded-503", ErrorKind::Overloaded),
    ];
    let files = fixtures(DIR, "errors", "json");
    assert_eq!(files.len(), kinds.len());
    for (name, body) in files {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = meta["status"].as_u64().unwrap() as u16;
        let e = GeminiDecisions::decode_error(status, &HeaderList::new(), &body);
        let want = &kinds.iter().find(|(n, _)| *n == name).unwrap().1;
        assert_eq!(&e.kind, want, "{name}");
        let out = GeminiDecisions::encode_error(&e);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &encoded_error(&meta, &body), &out.body);
        let via = GeminiDecisions::decode_response(status, &HeaderList::new(), &body, &egress());
        assert_eq!(&via.unwrap_err().kind, want, "{name}");
    }
    let e = GeminiDecisions::decode_error(
        429,
        &HeaderList::new(),
        &fixture("errors", "rate-limit-429"),
    );
    assert_eq!(e.retry_after, Some(std::time::Duration::from_secs(21)));
    let e = GeminiDecisions::decode_error(
        400,
        &HeaderList::new(),
        &fixture("errors", "invalid-option-key-400"),
    );
    assert_eq!(e.param.as_deref(), Some("questions[department].criteria"));
}

fn egress() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Decide, StreamMode::Off)
}

#[test]
fn errors_of_other_formats_get_google_s_envelope_and_blocks_the_safety_code() {
    let limited = KppError::new(ErrorKind::RateLimited, "Slow down.")
        .with_retry_after(std::time::Duration::from_secs(3))
        .with_origin(DialectId::OpenAiDecisions)
        .with_status(429);
    let out = GeminiDecisions::encode_error(&limited);
    assert_eq!(out.status, 429);
    let v: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(v["error"]["status"], "RESOURCE_EXHAUSTED");
    assert_eq!(out.headers.get("retry-after"), Some("3"));

    let filtered = KppError::new(ErrorKind::ContentFiltered, "No.")
        .with_origin(DialectId::OpenAiDecisions)
        .with_status(400);
    let out = GeminiDecisions::encode_error(&filtered);
    assert_eq!(out.status, 400);
    let v: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(v, json!({"error": {"code": "safety", "message": "No."}}));

    let e = GeminiDecisions::decode_error(
        400,
        &HeaderList::new(),
        br#"{"error":{"code":"other","message":"?"}}"#,
    );
    assert_eq!(e.kind, ErrorKind::InvalidRequest);
    assert_eq!(super::error::code_of_block("other"), "content_blocked");
    assert_eq!(super::error::code_of_block("blocklist"), "blocklist");
}

#[test]
fn a_blocked_input_reaches_every_app_as_content_filtered() {
    let req = decode(&fixture("requests", "blocked"));
    let (_, c) = encode(&req).unwrap();
    let e = GeminiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "blocked"),
        &c,
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::ContentFiltered);
    assert_eq!(e.message, "The input was blocked by the safety filters.");
    let out = GeminiDecisions::encode_error(&e);
    assert_eq!(out.status, 400);
    let v: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(v["error"]["code"], "safety");
    for app in [DialectId::OpenAiDecisions, DialectId::SystemOne] {
        let out = dispatch::encode_error(app, &e);
        assert_eq!(out.status, 400, "{app:?}");
    }

    for (reason, code) in [
        ("other", "content_blocked"),
        ("prohibited_content", "prohibited_content"),
    ] {
        let body = json!({"answers": {}, "prompt_feedback": {"block_reason": reason}});
        let e = GeminiDecisions::decode_response(
            200,
            &HeaderList::new(),
            &serde_json::to_vec(&body).unwrap(),
            &c,
        )
        .unwrap_err();
        assert!(e.message.contains(reason), "{}", e.message);
        let v: Value = serde_json::from_slice(&GeminiDecisions::encode_error(&e).body).unwrap();
        assert_eq!(v["error"]["code"], code);
    }
    let unblocked = json!({"answers": {"is_spam": {"type": "noul", "noul": 0.2, "confidence": 0.8}}, "prompt_feedback": {}});
    let body = serde_json::to_vec(&unblocked).unwrap();
    let r = GeminiDecisions::decode_response(200, &HeaderList::new(), &body, &c).unwrap();
    let out =
        GeminiDecisions::encode_response(&r, &EncodeCtx::for_request(Request::Decide(req.clone())))
            .unwrap();
    assert_json_eq("prompt_feedback without a block", &body, &out);
    let bad = json!({"answers": {}, "prompt_feedback": "blocked"});
    let e = GeminiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&bad).unwrap(),
        &c,
    )
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::ProviderError);
}

#[test]
fn decoding_refuses_what_the_decisions_api_refuses() {
    let q = json!({"a": {"type": "noul", "instructions": "A?"}});
    let cases = [
        (json!({"model": "m", "state": "s", "questions": q}), "state"),
        (
            json!({"model": "m", "input": "x", "questions": [{"type": "predicate"}]}),
            "questions",
        ),
        (
            json!({"model": "m", "input": "x", "questions": "q"}),
            "questions",
        ),
        (json!({"model": "m", "input": 5, "questions": q}), "input"),
        (json!({"model": "m", "questions": q}), "input"),
        (
            json!({"model": "m", "input": [5], "questions": q}),
            "input[0]",
        ),
        (
            json!({"model": "m", "input": [{"type": "image", "mime_type": "image/png"}], "questions": q}),
            "input[0]",
        ),
        (
            json!({"model": "m", "input": [{"type": "image", "data": PNG}], "questions": q}),
            "input[0].mime_type",
        ),
        (
            json!({"model": "m", "input": [{"type": "image", "data": PNG, "uri": "https://example.com/a.png", "mime_type": "image/png"}], "questions": q}),
            "input[0]",
        ),
        (
            json!({"model": "m", "input": [{"type": "image", "data": "not base64!", "mime_type": "image/png"}], "questions": q}),
            "input[0].data",
        ),
        (
            json!({"model": "m", "input": [{"type": "text"}], "questions": q}),
            "input[0]",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "box"}}}),
            "questions.a.type",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": "noul"}}),
            "questions.a",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "noul", "criteria": "yes"}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "noul", "criteria": {"true_description": 1}}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "choice"}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "choice", "criteria": ["x"]}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "choice", "criteria": {"x": null}}}}),
            "questions.a.criteria.x",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "score"}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "score", "criteria": [1]}}}),
            "questions.a.criteria",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "noul", "instructions": 5}}}),
            "questions.a.instructions",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "noul", "instructions": [{"type": "image", "uri": "https://example.com/a.png"}]}}}),
            "questions.a.instructions[0]",
        ),
        (
            json!({"model": "m", "input": "x", "questions": {"a": {"type": "noul", "instructions": {"type": "image", "uri": "https://example.com/a.png"}}}}),
            "questions.a.instructions",
        ),
    ];
    for (body, param) in cases {
        let e = decode_err(&body);
        assert_eq!(e.kind, ErrorKind::InvalidRequest, "{body}: {e}");
        assert_eq!(e.param.as_deref(), Some(param), "{body}: {e}");
    }
    let e = GeminiDecisions::decode_request(RequestKind::Generate, b"{}", &HeaderList::new())
        .unwrap_err();
    assert!(feature(&e).is_some());
    let e =
        GeminiDecisions::decode_request(RequestKind::Decide, b"{", &HeaderList::new()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::InvalidRequest);
}

#[test]
fn odd_forms_round_trip_too() {
    for body in [
        json!({"model": "m", "input": [{"type": "thought", "summary": "x"}, {"type": "text", "text": "t", "annotations": []}], "questions": {"a": {"type": "noul", "instructions": [{"type": "text", "text": "A?"}, {"type": "future"}]}}}),
        json!({"model": "m", "input": [], "questions": {"a": {"type": "noul", "criteria": {}}}}),
        json!({"model": "m", "input": {"type": "image", "uri": "https://example.com/x", "resolution": "ultra_high"}, "questions": {"a": {"type": "noul", "instructions": "", "x": null}}}),
        json!({"model": "m", "input": [{"type": "document", "uri": "files/abc", "mime_type": "application/pdf"}], "questions": {"a": {"type": "score", "instructions": "A?", "criteria": ["x", "y"]}}}),
        json!({"model": "m", "input": [{"type": "video", "uri": "https://example.com/a.mp4", "mime_type": "video/mp4", "name": "clip", "processing": {"fps": 1}}], "questions": {"a": {"type": "noul", "instructions": "A?", "criteria": {"true_description": "", "false_description": null}}}}),
    ] {
        let raw = serde_json::to_vec(&body).unwrap();
        let req = decode(&raw);
        let (enc, _) = encode(&req).unwrap_or_else(|e| panic!("{body}: {e}"));
        assert_json_eq(&body.to_string(), &raw, &enc.body);
    }
}

#[test]
fn odd_answers_round_trip_too() {
    let req = decode(&fixture("requests", "multiple-questions"));
    let (_, c) = encode(&req).unwrap();
    for body in [
        json!({"answers": {"is_defect": {"type": "noul", "noul": 1}, "next_action": {"type": "choice", "choice": "instant_refund", "probabilities": {"instant_refund": 1}}, "churn_risk": {"type": "score", "score": 0, "probabilities": {"0": 1}, "confidence": 1}}}),
        json!({"answers": {"churn_risk": {"type": "score", "score": 1.5, "legend": {"1": "Mild frustration"}, "probabilities": {"1": 0.5, "2": 0.5}, "confidence": 0.5, "x": [1]}, "next_action": {"type": "choice", "choice": "human_escalation", "probabilities": {}, "confidence": 0.3}, "is_defect": {"type": "noul", "noul": 0.5, "confidence": 0.5}}, "usage": null, "model_version": "gemini-decision-1-001"}),
        json!({"answers": {"is_defect": {"type": "noul", "noul": 0.01, "confidence": 0.99}, "next_action": {"type": "choice", "choice": "human_escalation", "probabilities": {"human_escalation": 0.9}, "confidence": 0.9}, "churn_risk": {"type": "score", "score": 3.0, "probabilities": {"3": 1.0}, "confidence": 1.0}}, "usage": {"total_input_tokens": 9, "total_tokens": 9, "total_thought_tokens": 0, "grounding_tool_count": []}}),
    ] {
        let raw = serde_json::to_vec(&body).unwrap();
        let reply = GeminiDecisions::decode_response(200, &HeaderList::new(), &raw, &c)
            .unwrap_or_else(|e| panic!("{body}: {e}"));
        let out = GeminiDecisions::encode_response(
            &reply,
            &EncodeCtx::for_request(Request::Decide(req.clone())),
        )
        .unwrap();
        assert_json_eq(&body.to_string(), &raw, &out);
    }
}

#[test]
fn replies_the_decisions_api_never_sends_are_refused() {
    let req = decode(&fixture("requests", "multiple-questions"));
    let (_, c) = encode(&req).unwrap();
    let noul = json!({"type": "noul", "noul": 0.5, "confidence": 0.5});
    let choice = json!({"type": "choice", "choice": "instant_refund", "probabilities": {"instant_refund": 1.0}, "confidence": 1.0});
    let score =
        json!({"type": "score", "score": 0.0, "probabilities": {"0": 1.0}, "confidence": 1.0});
    let with = |answers: Value| json!({"answers": answers});
    for body in [
        json!({"usage": {}}),
        json!({"answers": []}),
        with(json!({"is_defect": noul, "next_action": choice})),
        with(json!({"is_defect": noul, "next_action": choice, "churn_risk": score, "extra": noul})),
        with(json!({"is_defect": choice, "next_action": choice, "churn_risk": score})),
        with(json!({"is_defect": {"type": "noul"}, "next_action": choice, "churn_risk": score})),
        with(
            json!({"is_defect": noul, "next_action": {"type": "choice", "choice": "none", "probabilities": {}}, "churn_risk": score}),
        ),
        with(
            json!({"is_defect": noul, "next_action": {"type": "choice", "choice": "instant_refund", "probabilities": {"none": 1.0}}, "churn_risk": score}),
        ),
        with(
            json!({"is_defect": noul, "next_action": {"type": "choice", "choice": "instant_refund", "probabilities": []}, "churn_risk": score}),
        ),
        with(
            json!({"is_defect": noul, "next_action": {"type": "choice", "choice": "instant_refund", "probabilities": {"instant_refund": "1"}}, "churn_risk": score}),
        ),
        with(
            json!({"is_defect": noul, "next_action": {"type": "choice", "choice": 1, "probabilities": {}}, "churn_risk": score}),
        ),
        with(
            json!({"is_defect": noul, "next_action": choice, "churn_risk": {"type": "score", "score": 0, "probabilities": {"4": 1.0}}}),
        ),
        with(
            json!({"is_defect": noul, "next_action": choice, "churn_risk": {"type": "score", "score": 0, "probabilities": {"01": 1.0}}}),
        ),
        with(
            json!({"is_defect": noul, "next_action": choice, "churn_risk": {"type": "score", "probabilities": {}}}),
        ),
        json!({"answers": {"is_defect": noul, "next_action": choice, "churn_risk": score}, "usage": {"total_input_tokens": "many"}}),
        json!("answers"),
    ] {
        let raw = serde_json::to_vec(&body).unwrap();
        let e = GeminiDecisions::decode_response(200, &HeaderList::new(), &raw, &c).unwrap_err();
        assert_eq!(e.kind, ErrorKind::ProviderError, "{body}: {e}");
    }
    let ok = serde_json::to_vec(&with(
        json!({"is_defect": noul, "next_action": choice, "churn_risk": score}),
    ))
    .unwrap();
    let e = GeminiDecisions::decode_response(200, &HeaderList::new(), &ok, &egress()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::ProviderError);
    let generate = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(GeminiDecisions::decode_response(200, &HeaderList::new(), &ok, &generate).is_err());
}

#[test]
fn replies_without_their_request_or_with_other_answers_are_refused() {
    let (req, r) = reply_of("choice");
    let reply = Reply::Decide(r.clone());
    assert!(
        GeminiDecisions::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Decide, StreamMode::Off)
        )
        .is_err()
    );
    let mut fewer = r.clone();
    fewer.answers.clear();
    assert!(
        GeminiDecisions::encode_response(
            &Reply::Decide(fewer),
            &EncodeCtx::for_request(Request::Decide(req.clone()))
        )
        .is_err()
    );
    let mut wrong = r.clone();
    wrong.answers[0].value = AnswerValue::Predicate(PredicateAnswer::new(0.5));
    assert!(reply_to(DIALECT, &Reply::Decide(wrong), &req).is_err());
    let mut out_of_range = r;
    out_of_range.answers[0].value = AnswerValue::Choice(ChoiceAnswer::new(9, vec![]));
    assert!(reply_to(DIALECT, &Reply::Decide(out_of_range), &req).is_err());
    let mut nan = DecideResponse::new(
        "m",
        vec![Answer::new(AnswerValue::Predicate(PredicateAnswer::new(
            f64::NAN,
        )))],
    );
    nan.usage = Usage::new(1, 0);
    let one = decode(&fixture("requests", "noul"));
    assert!(reply_to(DIALECT, &Reply::Decide(nan), &one).is_err());
    let embed = Reply::Embed(kaeso_kpp::EmbedResponse::default());
    assert!(
        GeminiDecisions::encode_response(
            &embed,
            &EncodeCtx::new(RequestKind::Decide, StreamMode::Off)
        )
        .is_err()
    );
}

#[test]
fn decision_requests_are_never_streamed() {
    let mut enc =
        GeminiDecisions::stream_encoder(&EncodeCtx::new(RequestKind::Decide, StreamMode::Off));
    assert!(
        enc.encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
            .is_err()
    );
    assert!(enc.finish().unwrap().is_empty());
    let mut dec = GeminiDecisions::stream_decoder(&egress());
    assert!(
        matches!(dec.feed(b"data: {}").as_slice(), [StreamEvent::Error(e)] if feature(e) == Some("streaming"))
    );
    assert!(dec.finish().is_empty());
}

#[test]
fn listings_are_geminis() {
    let list = kaeso_kpp::ModelList::new(vec![kaeso_kpp::ModelInfo::new("gemini-decision-1")]);
    let out = GeminiDecisions::encode_models(&list).unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["models"][0]["name"], "models/gemini-decision-1");
    let back = GeminiDecisions::decode_models(200, &HeaderList::new(), &out).unwrap();
    assert_eq!(back.models[0].id, "gemini-decision-1");
    let one = GeminiDecisions::encode_model(&list.models[0]).unwrap();
    assert_eq!(
        GeminiDecisions::decode_model(200, &HeaderList::new(), &one)
            .unwrap()
            .id,
        "gemini-decision-1"
    );
    assert!(GeminiDecisions::decode_models_query("pageSize=5").is_ok());
    assert!(
        GeminiDecisions::encode_model_request("gemini-decision-1")
            .path
            .contains("gemini-decision-1")
    );
    let q = kaeso_kpp::ModelsQuery::default();
    assert!(GeminiDecisions::encode_models_request(&q).is_ok());
    assert!(GeminiDecisions::encode_models_request_with(&q, &egress()).is_ok());
    assert!(GeminiDecisions::encode_model_request_with("m", &egress()).is_ok());
}

#[test]
fn request_limits_count_questions_options_and_blocks() {
    let many: serde_json::Map<String, Value> = (0..=crate::limits::MAX_DECISION_QUESTIONS)
        .map(|i| {
            (
                format!("q{i}"),
                json!({"type": "noul", "instructions": "?"}),
            )
        })
        .collect();
    let e = decode_err(&json!({"model": "m", "input": "x", "questions": many}));
    assert_eq!(e.param.as_deref(), Some("questions"));
    let options: serde_json::Map<String, Value> = (0..=crate::limits::MAX_DECISION_OPTIONS)
        .map(|i| (format!("o{i}"), json!("")))
        .collect();
    let e = decode_err(
        &json!({"model": "m", "input": "x", "questions": {"a": {"type": "choice", "criteria": options}}}),
    );
    assert_eq!(e.param.as_deref(), Some("questions"));
    let blocks: Vec<Value> = (0..=crate::limits::MAX_BLOCKS_PER_MESSAGE)
        .map(|_| json!({"type": "text", "text": "x"}))
        .collect();
    let e =
        decode_err(&json!({"model": "m", "input": blocks, "questions": {"a": {"type": "noul"}}}));
    assert_eq!(e.param.as_deref(), Some("input"));
    assert!(
        e.message.starts_with("`input` holds more than"),
        "{}",
        e.message
    );
}

#[test]
fn headers_and_paths_follow_the_context() {
    let req = Request::Decide(decode(&fixture("requests", "noul")));
    let mut c = EgressCtx::new(&req).with_base_path("/custom/decisions/");
    let enc = GeminiDecisions::encode_request(&req, &mut c).unwrap();
    assert_eq!(enc.path, "/custom/decisions");
    assert_eq!(enc.headers.get("content-type"), Some("application/json"));
    assert_eq!(c.decision_keys().collect::<Vec<_>>(), vec!["is_damaged"]);
    let generate = Request::Generate(kaeso_kpp::GenerateRequest::default());
    let mut c = EgressCtx::new(&generate);
    assert!(GeminiDecisions::check(&generate, &c).is_err());
    assert!(GeminiDecisions::encode_request(&generate, &mut c).is_err());
}

#[test]
fn keys_follow_googles_rules() {
    for ok in ["a", "is_damaged", "A-9_", &"x".repeat(64)] {
        assert!(is_question_key(ok), "{ok}");
    }
    for no in ["", "has space", "ä", "a.b", &"x".repeat(65)] {
        assert!(!is_question_key(no), "{no}");
    }
    assert_eq!(option_key_problem("needs review"), None);
    assert_eq!(option_key_problem("ünïcode"), None);
    assert_eq!(option_key_problem(""), Some("is empty"));
    assert!(option_key_problem(&"x".repeat(129)).is_some());
    assert!(option_key_problem(&"é".repeat(128)).is_none());
    assert!(option_key_problem(&"日本語".repeat(43)).is_some());
    assert!(option_key_problem(" a").is_some());
    assert!(option_key_problem("a ").is_some());
    assert!(option_key_problem("a\u{7}b").is_some());
    assert!(option_key_problem("a\u{a0}b").is_some());
}

#[test]
fn names_become_positions_where_google_refuses_them() {
    for names in [
        vec![Some("a"), None],
        vec![Some("a"), Some("a")],
        vec![Some("a"), Some("has space")],
    ] {
        let req = text_req(names.iter().map(|n| noul(*n)).collect());
        let (enc, c) = encode(&req).unwrap();
        let v = body_of(&enc);
        assert_eq!(
            v["questions"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["q0", "q1"]
        );
        assert_eq!(c.adapted, vec!["names"]);
        assert_eq!(c.decision_keys().collect::<Vec<_>>(), vec!["q0", "q1"]);
        let reply = json!({"answers": {"q1": {"type": "noul", "noul": 0.2, "confidence": 0.8}, "q0": {"type": "noul", "noul": 0.7, "confidence": 0.7}}});
        let r = decided(GeminiDecisions::decode_response(
            200,
            &HeaderList::new(),
            &serde_json::to_vec(&reply).unwrap(),
            &c,
        ));
        assert!(matches!(&r.answers[0].value, AnswerValue::Predicate(p) if p.probability == 0.7));
        let mut openai = req.clone();
        openai.extras = Extras::new(DialectId::OpenAiDecisions);
        let back = reply_to(DialectId::OpenAiDecisions, &Reply::Decide(r), &openai).unwrap();
        assert_eq!(back["answers"][0]["name"], json!(names[0]));
    }
}

#[test]
fn options_cross_with_googles_key_rules() {
    let mut booleans = Choice::new(vec![
        ChoiceOption::new(
            ChoiceValue::Bool(true),
            Some(Description::Text("yes".into())),
        ),
        ChoiceOption::new(ChoiceValue::Bool(false), None),
    ]);
    let req = text_req(vec![text_q(
        Some("b"),
        QuestionKind::Choice(booleans.clone()),
    )]);
    let (enc, c) = encode(&req).unwrap();
    assert_eq!(
        body_of(&enc)["questions"]["b"]["criteria"],
        json!({"true": "yes", "false": ""})
    );
    assert_eq!(c.adapted, vec!["booleans"]);
    let reply = json!({"answers": {"b": {"type": "choice", "choice": "false", "probabilities": {"true": 0.3, "false": 0.7}, "confidence": 0.7}}});
    let r = decided(GeminiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&reply).unwrap(),
        &c,
    ));
    assert!(matches!(&r.answers[0].value, AnswerValue::Choice(a) if a.option == 1));
    let back = reply_to(DialectId::OpenAiDecisions, &Reply::Decide(r), &req).unwrap();
    assert_eq!(back["answers"][0]["choice"], json!(false));
    close(&back["answers"][0]["confidence"], 0.4);

    booleans
        .options
        .push(ChoiceOption::new(ChoiceValue::Text("true".into()), None));
    let e = encode_err(&text_req(vec![text_q(
        Some("b"),
        QuestionKind::Choice(booleans),
    )]));
    assert_eq!(feature(&e), Some("repeated choice value"));
    for bad in [" lead", "trail ", "bell\u{7}", "nb\u{a0}sp", ""] {
        let e = encode_err(&text_req(vec![text_q(Some("c"), choice_of(&[bad, "ok"]))]));
        assert_eq!(feature(&e), Some("option key"), "{bad:?}");
        assert_eq!(e.param.as_deref(), Some("questions[0].options[0]"));
    }
    for n in [1, 256] {
        let values: Vec<String> = (0..n).map(|i| format!("o{i}")).collect();
        let refs: Vec<&str> = values.iter().map(String::as_str).collect();
        let e = encode_err(&text_req(vec![text_q(Some("c"), choice_of(&refs))]));
        assert_eq!(feature(&e), Some("options"), "{n}");
    }
    for n in [1, 256] {
        let e = encode_err(&text_req(vec![text_q(Some("s"), score_of(n))]));
        assert_eq!(feature(&e), Some("levels"), "{n}");
    }
    assert!(encode(&text_req(vec![text_q(Some("s"), score_of(255))])).is_ok());
    let e = encode_err(&text_req(vec![text_q(
        None,
        QuestionKind::Box(kaeso_kpp::BoxQuestion::new()),
    )]));
    assert_eq!(feature(&e), Some("box question"));
    let e = encode_err(&text_req((0..1001).map(|_| noul(None)).collect()));
    assert_eq!(feature(&e), Some("questions"));
}

#[test]
fn levels_become_texts_and_labels_join_their_descriptions() {
    let levels = Score::new(vec![
        Level::new(
            Some("Low".into()),
            Some(Description::Text("Barely.".into())),
        ),
        Level::new(Some("High".into()), None),
        Level::new(
            None,
            Some(Description::Json(
                RawJson::from_string(r#"{"x":1}"#.into()).unwrap(),
            )),
        ),
    ]);
    let req = text_req(vec![text_q(Some("s"), QuestionKind::Score(levels))]);
    let (enc, c) = encode(&req).unwrap();
    assert_eq!(
        body_of(&enc)["questions"]["s"]["criteria"],
        json!(["Low: Barely.", "High", r#"{"x":1}"#])
    );
    assert_eq!(c.adapted, vec!["levels", "criteria"]);
    let reply = json!({"answers": {"s": {"type": "score", "score": 1.0, "legend": {}, "probabilities": {"0": 0.2, "1": 0.6, "2": 0.2}, "confidence": 0.6}}});
    let r = decided(GeminiDecisions::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&reply).unwrap(),
        &c,
    ));
    let back = reply_to(DialectId::OpenAiDecisions, &Reply::Decide(r.clone()), &req).unwrap();
    assert_eq!(back["answers"][0]["probabilities"][0]["label"], "Low");
    let mut answer = r.answers[0].clone();
    answer.extras = Extras::default();
    let fresh = DecideResponse::new("m", vec![answer]);
    let gemini = reply_to(DIALECT, &Reply::Decide(fresh), &req).unwrap();
    assert_eq!(
        gemini["answers"]["s"]["legend"],
        json!({"0": "Low: Barely.", "1": "High", "2": r#"{"x":1}"#})
    );
    close(&gemini["answers"]["s"]["confidence"], 0.6);
    for empty in [
        Level::new(None, None),
        Level::new(Some(String::new()), None),
    ] {
        let e = encode_err(&text_req(vec![text_q(
            Some("s"),
            QuestionKind::Score(Score::new(vec![empty, Level::new(Some("x".into()), None)])),
        )]));
        assert_eq!(feature(&e), Some("score level"));
    }
}

#[test]
fn openai_requests_cross_to_gemini() {
    let body = json!({
        "model": "gemini-decision-1",
        "input": [{"role": "user", "content": [
            {"type": "input_text", "text": "Look:", "prompt_cache_breakpoint": {"mode": "explicit"}},
            {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "low"},
            {"type": "input_image", "image_url": "https://example.com/a.jpg", "detail": "auto"},
            {"type": "input_json", "data": {"order": 1}}
        ]}],
        "questions": [
            {"type": "predicate", "name": "ok", "instructions": [{"type": "input_text", "text": "Fine? "}, {"type": "input_json", "data": {"x": 1}}]},
            {"type": "choice", "name": "pick", "instructions": "Which?", "choices": [{"value": "a"}, {"value": "b", "description": "B"}]}
        ],
        "prompt_cache_key": "k",
        "safety_identifier": "u"
    });
    let req = match OpenAiDecisions::decode_request(
        RequestKind::Decide,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    ) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    };
    let (enc, c) = encode(&req).unwrap();
    let v = body_of(&enc);
    assert_eq!(
        v["input"],
        json!([
            {"type": "text", "text": "Look:"},
            {"type": "image", "data": PNG, "mime_type": "image/png", "resolution": "low"},
            {"type": "image", "uri": "https://example.com/a.jpg", "mime_type": "image/jpeg"},
            {"type": "text", "text": r#"{"order":1}"#}
        ])
    );
    assert_eq!(
        v["questions"]["ok"]["instructions"],
        json!([{"type": "text", "text": "Fine? "}, {"type": "text", "text": r#"{"x":1}"#}])
    );
    assert_eq!(
        v["questions"]["pick"]["criteria"],
        json!({"a": "", "b": "B"})
    );
    assert_eq!(c.adapted, vec!["instructions"]);
    assert_eq!(c.left_out, vec!["cache", "user"]);
}

#[test]
fn what_gemini_cant_take_is_refused_before_sending() {
    let cases: Vec<(DecisionInput, &str)> = vec![
        (
            DecisionInput::Messages(vec![
                Message::new(Role::User, vec![Block::text("a")]),
                Message::new(Role::Assistant, vec![Block::text("b")]),
            ]),
            "conversation input",
        ),
        (
            DecisionInput::Messages(vec![Message::new(Role::System, vec![Block::text("a")])]),
            "conversation input",
        ),
        (
            user(vec![Block::Image(ImageBlock::new(Source::Url(
                "http://example.com/a.png".into(),
            )))]),
            "image URL",
        ),
        (
            user(vec![Block::Image(ImageBlock::new(Source::Url(
                "gs://bucket/a.png".into(),
            )))]),
            "Cloud Storage URI",
        ),
        (
            user(vec![Block::Video(kaeso_kpp::VideoBlock::new(Source::Url(
                "https://youtu.be/abc".into(),
            )))]),
            "YouTube URL",
        ),
        (
            user(vec![Block::Image(ImageBlock::new(Source::Url(
                "https://example.com/image".into(),
            )))]),
            "image URL without a known file extension",
        ),
        (
            user(vec![Block::Image(ImageBlock::new(Source::Url(
                "data:image/png,abc".into(),
            )))]),
            "image data URL that is not base64",
        ),
        (
            user(vec![Block::Document(kaeso_kpp::DocumentBlock::new(
                Source::text("text/plain", "hi"),
            ))]),
            "text document",
        ),
        (
            user(vec![Block::Refusal(kaeso_kpp::RefusalBlock::new("no"))]),
            "refusal part",
        ),
        (
            user(vec![Block::Image(ImageBlock::new(Source::file_id(
                DialectId::OpenAiDecisions,
                "file-1",
            )))]),
            "image source",
        ),
    ];
    for (input, want) in cases {
        let mut req = text_req(vec![noul(Some("a"))]);
        req.input = input;
        let e = encode_err(&req);
        assert!(
            feature(&e) == Some(want) || e.message.contains("file id"),
            "{want}: {e:?}"
        );
    }
    let mut q = noul(Some("a"));
    q.instructions = Some(Instructions::Blocks(vec![Block::Image(ImageBlock::new(
        Source::Url("https://example.com/a.png".into()),
    ))]));
    assert_eq!(
        feature(&encode_err(&text_req(vec![q]))),
        Some("image in question instructions")
    );
    let mut q = noul(Some("a"));
    q.instructions = Some(Instructions::Blocks(vec![Block::Refusal(
        kaeso_kpp::RefusalBlock::new("no"),
    )]));
    assert_eq!(
        feature(&encode_err(&text_req(vec![q]))),
        Some("refusal part")
    );
}

#[test]
fn system_one_requests_cross_to_gemini() {
    let body = json!({
        "model": "gemini-decision-1",
        "state": {"order": {"id": "8821"}, "tier": "Gold"},
        "questions": {
            "is_gold": {"type": "noul", "instructions": {"field": "tier"}, "criteria": {"true": {"tier": "Gold"}, "false": "Anything else."}},
            "pick": {"type": "choice", "instructions": "Which?", "criteria": {"a": null, "b": "B"}}
        }
    });
    let req = match SystemOne::decode_request(
        RequestKind::Decide,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    ) {
        Ok(Request::Decide(r)) => r,
        other => panic!("{other:?}"),
    };
    let (enc, c) = encode(&req).unwrap();
    let v = body_of(&enc);
    assert_eq!(
        v["input"],
        json!(r#"{"order":{"id":"8821"},"tier":"Gold"}"#)
    );
    assert_eq!(
        v["questions"]["is_gold"]["instructions"],
        json!(r#"{"field":"tier"}"#)
    );
    assert_eq!(
        v["questions"]["is_gold"]["criteria"],
        json!({"true_description": r#"{"tier":"Gold"}"#, "false_description": "Anything else."})
    );
    assert_eq!(
        v["questions"]["pick"]["criteria"],
        json!({"a": "", "b": "B"})
    );
    assert_eq!(c.adapted, vec!["instructions", "criteria"]);
}

fn with_image(input: DecisionInput) -> DecideRequest {
    let mut req = text_req(vec![noul(Some("a"))]);
    req.input = input;
    req.with_asset(
        "<img>",
        Asset::new(Source::base64("image/png", PNG).unwrap()),
    )
}

#[test]
fn asset_references_become_image_blocks_in_place() {
    let req = with_image(DecisionInput::Text("Before <img> after".into()));
    let (enc, _) = encode(&req).unwrap();
    assert_eq!(
        body_of(&enc)["input"],
        json!([{"type": "text", "text": "Before "}, {"type": "image", "data": PNG, "mime_type": "image/png"}, {"type": "text", "text": " after"}])
    );
    let list = RawJson::from_string(r#"["Photo:", "<img>", {"n": 1}, "end"]"#.into()).unwrap();
    let (enc, _) = encode(&with_image(DecisionInput::Json(list))).unwrap();
    assert_eq!(
        body_of(&enc)["input"],
        json!([{"type": "text", "text": "Photo:"}, {"type": "image", "data": PNG, "mime_type": "image/png"}, {"type": "text", "text": "{\"n\": 1}\nend"}])
    );
    let list = RawJson::from_string(r#"[{"n": 1}, "a <img>"]"#.into()).unwrap();
    let (enc, _) = encode(&with_image(DecisionInput::Json(list))).unwrap();
    assert_eq!(
        body_of(&enc)["input"][0],
        json!({"type": "text", "text": "{\"n\": 1}\na "})
    );
    let (enc, _) = encode(&with_image(user(vec![Block::text("see <img>")]))).unwrap();
    assert_eq!(body_of(&enc)["input"][1]["type"], "image");

    for input in [
        DecisionInput::Json(RawJson::from_string(r#"{"photo": "<img>"}"#.into()).unwrap()),
        DecisionInput::Json(RawJson::from_string(r#"[{"photo": "<img>"}]"#.into()).unwrap()),
    ] {
        assert_eq!(
            feature(&encode_err(&with_image(input))),
            Some("image reference")
        );
    }
    let mut req = text_req(vec![text_q(
        Some("a"),
        QuestionKind::Predicate(Predicate::default()),
    )]);
    req.questions[0].instructions = Some(Instructions::Text("Is <img> broken?".into()));
    let req = req.with_asset(
        "<img>",
        Asset::new(Source::base64("image/png", PNG).unwrap()),
    );
    assert_eq!(
        feature(&encode_err(&req)),
        Some("image in question instructions")
    );
    let mut req = text_req(vec![text_q(
        Some("a"),
        QuestionKind::Predicate(Predicate::new(
            Some(Description::Text("like <img>".into())),
            None,
        )),
    )]);
    req = req.with_asset(
        "<img>",
        Asset::new(Source::base64("image/png", PNG).unwrap()),
    );
    assert_eq!(feature(&encode_err(&req)), Some("image in criteria"));
}

#[test]
fn gemini_requests_cross_to_openai_and_system_one() {
    let req = decode(&fixture("requests", "noul-criteria"));
    let (enc, c) = to_openai(&req).unwrap();
    let v = body_of(&enc);
    assert!(
        v["questions"][0]["instructions"]
            .as_str()
            .unwrap()
            .contains("It is true when: The product itself is broken or faulty.")
    );
    assert_eq!(c.adapted, vec!["criteria"]);
    let (enc, _) = to_system_one(&req).unwrap();
    assert_eq!(
        body_of(&enc)["questions"]["is_defect"]["criteria"]["true"],
        "The product itself is broken or faulty."
    );

    let req = decode(&fixture("requests", "image"));
    let (enc, _) = to_openai(&req).unwrap();
    let image = &body_of(&enc)["input"][0]["content"][1];
    assert_eq!(image["detail"], "low");
    assert!(
        image["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    let (enc, _) = to_system_one(&req).unwrap();
    assert!(body_of(&enc)["assets"].is_object());

    for name in ["audio-uri", "video-uri", "document"] {
        let req = decode(&fixture("requests", name));
        let e = to_openai(&req).unwrap_err();
        assert!(
            feature(&e).is_some_and(|f| f.ends_with("part")),
            "{name}: {e:?}"
        );
        assert!(to_system_one(&req).is_err(), "{name}");
    }
    let req = decode(&fixture("requests", "files-api"));
    assert!(to_openai(&req).unwrap_err().message.contains("file id"));
    let req = decode(&fixture("requests", "unknown-fields"));
    assert!(to_openai(&req).is_err());
    let mut req = decode(&fixture("requests", "video-uri"));
    req.input = user(vec![Block::text("x")]);
    let mut q = noul(Some("a"));
    q.instructions = Some(Instructions::Blocks(vec![Block::text("A?")]));
    req.questions = vec![q];
    assert!(to_openai(&req).is_ok());
}

#[test]
fn replies_cross_with_confidence_recomputed_by_the_apps_rule() {
    let (req, r) = reply_of("multiple-questions");
    let mut openai = req.clone();
    openai.extras = Extras::new(DialectId::OpenAiDecisions);
    let v = reply_to(
        DialectId::OpenAiDecisions,
        &Reply::Decide(r.clone()),
        &openai,
    )
    .unwrap();
    assert!(v["answers"][0].get("confidence").is_none());
    close(&v["answers"][1]["confidence"], (0.76 - 0.25) / 0.75);
    assert_eq!(v["model"], "gemini-decision-1");
    let v = reply_to(DialectId::SystemOne, &Reply::Decide(r.clone()), &req).unwrap();
    close(
        &v["answers"]["next_action"]["confidence"],
        (0.76 - 0.25) / 0.75,
    );
    assert_eq!(v["usage"]["input_tokens"], 324);
    let v = reply_to(DIALECT, &Reply::Decide(r), &req).unwrap();
    close(&v["answers"]["next_action"]["confidence"], 0.76);

    let other = DecideResponse::new(
        "jev-1.14.0",
        vec![
            Answer::new(AnswerValue::Predicate(PredicateAnswer::new(0.3))),
            Answer::new(AnswerValue::Choice(ChoiceAnswer::new(
                1,
                vec![Some(0.1), Some(0.5), None, Some(0.4)],
            )))
            .with_confidence(Some(0.2)),
            Answer::new(AnswerValue::Score(ScoreAnswer::new(
                2.0,
                vec![Some(0.1), Some(0.2), Some(0.3), Some(0.4)],
            ))),
        ],
    );
    let mut other = other;
    other.usage = Usage::new(100, 5);
    other.usage.cache_read_tokens = 20;
    other.usage.input_image_tokens = Some(30);
    other.usage.input_audio_tokens = Some(10);
    let v = reply_to(DIALECT, &Reply::Decide(other.clone()), &req).unwrap();
    close(&v["answers"]["is_defect"]["confidence"], 0.7);
    close(&v["answers"]["next_action"]["confidence"], 0.5);
    assert_eq!(
        v["answers"]["next_action"]["choice"],
        "replacement_shipment"
    );
    assert!(
        v["answers"]["next_action"]["probabilities"]
            .get("troubleshooting_flow")
            .is_none()
    );
    close(&v["answers"]["churn_risk"]["confidence"], 0.4);
    assert_eq!(
        v["answers"]["churn_risk"]["legend"]["3"],
        "Threatens to cancel"
    );
    assert!(v.get("model").is_none());
    assert_eq!(
        v["usage"],
        json!({
            "total_input_tokens": 120,
            "input_tokens_by_modality": [
                {"modality": "text", "tokens": 80},
                {"modality": "image", "tokens": 30},
                {"modality": "audio", "tokens": 10}
            ],
            "total_output_tokens": 5,
            "total_tokens": 125,
            "total_cached_tokens": 20
        })
    );
    let mut none = other.clone();
    none.answers[1].value = AnswerValue::Choice(ChoiceAnswer::new(0, vec![None; 4]));
    let v = reply_to(DIALECT, &Reply::Decide(none), &req).unwrap();
    assert!(v["answers"]["next_action"].get("confidence").is_none());
}

#[test]
fn refusals_reach_gemini_apps_as_content_filtered() {
    let (req, mut r) = reply_of("multiple-questions");
    r.answers[0].value = AnswerValue::Refusal(Refusal::new());
    r.answers[2].value = AnswerValue::Refusal(Refusal::new());
    let e = reply_to(DIALECT, &Reply::Decide(r), &req).unwrap_err();
    assert_eq!(e.kind, ErrorKind::ContentFiltered);
    assert!(
        e.message.contains("`is_defect`, `churn_risk`"),
        "{}",
        e.message
    );
    let out = GeminiDecisions::encode_error(&e);
    assert_eq!(out.status, 400);
    let v: Value = serde_json::from_slice(&out.body).unwrap();
    assert_eq!(v["error"]["code"], "safety");
}

#[test]
fn decision_requests_are_refused_by_codecs_of_other_kinds() {
    let req = Request::Decide(decode(&fixture("requests", "noul")));
    for dialect in [DialectId::OpenAiChat, DialectId::Gemini] {
        let mut c = EgressCtx::new(&req);
        let e = dispatch::encode_request(dialect, &req, &mut c).unwrap_err();
        assert!(
            e.message.contains("`/v1beta/decisions`"),
            "{dialect:?}: {e}"
        );
    }
}

#[test]
fn foreign_extras_and_unknown_blocks_are_refused_elsewhere() {
    let req = decode(&fixture("requests", "unknown-top-level-field"));
    let e = to_openai(&req).unwrap_err();
    assert!(e.message.contains("x_future_field"), "{e}");
    let body = json!({"model": "m", "input": [{"type": "thought", "summary": "x"}], "questions": {"a": {"type": "noul", "instructions": "A?"}}});
    let req = decode(&serde_json::to_vec(&body).unwrap());
    assert!(to_system_one(&req).is_err());
    let mut foreign = text_req(vec![noul(Some("a"))]);
    foreign.input = user(vec![Block::Unknown(kaeso_kpp::RawItem::new(
        DialectId::OpenAiDecisions,
        RawJson::from_string(r#"{"type":"x"}"#.into()).unwrap(),
    ))]);
    assert!(encode(&foreign).is_err());
}
