use indexmap::IndexMap;
use kaeso_kpp::{
    Answer, AnswerValue, BoxAnswer, ChoiceAnswer, DecideRequest, DecideResponse, Description,
    Extras, KppError, Origin, PredicateAnswer, QuestionKind, RawJson, RawUsage, Refusal, Reply,
    Request, RequestKind, ScoreAnswer, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Number, Value};

use super::DIALECT;
use super::assets::{quoted, raw};
use super::profile::Envelope;
use super::wire::{
    BoxAnswerWire, BoxWire, ChoiceAnswerWire, NoulAnswerWire, RefusalAnswerWire, ResponseWire,
    ScoreAnswerWire, UsageWire,
};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::decisions::{
    SentKind, choice_spread_confidence, option_key, question_keys, score_spread_confidence,
};
use crate::wire::{
    Ext, Maybe, entries_of, extra_for, extras_from, field_of, parse_provider, to_body,
    verbatim_from_form,
};

const NO_USAGE: &str = "no_usage";
const INTEGERS: &str = "integers";
const LEGEND: &str = "legend";
const NULL_CONFIDENCE: &str = "null_confidence";

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {detail}.",
        DIALECT.display_name()
    ))
}

fn float(n: &Number) -> Result<f64, KppError> {
    n.as_f64().ok_or_else(|| bad("a number is out of range"))
}

struct Numbers(Vec<Value>);

impl Numbers {
    fn read(&mut self, path: &str, n: &Number) -> Result<f64, KppError> {
        if !n.is_f64() {
            self.0.push(Value::from(path));
        }
        float(n)
    }
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(status, headers, body));
    }
    if ctx.kind != RequestKind::Decide {
        return Err(KppError::unsupported_by(
            format!("{:?} response", ctx.kind),
            DIALECT,
            "",
        ));
    }
    let origin = ctx.origin(DIALECT);
    let mut envelope = IndexMap::new();
    let inner: &[u8] = match ctx.system_one_profile.envelope {
        Envelope::Result => {
            let whole: &RawValue =
                serde_json::from_slice(body).map_err(|_| bad("the body is not JSON"))?;
            let entries = entries_of(whole).ok_or_else(|| bad("the envelope is not an object"))?;
            for (k, v) in &entries {
                if k != "result" {
                    envelope.insert(format!("envelope.{k}"), RawJson::from(*v));
                }
            }
            field_of(whole, "result")
                .ok_or_else(|| bad("the envelope has no `result`"))?
                .get()
                .as_bytes()
        }
        _ => body,
    };
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ResponseWire>(inner, DIALECT)?;
    let sent = &ctx.decision_questions;
    if sent.is_empty() {
        return Err(KppError::provider(
            "Kaeso can't match the provider's answers to questions: the request it sent is unknown.",
        ));
    }
    let entries =
        entries_of(w.answers.as_raw()).ok_or_else(|| bad("`answers` is not an object"))?;
    if let Some((key, _)) = entries
        .iter()
        .find(|(k, _)| !sent.iter().any(|s| s.key == *k))
    {
        return Err(bad(&format!("it answers `{key}`, which Kaeso didn't ask")));
    }
    let mut answers = Vec::with_capacity(sent.len());
    for question in sent {
        let Some((_, value)) = entries.iter().find(|(k, _)| *k == question.key) else {
            return Err(bad(&format!("it has no answer to `{}`", question.key)));
        };
        answers.push(decode_answer(
            &question.key,
            &question.kind,
            value,
            &origin,
        )?);
    }
    let mut resp = DecideResponse::new(w.model, answers);
    if let Some(u) = &w.usage {
        let parsed: UsageWire = u
            .parse()
            .map_err(|_| bad("`usage.input_tokens` is missing"))?;
        let mut usage = Usage::new(parsed.input_tokens, parsed.output_tokens);
        usage.raw = u.to_value().ok().map(|v| RawUsage::new(origin.clone(), v));
        resp.usage = usage;
    }
    let mut extras = extras_from(&origin, extra, verbatim);
    if w.usage.is_none() && !extras.form.contains_key(crate::wire::VERBATIM_FORM_KEY) {
        extras.form.insert(NO_USAGE.to_owned(), true.into());
    }
    extras.fields.extend(envelope);
    resp.extras = extras;
    Ok(Reply::Decide(resp))
}

fn decode_answer(
    key: &str,
    kind: &SentKind,
    raw: &RawValue,
    origin: &Origin,
) -> Result<Answer, KppError> {
    let ty = crate::wire::type_tag(raw)
        .ok()
        .flatten()
        .unwrap_or_default();
    let wrong = |what: &str| {
        bad(&format!(
            "the answer to `{key}` is not a valid {what} answer"
        ))
    };
    let mut numbers = Numbers(Vec::new());
    let mut form = Extras::new(origin.clone());
    let (value, confidence, extra, verbatim) = match (ty.as_str(), kind) {
        ("noul", SentKind::Predicate) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<NoulAnswerWire>::from_raw(raw).map_err(|_| wrong("noul"))?;
            let p = numbers.read("noul", &w.noul)?;
            (
                AnswerValue::Predicate(PredicateAnswer::new(p)),
                None,
                extra,
                verbatim,
            )
        }
        ("choice", SentKind::Choice(options)) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<ChoiceAnswerWire>::from_raw(raw).map_err(|_| wrong("choice"))?;
            let option = options.iter().position(|o| *o == w.choice).ok_or_else(|| {
                bad(&format!(
                    "the answer to `{key}` chose `{}`, which isn't an option",
                    w.choice
                ))
            })?;
            let probabilities = record(
                &w.probabilities,
                key,
                options.len(),
                |k| options.iter().position(|o| o == k),
                &mut numbers,
            )?;
            let confidence = confidence_of(&w.confidence, &mut numbers, &mut form)?;
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
            let score = numbers.read("score", &w.score)?;
            let probabilities = record(
                &w.probabilities,
                key,
                *levels,
                |k| {
                    k.parse::<usize>()
                        .ok()
                        .filter(|i| *i < *levels && k == i.to_string())
                },
                &mut numbers,
            )?;
            let confidence = confidence_of(&w.confidence, &mut numbers, &mut form)?;
            if let Some(legend) = &w.legend {
                form.form.insert(
                    LEGEND.to_owned(),
                    legend.to_value().map_err(|_| wrong("score"))?,
                );
            }
            (
                AnswerValue::Score(ScoreAnswer::new(score, probabilities)),
                confidence,
                extra,
                verbatim,
            )
        }
        ("box", SentKind::Box) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<BoxAnswerWire>::from_raw(raw).map_err(|_| wrong("box"))?;
            let b = &w.coordinates;
            let value = BoxAnswer::new(
                numbers.read("box.top", &b.top)?,
                numbers.read("box.bottom", &b.bottom)?,
                numbers.read("box.left", &b.left)?,
                numbers.read("box.right", &b.right)?,
            );
            for (k, v) in &w.coordinates.extra {
                form.fields.insert(format!("box.{k}"), v.clone());
            }
            let confidence = confidence_of(&w.confidence, &mut numbers, &mut form)?;
            (AnswerValue::Box(value), confidence, extra, verbatim)
        }
        ("refusal", _) => {
            let Ext {
                extra, verbatim, ..
            } = Ext::<RefusalAnswerWire>::from_raw(raw).map_err(|_| wrong("refusal"))?;
            (AnswerValue::Refusal(Refusal::new()), None, extra, verbatim)
        }
        (other, _) => {
            return Err(bad(&format!(
                "the answer to `{key}` is a `{other}` answer to a question of another type"
            )));
        }
    };
    let mut extras = extras_from(origin, extra, verbatim);
    extras.form.extend(form.form);
    extras.fields.extend(form.fields);
    if !numbers.0.is_empty() {
        extras
            .form
            .insert(INTEGERS.to_owned(), Value::Array(numbers.0));
    }
    let mut answer = Answer::new(value).with_confidence(confidence);
    answer.extras = extras;
    Ok(answer)
}

fn confidence_of(
    c: &Maybe<Number>,
    numbers: &mut Numbers,
    form: &mut Extras,
) -> Result<Option<f64>, KppError> {
    match c {
        Maybe::Value(n) => numbers.read("confidence", n).map(Some),
        Maybe::Null => {
            form.form.insert(NULL_CONFIDENCE.to_owned(), true.into());
            Ok(None)
        }
        Maybe::Absent => Ok(None),
    }
}

fn record(
    raw: &RawJson,
    key: &str,
    len: usize,
    index: impl Fn(&str) -> Option<usize>,
    numbers: &mut Numbers,
) -> Result<Vec<Option<f64>>, KppError> {
    let entries = entries_of(raw.as_raw())
        .ok_or_else(|| bad(&format!("the probabilities of `{key}` are not an object")))?;
    let mut out = vec![None; len];
    for (k, v) in entries {
        let i = index(&k).ok_or_else(|| {
            bad(&format!(
                "the answer to `{key}` gives a probability for `{k}`, which Kaeso didn't ask"
            ))
        })?;
        let n: Number = serde_json::from_str(v.get()).map_err(|_| {
            bad(&format!(
                "the probability of `{k}` in `{key}` is not a number"
            ))
        })?;
        let p = numbers.read(&format!("probabilities.{k}"), &n)?;
        if let Some(slot) = out.get_mut(i) {
            *slot = Some(p);
        }
    }
    Ok(out)
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

fn usage_value(u: &Usage) -> Value {
    let mut v = serde_json::json!({
        "input_tokens": u.total_input_tokens(),
        "output_tokens": u.output_tokens,
    });
    if let (Some(Value::Object(raw)), Value::Object(map)) = (u.raw_for(DIALECT), &mut v) {
        for (k, x) in raw {
            if !matches!(k.as_str(), "input_tokens" | "output_tokens") {
                map.insert(k.clone(), x.clone());
            }
        }
    }
    v
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Decide(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "System One apps receive decision answers only.",
        ));
    };
    let Some(Request::Decide(req)) = ctx.request.as_deref() else {
        return Err(KppError::provider(
            "Kaeso needs the app's request to write a System One reply: its answers are keyed by question.",
        ));
    };
    if req.questions.len() != r.answers.len() {
        return Err(KppError::provider(format!(
            "The reply has {} answers to {} questions.",
            r.answers.len(),
            req.questions.len()
        )));
    }
    let (keys, _) = question_keys(&req.questions, |_| true);
    let mut answers: IndexMap<String, RawJson> = IndexMap::new();
    for (i, (answer, key)) in r.answers.iter().zip(keys).enumerate() {
        answers.insert(key, encode_answer(req, i, answer)?);
    }
    let ser = |e: serde_json::Error| {
        KppError::provider(format!("Kaeso could not write the answers ({e})."))
    };
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(RawJson::from_serialize(&usage_value(&r.usage)).map_err(ser)?)
    };
    let w = ResponseWire {
        model: r.model.clone(),
        answers: RawJson::from_serialize(&answers).map_err(ser)?,
        usage,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
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
    let confidence = |recomputed: Option<f64>| -> Result<Maybe<Number>, KppError> {
        let value = if same { answer.confidence } else { recomputed };
        match value {
            Some(c) => Ok(Maybe::Value(num(c, "confidence", &ints)?)),
            None if same && answer.extras.form_flag(DIALECT, NULL_CONFIDENCE) => Ok(Maybe::Null),
            None => Ok(Maybe::Absent),
        }
    };
    let ser = |e: serde_json::Error| {
        KppError::provider(format!("Kaeso could not write an answer ({e})."))
    };
    let json = match (&answer.value, &question.kind) {
        (AnswerValue::Predicate(p), QuestionKind::Predicate(_)) => {
            let w = NoulAnswerWire {
                ty: "noul".to_owned(),
                noul: num(p.probability, "noul", &ints)?,
            };
            RawJson::from_serialize(&Ext::with(w, extra, verbatim)).map_err(ser)?
        }
        (AnswerValue::Choice(c), QuestionKind::Choice(q)) => {
            let keys: Vec<String> = q
                .options
                .iter()
                .filter_map(|o| option_key(&o.value))
                .collect();
            let choice = keys.get(c.option).cloned().ok_or_else(mismatch)?;
            let mut probabilities: IndexMap<String, Number> = IndexMap::new();
            for (k, p) in keys.iter().zip(&c.probabilities) {
                if let Some(p) = p {
                    probabilities.insert(k.clone(), num(*p, &format!("probabilities.{k}"), &ints)?);
                }
            }
            let w = ChoiceAnswerWire {
                ty: "choice".to_owned(),
                choice,
                confidence: confidence(choice_spread_confidence(&c.probabilities))?,
                probabilities: RawJson::from_serialize(&probabilities).map_err(ser)?,
            };
            RawJson::from_serialize(&Ext::with(w, extra, verbatim)).map_err(ser)?
        }
        (AnswerValue::Score(s), QuestionKind::Score(q)) => {
            let legend = match answer.extras.form_value(DIALECT, LEGEND) {
                Some(v) if same => RawJson::from_serialize(v).map_err(ser)?,
                _ => {
                    let mut map: IndexMap<String, RawJson> = IndexMap::new();
                    for (n, level) in q.levels.iter().enumerate() {
                        let text = match (&level.description, &level.label) {
                            (Some(Description::Text(t)), _) => quoted(t),
                            (Some(Description::Json(j)), _) => j.get().to_owned(),
                            (None, Some(label)) => quoted(label),
                            _ => "null".to_owned(),
                        };
                        map.insert(n.to_string(), raw(text)?);
                    }
                    RawJson::from_serialize(&map).map_err(ser)?
                }
            };
            let mut probabilities: IndexMap<String, Number> = IndexMap::new();
            for (n, p) in s.probabilities.iter().enumerate() {
                if let Some(p) = p {
                    probabilities.insert(
                        n.to_string(),
                        num(*p, &format!("probabilities.{n}"), &ints)?,
                    );
                }
            }
            let w = ScoreAnswerWire {
                ty: "score".to_owned(),
                score: num(s.score, "score", &ints)?,
                confidence: confidence(score_spread_confidence(&s.probabilities))?,
                legend: Some(legend),
                probabilities: RawJson::from_serialize(&probabilities).map_err(ser)?,
            };
            RawJson::from_serialize(&Ext::with(w, extra, verbatim)).map_err(ser)?
        }
        (AnswerValue::Box(b), QuestionKind::Box(_)) => {
            let box_extra = if same {
                crate::wire::nested_extra_for(&answer.extras, DIALECT, "box")
            } else {
                IndexMap::new()
            };
            let coordinates = BoxWire {
                top: num(b.top, "box.top", &ints)?,
                bottom: num(b.bottom, "box.bottom", &ints)?,
                left: num(b.left, "box.left", &ints)?,
                right: num(b.right, "box.right", &ints)?,
            };
            let conf = match confidence(answer.confidence)? {
                Maybe::Absent => Maybe::Null,
                other => other,
            };
            let w = BoxAnswerWire {
                ty: "box".to_owned(),
                coordinates: Ext::with(coordinates, box_extra, IndexMap::new()),
                confidence: conf,
            };
            RawJson::from_serialize(&Ext::with(w, extra, verbatim)).map_err(ser)?
        }
        (AnswerValue::Refusal(_), _) => {
            let w = RefusalAnswerWire {
                ty: "refusal".to_owned(),
            };
            RawJson::from_serialize(&Ext::with(w, extra, verbatim)).map_err(ser)?
        }
        _ => return Err(mismatch()),
    };
    Ok(json)
}
