use kaeso_kpp::{
    Answer, AnswerValue, ChoiceAnswer, DecideRequest, DecideResponse, Description, Extras,
    KppError, Origin, PredicateAnswer, QuestionKind, RawJson, RawUsage, Refusal, Reply, Request,
    RequestKind, ScoreAnswer, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Number, Value, json};

use super::DIALECT;
use super::parts::ser;
use super::request::value_key;
use super::wire::{
    ChoiceAnswerWire, ChoiceProbabilityWire, PredicateAnswerWire, RefusalAnswerWire, ResponseWire,
    ScoreAnswerWire, ScoreProbabilityWire, UsageWire,
};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::decisions::{SentKind, choice_spread_confidence, score_spread_confidence};
use crate::wire::{
    Ext, Maybe, extra_for, extras_from, parse_provider, to_body, type_tag, verbatim_from_form,
};

const NO_USAGE: &str = "no_usage";
const INTEGERS: &str = "integers";
/// The labels of a score answer's probabilities as the provider wrote them.
const LABELS: &str = "labels";
/// The order of an answer's probabilities where it isn't the order of the options or levels.
const ORDER: &str = "order";
/// An answer's `name` where it isn't the question's.
const NAME: &str = "name";
/// The fields of each probability entry the format doesn't document, by entry.
const ENTRY_FIELDS: &str = "entry_fields";

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {detail}.",
        DIALECT.display_name()
    ))
}

struct Numbers(Vec<Value>);

impl Numbers {
    fn read(&mut self, path: &str, n: &Number) -> Result<f64, KppError> {
        if !n.is_f64() {
            self.0.push(Value::from(path));
        }
        n.as_f64().ok_or_else(|| bad("a number is out of range"))
    }
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(crate::openai_responses::error::decode(
            DIALECT, status, headers, body,
        ));
    }
    if ctx.kind != RequestKind::Decide {
        return Err(KppError::unsupported_by(
            format!("{:?} response", ctx.kind),
            DIALECT,
            "",
        ));
    }
    let origin = ctx.origin(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ResponseWire>(body, DIALECT)?;
    let sent = &ctx.decision_questions;
    if sent.is_empty() {
        return Err(KppError::provider(
            "Kaeso can't match the provider's answers to questions: the request it sent is unknown.",
        ));
    }
    let items: Vec<&RawValue> = w
        .answers
        .parse()
        .map_err(|_| bad("`answers` is not a list"))?;
    if items.len() != sent.len() {
        return Err(bad(&format!(
            "it has {} answers to {} questions",
            items.len(),
            sent.len()
        )));
    }
    let mut answers = Vec::with_capacity(sent.len());
    for (i, (question, raw)) in sent.iter().zip(items).enumerate() {
        answers.push(decode_answer(
            i,
            &question.key,
            &question.kind,
            raw,
            &origin,
        )?);
    }
    let mut resp = DecideResponse::new(w.model, answers);
    let mut extras = extras_from(&origin, extra, verbatim);
    match &w.usage {
        Some(u) => {
            let parsed: UsageWire = u.parse().map_err(|_| bad("`usage` is not valid"))?;
            let cached = parsed
                .input_tokens_details
                .as_ref()
                .map_or(0, |d| d.cached_tokens);
            let written = parsed
                .input_tokens_details
                .as_ref()
                .map_or(0, |d| d.cache_write_tokens);
            // OpenAI counts cache reads and writes in `input_tokens`; KPP counts them apart.
            let mut usage = Usage::new(
                parsed
                    .input_tokens
                    .saturating_sub(cached)
                    .saturating_sub(written),
                parsed.output_tokens,
            );
            usage.cache_read_tokens = cached;
            usage.cache_write_tokens = written;
            usage.reasoning_tokens = parsed.output_tokens_details.map(|d| d.reasoning_tokens);
            usage.raw = u.to_value().ok().map(|v| RawUsage::new(origin.clone(), v));
            resp.usage = usage;
        }
        None => {
            if !extras.form.contains_key(crate::wire::VERBATIM_FORM_KEY) {
                extras.form.insert(NO_USAGE.to_owned(), true.into());
            }
        }
    }
    resp.extras = extras;
    Ok(Reply::Decide(resp))
}

fn name_form(name: &Maybe<String>, key: &str, form: &mut Extras) {
    let matches = match name {
        Maybe::Value(n) => n == key,
        Maybe::Null => key.is_empty(),
        Maybe::Absent => false,
    };
    if !matches {
        let value = match name {
            Maybe::Value(n) => Value::from(n.clone()),
            Maybe::Null => Value::Null,
            Maybe::Absent => json!({"absent": true}),
        };
        form.form.insert(NAME.to_owned(), value);
    }
}

fn value_text(raw: &RawJson) -> Option<String> {
    match raw.to_value().ok()? {
        Value::String(s) => serde_json::to_string(&s).ok(),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn order_form(order: Vec<usize>, form: &mut Extras) {
    if order.windows(2).any(|w| w.first() >= w.get(1)) {
        form.form.insert(
            ORDER.to_owned(),
            Value::Array(order.into_iter().map(Value::from).collect()),
        );
    }
}

fn decode_answer(
    i: usize,
    key: &str,
    kind: &SentKind,
    raw: &RawValue,
    origin: &Origin,
) -> Result<Answer, KppError> {
    let ty = type_tag(raw).ok().flatten().unwrap_or_default();
    let wrong = |what: &str| bad(&format!("answer {i} is not a valid {what} answer"));
    let mut numbers = Numbers(Vec::new());
    let mut form = Extras::new(origin.clone());
    let mut entry_fields: Vec<Value> = Vec::new();
    let (value, confidence, extra, verbatim) = match (ty.as_str(), kind) {
        ("predicate", SentKind::Predicate) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<PredicateAnswerWire>::from_raw(raw).map_err(|_| wrong("predicate"))?;
            name_form(&w.name, key, &mut form);
            let p = numbers.read("probability", &w.probability)?;
            (
                AnswerValue::Predicate(PredicateAnswer::new(p)),
                None,
                extra,
                verbatim,
            )
        }
        ("choice", SentKind::Choice(values)) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<ChoiceAnswerWire>::from_raw(raw).map_err(|_| wrong("choice"))?;
            name_form(&w.name, key, &mut form);
            let index =
                |v: &RawJson| value_text(v).and_then(|t| values.iter().position(|o| *o == t));
            let option = index(&w.choice).ok_or_else(|| {
                bad(&format!(
                    "answer {i} chose {}, which isn't an option",
                    w.choice.get()
                ))
            })?;
            let mut probabilities = vec![None; values.len()];
            let mut order = Vec::new();
            for (n, entry) in w.probabilities.iter().enumerate() {
                let at = index(&entry.value).ok_or_else(|| {
                    bad(&format!(
                        "answer {i} gives a probability for {}, which isn't an option",
                        entry.value.get()
                    ))
                })?;
                let slot = probabilities.get_mut(at).ok_or_else(|| wrong("choice"))?;
                if slot.is_some() {
                    return Err(bad(&format!(
                        "answer {i} gives an option two probabilities"
                    )));
                }
                *slot = Some(numbers.read(&format!("probabilities.{n}"), &entry.probability)?);
                order.push(at);
                entry_fields.push(entry_extra(entry));
            }
            order_form(order, &mut form);
            let confidence = w
                .confidence
                .as_ref()
                .map(|c| numbers.read("confidence", c))
                .transpose()?;
            (
                AnswerValue::Choice(ChoiceAnswer::new(option, probabilities)),
                confidence,
                extra,
                verbatim,
            )
        }
        ("score", SentKind::Score(levels)) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<ScoreAnswerWire>::from_raw(raw).map_err(|_| wrong("score"))?;
            name_form(&w.name, key, &mut form);
            let score = numbers.read("score", &w.score)?;
            let mut probabilities = vec![None; *levels];
            let mut order = Vec::new();
            let mut labels = Vec::new();
            for (n, entry) in w.probabilities.iter().enumerate() {
                let at = usize::try_from(entry.value)
                    .ok()
                    .filter(|v| v < levels)
                    .ok_or_else(|| {
                        bad(&format!(
                            "answer {i} gives a probability for level {}, which wasn't asked",
                            entry.value
                        ))
                    })?;
                let slot = probabilities.get_mut(at).ok_or_else(|| wrong("score"))?;
                if slot.is_some() {
                    return Err(bad(&format!("answer {i} gives a level two probabilities")));
                }
                *slot = Some(numbers.read(&format!("probabilities.{n}"), &entry.probability)?);
                order.push(at);
                labels.push(Value::from(entry.label.clone()));
                entry_fields.push(entry_extra(entry));
            }
            order_form(order, &mut form);
            form.form.insert(LABELS.to_owned(), Value::Array(labels));
            let confidence = w
                .confidence
                .as_ref()
                .map(|c| numbers.read("confidence", c))
                .transpose()?;
            (
                AnswerValue::Score(ScoreAnswer::new(score, probabilities)),
                confidence,
                extra,
                verbatim,
            )
        }
        ("refusal", _) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<RefusalAnswerWire>::from_raw(raw).map_err(|_| wrong("refusal"))?;
            name_form(&w.name, key, &mut form);
            (AnswerValue::Refusal(Refusal::new()), None, extra, verbatim)
        }
        (other, _) => {
            return Err(bad(&format!(
                "answer {i} is a `{other}` answer to a question of another type"
            )));
        }
    };
    let mut extras = extras_from(origin, extra, verbatim);
    extras.form.extend(form.form);
    if entry_fields
        .iter()
        .any(|v| v.as_object().is_some_and(|m| !m.is_empty()))
    {
        extras
            .form
            .insert(ENTRY_FIELDS.to_owned(), Value::Array(entry_fields));
    }
    if !numbers.0.is_empty() {
        extras
            .form
            .insert(INTEGERS.to_owned(), Value::Array(numbers.0));
    }
    let mut answer = Answer::new(value).with_confidence(confidence);
    answer.extras = extras;
    Ok(answer)
}

fn entry_extra<T>(entry: &Ext<T>) -> Value {
    Value::Object(
        entry
            .extra
            .iter()
            .chain(&entry.verbatim)
            .filter_map(|(k, v)| Some((k.clone(), v.to_value().ok()?)))
            .collect(),
    )
}

fn integers(extras: &Extras) -> Vec<String> {
    match extras.form_value(DIALECT, INTEGERS) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn num(value: f64, path: &str, ints: &[String]) -> Result<Number, KppError> {
    let whole = value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0;
    if whole && ints.iter().any(|p| p == path) {
        return Ok(Number::from(value as i64));
    }
    Number::from_f64(value).ok_or_else(|| {
        KppError::provider(format!(
            "The answer holds `{value}`, which JSON can't write."
        ))
    })
}

fn merge(into: &mut Value, from: &Value) {
    if let (Value::Object(into), Value::Object(from)) = (into, from) {
        for (k, v) in from {
            match into.get_mut(k) {
                Some(slot @ Value::Object(_)) => merge(slot, v),
                Some(_) => {}
                None => {
                    into.insert(k.clone(), v.clone());
                }
            }
        }
    }
}

fn usage_value(u: &Usage) -> Value {
    let raw = u.raw_for(DIALECT);
    let total = raw
        .and_then(|r| r.get("total_tokens"))
        .cloned()
        .unwrap_or_else(|| Value::from(u.total_tokens()));
    let mut v = json!({
        "input_tokens": u.total_input_tokens(),
        "input_tokens_details": {
            "cached_tokens": u.cache_read_tokens,
            "cache_write_tokens": u.cache_write_tokens,
        },
        "output_tokens": u.output_tokens,
        "output_tokens_details": {"reasoning_tokens": u.reasoning_tokens.unwrap_or(0)},
        "total_tokens": total,
    });
    if let Some(raw) = raw {
        merge(&mut v, raw);
    }
    v
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Decide(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "OpenAI Decisions apps receive decision answers only.",
        ));
    };
    let Some(Request::Decide(req)) = ctx.request.as_deref() else {
        return Err(KppError::provider(
            "Kaeso needs the app's request to write an OpenAI Decisions reply: its answers name the questions and options.",
        ));
    };
    if req.questions.len() != r.answers.len() {
        return Err(KppError::provider(format!(
            "The reply has {} answers to {} questions.",
            r.answers.len(),
            req.questions.len()
        )));
    }
    let mut answers = Vec::with_capacity(r.answers.len());
    for (i, answer) in r.answers.iter().enumerate() {
        answers.push(encode_answer(req, i, answer)?);
    }
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(ser(&usage_value(&r.usage))?)
    };
    let w = ResponseWire {
        model: r.model.clone(),
        answers: ser(&answers)?,
        usage,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}

/// The label a level's probability is written with: its label, else its description.
fn label_of(level: &kaeso_kpp::Level) -> String {
    match (&level.label, &level.description) {
        (Some(label), _) => label.clone(),
        (None, Some(Description::Text(t))) => t.clone(),
        (None, Some(Description::Json(j))) => j.get().to_owned(),
        _ => String::new(),
    }
}

fn order_of(extras: &Extras, same: bool, probabilities: &[Option<f64>]) -> Vec<usize> {
    let given: Option<Vec<usize>> = match extras.form_value(DIALECT, ORDER) {
        Some(Value::Array(items)) if same => items
            .iter()
            .map(|v| v.as_u64().and_then(|n| usize::try_from(n).ok()))
            .collect(),
        _ => None,
    };
    given.unwrap_or_else(|| {
        probabilities
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_some())
            .map(|(i, _)| i)
            .collect()
    })
}

fn entry_fields_of(extras: &Extras, same: bool, n: usize) -> indexmap::IndexMap<String, RawJson> {
    let entry = match extras.form_value(DIALECT, ENTRY_FIELDS) {
        Some(Value::Array(items)) if same => items.get(n).cloned(),
        _ => None,
    };
    match entry {
        Some(Value::Object(map)) => map
            .into_iter()
            .filter_map(|(k, v)| Some((k, RawJson::from_serialize(&v).ok()?)))
            .collect(),
        _ => indexmap::IndexMap::new(),
    }
}

fn encode_answer(req: &DecideRequest, i: usize, answer: &Answer) -> Result<RawJson, KppError> {
    let question = req
        .questions
        .get(i)
        .ok_or_else(|| KppError::provider("An answer has no question."))?;
    let same = answer.extras.is_from(DIALECT);
    let ints = if same {
        integers(&answer.extras)
    } else {
        Vec::new()
    };
    let name = match answer.extras.form_value(DIALECT, NAME) {
        Some(Value::Object(_)) if same => Maybe::Absent,
        Some(Value::Null) if same => Maybe::Null,
        Some(Value::String(s)) if same => Maybe::Value(s.clone()),
        _ => Maybe::from_option(question.name.clone(), Maybe::Null),
    };
    let extra = extra_for(&answer.extras, DIALECT);
    let verbatim = verbatim_from_form(&answer.extras, DIALECT, "");
    let mismatch = || {
        KppError::provider(format!(
            "The provider answered the {} question {} with a {} answer.",
            question.type_name(),
            i,
            answer.value.as_str()
        ))
    };
    let confidence = |recomputed: Option<f64>| -> Result<Option<Number>, KppError> {
        let value = if same { answer.confidence } else { recomputed };
        value.map(|c| num(c, "confidence", &ints)).transpose()
    };
    let json = match (&answer.value, &question.kind) {
        (AnswerValue::Predicate(p), QuestionKind::Predicate(_)) => {
            let w = PredicateAnswerWire {
                ty: "predicate".to_owned(),
                name,
                probability: num(p.probability, "probability", &ints)?,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        (AnswerValue::Choice(c), QuestionKind::Choice(q)) => {
            let value = |at: usize| -> Result<RawJson, KppError> {
                let option = q.options.get(at).ok_or_else(mismatch)?;
                let text = value_key(&option.value).ok_or_else(mismatch)?;
                RawJson::from_string(text).map_err(|e| KppError::provider(e.to_string()))
            };
            let mut probabilities = Vec::new();
            for (n, at) in order_of(&answer.extras, same, &c.probabilities)
                .into_iter()
                .enumerate()
            {
                let Some(Some(p)) = c.probabilities.get(at) else {
                    continue;
                };
                let w = ChoiceProbabilityWire {
                    value: value(at)?,
                    probability: num(*p, &format!("probabilities.{n}"), &ints)?,
                };
                probabilities.push(Ext::with(
                    w,
                    entry_fields_of(&answer.extras, same, n),
                    indexmap::IndexMap::new(),
                ));
            }
            let w = ChoiceAnswerWire {
                ty: "choice".to_owned(),
                name,
                choice: value(c.option)?,
                probabilities,
                confidence: confidence(choice_spread_confidence(&c.probabilities))?,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        (AnswerValue::Score(s), QuestionKind::Score(q)) => {
            let labels: Option<Vec<String>> = match answer.extras.form_value(DIALECT, LABELS) {
                Some(Value::Array(items)) if same => items
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned))
                    .collect(),
                _ => None,
            };
            let mut probabilities = Vec::new();
            for (n, at) in order_of(&answer.extras, same, &s.probabilities)
                .into_iter()
                .enumerate()
            {
                let Some(Some(p)) = s.probabilities.get(at) else {
                    continue;
                };
                let label = match labels.as_ref().and_then(|l| l.get(n)) {
                    Some(l) => l.clone(),
                    None => q.levels.get(at).map(label_of).ok_or_else(mismatch)?,
                };
                let w = ScoreProbabilityWire {
                    value: at as u64,
                    label,
                    probability: num(*p, &format!("probabilities.{n}"), &ints)?,
                };
                probabilities.push(Ext::with(
                    w,
                    entry_fields_of(&answer.extras, same, n),
                    indexmap::IndexMap::new(),
                ));
            }
            let w = ScoreAnswerWire {
                ty: "score".to_owned(),
                name,
                score: num(s.score, "score", &ints)?,
                probabilities,
                confidence: confidence(score_spread_confidence(&s.probabilities))?,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        (AnswerValue::Refusal(_), _) => {
            let w = RefusalAnswerWire {
                ty: "refusal".to_owned(),
                name,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        _ => return Err(mismatch()),
    };
    Ok(json)
}
