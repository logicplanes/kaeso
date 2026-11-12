use kaeso_kpp::{
    Block, CacheTtl, Choice, ChoiceOption, ChoiceValue, DecideRequest, DecisionCache,
    DecisionInput, Description, Instructions, KppError, Level, Origin, Predicate, Question,
    QuestionKind, RawJson, Request, RequestKind, Score,
};
use serde_json::value::RawValue;

use super::DIALECT;
use super::parts::{PartWriter, Place, decode_item, decode_part, ser};
use super::profile::DecisionsProfile;
use super::state::{Piece, image_of, pieces, reference_in, reference_in_text};
use super::wire::{CacheOptionsWire, ChoiceWire, LevelWire, QuestionWire, RequestWire};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header, under_api_root};
use crate::cross::check_foreign;
use crate::decisions::{SentQuestion, sent_kind};
use crate::wire::{
    Ext, extra_for, extras_from, field_of, to_body, verbatim_from_form, without_position,
};

/// Marks a request whose `prompt_cache_options` was an object of the documented fields.
const FORM_CACHE_OPTIONS: &str = "cache_options";
/// The request field a cache setting KPP can't type travels in.
const CACHE_OPTIONS: &str = "prompt_cache_options";

/// The fewest and most options a choice question may have, and levels a score question may
/// have (<https://developers.openai.com/api/reference/resources/decisions/methods/create>).
pub(super) const MIN_OPTIONS: usize = 2;
pub(super) const MAX_OPTIONS: usize = 255;
pub(super) const MIN_LEVELS: usize = 2;
pub(super) const MAX_LEVELS: usize = 10;

/// How criteria of another format, which OpenAI's predicates have no field for, are written into
/// the instructions (D11 on the cross-format page).
pub(super) const WHEN_TRUE: &str = "It is true when: ";
pub(super) const WHEN_FALSE: &str = "It is false when: ";

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
        "The Decisions API answers decision questions only; send other requests to their own endpoints.",
    )
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
    match [
        "model",
        "input",
        "questions",
        "prompt_cache_key",
        "safety_identifier",
    ]
    .into_iter()
    .find(|f| text.contains(&format!("`{f}`")))
    {
        Some(f) => err.with_param(f),
        None => err,
    }
}

fn ttl_of(text: String) -> CacheTtl {
    match text.as_str() {
        "5m" => CacheTtl::FiveMinutes,
        "1h" => CacheTtl::OneHour,
        _ => CacheTtl::Other(text),
    }
}

fn ttl_text(ttl: &CacheTtl) -> Option<String> {
    match ttl {
        CacheTtl::FiveMinutes => Some("5m".to_owned()),
        CacheTtl::OneHour => Some("1h".to_owned()),
        CacheTtl::Other(t) => Some(t.clone()),
        _ => None,
    }
}

pub(super) fn decode(
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
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
    let input = if w.input.get().starts_with('"') {
        DecisionInput::Text(
            w.input
                .parse()
                .map_err(|_| invalid("`input` is not a valid string.", "input"))?,
        )
    } else {
        let items: Vec<&RawValue> = w.input.parse().map_err(|_| {
            invalid(
                "`input` must be a string or a list of messages and tool items.",
                "input",
            )
        })?;
        DecisionInput::Messages(
            items
                .into_iter()
                .enumerate()
                .map(|(i, item)| decode_item(item, &origin, i))
                .collect::<Result<_, _>>()?,
        )
    };
    let questions: Vec<&RawValue> = w
        .questions
        .parse()
        .map_err(|_| invalid("`questions` must be a list of questions.", "questions"))?;
    let mut req = DecideRequest::new(w.model, input);
    for (i, q) in questions.into_iter().enumerate() {
        req.questions.push(decode_question(i, q, &origin)?);
    }
    let mut extras = extras_from(&origin, extra, verbatim);
    if let Some(key) = w.prompt_cache_key {
        req.cache = Some(DecisionCache::new(Some(key)));
    }
    if let Some(options) = w.prompt_cache_options {
        match Ext::<CacheOptionsWire>::from_raw(options.as_raw()) {
            Ok(o) if o.extra.is_empty() && o.verbatim.is_empty() => {
                let cache = req.cache.get_or_insert_with(DecisionCache::default);
                cache.mode = o.known.mode;
                cache.ttl = o.known.ttl.map(ttl_of);
                extras
                    .form
                    .insert(FORM_CACHE_OPTIONS.to_owned(), true.into());
            }
            _ => {
                extras.fields.insert(CACHE_OPTIONS.to_owned(), options);
            }
        }
    }
    req.user = w.safety_identifier;
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            extras.insert_header(name, value);
        }
    }
    req.extras = extras;
    Ok(Request::Decide(req))
}

fn decode_question(i: usize, raw: &RawValue, origin: &Origin) -> Result<Question, KppError> {
    let param = format!("questions[{i}]");
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<QuestionWire>::from_raw(raw).map_err(|e| {
        invalid(
            format!("The question is not valid: {}.", without_position(&e)),
            &param,
        )
    })?;
    let list = |raw: &RawJson, field: &str| -> Result<Vec<RawJson>, KppError> {
        raw.parse::<Vec<&RawValue>>()
            .map(|items| items.into_iter().map(RawJson::from).collect())
            .map_err(|_| {
                invalid(
                    format!("`{field}` must be a list."),
                    format!("{param}.{field}"),
                )
            })
    };
    let refuse_field = |field: &str, ty: &str| {
        invalid(
            format!("A `{ty}` question takes no `{field}`."),
            format!("{param}.{field}"),
        )
    };
    let kind = match w.ty.as_str() {
        "predicate" => {
            if w.choices.is_some() {
                return Err(refuse_field("choices", "predicate"));
            }
            if w.levels.is_some() {
                return Err(refuse_field("levels", "predicate"));
            }
            QuestionKind::Predicate(Predicate::default())
        }
        "choice" => {
            if w.levels.is_some() {
                return Err(refuse_field("levels", "choice"));
            }
            let Some(choices) = &w.choices else {
                return Err(invalid(
                    "A `choice` question needs `choices`.",
                    format!("{param}.choices"),
                ));
            };
            let mut options = Vec::new();
            for (oi, c) in list(choices, "choices")?.iter().enumerate() {
                let p = format!("{param}.choices[{oi}]");
                let Ext {
                    known: c,
                    extra,
                    verbatim,
                } = Ext::<ChoiceWire>::from_raw(c.as_raw()).map_err(|e| {
                    invalid(
                        format!("The choice is not valid: {}.", without_position(&e)),
                        &p,
                    )
                })?;
                let value = match c.value.to_value() {
                    Ok(serde_json::Value::String(s)) => ChoiceValue::Text(s),
                    Ok(serde_json::Value::Bool(b)) => ChoiceValue::Bool(b),
                    _ => {
                        return Err(invalid(
                            "A choice's `value` must be a string or a boolean.",
                            format!("{p}.value"),
                        ));
                    }
                };
                let mut option = ChoiceOption::new(value, c.description.map(Description::Text));
                option.extras = extras_from(origin, extra, verbatim);
                options.push(option);
            }
            QuestionKind::Choice(Choice::new(options))
        }
        "score" => {
            if w.choices.is_some() {
                return Err(refuse_field("choices", "score"));
            }
            let Some(levels) = &w.levels else {
                return Err(invalid(
                    "A `score` question needs `levels`.",
                    format!("{param}.levels"),
                ));
            };
            let mut out = Vec::new();
            for (li, l) in list(levels, "levels")?.iter().enumerate() {
                let p = format!("{param}.levels[{li}]");
                let Ext {
                    known: l,
                    extra,
                    verbatim,
                } = Ext::<LevelWire>::from_raw(l.as_raw()).map_err(|e| {
                    invalid(
                        format!("The level is not valid: {}.", without_position(&e)),
                        &p,
                    )
                })?;
                let mut level = Level::new(Some(l.label), l.description.map(Description::Text));
                level.extras = extras_from(origin, extra, verbatim);
                out.push(level);
            }
            QuestionKind::Score(Score::new(out))
        }
        other => {
            return Err(invalid(
                format!(
                    "`{other}` is not an OpenAI Decisions question type; use `predicate`, `choice` or `score`."
                ),
                format!("{param}.type"),
            ));
        }
    };
    let mut q = Question::new(w.name, kind);
    q.instructions = match w.instructions {
        None => None,
        Some(raw) if raw.get().starts_with('"') => {
            Some(Instructions::Text(raw.parse().map_err(|_| {
                invalid(
                    "`instructions` is not a valid string.",
                    format!("{param}.instructions"),
                )
            })?))
        }
        Some(raw) => {
            let p = format!("{param}.instructions");
            let parts: Vec<&RawValue> = raw
                .parse()
                .map_err(|_| invalid("`instructions` must be a string or a list of parts.", &p))?;
            Some(Instructions::Blocks(
                parts
                    .into_iter()
                    .enumerate()
                    .map(|(pi, part)| {
                        decode_part(part, Place::Instructions, origin, &format!("{p}[{pi}]"))
                    })
                    .collect::<Result<_, _>>()?,
            ))
        }
    };
    q.extras = extras_from(origin, extra, verbatim);
    Ok(q)
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Decide(r) = req else {
        return Err(other_kind(req.kind()));
    };
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
    ctx.decision_questions = built.sent;
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    Ok(EncodedRequest::post_json(built.path, headers, built.body))
}

struct Built {
    body: Vec<u8>,
    path: String,
    sent: Vec<SentQuestion>,
    adapted: Vec<&'static str>,
}

struct Writer<'r> {
    r: &'r DecideRequest,
    profile: DecisionsProfile,
    dry: bool,
    adapted: Vec<&'static str>,
}

/// The key an answer to `q` is matched by: its name, or nothing.
pub(super) fn name_key(q: &Question) -> String {
    q.name.clone().unwrap_or_default()
}

/// The JSON of a choice value, as answers name it.
pub(super) fn value_key(value: &ChoiceValue) -> Option<String> {
    match value {
        ChoiceValue::Text(t) => serde_json::to_string(t).ok(),
        ChoiceValue::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

impl Writer<'_> {
    fn note(&mut self, what: &'static str) {
        if !self.adapted.contains(&what) {
            self.adapted.push(what);
        }
    }

    fn parts(&mut self, images: bool) -> PartWriter<'_> {
        PartWriter {
            dry: self.dry,
            adapted: &mut self.adapted,
            images,
            name: self.profile.name,
        }
    }

    fn image_part(
        &mut self,
        reference: &str,
        asset: &kaeso_kpp::Asset,
        images: bool,
    ) -> Result<RawJson, KppError> {
        self.parts(images)
            .image(&image_of(asset), &format!("assets.{reference}"))
    }

    /// The parts a string that holds asset references becomes: its text and, in place of each
    /// reference, the image.
    fn text_with_images(&mut self, text: &str, images: bool) -> Result<Vec<RawJson>, KppError> {
        let r = self.r;
        let assets = &r.assets;
        let mut out = Vec::new();
        for piece in pieces(text, assets) {
            match piece {
                Piece::Text(t) => out.push(self.parts(images).text_part(t)?),
                Piece::Image(reference, asset) => {
                    out.push(self.image_part(reference, asset, images)?);
                }
            }
        }
        Ok(out)
    }

    fn nested_reference(&self, reference: &str, param: &str) -> KppError {
        unsupported(
            "image reference",
            format!(
                "{} takes images as parts of the input or the instructions, so the image `{reference}` must stand in a string of the state, of a state list or of the instructions; move it there.",
                self.profile.name
            ),
            param,
        )
    }

    /// The parts of a structured value with images: each item of a list as a part, strings cut
    /// around their references.
    fn json_with_images(
        &mut self,
        raw: &RawJson,
        images: bool,
        param: &str,
    ) -> Result<Vec<RawJson>, KppError> {
        let Ok(items) = raw.parse::<Vec<&RawValue>>() else {
            let reference = reference_in(raw.as_raw(), &self.r.assets).unwrap_or_default();
            return Err(self.nested_reference(reference, param));
        };
        let mut out = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let p = format!("{param}[{i}]");
            if item.get().starts_with('"') {
                let text: String = serde_json::from_str(item.get())
                    .map_err(|_| invalid("A string is not valid.", &p))?;
                out.extend(self.text_with_images(&text, images)?);
                continue;
            }
            if let Some(reference) = reference_in(item, &self.r.assets) {
                return Err(self.nested_reference(reference, &p));
            }
            out.push(self.parts(images).json_value(&RawJson::from(item))?);
        }
        Ok(out)
    }

    fn input(&mut self) -> Result<RawJson, KppError> {
        let r = self.r;
        let user = |content: Vec<RawJson>| -> Result<RawJson, KppError> {
            let message = super::wire::MessageWire {
                ty: None,
                role: "user".to_owned(),
                content: ser(&content)?,
            };
            ser(&vec![ser(&message)?])
        };
        match &r.input {
            DecisionInput::Text(t) => match reference_in_text(t, &r.assets) {
                None => ser(t),
                Some(_) => {
                    let parts = self.text_with_images(t, true)?;
                    user(parts)
                }
            },
            DecisionInput::Json(raw) => {
                if reference_in(raw.as_raw(), &r.assets).is_some() {
                    let parts = self.json_with_images(raw, true, "input")?;
                    return user(parts);
                }
                let part = self.parts(true).json_value(raw)?;
                user(vec![part])
            }
            DecisionInput::Messages(messages) => {
                let items = self.parts(true).items(messages)?;
                ser(&items)
            }
            _ => Err(unsupported(
                "input",
                format!("{} can't take this input.", self.profile.name),
                "input",
            )),
        }
    }

    fn description_text(&mut self, d: &Description, param: &str) -> Result<String, KppError> {
        let text = match d {
            Description::Text(t) => t.clone(),
            Description::Json(raw) => {
                self.note("criteria");
                raw.get().to_owned()
            }
            _ => {
                return Err(unsupported(
                    "description",
                    format!("{} can't take this description.", self.profile.name),
                    param,
                ));
            }
        };
        if let Some(reference) = reference_in_text(&text, &self.r.assets) {
            return Err(unsupported(
                "image in criteria",
                format!(
                    "{} takes no images in options or levels, so the image `{reference}` can't be sent; describe it in text or put it in the instructions.",
                    self.profile.name
                ),
                param,
            ));
        }
        Ok(text)
    }

    fn instructions(&mut self, q: &Question, param: &str) -> Result<Instr, KppError> {
        let images = self.profile.instruction_images;
        let p = format!("{param}.instructions");
        Ok(match &q.instructions {
            None => Instr::None,
            Some(Instructions::Text(t)) => match reference_in_text(t, &self.r.assets) {
                None => Instr::Text(t.clone()),
                Some(_) => Instr::Parts(self.text_with_images(t, images)?),
            },
            Some(Instructions::Json(raw)) => {
                if reference_in(raw.as_raw(), &self.r.assets).is_some() {
                    Instr::Parts(self.json_with_images(raw, images, &p)?)
                } else {
                    self.note("instructions");
                    Instr::Text(raw.get().to_owned())
                }
            }
            Some(Instructions::Blocks(blocks)) => {
                let mut parts = Vec::with_capacity(blocks.len());
                for (i, b) in blocks.iter().enumerate() {
                    let bp = format!("{p}[{i}]");
                    if let Block::Text(t) = b
                        && !t.extras.is_from(DIALECT)
                        && reference_in_text(&t.text, &self.r.assets).is_some()
                    {
                        parts.extend(self.text_with_images(&t.text, images)?);
                        continue;
                    }
                    parts.push(self.parts(images).part(b, Place::Instructions, &bp)?);
                }
                Instr::Parts(parts)
            }
            Some(_) => {
                return Err(unsupported(
                    "instructions",
                    format!("{} can't take these instructions.", self.profile.name),
                    p,
                ));
            }
        })
    }

    fn question(&mut self, i: usize, q: &Question) -> Result<(RawJson, Vec<String>), KppError> {
        let param = format!("questions[{i}]");
        let same = q.extras.is_from(DIALECT);
        let name = self.profile.name;
        let mut instructions = self.instructions(q, &param)?;
        let mut values = Vec::new();
        let (ty, choices, levels) = match &q.kind {
            QuestionKind::Predicate(p) => {
                let mut lines = Vec::new();
                for (lead, d) in [(WHEN_TRUE, &p.when_true), (WHEN_FALSE, &p.when_false)] {
                    if let Some(d) = d {
                        let text = self.description_text(d, &format!("{param}.criteria"))?;
                        lines.push(format!("{lead}{text}"));
                    }
                }
                if !lines.is_empty() {
                    self.note("criteria");
                    instructions =
                        instructions.with_criteria(&lines.join("\n"), &mut self.parts(true))?;
                }
                ("predicate", None, None)
            }
            QuestionKind::Choice(c) => {
                let n = c.options.len();
                if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&n) {
                    return Err(unsupported(
                        "options",
                        format!(
                            "{name} takes {MIN_OPTIONS} to {MAX_OPTIONS} options per choice question; this one has {n}."
                        ),
                        param,
                    ));
                }
                let mut out = Vec::with_capacity(n);
                for (oi, o) in c.options.iter().enumerate() {
                    let p = format!("{param}.options[{oi}]");
                    let value = match &o.value {
                        ChoiceValue::Text(t) => ser(t)?,
                        ChoiceValue::Bool(b) => ser(b)?,
                        _ => {
                            return Err(unsupported(
                                "choice value",
                                format!("{name} takes string and boolean values."),
                                p,
                            ));
                        }
                    };
                    values.push(value_key(&o.value).unwrap_or_default());
                    let description = o
                        .description
                        .as_ref()
                        .map(|d| self.description_text(d, &p))
                        .transpose()?;
                    let w = ChoiceWire { value, description };
                    out.push(ser(&Ext::with(
                        w,
                        extra_for(&o.extras, DIALECT),
                        verbatim_from_form(&o.extras, DIALECT, ""),
                    ))?);
                }
                ("choice", Some(ser(&out)?), None)
            }
            QuestionKind::Score(s) => {
                let n = s.levels.len();
                if !(MIN_LEVELS..=MAX_LEVELS).contains(&n) {
                    return Err(unsupported(
                        "levels",
                        format!(
                            "{name} takes {MIN_LEVELS} to {MAX_LEVELS} levels per score question; this one has {n}."
                        ),
                        param,
                    ));
                }
                let mut out = Vec::with_capacity(n);
                for (li, l) in s.levels.iter().enumerate() {
                    let p = format!("{param}.levels[{li}]");
                    let description = l
                        .description
                        .as_ref()
                        .map(|d| self.description_text(d, &p))
                        .transpose()?;
                    let w = match (&l.label, description) {
                        (Some(label), description) => LevelWire {
                            label: label.clone(),
                            description,
                        },
                        (None, Some(text)) => LevelWire {
                            label: text,
                            description: None,
                        },
                        (None, None) => LevelWire {
                            label: String::new(),
                            description: None,
                        },
                    };
                    out.push(ser(&Ext::with(
                        w,
                        extra_for(&l.extras, DIALECT),
                        verbatim_from_form(&l.extras, DIALECT, ""),
                    ))?);
                }
                ("score", None, Some(ser(&out)?))
            }
            QuestionKind::Box(_) => {
                return Err(unsupported(
                    "box question",
                    format!(
                        "{name} answers no box questions; send them to a System One provider that does, such as TypeSafe."
                    ),
                    param,
                ));
            }
            _ => {
                return Err(unsupported(
                    "question type",
                    format!("{name} can't take this question type."),
                    param,
                ));
            }
        };
        let instructions = match instructions {
            Instr::None if same => None,
            Instr::None => Some(ser(&"")?),
            Instr::Text(t) => Some(ser(&t)?),
            Instr::Parts(parts) => Some(ser(&parts)?),
        };
        let w = QuestionWire {
            ty: ty.to_owned(),
            name: q.name.clone(),
            instructions,
            choices,
            levels,
        };
        let json = ser(&Ext::with(
            w,
            extra_for(&q.extras, DIALECT),
            verbatim_from_form(&q.extras, DIALECT, ""),
        ))?;
        Ok((json, values))
    }
}

/// A question's instructions on their way out.
enum Instr {
    None,
    Text(String),
    Parts(Vec<RawJson>),
}

impl Instr {
    fn with_criteria(self, lines: &str, parts: &mut PartWriter<'_>) -> Result<Instr, KppError> {
        Ok(match self {
            Instr::None => Instr::Text(lines.to_owned()),
            Instr::Text(t) if t.is_empty() => Instr::Text(lines.to_owned()),
            Instr::Text(t) => Instr::Text(format!("{t}\n\n{lines}")),
            Instr::Parts(mut p) => {
                p.push(parts.text_part(lines)?);
                Instr::Parts(p)
            }
        })
    }
}

fn build(r: &DecideRequest, ctx: &EgressCtx, dry: bool) -> Result<Built, KppError> {
    let profile = ctx.decisions_profile;
    if let Some(max) = profile.max_questions
        && r.questions.len() > max
    {
        return Err(unsupported(
            "questions",
            format!(
                "{} takes at most {max} questions per request; this one has {}.",
                profile.name,
                r.questions.len()
            ),
            "questions",
        ));
    }
    let same = r.extras.is_from(DIALECT);
    let mut w = Writer {
        r,
        profile,
        dry,
        adapted: Vec::new(),
    };
    let input = w.input()?;
    let mut questions = Vec::with_capacity(r.questions.len());
    let mut sent = Vec::with_capacity(r.questions.len());
    for (i, q) in r.questions.iter().enumerate() {
        let (json, values) = w.question(i, q)?;
        sent.push(SentQuestion {
            key: name_key(q),
            kind: sent_kind(q, values),
        });
        questions.push(json);
    }
    let cache = r.cache.as_ref();
    let options = cache.filter(|c| {
        c.mode.is_some()
            || c.ttl.is_some()
            || (same && r.extras.form_flag(DIALECT, FORM_CACHE_OPTIONS))
    });
    let prompt_cache_options = match options {
        Some(c) => Some(ser(&CacheOptionsWire {
            mode: c.mode.clone(),
            ttl: c.ttl.as_ref().and_then(ttl_text),
        })?),
        None => None,
    };
    let out = RequestWire {
        model: r.model.clone(),
        input,
        questions: ser(&questions)?,
        prompt_cache_key: cache.and_then(|c| c.key.clone()),
        prompt_cache_options,
        safety_identifier: r.user.clone(),
    };
    let body = to_body(&Ext::with(
        out,
        extra_for(&r.extras, DIALECT),
        verbatim_from_form(&r.extras, DIALECT, ""),
    ))?;
    let path = match &ctx.base_path {
        Some(base) => base.clone(),
        None => under_api_root(ctx, profile.path),
    };
    Ok(Built {
        body,
        path,
        sent,
        adapted: w.adapted,
    })
}
