//! Decision replies of the mock provider: each request is answered in its own format, one answer
//! per question it asked, so the gateway reads back the questions it sent.
//!
//! - Each yes/no question is answered `0.82`, each choice with its first option, each score near
//!   its top level and each box question with one box.
//! - `[mock:refuse]` in the request refuses every question, as OpenAI may.
//! - Usage depends on the format: OpenAI reports 1,000 of 1,200 input tokens as cached,
//!   Gemini counts 258 image tokens when the input has an image, and System One reports one
//!   output token per question.

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use kaeso_codecs::{EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Answer, AnswerValue, Block, BoxAnswer, ChoiceAnswer, DecideRequest, DecideResponse,
    DecisionInput, DialectId, Instructions, PredicateAnswer, QuestionKind, Refusal, Reply,
    Request, RequestKind, ScoreAnswer, Usage,
};

/// The decision format a request to `path` is in; `None` for any other request.
pub fn dialect(path: &str, body: &[u8]) -> Option<DialectId> {
    if path == "/v1beta/decisions" {
        return Some(DialectId::GeminiDecisions);
    }
    if path.ends_with("/v1/systemone") {
        return Some(DialectId::SystemOne);
    }
    if path.ends_with("/v1/decisions") {
        // Perplexity's Decider takes a System One body at `/v1/decisions`.
        let system_one = serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .is_some_and(|v| v.get("state").is_some() && v.get("input").is_none());
        return Some(if system_one {
            DialectId::SystemOne
        } else {
            DialectId::OpenAiDecisions
        });
    }
    None
}

/// A reply to the decision request, in its own format.
pub fn answer(dialect: DialectId, body: &Bytes, headers: &HeaderMap) -> Response {
    let mut listed = HeaderList::new();
    for (name, value) in headers {
        if let Ok(v) = value.to_str() {
            listed.push(name.as_str(), v);
        }
    }
    let req = match dispatch::decode_request(dialect, RequestKind::Decide, body, &listed) {
        Ok(Request::Decide(d)) => d,
        Ok(_) | Err(_) => return StatusCode::UNPROCESSABLE_ENTITY.into_response(),
    };
    let text = String::from_utf8_lossy(body);
    let refuse = text.contains("[mock:refuse]");
    let mut reply = DecideResponse::new(
        req.model.clone(),
        req.questions
            .iter()
            .map(|q| answer_to(&q.kind, refuse))
            .collect(),
    );
    reply.usage = usage(dialect, &req);
    let ctx = EncodeCtx::for_request(Arc::new(Request::Decide(req)));
    match dispatch::encode_response(dialect, &Reply::Decide(reply), &ctx) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-request-id", "req_mock_decision")
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(e) => panic!("mock: cannot write a {dialect} decision reply: {e}"),
    }
}

fn answer_to(kind: &QuestionKind, refuse: bool) -> Answer {
    if refuse {
        return Answer::new(AnswerValue::Refusal(Refusal::new()));
    }
    match kind {
        QuestionKind::Choice(c) => {
            let n = c.options.len().max(1);
            let rest = if n > 1 { 0.3 / (n - 1) as f64 } else { 0.0 };
            let probabilities = (0..n)
                .map(|i| Some(if i == 0 { 0.7 } else { rest }))
                .collect();
            Answer::new(AnswerValue::Choice(ChoiceAnswer::new(0, probabilities)))
        }
        QuestionKind::Score(s) => {
            let n = s.levels.len().max(1);
            let probabilities: Vec<f64> = (0..n)
                .map(|k| match n - k {
                    1 if n == 1 => 1.0,
                    1 => 0.6,
                    2 => 0.4,
                    _ => 0.0,
                })
                .collect();
            let score = probabilities
                .iter()
                .enumerate()
                .map(|(k, p)| k as f64 * p)
                .sum();
            Answer::new(AnswerValue::Score(ScoreAnswer::new(
                score,
                probabilities.into_iter().map(Some).collect(),
            )))
        }
        QuestionKind::Box(_) => Answer::new(AnswerValue::Box(BoxAnswer::new(0.1, 0.9, 0.2, 0.8))),
        _ => Answer::new(AnswerValue::Predicate(PredicateAnswer::new(0.82))),
    }
}

fn has_image(req: &DecideRequest) -> bool {
    let image = |b: &Block| matches!(b, Block::Image(_));
    !req.assets.is_empty()
        || matches!(&req.input, DecisionInput::Messages(m) if m.iter().flat_map(|m| &m.blocks).any(image))
        || req.questions.iter().any(
            |q| matches!(&q.instructions, Some(Instructions::Blocks(b)) if b.iter().any(image)),
        )
}

fn usage(dialect: DialectId, req: &DecideRequest) -> Usage {
    match dialect {
        DialectId::OpenAiDecisions => {
            let mut u = Usage::new(200, 0);
            u.cache_read_tokens = 1000;
            u
        }
        DialectId::GeminiDecisions => {
            let mut u = Usage::new(300, 0);
            if has_image(req) {
                u.input_image_tokens = Some(258);
            }
            u
        }
        _ => Usage::new(120, req.questions.len() as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_formats_are_told_by_path_and_perplexitys_by_its_body() {
        assert_eq!(
            dialect("/v1beta/decisions", b"{}"),
            Some(DialectId::GeminiDecisions)
        );
        assert_eq!(
            dialect("/api/v1/systemone", b"{}"),
            Some(DialectId::SystemOne)
        );
        assert_eq!(
            dialect("/v1/decisions", br#"{"model":"m","input":"x"}"#),
            Some(DialectId::OpenAiDecisions)
        );
        assert_eq!(
            dialect("/v1/decisions", br#"{"model":"m","state":"x"}"#),
            Some(DialectId::SystemOne)
        );
        assert_eq!(dialect("/v1/chat/completions", b"{}"), None);
    }
}
