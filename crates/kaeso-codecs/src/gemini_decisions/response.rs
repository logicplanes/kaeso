use indexmap::IndexMap;
use kaeso_kpp::{
    Answer, AnswerValue, ChoiceAnswer, DecideRequest, DecideResponse, Description, ErrorKind,
    Extras, KppError, Origin, PredicateAnswer, QuestionKind, RawJson, RawUsage, Reply, Request,
    RequestKind, ScoreAnswer, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Number, Value, json};

use super::DIALECT;
use super::blocks::ser;
use super::wire::{
    ChoiceAnswerWire, NoulAnswerWire, PromptFeedbackWire, ResponseWire, ScoreAnswerWire, UsageWire,
};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::decisions::{SentKind, option_key, question_keys, top_probability};
use crate::wire::{
    Ext, entries_of, extra_for, extras_from, parse_provider, to_body, type_tag, verbatim_from_form,
};

const NO_USAGE: &str = "no_usage";
const INTEGERS: &str = "integers";
const LEGEND: &str = "legend";
/// Marks a score answer the provider wrote without `legend`.
const NO_LEGEND: &str = "no_legend";
const PROMPT_FEEDBACK: &str = "prompt_feedback";

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

/// The error a reply whose input the safety filters blocked becomes: no question was answered,
/// so every app gets `ContentFiltered` in its own format (D10).
fn blocked(feedback: &PromptFeedbackWire, body: &[u8]) -> KppError {
    let reason = feedback.block_reason.as_deref().unwrap_or_default();
    let message = match &feedback.block_reason_message {
        Some(m) if !m.is_empty() => m.clone(),
        _ => format!(
            "Gemini blocked the input (`{reason}`), so no question was answered; change the input and try again."
        ),
    };
    let mut e = KppError::new(ErrorKind::ContentFiltered, message)
        .with_status(400)
        .with_origin(DIALECT);
    if let Ok(v) = serde_json::from_slice::<Value>(body) {
        e = e.with_raw(v);
    }
    e
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
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ResponseWire>(body, DIALECT)?;
    if let Some(feedback) = &w.prompt_feedback {
        let parsed: PromptFeedbackWire = feedback
            .parse()
            .map_err(|_| bad("`prompt_feedback` is not valid"))?;
        if parsed.block_reason.is_some() {
            return Err(blocked(&parsed, body));
        }
    }
    let sent = &ctx.decision_questions;
    if sent.is_empty() {
        return Err(KppError::provider(
            "Kaeso can't match the provider's answers to questions: the request it sent is unknown.",
        ));
    }
    let answers = w
        .answers
        .as_ref()
        .ok_or_else(|| bad("it has no `answers`"))?;
    let entries = entries_of(answers.as_raw()).ok_or_else(|| bad("`answers` is not an object"))?;
    if let Some((key, _)) = entries
        .iter()
        .find(|(k, _)| !sent.iter().any(|s| s.key == *k))
    {
        return Err(bad(&format!("it answers `{key}`, which Kaeso didn't ask")));
    }
    let mut out = Vec::with_capacity(sent.len());
    for question in sent {
        let Some((_, value)) = entries.iter().find(|(k, _)| *k == question.key) else {
            return Err(bad(&format!("it has no answer to `{}`", question.key)));
        };
        out.push(decode_answer(
            &question.key,
            &question.kind,
            value,
            &origin,
        )?);
    }
    let model = ctx.decision_model.clone().unwrap_or_default();
    let mut resp = DecideResponse::new(model, out);
    let mut extras = extras_from(&origin, extra, verbatim);
    match &w.usage {
        Some(u) => resp.usage = usage_of(u, &origin)?,
        None => {
            if !extras.form.contains_key(crate::wire::VERBATIM_FORM_KEY) {
                extras.form.insert(NO_USAGE.to_owned(), true.into());
            }
        }
    }
    if let Some(feedback) = w.prompt_feedback {
        extras.fields.insert(PROMPT_FEEDBACK.to_owned(), feedback);
    }
    resp.extras = extras;
    Ok(Reply::Decide(resp))
}

// Video frames and PDF pages count as image tokens (Decisions guide, "Request and response
// reference"); the rest of the input is text.
fn usage_of(raw: &RawJson, origin: &Origin) -> Result<Usage, KppError> {
    let u: UsageWire = raw.parse().map_err(|_| bad("`usage` is not valid"))?;
    let mut image = None::<u64>;
    let mut audio = None::<u64>;
    for m in &u.input_tokens_by_modality {
        let slot = match m.modality.as_str() {
            "image" | "video" | "document" => &mut image,
            "audio" => &mut audio,
            _ => continue,
        };
        *slot = Some(slot.unwrap_or(0).saturating_add(m.tokens));
    }
    let mut usage = Usage::new(
        u.total_input_tokens.saturating_sub(u.total_cached_tokens),
        u.total_output_tokens,
    );
    usage.cache_read_tokens = u.total_cached_tokens;
    usage.input_image_tokens = image;
    usage.input_audio_tokens = audio;
    usage.reasoning_tokens = u.total_thought_tokens;
    usage.raw = raw
        .to_value()
        .ok()
        .map(|v| RawUsage::new(origin.clone(), v));
    Ok(usage)
}

fn decode_answer(
    key: &str,
    kind: &SentKind,
    raw: &RawValue,
    origin: &Origin,
) -> Result<Answer, KppError> {
    let ty = type_tag(raw).ok().flatten().unwrap_or_default();
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
            let c = w
                .confidence
                .as_ref()
                .map(|c| numbers.read("confidence", c))
                .transpose()?;
            (
                AnswerValue::Predicate(PredicateAnswer::new(p)),
                c,
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
            let c = w
                .confidence
                .as_ref()
                .map(|c| numbers.read("confidence", c))
                .transpose()?;
            (
                AnswerValue::Choice(ChoiceAnswer::new(option, probabilities)),
                c,
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
            let c = w
                .confidence
                .as_ref()
                .map(|c| numbers.read("confidence", c))
                .transpose()?;
            match &w.legend {
                Some(legend) => {
                    form.form.insert(
                        LEGEND.to_owned(),
                        legend.to_value().map_err(|_| wrong("score"))?,
                    );
                }
                None => {
                    form.form.insert(NO_LEGEND.to_owned(), true.into());
                }
            }
            (
                AnswerValue::Score(ScoreAnswer::new(score, probabilities)),
                c,
                extra,
                verbatim,
            )
        }
        (other, _) => {
            return Err(bad(&format!(
                "the answer to `{key}` is a `{other}` answer to a question of another type"
            )));
        }
    };
    let mut extras = extras_from(origin, extra, verbatim);
    extras.form.extend(form.form);
    if !numbers.0.is_empty() {
        extras
            .form
            .insert(INTEGERS.to_owned(), Value::Array(numbers.0));
    }
    let mut answer = Answer::new(value).with_confidence(confidence);
    answer.extras = extras;
    Ok(answer)
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

// A reply of another format gets the usage the Interactions `Usage` writes: the input by
// modality, with text the rest of what isn't image or audio.
fn usage_value(u: &Usage) -> Value {
    if let Some(raw) = u.raw_for(DIALECT) {
        return raw.clone();
    }
    let total = u.total_input_tokens();
    let image = u.input_image_tokens.unwrap_or(0);
    let audio = u.input_audio_tokens.unwrap_or(0);
    let mut modalities = vec![json!({
        "modality": "text",
        "tokens": total.saturating_sub(image).saturating_sub(audio),
    })];
    if image > 0 {
        modalities.push(json!({"modality": "image", "tokens": image}));
    }
    if audio > 0 {
        modalities.push(json!({"modality": "audio", "tokens": audio}));
    }
    let mut v = json!({
        "total_input_tokens": total,
        "input_tokens_by_modality": modalities,
        "total_output_tokens": u.output_tokens,
        "total_tokens": u.total_tokens(),
    });
    if u.cache_read_tokens > 0
        && let Value::Object(map) = &mut v
    {
        map.insert("total_cached_tokens".into(), u.cache_read_tokens.into());
    }
    v
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Decide(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "Gemini Decisions apps receive decision answers only.",
        ));
    };
    let Some(Request::Decide(req)) = ctx.request.as_deref() else {
        return Err(KppError::provider(
            "Kaeso needs the app's request to write a Gemini Decisions reply: its answers are keyed by question.",
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
    let refused: Vec<String> = r
        .answers
        .iter()
        .zip(&keys)
        .filter(|(a, _)| matches!(a.value, AnswerValue::Refusal(_)))
        .map(|(_, k)| format!("`{k}`"))
        .collect();
    if !refused.is_empty() {
        return Err(KppError::new(
            ErrorKind::ContentFiltered,
            format!(
                "The model declined to answer {}; Gemini's Decisions API has no answer for that, so no answers are returned.",
                refused.join(", ")
            ),
        )
        .with_status(400));
    }
    let mut answers: IndexMap<String, RawJson> = IndexMap::new();
    for (i, (answer, key)) in r.answers.iter().zip(keys).enumerate() {
        answers.insert(key, encode_answer(req, i, answer)?);
    }
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(ser(&usage_value(&r.usage))?)
    };
    let w = ResponseWire {
        answers: Some(ser(&answers)?),
        usage,
        prompt_feedback: None,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}

/// The legend of a level: its text as the request has it.
fn legend_text(level: &kaeso_kpp::Level) -> String {
    match (&level.description, &level.label) {
        (Some(Description::Text(t)), None) => t.clone(),
        (Some(Description::Text(t)), Some(label)) => format!("{label}: {t}"),
        (Some(Description::Json(j)), _) => j.get().to_owned(),
        (None, Some(label)) => label.clone(),
        _ => String::new(),
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
            let w = NoulAnswerWire {
                ty: "noul".to_owned(),
                noul: num(p.probability, "noul", &ints)?,
                confidence: confidence(Some(p.probability.max(1.0 - p.probability)))?,
            };
            ser(&Ext::with(w, extra, verbatim))?
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
                probabilities: ser(&probabilities)?,
                confidence: confidence(top_probability(&c.probabilities))?,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        (AnswerValue::Score(s), QuestionKind::Score(q)) => {
            let legend = match answer.extras.form_value(DIALECT, LEGEND) {
                _ if same && answer.extras.form_flag(DIALECT, NO_LEGEND) => None,
                Some(v) if same => Some(ser(v)?),
                _ => {
                    let map: IndexMap<String, String> = q
                        .levels
                        .iter()
                        .enumerate()
                        .map(|(n, level)| (n.to_string(), legend_text(level)))
                        .collect();
                    Some(ser(&map)?)
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
                legend,
                probabilities: ser(&probabilities)?,
                confidence: confidence(top_probability(&s.probabilities))?,
            };
            ser(&Ext::with(w, extra, verbatim))?
        }
        _ => return Err(mismatch()),
    };
    Ok(json)
}
