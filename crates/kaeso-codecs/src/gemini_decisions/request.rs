use indexmap::IndexMap;
use kaeso_kpp::{
    Block, Choice, ChoiceOption, ChoiceValue, DecideRequest, DecisionInput, Description, Extras,
    Instructions, KppError, Level, Message, Origin, Predicate, Question, QuestionKind, RawJson,
    Request, RequestKind, Role, Score,
};
use serde_json::value::RawValue;

use super::blocks::{Place, decode_block, media_json, ser, text_block};
use super::wire::{NoulCriteriaWire, QuestionWire, RequestWire};
use super::{DIALECT, GEMINI_DECISIONS_PATH};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_foreign;
use crate::decisions::{SentQuestion, option_key, question_keys, sent_kind};
use crate::gemini::parts::Media;
use crate::openai_decisions::state::{Piece, pieces, reference_in, reference_in_text};
use crate::wire::{
    Ext, entries_of, extra_for, extras_from, field_of, to_body, verbatim_from_form,
    without_position,
};

/// Marks an input or instructions written as one content block instead of a list.
const FORM_OBJECT: &str = "object";
/// Marks a `noul` question written with `criteria`.
const FORM_CRITERIA: &str = "criteria";

/// The most questions one request may ask (Decisions guide, "Limits").
pub(super) const MAX_QUESTIONS: usize = 1_000;
/// The fewest and most options of a choice question and levels of a score question.
pub(super) const MIN_OPTIONS: usize = 2;
pub(super) const MAX_OPTIONS: usize = 255;
pub(super) const MIN_LEVELS: usize = 2;
pub(super) const MAX_LEVELS: usize = 255;
/// The longest question key, in ASCII characters, and option key, in characters and bytes.
const MAX_QUESTION_KEY: usize = 64;
const MAX_OPTION_CHARS: usize = 128;
const MAX_OPTION_BYTES: usize = 512;

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn unsupported(feature: &str, message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::unsupported(feature, message.into()).with_param(param)
}

fn other_kind(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        DIALECT,
        "Gemini's Decisions API answers decision questions only; send other requests to their own endpoints.",
    )
}

/// Whether `key` may name a question: 1 to 64 ASCII letters, digits, `_` and `-`.
pub(super) fn is_question_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_QUESTION_KEY
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// What keeps `key` from naming an option: 1 to 128 characters and at most 512 bytes, inner
/// ASCII spaces allowed, no space at either end, no control character, no other space.
pub(super) fn option_key_problem(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        return Some("is empty");
    }
    if key.chars().count() > MAX_OPTION_CHARS || key.len() > MAX_OPTION_BYTES {
        return Some("is longer than 128 characters or 512 bytes");
    }
    if key.starts_with(char::is_whitespace) || key.ends_with(char::is_whitespace) {
        return Some("starts or ends with a space");
    }
    if key.chars().any(char::is_control) {
        return Some("holds a control character");
    }
    if key.chars().any(|c| c.is_whitespace() && c != ' ') {
        return Some("holds a space other than the ASCII one");
    }
    None
}

fn parse_error(body: &[u8], e: &serde_json::Error) -> KppError {
    if let Ok(raw) = serde_json::from_slice::<&RawValue>(body)
        && field_of(raw, "state").is_some()
        && field_of(raw, "input").is_none()
    {
        return invalid(
            "This is a System One request, with `state` and no `input`; send it to `/v1/systemone`.",
            "state",
        );
    }
    let text = without_position(e);
    let err = KppError::invalid_request(format!(
        "The request body is not a valid {} request: {text}.",
        DIALECT.display_name()
    ));
    match ["model", "input", "questions"]
        .into_iter()
        .find(|f| text.contains(&format!("`{f}`")))
    {
        Some(f) => err.with_param(f),
        None => err,
    }
}

pub(super) fn decode(
    kind: RequestKind,
    body: &[u8],
    _headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind != RequestKind::Decide {
        return Err(other_kind(kind));
    }
    let origin = Origin::new(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<RequestWire>::from_slice(body).map_err(|e| parse_error(body, &e))?;
    let input = decode_input(&w.input, &origin)?;
    if w.questions.get().starts_with('[') {
        return Err(invalid(
            "`questions` must be an object of questions keyed by name; send OpenAI Decisions requests, with a list of questions, to `/v1/decisions`.",
            "questions",
        ));
    }
    let entries = entries_of(w.questions.as_raw()).ok_or_else(|| {
        invalid(
            "`questions` must be an object of questions keyed by name.",
            "questions",
        )
    })?;
    let mut req = DecideRequest::new(w.model, input);
    for (key, value) in entries {
        req.questions.push(decode_question(&key, value, &origin)?);
    }
    req.extras = extras_from(&origin, extra, verbatim);
    Ok(Request::Decide(req))
}

fn decode_input(raw: &RawJson, origin: &Origin) -> Result<DecisionInput, KppError> {
    let text = raw.get();
    if text.starts_with('"') {
        return raw
            .parse()
            .map(DecisionInput::Text)
            .map_err(|_| invalid("`input` is not a valid string.", "input"));
    }
    let mut message = Message::new(Role::User, Vec::new());
    message.extras = Extras::new(origin.clone());
    if text.starts_with('{') {
        message
            .blocks
            .push(decode_block(raw.as_raw(), Place::Input, origin, "input")?);
        message
            .extras
            .form
            .insert(FORM_OBJECT.to_owned(), true.into());
        return Ok(DecisionInput::Messages(vec![message]));
    }
    let items: Vec<&RawValue> = raw.parse().map_err(|_| {
        invalid(
            "`input` must be a string, a content block or a list of content blocks.",
            "input",
        )
    })?;
    for (i, item) in items.into_iter().enumerate() {
        message.blocks.push(decode_block(
            item,
            Place::Input,
            origin,
            &format!("input[{i}]"),
        )?);
    }
    Ok(DecisionInput::Messages(vec![message]))
}

fn decode_instructions(
    raw: &RawJson,
    origin: &Origin,
    param: &str,
    extras: &mut Extras,
) -> Result<Instructions, KppError> {
    let text = raw.get();
    if text.starts_with('"') {
        return raw
            .parse()
            .map(Instructions::Text)
            .map_err(|_| invalid("`instructions` is not a valid string.", param));
    }
    if text.starts_with('{') {
        extras.form.insert(FORM_OBJECT.to_owned(), true.into());
        return Ok(Instructions::Blocks(vec![decode_block(
            raw.as_raw(),
            Place::Instructions,
            origin,
            param,
        )?]));
    }
    let items: Vec<&RawValue> = raw.parse().map_err(|_| {
        invalid(
            "`instructions` must be a string, a text block or a list of text blocks.",
            param,
        )
    })?;
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| decode_block(item, Place::Instructions, origin, &format!("{param}[{i}]")))
        .collect::<Result<_, _>>()
        .map(Instructions::Blocks)
}

fn decode_question(key: &str, value: &RawValue, origin: &Origin) -> Result<Question, KppError> {
    let param = format!("questions.{key}");
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<QuestionWire>::from_raw(value).map_err(|e| {
        invalid(
            format!(
                "The question `{key}` is not valid: {}.",
                without_position(&e)
            ),
            &param,
        )
    })?;
    let mut extras = extras_from(origin, extra, verbatim);
    let criteria_param = format!("{param}.criteria");
    let kind = match w.ty.as_str() {
        "noul" => {
            let mut p = Predicate::default();
            if let Some(criteria) = &w.criteria {
                let c = Ext::<NoulCriteriaWire>::from_raw(criteria.as_raw()).map_err(|_| {
                    invalid(
                        "The criteria of a `noul` question must be an object with the strings `true_description` and `false_description`.",
                        &criteria_param,
                    )
                })?;
                p.when_true = c.known.true_description.map(Description::Text);
                p.when_false = c.known.false_description.map(Description::Text);
                p.extras = extras_from(origin, c.extra, c.verbatim);
                extras.form.insert(FORM_CRITERIA.to_owned(), true.into());
            }
            QuestionKind::Predicate(p)
        }
        "choice" => {
            let criteria = w
                .criteria
                .as_ref()
                .ok_or_else(|| invalid("A `choice` question needs `criteria`.", &criteria_param))?;
            let entries = entries_of(criteria.as_raw()).ok_or_else(|| {
                invalid(
                    "The criteria of a `choice` question must be an object of options.",
                    &criteria_param,
                )
            })?;
            let mut options = Vec::with_capacity(entries.len());
            for (name, value) in entries {
                let description: String = serde_json::from_str(value.get()).map_err(|_| {
                    invalid(
                        format!("The description of the option `{name}` must be a string."),
                        format!("{criteria_param}.{name}"),
                    )
                })?;
                options.push(ChoiceOption::new(
                    ChoiceValue::Text(name.into_owned()),
                    Some(Description::Text(description)),
                ));
            }
            QuestionKind::Choice(Choice::new(options))
        }
        "score" => {
            let criteria = w
                .criteria
                .as_ref()
                .ok_or_else(|| invalid("A `score` question needs `criteria`.", &criteria_param))?;
            let levels: Vec<String> = criteria.parse().map_err(|_| {
                invalid(
                    "The criteria of a `score` question must be a list of strings.",
                    &criteria_param,
                )
            })?;
            QuestionKind::Score(Score::new(
                levels
                    .into_iter()
                    .map(|l| Level::new(None, Some(Description::Text(l))))
                    .collect(),
            ))
        }
        other => {
            return Err(invalid(
                format!(
                    "`{other}` is not a Gemini Decisions question type; use `noul`, `choice` or `score`."
                ),
                format!("{param}.type"),
            ));
        }
    };
    let instructions = match &w.instructions {
        Some(raw) => Some(decode_instructions(
            raw,
            origin,
            &format!("{param}.instructions"),
            &mut extras,
        )?),
        None => None,
    };
    let mut q = Question::new(Some(key.to_owned()), kind);
    q.instructions = instructions;
    q.extras = extras;
    Ok(q)
}

fn conversation() -> KppError {
    unsupported(
        "conversation input",
        "Gemini's Decisions API takes one input, not a conversation; send one user message.",
        "input",
    )
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Decide(r) = req else {
        return Err(other_kind(req.kind()));
    };
    // A conversation can't be sent at all, whatever its items carry.
    if let DecisionInput::Messages(messages) = &r.input
        && !matches!(messages.as_slice(), [m] if m.role == Role::User)
    {
        return Err(conversation());
    }
    check_foreign(req, &ctx.origin(DIALECT))?;
    r.validate()?;
    build(r, ctx, true).map(|_| ())
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, ctx)?;
    let Request::Decide(r) = req else {
        return Err(other_kind(req.kind()));
    };
    let built = build(r, ctx, false)?;
    for note in built.adapted {
        if !ctx.adapted.iter().any(|a| a == note) {
            ctx.adapted.push(note.to_owned());
        }
    }
    for note in built.left_out {
        if !ctx.left_out.iter().any(|a| a == note) {
            ctx.left_out.push(note.to_owned());
        }
    }
    ctx.decision_questions = built.sent;
    ctx.decision_model = Some(r.model.clone());
    let headers = HeaderList::new().with("content-type", "application/json");
    Ok(EncodedRequest::post_json(built.path, headers, built.body))
}

struct Built {
    body: Vec<u8>,
    path: String,
    sent: Vec<SentQuestion>,
    adapted: Vec<&'static str>,
    left_out: Vec<&'static str>,
}

/// A piece of the input on its way out: text, which joins the text next to it, or a block.
enum Out {
    Text(String),
    Block(RawJson),
}

fn push_text(out: &mut Vec<Out>, text: &str, separator: &str) {
    match out.last_mut() {
        Some(Out::Text(t)) => {
            t.push_str(separator);
            t.push_str(text);
        }
        _ => out.push(Out::Text(text.to_owned())),
    }
}

struct Writer<'r> {
    r: &'r DecideRequest,
    dry: bool,
    adapted: Vec<&'static str>,
    left_out: Vec<&'static str>,
}

impl Writer<'_> {
    fn note(list: &mut Vec<&'static str>, what: &'static str) {
        if !list.contains(&what) {
            list.push(what);
        }
    }

    /// The pieces a string that holds asset references becomes: its text and, in place of each
    /// reference, the image.
    fn text_with_images(
        &self,
        text: &str,
        out: &mut Vec<Out>,
        first: bool,
    ) -> Result<(), KppError> {
        let assets = &self.r.assets;
        for (i, piece) in pieces(text, assets).into_iter().enumerate() {
            match piece {
                Piece::Text(t) => {
                    let separator = if i == 0 && !first { "\n" } else { "" };
                    push_text(out, t, separator);
                }
                Piece::Image(reference, asset) => out.push(Out::Block(media_json(
                    Media::Image,
                    &asset.source,
                    &asset.extras,
                    self.dry,
                    &format!("assets.{reference}"),
                )?)),
            }
        }
        Ok(())
    }

    fn nested_reference(reference: &str, param: &str) -> KppError {
        unsupported(
            "image reference",
            format!(
                "Gemini takes images as blocks of the input, so the image `{reference}` must stand in the input string or in a string of a state list; move it there."
            ),
            param,
        )
    }

    // Each item of a state list in order: strings cut around their references, other values as
    // their JSON; text of one item and the next is joined with a line break.
    fn json_with_images(&self, raw: &RawJson) -> Result<Vec<Out>, KppError> {
        let Ok(items) = raw.parse::<Vec<&RawValue>>() else {
            let reference = reference_in(raw.as_raw(), &self.r.assets).unwrap_or_default();
            return Err(Self::nested_reference(reference, "input"));
        };
        let mut out = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let param = format!("input[{i}]");
            if item.get().starts_with('"') {
                let text: String = serde_json::from_str(item.get())
                    .map_err(|_| invalid("A string is not valid.", &param))?;
                self.text_with_images(&text, &mut out, i == 0)?;
                continue;
            }
            if let Some(reference) = reference_in(item, &self.r.assets) {
                return Err(Self::nested_reference(reference, &param));
            }
            push_text(&mut out, item.get(), if i == 0 { "" } else { "\n" });
        }
        Ok(out)
    }

    fn outs(outs: Vec<Out>) -> Result<RawJson, KppError> {
        let mut items = Vec::with_capacity(outs.len());
        for o in outs {
            items.push(match o {
                Out::Text(t) => text_block(&t, &Extras::default())?,
                Out::Block(b) => b,
            });
        }
        ser(&items)
    }

    fn input(&mut self) -> Result<RawJson, KppError> {
        let r = self.r;
        match &r.input {
            DecisionInput::Text(t) => match reference_in_text(t, &r.assets) {
                None => ser(t),
                Some(_) => {
                    let mut out = Vec::new();
                    self.text_with_images(t, &mut out, true)?;
                    Self::outs(out)
                }
            },
            DecisionInput::Json(raw) => {
                if reference_in(raw.as_raw(), &r.assets).is_some() {
                    let out = self.json_with_images(raw)?;
                    return Self::outs(out);
                }
                ser(&raw.get())
            }
            DecisionInput::Messages(messages) => match messages.as_slice() {
                [m] if m.role == Role::User => self.message(m),
                _ => Err(conversation()),
            },
            _ => Err(unsupported(
                "input",
                "Gemini's Decisions API can't take this input.",
                "input",
            )),
        }
    }

    fn message(&mut self, m: &Message) -> Result<RawJson, KppError> {
        let mut items = Vec::with_capacity(m.blocks.len());
        for (i, block) in m.blocks.iter().enumerate() {
            let param = format!("input[{i}]");
            if block.cache().is_some() {
                Self::note(&mut self.left_out, "cache");
            }
            match block {
                Block::Text(t)
                    if !t.extras.is_from(DIALECT)
                        && reference_in_text(&t.text, &self.r.assets).is_some() =>
                {
                    let mut out = Vec::new();
                    self.text_with_images(&t.text, &mut out, true)?;
                    for o in out {
                        items.push(match o {
                            Out::Text(t) => text_block(&t, &Extras::default())?,
                            Out::Block(b) => b,
                        });
                    }
                }
                Block::Text(t) => items.push(text_block(&t.text, &t.extras)?),
                Block::Json(j) => items.push(text_block(j.data.get(), &Extras::default())?),
                Block::Image(b) => {
                    items.push(media_json(
                        Media::Image,
                        &b.source,
                        &b.extras,
                        self.dry,
                        &param,
                    )?);
                }
                Block::Audio(b) => {
                    items.push(media_json(
                        Media::Audio,
                        &b.source,
                        &b.extras,
                        self.dry,
                        &param,
                    )?);
                }
                Block::Video(b) => {
                    items.push(media_json(
                        Media::Video,
                        &b.source,
                        &b.extras,
                        self.dry,
                        &param,
                    )?);
                }
                Block::Document(b) => {
                    items.push(media_json(
                        Media::Document,
                        &b.source,
                        &b.extras,
                        self.dry,
                        &param,
                    )?);
                }
                Block::Unknown(item) if item.origin.is(DIALECT) => items.push(item.raw.clone()),
                other => {
                    return Err(unsupported(
                        &format!("{} part", other.kind()),
                        format!(
                            "Gemini's Decisions API takes text, images, audio, video and PDF documents, not {} parts; remove it.",
                            other.kind()
                        ),
                        param,
                    ));
                }
            }
        }
        let object = m.extras.form_flag(DIALECT, FORM_OBJECT);
        match items.as_slice() {
            [one] if object => Ok(one.clone()),
            _ => ser(&items),
        }
    }

    fn no_images(&self, text: &str, feature: &str, param: &str) -> Result<(), KppError> {
        match reference_in_text(text, &self.r.assets) {
            None => Ok(()),
            Some(reference) => Err(unsupported(
                feature,
                format!(
                    "Gemini's Decisions API takes text only in questions, so the image `{reference}` can't be sent there; put it in the input."
                ),
                param,
            )),
        }
    }

    fn description_text(&mut self, d: &Description, param: &str) -> Result<String, KppError> {
        let text = match d {
            Description::Text(t) => t.clone(),
            Description::Json(raw) => {
                Self::note(&mut self.adapted, "criteria");
                raw.get().to_owned()
            }
            _ => {
                return Err(unsupported(
                    "description",
                    "Gemini's Decisions API can't take this description.",
                    param,
                ));
            }
        };
        self.no_images(&text, "image in criteria", param)?;
        Ok(text)
    }

    fn instructions(&mut self, q: &Question, param: &str) -> Result<Option<RawJson>, KppError> {
        let p = format!("{param}.instructions");
        let feature = "image in question instructions";
        let same = q.extras.is_from(DIALECT);
        Ok(match &q.instructions {
            None if same => None,
            None => Some(ser(&"")?),
            Some(Instructions::Text(t)) => {
                self.no_images(t, feature, &p)?;
                Some(ser(t)?)
            }
            Some(Instructions::Json(raw)) => {
                self.no_images(raw.get(), feature, &p)?;
                Self::note(&mut self.adapted, "instructions");
                Some(ser(&raw.get())?)
            }
            Some(Instructions::Blocks(blocks)) => {
                let mut items = Vec::with_capacity(blocks.len());
                for (i, b) in blocks.iter().enumerate() {
                    let bp = format!("{p}[{i}]");
                    items.push(match b {
                        Block::Text(t) => {
                            if !t.extras.is_from(DIALECT) {
                                self.no_images(&t.text, feature, &bp)?;
                            }
                            text_block(&t.text, &t.extras)?
                        }
                        Block::Json(j) => {
                            self.no_images(j.data.get(), feature, &bp)?;
                            Self::note(&mut self.adapted, "instructions");
                            text_block(j.data.get(), &Extras::default())?
                        }
                        Block::Unknown(item) if item.origin.is(DIALECT) => item.raw.clone(),
                        Block::Image(_) => {
                            return Err(unsupported(
                                feature,
                                "Gemini's Decisions API takes text only in question instructions; put the image in the input.",
                                bp,
                            ));
                        }
                        other => {
                            return Err(unsupported(
                                &format!("{} part", other.kind()),
                                format!(
                                    "Gemini's Decisions API takes text only in question instructions, not {} parts.",
                                    other.kind()
                                ),
                                bp,
                            ));
                        }
                    });
                }
                match items.as_slice() {
                    [one] if same && q.extras.form_flag(DIALECT, FORM_OBJECT) => Some(one.clone()),
                    _ => Some(ser(&items)?),
                }
            }
            Some(_) => {
                return Err(unsupported(
                    "instructions",
                    "Gemini's Decisions API can't take these instructions.",
                    p,
                ));
            }
        })
    }

    fn level_text(&mut self, level: &Level, param: &str) -> Result<String, KppError> {
        let text = match (&level.label, &level.description) {
            (None, Some(d)) => self.description_text(d, param)?,
            (Some(label), None) => label.clone(),
            (Some(label), Some(d)) => {
                Self::note(&mut self.adapted, "levels");
                let d = self.description_text(d, param)?;
                format!("{label}: {d}")
            }
            (None, None) => String::new(),
        };
        if text.is_empty() {
            return Err(unsupported(
                "score level",
                "Gemini's Decisions API needs a text for every level of a score question; give the level a label or description.",
                param,
            ));
        }
        self.no_images(&text, "image in criteria", param)?;
        Ok(text)
    }

    fn question(&mut self, i: usize, q: &Question) -> Result<(RawJson, Vec<String>), KppError> {
        let param = format!("questions[{i}]");
        let same = q.extras.is_from(DIALECT);
        let instructions = self.instructions(q, &param)?;
        let mut options = Vec::new();
        let (ty, criteria) = match &q.kind {
            QuestionKind::Predicate(p) => {
                let marked = same && q.extras.form_flag(DIALECT, FORM_CRITERIA);
                let criteria = if marked || p.when_true.is_some() || p.when_false.is_some() {
                    let cp = format!("{param}.criteria");
                    let w = NoulCriteriaWire {
                        true_description: p
                            .when_true
                            .as_ref()
                            .map(|d| self.description_text(d, &cp))
                            .transpose()?,
                        false_description: p
                            .when_false
                            .as_ref()
                            .map(|d| self.description_text(d, &cp))
                            .transpose()?,
                    };
                    Some(ser(&Ext::with(
                        w,
                        extra_for(&p.extras, DIALECT),
                        verbatim_from_form(&p.extras, DIALECT, ""),
                    ))?)
                } else {
                    None
                };
                ("noul", criteria)
            }
            QuestionKind::Choice(c) => {
                let n = c.options.len();
                if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&n) {
                    return Err(unsupported(
                        "options",
                        format!(
                            "Gemini's Decisions API takes {MIN_OPTIONS} to {MAX_OPTIONS} options per choice question; this one has {n}."
                        ),
                        param,
                    ));
                }
                let mut map: IndexMap<String, RawJson> = IndexMap::new();
                for (oi, option) in c.options.iter().enumerate() {
                    let op = format!("{param}.options[{oi}]");
                    if matches!(option.value, ChoiceValue::Bool(_)) {
                        Self::note(&mut self.adapted, "booleans");
                    }
                    let key = option_key(&option.value).ok_or_else(|| {
                        unsupported(
                            "choice value",
                            "Gemini's Decisions API takes text option keys.",
                            &op,
                        )
                    })?;
                    if let Some(problem) = option_key_problem(&key) {
                        return Err(unsupported(
                            "option key",
                            format!(
                                "Gemini's option keys are 1 to 128 characters with no space at either end and no control character; `{key}` {problem}."
                            ),
                            op,
                        ));
                    }
                    if map.contains_key(&key) {
                        return Err(unsupported(
                            "repeated choice value",
                            format!(
                                "Gemini names options by text, so `{key}` would name two options; give every option its own value."
                            ),
                            op,
                        ));
                    }
                    let description = match &option.description {
                        Some(d) => self.description_text(d, &op)?,
                        None => String::new(),
                    };
                    map.insert(key.clone(), ser(&description)?);
                    options.push(key);
                }
                ("choice", Some(ser(&map)?))
            }
            QuestionKind::Score(s) => {
                let n = s.levels.len();
                if !(MIN_LEVELS..=MAX_LEVELS).contains(&n) {
                    return Err(unsupported(
                        "levels",
                        format!(
                            "Gemini's Decisions API takes {MIN_LEVELS} to {MAX_LEVELS} levels per score question; this one has {n}."
                        ),
                        param,
                    ));
                }
                let mut levels = Vec::with_capacity(n);
                for (li, level) in s.levels.iter().enumerate() {
                    levels.push(self.level_text(level, &format!("{param}.levels[{li}]"))?);
                }
                ("score", Some(ser(&levels)?))
            }
            QuestionKind::Box(_) => {
                return Err(unsupported(
                    "box question",
                    "Gemini's Decisions API answers no box questions; send them to a System One provider that does, such as TypeSafe.",
                    param,
                ));
            }
            _ => {
                return Err(unsupported(
                    "question type",
                    "Gemini's Decisions API can't take this question type.",
                    param,
                ));
            }
        };
        let w = QuestionWire {
            ty: ty.to_owned(),
            instructions,
            criteria,
        };
        let json = ser(&Ext::with(
            w,
            extra_for(&q.extras, DIALECT),
            verbatim_from_form(&q.extras, DIALECT, ""),
        ))?;
        Ok((json, options))
    }
}

fn build(r: &DecideRequest, ctx: &EgressCtx, dry: bool) -> Result<Built, KppError> {
    if r.questions.len() > MAX_QUESTIONS {
        return Err(unsupported(
            "questions",
            format!(
                "Gemini's Decisions API takes at most 1,000 questions per request; this one has {}.",
                r.questions.len()
            ),
            "questions",
        ));
    }
    let mut w = Writer {
        r,
        dry,
        adapted: Vec::new(),
        left_out: Vec::new(),
    };
    let (keys, renamed) = question_keys(&r.questions, is_question_key);
    if renamed {
        Writer::note(&mut w.adapted, "names");
    }
    if r.cache.is_some() {
        Writer::note(&mut w.left_out, "cache");
    }
    if r.user.is_some() {
        Writer::note(&mut w.left_out, "user");
    }
    let input = w.input()?;
    let mut questions: IndexMap<String, RawJson> = IndexMap::new();
    let mut sent = Vec::with_capacity(r.questions.len());
    for (i, (q, key)) in r.questions.iter().zip(&keys).enumerate() {
        let (json, options) = w.question(i, q)?;
        sent.push(SentQuestion {
            key: key.clone(),
            kind: sent_kind(q, options),
        });
        questions.insert(key.clone(), json);
    }
    let out = RequestWire {
        model: r.model.clone(),
        input,
        questions: ser(&questions)?,
    };
    let body = to_body(&Ext::with(
        out,
        extra_for(&r.extras, DIALECT),
        verbatim_from_form(&r.extras, DIALECT, ""),
    ))?;
    let path = ctx
        .base_path
        .clone()
        .unwrap_or_else(|| GEMINI_DECISIONS_PATH.to_owned());
    Ok(Built {
        body,
        path,
        sent,
        adapted: w.adapted,
        left_out: w.left_out,
    })
}
