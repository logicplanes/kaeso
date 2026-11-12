use std::collections::HashSet;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::block::{Block, CacheTtl, Source};
use crate::error::KppError;
use crate::extras::Extras;
use crate::raw::RawJson;
use crate::request::Message;
use crate::response::Usage;

/// A decision request: typed questions about one input, answered with probabilities instead of
/// generated text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DecideRequest {
    pub model: String,
    /// What every question is about: the state of System One, the input of OpenAI's and
    /// Gemini's Decisions APIs.
    pub input: DecisionInput,
    /// The questions, in order; `answers[i]` of the reply answers `questions[i]`.
    pub questions: Vec<Question>,
    /// Media the input, instructions and descriptions reference by their key, written as a whole
    /// string or inside a longer one (System One's `assets`).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub assets: IndexMap<String, Asset>,
    /// A stable identifier of the app's end user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// How the provider should cache the request's prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<DecisionCache>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl DecideRequest {
    pub fn new(model: impl Into<String>, input: DecisionInput) -> Self {
        DecideRequest {
            model: model.into(),
            input,
            questions: Vec::new(),
            assets: IndexMap::new(),
            user: None,
            cache: None,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_question(mut self, question: Question) -> Self {
        self.questions.push(question);
        self
    }

    #[must_use]
    pub fn with_asset(mut self, reference: impl Into<String>, asset: Asset) -> Self {
        self.assets.insert(reference.into(), asset);
        self
    }

    /// Checks what every decision request needs, whatever its format: a model, at least one
    /// question, options and levels to choose from, names used once, and assets that are
    /// referenced. The limits of each format, such as at least two options, are the codecs'.
    pub fn validate(&self) -> Result<(), KppError> {
        if self.model.is_empty() {
            return Err(invalid("`model` must not be empty.", "model"));
        }
        if self.questions.is_empty() {
            return Err(invalid("Ask at least one question.", "questions"));
        }
        let mut names: HashSet<&str> = HashSet::new();
        for (i, q) in self.questions.iter().enumerate() {
            if let Some(name) = q.name.as_deref().filter(|n| !names.insert(n)) {
                return Err(invalid(
                    format!("The question name `{name}` is used more than once."),
                    format!("questions[{i}]"),
                ));
            }
            match &q.kind {
                QuestionKind::Choice(c) if c.options.is_empty() => {
                    return Err(invalid(
                        "A choice question needs at least one option.",
                        format!("questions[{i}]"),
                    ));
                }
                QuestionKind::Score(s) if s.levels.is_empty() => {
                    return Err(invalid(
                        "A score question needs at least one level.",
                        format!("questions[{i}]"),
                    ));
                }
                _ => {}
            }
        }
        if !self.assets.is_empty() {
            let mut texts = Vec::new();
            self.texts(&mut texts);
            for reference in self.assets.keys() {
                if reference.trim().is_empty() {
                    return Err(invalid(
                        "An asset reference must not be empty.",
                        format!("assets.{reference}"),
                    ));
                }
                if !texts.iter().any(|t| t.contains(reference.as_str())) {
                    return Err(invalid(
                        format!(
                            "The asset `{reference}` is not referenced; write its reference where the image belongs."
                        ),
                        format!("assets.{reference}"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// What a model must be able to do to serve the request.
    pub fn required_capabilities(&self) -> crate::Capabilities {
        let mut caps = crate::Capabilities {
            images: !self.assets.is_empty(),
            cache_hints: self.cache.is_some(),
            ..crate::Capabilities::default()
        };
        let mut note = |block: &Block| match block {
            Block::Image(_) => caps.images = true,
            Block::Audio(_) => caps.audio_input = true,
            Block::Video(_) => caps.video_input = true,
            Block::Document(doc) => match doc.source {
                Source::Text(_) => caps.text_documents = true,
                _ => caps.pdf = true,
            },
            _ => {}
        };
        if let DecisionInput::Messages(messages) = &self.input {
            messages.iter().flat_map(|m| &m.blocks).for_each(&mut note);
        }
        for q in &self.questions {
            if let Some(Instructions::Blocks(blocks)) = &q.instructions {
                blocks.iter().for_each(&mut note);
            }
        }
        caps.box_questions = self
            .questions
            .iter()
            .any(|q| matches!(q.kind, QuestionKind::Box(_)));
        caps
    }

    // Every string an asset reference may stand in: strings of the input, the instructions and
    // the descriptions, without object keys, which are never replaced.
    fn texts(&self, out: &mut Vec<String>) {
        match &self.input {
            DecisionInput::Text(t) => out.push(t.clone()),
            DecisionInput::Json(raw) => strings_of(raw, out),
            DecisionInput::Messages(messages) => {
                for block in messages.iter().flat_map(|m| &m.blocks) {
                    block_texts(block, out);
                }
            }
        }
        for q in &self.questions {
            match &q.instructions {
                Some(Instructions::Text(t)) => out.push(t.clone()),
                Some(Instructions::Json(raw)) => strings_of(raw, out),
                Some(Instructions::Blocks(blocks)) => {
                    blocks.iter().for_each(|b| block_texts(b, out));
                }
                None => {}
            }
            let mut description = |d: &Option<Description>| match d {
                Some(Description::Text(t)) => out.push(t.clone()),
                Some(Description::Json(raw)) => strings_of(raw, out),
                None => {}
            };
            match &q.kind {
                QuestionKind::Predicate(p) => {
                    description(&p.when_true);
                    description(&p.when_false);
                }
                QuestionKind::Choice(c) => {
                    c.options.iter().for_each(|o| description(&o.description))
                }
                QuestionKind::Score(s) => s.levels.iter().for_each(|l| description(&l.description)),
                QuestionKind::Box(_) => {}
            }
        }
    }
}

fn block_texts(block: &Block, out: &mut Vec<String>) {
    match block {
        Block::Text(t) => out.push(t.text.clone()),
        Block::Json(j) => strings_of(&j.data, out),
        _ => {}
    }
}

fn strings_of(raw: &RawJson, out: &mut Vec<String>) {
    fn walk(v: Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s),
            Value::Array(items) => items.into_iter().for_each(|i| walk(i, out)),
            Value::Object(map) => map.into_iter().for_each(|(_, v)| walk(v, out)),
            _ => {}
        }
    }
    if let Ok(v) = raw.to_value() {
        walk(v, out);
    }
}

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

/// What a decision request's questions are about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DecisionInput {
    /// Plain text.
    Text(String),
    /// Structured data, written as the app wrote it.
    Json(RawJson),
    /// Messages with text, images, audio, video, documents and JSON parts, as OpenAI's and
    /// Gemini's formats send them, or a System One state of text and image parts.
    Messages(Vec<Message>),
}

/// An image a decision request references by its key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Asset {
    pub source: Source,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Asset {
    pub fn new(source: Source) -> Self {
        Asset {
            source,
            extras: Extras::default(),
        }
    }
}

/// One question of a decision request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Question {
    /// The name the app gave the question: System One's and Gemini's map key, OpenAI's optional
    /// `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Instructions>,
    pub kind: QuestionKind,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Question {
    pub fn new(name: Option<String>, kind: QuestionKind) -> Self {
        Question {
            name,
            instructions: None,
            kind,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_instructions(mut self, instructions: Instructions) -> Self {
        self.instructions = Some(instructions);
        self
    }

    /// The question type's KPP name: `predicate`, `choice`, `score` or `box`.
    pub fn type_name(&self) -> &'static str {
        self.kind.as_str()
    }
}

/// What a question asks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Instructions {
    Text(String),
    Json(RawJson),
    /// Text, image and JSON parts in order.
    Blocks(Vec<Block>),
}

/// The type of a question and what it chooses from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum QuestionKind {
    /// A yes/no question (System One's and Gemini's `noul`, OpenAI's `predicate`), answered with
    /// the probability of yes.
    Predicate(Predicate),
    /// One of several options.
    Choice(Choice),
    /// A position on an ordered scale of levels, lowest first.
    Score(Score),
    /// Where in an image something is.
    Box(BoxQuestion),
}

impl QuestionKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            QuestionKind::Predicate(_) => "predicate",
            QuestionKind::Choice(_) => "choice",
            QuestionKind::Score(_) => "score",
            QuestionKind::Box(_) => "box",
        }
    }
}

/// A yes/no question, with what counts as yes and as no.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Predicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_true: Option<Description>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_false: Option<Description>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Predicate {
    pub fn new(when_true: Option<Description>, when_false: Option<Description>) -> Self {
        Predicate {
            when_true,
            when_false,
            extras: Extras::default(),
        }
    }
}

/// A question that picks one option.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Choice {
    pub options: Vec<ChoiceOption>,
}

impl Choice {
    pub fn new(options: Vec<ChoiceOption>) -> Self {
        Choice { options }
    }
}

/// One option of a choice question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChoiceOption {
    pub value: ChoiceValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Description>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ChoiceOption {
    pub fn new(value: ChoiceValue, description: Option<Description>) -> Self {
        ChoiceOption {
            value,
            description,
            extras: Extras::default(),
        }
    }
}

/// An option's value: a key or text, or OpenAI's `true` and `false`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChoiceValue {
    Text(String),
    Bool(bool),
}

/// What an option, level or answer means: text, or structured data as written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Description {
    Text(String),
    Json(RawJson),
}

/// A question that rates on ordered levels.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Score {
    /// The levels, lowest first; level `i` scores `i`.
    pub levels: Vec<Level>,
}

impl Score {
    pub fn new(levels: Vec<Level>) -> Self {
        Score { levels }
    }
}

/// One level of a score question.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Level {
    /// A short name, as OpenAI's `label`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Description>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Level {
    pub fn new(label: Option<String>, description: Option<Description>) -> Self {
        Level {
            label,
            description,
            extras: Extras::default(),
        }
    }
}

/// A question that locates what its instructions describe in an image.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BoxQuestion {}

impl BoxQuestion {
    pub fn new() -> Self {
        BoxQuestion {}
    }
}

/// How the provider caches a decision request's prefix (OpenAI's `prompt_cache_key` and
/// `prompt_cache_options`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DecisionCache {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheTtl>,
}

impl DecisionCache {
    pub fn new(key: Option<String>) -> Self {
        DecisionCache {
            key,
            mode: None,
            ttl: None,
        }
    }
}

/// The answers to a decision request, one per question in question order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DecideResponse {
    pub model: String,
    pub answers: Vec<Answer>,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl DecideResponse {
    pub fn new(model: impl Into<String>, answers: Vec<Answer>) -> Self {
        DecideResponse {
            model: model.into(),
            answers,
            ..DecideResponse::default()
        }
    }
}

/// The answer to one question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Answer {
    pub value: AnswerValue,
    /// The provider's confidence, from 0 to 1. What it measures differs between formats; a
    /// reply that crosses formats recomputes it from the probabilities by the app's format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl Answer {
    pub fn new(value: AnswerValue) -> Self {
        Answer {
            value,
            confidence: None,
            extras: Extras::default(),
        }
    }

    #[must_use]
    pub fn with_confidence(mut self, confidence: Option<f64>) -> Self {
        self.confidence = confidence;
        self
    }
}

/// What a question was answered with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnswerValue {
    Predicate(PredicateAnswer),
    Choice(ChoiceAnswer),
    Score(ScoreAnswer),
    Box(BoxAnswer),
    /// The model declined to answer this question.
    Refusal(Refusal),
}

impl AnswerValue {
    /// The answer type's KPP name: `predicate`, `choice`, `score`, `box` or `refusal`.
    pub const fn as_str(&self) -> &'static str {
        match self {
            AnswerValue::Predicate(_) => "predicate",
            AnswerValue::Choice(_) => "choice",
            AnswerValue::Score(_) => "score",
            AnswerValue::Box(_) => "box",
            AnswerValue::Refusal(_) => "refusal",
        }
    }
}

/// The probability that the answer to a yes/no question is yes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PredicateAnswer {
    pub probability: f64,
}

impl PredicateAnswer {
    pub fn new(probability: f64) -> Self {
        PredicateAnswer { probability }
    }
}

/// The chosen option and a probability per option.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChoiceAnswer {
    /// The index of the chosen option in the question's options.
    pub option: usize,
    /// The probability of each option, by index; `None` where the provider gave none.
    pub probabilities: Vec<Option<f64>>,
}

impl ChoiceAnswer {
    pub fn new(option: usize, probabilities: Vec<Option<f64>>) -> Self {
        ChoiceAnswer {
            option,
            probabilities,
        }
    }
}

/// The expected level, `Σ i·p_i` over the levels numbered from 0, and a probability per level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScoreAnswer {
    pub score: f64,
    /// The probability of each level, by index; `None` where the provider gave none.
    pub probabilities: Vec<Option<f64>>,
}

impl ScoreAnswer {
    pub fn new(score: f64, probabilities: Vec<Option<f64>>) -> Self {
        ScoreAnswer {
            score,
            probabilities,
        }
    }
}

/// A box's edges as fractions of the image's height (`top`, `bottom`, from the top) and width
/// (`left`, `right`, from the left).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BoxAnswer {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl BoxAnswer {
    pub fn new(top: f64, bottom: f64, left: f64, right: f64) -> Self {
        BoxAnswer {
            top,
            bottom,
            left,
            right,
        }
    }
}

/// A question the model declined to answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Refusal {}

impl Refusal {
    pub fn new() -> Self {
        Refusal {}
    }
}
