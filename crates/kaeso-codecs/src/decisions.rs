//! What every decision codec shares: the refusal other codecs give a decision request, the
//! question keys a keyed format is sent, and the documented `confidence` rules that a reply
//! crossing formats is recomputed with.

use kaeso_kpp::{
    ChoiceValue, DialectId, KppError, Question, QuestionKind, Reply, Request, RequestKind,
};

/// The hint every codec that isn't a decision codec gives a decision request.
pub(crate) const DECISIONS_HINT: &str =
    "Send decision requests to `/v1/decisions`, `/v1/systemone` or `/v1beta/decisions`.";

fn not_decisions(dialect: DialectId) -> KppError {
    KppError::unsupported_by("decision request", dialect, DECISIONS_HINT)
}

/// Refuses a decision request in a codec of another kind.
pub(crate) fn refuse_kind(kind: RequestKind, dialect: DialectId) -> Result<(), KppError> {
    if kind == RequestKind::Decide {
        return Err(not_decisions(dialect));
    }
    Ok(())
}

pub(crate) fn refuse_request(req: &Request, dialect: DialectId) -> Result<(), KppError> {
    refuse_kind(req.kind(), dialect)
}

pub(crate) fn refuse_reply(reply: &Reply, dialect: DialectId) -> Result<(), KppError> {
    refuse_kind(reply.kind(), dialect)
}

/// The type of question a keyed answer was asked as, and what it was asked to choose from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SentQuestion {
    /// The key the question was sent under.
    pub(crate) key: String,
    pub(crate) kind: SentKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SentKind {
    Predicate,
    /// The option keys, in option order.
    Choice(Vec<String>),
    /// The number of levels.
    Score(usize),
    Box,
}

/// The key each question is sent under to a format that keys questions by name: the names when
/// every question has one, used once and valid for the target, `q0`…`qN-1` for every question
/// otherwise, so that no renamed key can collide with a kept one. The flag says whether they were
/// renamed.
pub(crate) fn question_keys(
    questions: &[Question],
    valid: impl Fn(&str) -> bool,
) -> (Vec<String>, bool) {
    let mut seen = std::collections::HashSet::new();
    let kept: Option<Vec<String>> = questions
        .iter()
        .map(|q| {
            q.name
                .as_deref()
                .filter(|n| valid(n) && seen.insert(*n))
                .map(str::to_owned)
        })
        .collect();
    match kept {
        Some(keys) => (keys, false),
        None => (
            (0..questions.len()).map(|i| format!("q{i}")).collect(),
            true,
        ),
    }
}

/// The key a choice option is sent under to a format whose options are object keys: its text, or
/// `"true"` and `"false"` for OpenAI's boolean values.
pub(crate) fn option_key(value: &ChoiceValue) -> Option<String> {
    match value {
        ChoiceValue::Text(t) => Some(t.clone()),
        ChoiceValue::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(crate) fn sent_kind(question: &Question, options: Vec<String>) -> SentKind {
    match &question.kind {
        QuestionKind::Choice(_) => SentKind::Choice(options),
        QuestionKind::Score(s) => SentKind::Score(s.levels.len()),
        QuestionKind::Box(_) => SentKind::Box,
        _ => SentKind::Predicate,
    }
}

/// TypeSafe's (and Perplexity's) confidence of a choice: how far the top probability is above an
/// even split, `(p_max − 1/n) / (1 − 1/n)`, and 1 for a single option
/// (<https://docs.typesafe.ai/confidence#choice>). Missing probabilities count as 0.
pub(crate) fn choice_spread_confidence(probabilities: &[Option<f64>]) -> Option<f64> {
    let n = probabilities.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(1.0);
    }
    let top = top_probability(probabilities)?;
    let even = 1.0 / n as f64;
    Some(((top - even) / (1.0 - even)).clamp(0.0, 1.0))
}

/// TypeSafe's confidence of a score: one minus the probability-weighted distance
/// from the most likely level over that of an even spread, floored at 0, and 1 for a single level
/// (<https://docs.typesafe.ai/confidence#score>). Missing probabilities count as 0; the first of
/// equally likely levels is the most likely.
pub(crate) fn score_spread_confidence(probabilities: &[Option<f64>]) -> Option<f64> {
    let n = probabilities.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(1.0);
    }
    let p = |i: usize| probabilities.get(i).copied().flatten().unwrap_or(0.0);
    let m = (0..n).fold(0, |best, i| if p(i) > p(best) { i } else { best });
    let spread: f64 = (0..n).map(|i| p(i) * (i as f64 - m as f64).abs()).sum();
    let middle = (n as f64 - 1.0) / 2.0;
    let uniform = (0..n).map(|i| (i as f64 - middle).abs()).sum::<f64>() / n as f64;
    Some((1.0 - spread / uniform).max(0.0))
}

/// The top probability, Gemini's confidence of a choice or score.
pub(crate) fn top_probability(probabilities: &[Option<f64>]) -> Option<f64> {
    probabilities.iter().flatten().copied().reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{Choice, ChoiceOption, Predicate, RawJson};

    fn close(a: Option<f64>, b: f64) {
        let a = a.unwrap();
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn choice_confidence_follows_the_documented_examples() {
        close(choice_spread_confidence(&[Some(0.75), Some(0.25)]), 0.5);
        close(
            choice_spread_confidence(&[Some(0.6), Some(0.3), Some(0.1)]),
            0.4,
        );
        close(
            choice_spread_confidence(&[Some(0.6), Some(0.2), Some(0.2)]),
            0.4,
        );
        close(choice_spread_confidence(&[Some(1.0)]), 1.0);
        close(choice_spread_confidence(&[Some(0.9), None]), 0.8);
        assert_eq!(choice_spread_confidence(&[]), None);
        assert_eq!(choice_spread_confidence(&[None, None]), None);
    }

    #[test]
    fn score_confidence_counts_the_distance_between_levels() {
        close(
            score_spread_confidence(&[Some(0.0), Some(0.5), Some(0.5)]),
            0.25,
        );
        close(
            score_spread_confidence(&[Some(0.5), Some(0.0), Some(0.5)]),
            0.0,
        );
        // Perplexity measures the spread against the distances from the top level instead, and
        // reports 0.78 for this answer; a System One reply keeps the provider's own value.
        close(
            score_spread_confidence(&[
                Some(0.008423954913615923),
                Some(0.199283458194343),
                Some(0.7922925868920411),
            ]),
            0.6758029479676377,
        );
        close(score_spread_confidence(&[Some(1.0)]), 1.0);
        close(score_spread_confidence(&[Some(1.0), Some(0.0)]), 1.0);
        assert_eq!(score_spread_confidence(&[]), None);
    }

    #[test]
    fn gemini_confidence_is_the_top_probability() {
        close(top_probability(&[Some(0.5), Some(0.3), Some(0.2)]), 0.5);
        assert_eq!(top_probability(&[None]), None);
    }

    #[test]
    fn names_are_kept_only_when_every_question_has_a_valid_unique_one() {
        let q = |name: Option<&str>| {
            kaeso_kpp::Question::new(
                name.map(str::to_owned),
                QuestionKind::Predicate(Predicate::default()),
            )
        };
        let any = |_: &str| true;
        assert_eq!(
            question_keys(&[q(Some("a")), q(Some("b"))], any),
            (vec!["a".to_owned(), "b".to_owned()], false)
        );
        for questions in [
            vec![q(Some("a")), q(None)],
            vec![q(Some("a")), q(Some("a"))],
        ] {
            assert_eq!(
                question_keys(&questions, any),
                (vec!["q0".to_owned(), "q1".to_owned()], true)
            );
        }
        let short = |n: &str| n.len() < 3;
        assert!(question_keys(&[q(Some("ok")), q(Some("too long"))], short).1);
        let choice = kaeso_kpp::Question::new(
            None,
            QuestionKind::Choice(Choice::new(vec![ChoiceOption::new(
                ChoiceValue::Bool(true),
                None,
            )])),
        );
        assert_eq!(
            sent_kind(&choice, vec!["true".to_owned()]),
            SentKind::Choice(vec!["true".to_owned()])
        );
        assert_eq!(
            option_key(&ChoiceValue::Bool(false)).as_deref(),
            Some("false")
        );
        let _ = RawJson::null();
    }
}
