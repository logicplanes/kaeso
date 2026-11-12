use indexmap::IndexMap;
use kaeso_kpp::{
    Asset, Block, Choice, ChoiceOption, ChoiceValue, DecideRequest, DecisionInput, Description,
    Extras, Instructions, KppError, Level, Origin, Predicate, Question, QuestionKind, RawJson,
    Request, RequestKind, Role, Score, Source,
};
use serde_json::value::RawValue;

use super::DIALECT;
use super::assets::{
    STATE_PART, check_image, data_url, fresh_reference, image_part, quoted, raw, rewrite, strings,
    take_state_images,
};
use super::profile::{MediaPlacement, ModelField, SystemOneProfile};
use super::wire::{AssetWire, NoulCriteriaWire, QuestionWire, RequestIn, RequestOut};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header, under_api_root};
use crate::cross::check_foreign;
use crate::decisions::{SentQuestion, option_key, question_keys, sent_kind};
use crate::image_size::image_info;
use crate::multipart::{
    FormError, FormField, FormLimits, FormPart, FormWriter, Upload, form_boundary, read_form,
};
use crate::wire::{
    B64, Ext, entries_of, extra_for, extras_from, to_body, verbatim_from_form, without_position,
};

/// Marks a request an app uploaded as `multipart/form-data`.
pub(super) const FORM_MULTIPART: &str = "multipart";
const FORM_CRITERIA_OBJECT: &str = "criteria_object";
const FORM_NULL: &str = "null";
const FORM_NO_TYPE: &str = "no_type";
const FORM_FORMAT: &str = "format";
const FORM_PART: &str = "part";
const FORM_PART_TYPE: &str = "part_type";
const FORM_FILENAME: &str = "filename";
const DATA_BASE64: &str = "data_base64";
const IMAGES_HEADER: &str = "typesafe-danger-zone-images";

/// The most bytes one part of an uploaded System One form may hold.
pub(super) const MAX_PART_BYTES: usize = 64 * 1024 * 1024;

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn not_valid(detail: &str) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        DIALECT.display_name()
    ))
}

fn other_kind(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        DIALECT,
        "System One answers decision questions only; send other requests to their own endpoints.",
    )
}

pub(super) fn decode(
    kind: RequestKind,
    upload: Upload<'_>,
    headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind != RequestKind::Decide {
        return Err(other_kind(kind));
    }
    let multipart = headers.get("content-type").is_some_and(|ct| {
        ct.trim_start()
            .to_ascii_lowercase()
            .starts_with("multipart/")
    });
    let r = if multipart {
        decode_multipart(upload, headers)?
    } else {
        decode_json(upload.bytes(), headers, None)?
    };
    Ok(Request::Decide(r))
}

struct Files<'a> {
    upload: Upload<'a>,
    parts: Vec<FormPart<'a>>,
    used: Vec<bool>,
}

fn form_error(e: FormError) -> KppError {
    match e {
        FormError::PartTooLarge { .. } => KppError::new(
            kaeso_kpp::ErrorKind::RequestTooLarge,
            format!("The upload is too large: {e}."),
        )
        .with_status(413),
        other => KppError::invalid_request(format!(
            "The request is not a valid System One upload: {other}."
        )),
    }
}

fn decode_multipart(upload: Upload<'_>, headers: &HeaderList) -> Result<DecideRequest, KppError> {
    let content_type = headers.get("content-type").unwrap_or_default();
    let boundary = form_boundary(content_type).map_err(form_error)?;
    let parts = read_form(
        upload.bytes(),
        &boundary,
        FormLimits::new(crate::limits::MAX_DECISION_ASSETS + 1, MAX_PART_BYTES),
    )
    .map_err(form_error)?;
    let Some((first, files)) = parts.split_first() else {
        return Err(KppError::invalid_request(
            "The upload has no parts; send the JSON request as the first part, named `request`.",
        ));
    };
    if first.name != "request" {
        return Err(KppError::invalid_request(
            "The first part must be named `request` and hold the JSON request.",
        ));
    }
    let mut names = std::collections::HashSet::new();
    for part in files {
        if part.name.is_empty() || part.name == "request" || !names.insert(part.name.as_str()) {
            return Err(KppError::invalid_request(format!(
                "Every image part needs a unique, non-empty name other than `request`; `{}` is not one.",
                part.name
            )));
        }
        let allowed = part.content_type.as_deref().is_none_or(|t| {
            let essence = t
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            matches!(
                essence.as_str(),
                "image/png" | "image/jpeg" | "image/jpg" | "application/octet-stream"
            )
        });
        if !allowed {
            return Err(KppError::invalid_request(format!(
                "The part `{}` must be `image/png`, `image/jpeg` or `application/octet-stream`.",
                part.name
            )));
        }
    }
    crate::limits::request_shape(DIALECT, first.body)?;
    let mut files = Files {
        upload,
        parts: files.to_vec(),
        used: vec![false; files.len()],
    };
    let mut r = decode_json(first.body, headers, Some(&mut files))?;
    if let Some(unused) = files
        .parts
        .iter()
        .zip(&files.used)
        .find(|(_, used)| !**used)
    {
        return Err(KppError::invalid_request(format!(
            "No asset uses the part `{}`; name it in an asset's `part` or remove it.",
            unused.0.name
        )));
    }
    r.extras.form.insert(FORM_MULTIPART.to_owned(), true.into());
    Ok(r)
}

fn parse_error(e: &serde_json::Error) -> KppError {
    let text = without_position(e);
    let field = ["model", "state", "questions", "assets"]
        .into_iter()
        .find(|f| text.contains(&format!("`{f}`")));
    let err = not_valid(&text);
    match field {
        Some(f) => err.with_param(f),
        None => err,
    }
}

fn decode_json(
    body: &[u8],
    headers: &HeaderList,
    mut files: Option<&mut Files<'_>>,
) -> Result<DecideRequest, KppError> {
    let origin = Origin::new(DIALECT);
    let w = Ext::<RequestIn>::from_slice(body).map_err(|e| parse_error(&e))?;
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut assets = IndexMap::new();
    if let Some(raw) = &w.assets {
        let entries = entries_of(raw.as_raw()).ok_or_else(|| {
            invalid(
                "`assets` must be an object of images keyed by their reference.",
                "assets",
            )
        })?;
        for (reference, value) in entries {
            let asset = decode_asset(&reference, value, files.as_deref_mut(), &origin)?;
            assets.insert(reference.into_owned(), asset);
        }
    }
    let text = std::str::from_utf8(body).unwrap_or_default();
    let state = match take_state_images(w.state.as_raw(), text, &mut assets)? {
        Some(new) => raw(new)?,
        None => w.state,
    };
    let input = match state.parse::<String>() {
        Ok(s) if state.get().starts_with('"') => DecisionInput::Text(s),
        _ => DecisionInput::Json(state),
    };
    let entries = entries_of(w.questions.as_raw()).ok_or_else(|| {
        invalid(
            "`questions` must be an object of named questions.",
            "questions",
        )
    })?;
    let mut req = DecideRequest::new(w.model, input);
    for (key, value) in entries {
        req.questions.push(decode_question(&key, value, &origin)?);
    }
    req.assets = assets;
    let mut extras = extras_from(&origin, extra, verbatim);
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            extras.insert_header(name, value);
        }
    }
    req.extras = extras;
    Ok(req)
}

fn media_of_format(format: &str) -> Option<&'static str> {
    match format {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}

fn decode_asset(
    reference: &str,
    value: &RawValue,
    files: Option<&mut Files<'_>>,
    origin: &Origin,
) -> Result<Asset, KppError> {
    let param = format!("assets.{reference}");
    let w = Ext::<AssetWire>::from_raw(value).map_err(|e| {
        invalid(
            format!(
                "The asset `{reference}` is not valid: {}.",
                without_position(&e)
            ),
            &param,
        )
    })?;
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut extras = extras_from(origin, extra, verbatim);
    match w.ty.as_deref() {
        Some("image") => {}
        Some(other) => {
            return Err(invalid(
                format!(
                    "The asset `{reference}` has the type `{other}`; System One assets are images."
                ),
                format!("{param}.type"),
            ));
        }
        None => {
            extras.form.insert(FORM_NO_TYPE.to_owned(), true.into());
        }
    }
    let declared = match w.format.as_deref() {
        Some(f) => {
            extras.form.insert(FORM_FORMAT.to_owned(), f.into());
            Some(media_of_format(f).ok_or_else(|| {
                invalid(
                    format!(
                        "The asset `{reference}` has the format `{f}`; use `png`, `jpg` or `jpeg`."
                    ),
                    format!("{param}.format"),
                )
            })?)
        }
        None => None,
    };
    let source = match (w.part, files) {
        (Some(name), Some(files)) => {
            let Some(i) = files.parts.iter().position(|p| p.name == name) else {
                return Err(invalid(
                    format!(
                        "The asset `{reference}` names the part `{name}`, which the upload doesn't have."
                    ),
                    format!("{param}.part"),
                ));
            };
            if files.used.get(i).copied().unwrap_or(true) {
                return Err(invalid(
                    format!(
                        "The part `{name}` is used by more than one asset; upload it once per asset."
                    ),
                    format!("{param}.part"),
                ));
            }
            if let Some(used) = files.used.get_mut(i) {
                *used = true;
            }
            let part = files
                .parts
                .get(i)
                .cloned()
                .ok_or_else(|| invalid("The part is missing.", &param))?;
            let bytes = files.upload.share(part.body);
            let sniffed = image_info(&bytes).map(|info| info.kind.media_type());
            let from_type = part
                .content_type
                .as_deref()
                .map(|t| {
                    t.split(';')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase()
                })
                .and_then(|t| match t.as_str() {
                    "image/png" => Some("image/png"),
                    "image/jpeg" | "image/jpg" => Some("image/jpeg"),
                    _ => None,
                });
            let media = declared
                .or(from_type)
                .or(sniffed)
                .unwrap_or("application/octet-stream");
            extras.form.insert(FORM_PART.to_owned(), name.into());
            if let Some(t) = &part.content_type {
                extras
                    .form
                    .insert(FORM_PART_TYPE.to_owned(), t.clone().into());
            }
            if let Some(f) = &part.filename {
                extras
                    .form
                    .insert(FORM_FILENAME.to_owned(), f.clone().into());
            }
            if let Some(ignored) = &w.data_base64 {
                let text = match ignored {
                    B64::Text(t) => t.clone(),
                    B64::Bytes(_) => String::new(),
                };
                extras.fields.insert(
                    DATA_BASE64.to_owned(),
                    RawJson::from_serialize(&text)
                        .map_err(|e| KppError::provider(e.to_string()))?,
                );
            }
            Source::bytes(media, bytes)
        }
        (Some(_), None) => {
            return Err(invalid(
                format!(
                    "The asset `{reference}` names an uploaded part, but the request is not a `multipart/form-data` upload."
                ),
                format!("{param}.part"),
            ));
        }
        (None, _) => {
            let Some(data) = w.data_base64 else {
                return Err(invalid(
                    format!(
                        "The asset `{reference}` needs `data_base64` or, in an upload, `part`."
                    ),
                    &param,
                ));
            };
            let Some(media) = declared else {
                return Err(invalid(
                    format!("The asset `{reference}` needs a `format` with `data_base64`."),
                    format!("{param}.format"),
                ));
            };
            data.into_source(media, &format!("{param}.data_base64"))?
        }
    };
    let mut asset = Asset::new(source);
    asset.extras = extras;
    Ok(asset)
}

fn description_of(raw: RawJson) -> Option<Description> {
    if raw.get() == "null" {
        return None;
    }
    match raw.parse::<String>() {
        Ok(s) if raw.get().starts_with('"') => Some(Description::Text(s)),
        _ => Some(Description::Json(raw)),
    }
}

fn instructions_of(raw: RawJson) -> Instructions {
    match raw.parse::<String>() {
        Ok(s) if raw.get().starts_with('"') => Instructions::Text(s),
        _ => Instructions::Json(raw),
    }
}

fn decode_question(key: &str, value: &RawValue, origin: &Origin) -> Result<Question, KppError> {
    let param = format!("questions.{key}");
    let w = Ext::<QuestionWire>::from_raw(value).map_err(|e| {
        invalid(
            format!(
                "The question `{key}` is not valid: {}.",
                without_position(&e)
            ),
            &param,
        )
    })?;
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut extras = extras_from(origin, extra, verbatim);
    let criteria_param = format!("{param}.criteria");
    let kind = match w.ty.as_str() {
        "noul" => {
            let mut p = Predicate::default();
            if let Some(criteria) = w.criteria {
                let c = Ext::<NoulCriteriaWire>::from_raw(criteria.as_raw()).map_err(|_| {
                    invalid("The criteria of a `noul` question must be an object with `true` and `false`.", &criteria_param)
                })?;
                p.when_true = c.known.when_true.and_then(description_of);
                p.when_false = c.known.when_false.and_then(description_of);
                p.extras = extras_from(origin, c.extra, c.verbatim);
                extras
                    .form
                    .insert(FORM_CRITERIA_OBJECT.to_owned(), true.into());
            }
            QuestionKind::Predicate(p)
        }
        "choice" => {
            let criteria = w
                .criteria
                .ok_or_else(|| invalid("A `choice` question needs `criteria`.", &criteria_param))?;
            let entries = entries_of(criteria.as_raw()).ok_or_else(|| {
                invalid(
                    "The criteria of a `choice` question must be an object of options.",
                    &criteria_param,
                )
            })?;
            let options = entries
                .into_iter()
                .map(|(name, value)| {
                    let raw = RawJson::from(value);
                    let mut option = ChoiceOption::new(ChoiceValue::Text(name.into_owned()), None);
                    if raw.get() == "null" {
                        option.extras =
                            Extras::new(origin.clone()).with_form(FORM_NULL, true.into());
                    } else {
                        option.description = description_of(raw);
                    }
                    option
                })
                .collect();
            QuestionKind::Choice(Choice::new(options))
        }
        "score" => {
            let criteria = w
                .criteria
                .ok_or_else(|| invalid("A `score` question needs `criteria`.", &criteria_param))?;
            let items = criteria.parse::<Vec<&RawValue>>().map_err(|_| {
                invalid(
                    "The criteria of a `score` question must be an array of levels.",
                    &criteria_param,
                )
            })?;
            let levels = items
                .into_iter()
                .map(|item| Level::new(None, description_of(RawJson::from(item))))
                .collect();
            QuestionKind::Score(Score::new(levels))
        }
        "box" => {
            if let Some(criteria) = w.criteria {
                extras.fields.insert("criteria".to_owned(), criteria);
            }
            QuestionKind::Box(kaeso_kpp::BoxQuestion::new())
        }
        other => {
            return Err(invalid(
                format!(
                    "`{other}` is not a System One question type; use `noul`, `choice`, `score` or `box`."
                ),
                format!("{param}.type"),
            ));
        }
    };
    let mut q = Question::new(Some(key.to_owned()), kind);
    q.instructions = w.instructions.map(instructions_of);
    q.extras = extras;
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
    for note in built.left_out {
        if !ctx.left_out.iter().any(|a| a == note) {
            ctx.left_out.push(note.to_owned());
        }
    }
    ctx.decision_questions = built.sent;
    let mut headers = HeaderList::new().with("content-type", built.content_type);
    if built.images_header {
        headers.push("typesafe-danger-zone-images", "true");
    }
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            let set = built.images_header && name.eq_ignore_ascii_case(IMAGES_HEADER);
            if is_forwarded_header(DIALECT, name) && !set {
                headers.push(name, value);
            }
        }
    }
    Ok(EncodedRequest::post_json(built.path, headers, built.body))
}

struct Built {
    body: Vec<u8>,
    content_type: String,
    path: String,
    images_header: bool,
    sent: Vec<SentQuestion>,
    adapted: Vec<&'static str>,
    left_out: Vec<&'static str>,
}

struct Writer<'r> {
    profile: SystemOneProfile,
    dry: bool,
    haystack: String,
    assets: IndexMap<String, Asset>,
    made: usize,
    adapted: Vec<&'static str>,
    left_out: Vec<&'static str>,
    request: &'r DecideRequest,
}

fn unsupported(feature: &str, message: String) -> KppError {
    KppError::unsupported(feature, message)
}

impl Writer<'_> {
    fn note(list: &mut Vec<&'static str>, what: &'static str) {
        if !list.contains(&what) {
            list.push(what);
        }
    }

    fn new_image(&mut self, block: &kaeso_kpp::ImageBlock) -> String {
        let reference = fresh_reference(&self.haystack, &self.assets, self.made);
        self.made += 1;
        self.haystack.push('\n');
        self.haystack.push_str(&reference);
        let mut asset = Asset::new(block.source.clone());
        asset.extras = block.extras.clone();
        self.assets.insert(reference.clone(), asset);
        reference
    }

    fn blocks(&mut self, blocks: &[Block], param: &str) -> Result<String, KppError> {
        let mut items = Vec::with_capacity(blocks.len());
        for (i, block) in blocks.iter().enumerate() {
            if block.cache().is_some() {
                Self::note(&mut self.left_out, "cache");
            }
            items.push(match block {
                Block::Text(t) => quoted(&t.text),
                Block::Json(j) => j.data.get().to_owned(),
                Block::Image(b) => quoted(&self.new_image(b)),
                other => {
                    return Err(unsupported(
                        &format!("{} part", other.kind()),
                        format!(
                            "System One takes text, JSON and images, not a {} part; remove it.",
                            other.kind()
                        ),
                    )
                    .with_param(format!("{param}[{i}]")));
                }
            });
        }
        Ok(match items.as_slice() {
            [one] => one.clone(),
            _ => format!("[{}]", items.join(",")),
        })
    }

    fn state(&mut self) -> Result<String, KppError> {
        match &self.request.input {
            DecisionInput::Text(t) => Ok(quoted(t)),
            DecisionInput::Json(raw) => Ok(raw.get().to_owned()),
            DecisionInput::Messages(messages) => match messages.as_slice() {
                [m] if m.role == Role::User => self.blocks(&m.blocks, "input"),
                _ => Err(unsupported(
                    "conversation input",
                    "System One takes one state, not a conversation; send one user message."
                        .to_owned(),
                )
                .with_param("input")),
            },
            _ => Err(unsupported(
                "input",
                "System One can't take this input.".to_owned(),
            )),
        }
    }

    fn description(d: Option<&Description>) -> Result<String, KppError> {
        match d {
            Some(Description::Text(t)) => Ok(quoted(t)),
            Some(Description::Json(raw)) => Ok(raw.get().to_owned()),
            None => Ok("null".to_owned()),
            Some(_) => Err(unsupported(
                "description",
                "System One can't take this description.".to_owned(),
            )),
        }
    }

    fn question(&mut self, i: usize, q: &Question) -> Result<(RawJson, Vec<String>), KppError> {
        let profile = self.profile;
        let param = format!("questions[{i}]");
        let same = q.extras.is_from(DIALECT);
        let instructions = match &q.instructions {
            Some(Instructions::Text(t)) => Some(quoted(t)),
            Some(Instructions::Json(raw)) => Some(raw.get().to_owned()),
            Some(Instructions::Blocks(blocks)) => {
                Some(self.blocks(blocks, &format!("{param}.instructions"))?)
            }
            Some(_) => {
                return Err(unsupported(
                    "instructions",
                    "System One can't take these instructions.".to_owned(),
                )
                .with_param(param));
            }
            None => None,
        };
        let mut options = Vec::new();
        let (ty, criteria) = match &q.kind {
            QuestionKind::Predicate(p) => {
                let marked = same && q.extras.form_flag(DIALECT, FORM_CRITERIA_OBJECT);
                let any = p.when_true.is_some() || p.when_false.is_some();
                let criteria = if any || marked {
                    let w = NoulCriteriaWire {
                        when_true: p
                            .when_true
                            .as_ref()
                            .map(|d| Self::description(Some(d)).and_then(raw))
                            .transpose()?,
                        when_false: p
                            .when_false
                            .as_ref()
                            .map(|d| Self::description(Some(d)).and_then(raw))
                            .transpose()?,
                    };
                    let ext = Ext::with(
                        w,
                        extra_for(&p.extras, DIALECT),
                        verbatim_from_form(&p.extras, DIALECT, ""),
                    );
                    Some(
                        RawJson::from_serialize(&ext)
                            .map_err(|e| KppError::provider(e.to_string()))?,
                    )
                } else {
                    None
                };
                ("noul", criteria)
            }
            QuestionKind::Choice(c) => {
                if let Some(max) = profile.max_options
                    && c.options.len() > max
                {
                    return Err(unsupported(
                        "options",
                        format!(
                            "{} takes at most {max} options per choice question; this one has {}.",
                            profile.name,
                            c.options.len()
                        ),
                    )
                    .with_param(param));
                }
                let mut map: IndexMap<String, RawJson> = IndexMap::new();
                for option in &c.options {
                    if matches!(option.value, ChoiceValue::Bool(_)) {
                        Self::note(&mut self.adapted, "booleans");
                    }
                    let key = option_key(&option.value).ok_or_else(|| {
                        unsupported(
                            "choice value",
                            "System One takes text option names.".to_owned(),
                        )
                        .with_param(param.clone())
                    })?;
                    if map.contains_key(&key) {
                        return Err(unsupported(
                            "repeated choice value",
                            format!("System One names options by text, so `{key}` would name two options; give every option its own value."),
                        )
                        .with_param(param));
                    }
                    map.insert(
                        key.clone(),
                        raw(Self::description(option.description.as_ref())?)?,
                    );
                    options.push(key);
                }
                (
                    "choice",
                    Some(
                        RawJson::from_serialize(&map)
                            .map_err(|e| KppError::provider(e.to_string()))?,
                    ),
                )
            }
            QuestionKind::Score(s) => {
                if let Some(max) = profile.max_levels
                    && s.levels.len() > max
                {
                    return Err(unsupported(
                        "levels",
                        format!(
                            "{} takes at most {max} levels per score question; this one has {}.",
                            profile.name,
                            s.levels.len()
                        ),
                    )
                    .with_param(param));
                }
                let mut items = Vec::with_capacity(s.levels.len());
                for level in &s.levels {
                    items.push(match (&level.label, &level.description) {
                        (None, d @ Some(_)) => Self::description(d.as_ref())?,
                        (Some(label), None) => quoted(label),
                        (Some(label), Some(Description::Text(text))) => {
                            Self::note(&mut self.adapted, "levels");
                            quoted(&format!("{label}: {text}"))
                        }
                        _ => {
                            return Err(unsupported(
                                "score level",
                                "System One writes each level as one value; give every level a text label or description.".to_owned(),
                            )
                            .with_param(param));
                        }
                    });
                }
                ("score", Some(raw(format!("[{}]", items.join(",")))?))
            }
            QuestionKind::Box(_) => {
                if !profile.box_questions {
                    return Err(unsupported(
                        "box question",
                        format!(
                            "{} answers no box questions; send them to TypeSafe's Jev 1.14.",
                            profile.name
                        ),
                    )
                    .with_param(param));
                }
                ("box", None)
            }
            _ => {
                return Err(unsupported(
                    "question type",
                    "System One can't ask this question.".to_owned(),
                )
                .with_param(param));
            }
        };
        let w = QuestionWire {
            ty: ty.to_owned(),
            instructions: instructions.map(raw).transpose()?,
            criteria,
        };
        let ext = Ext::with(
            w,
            extra_for(&q.extras, DIALECT),
            verbatim_from_form(&q.extras, DIALECT, ""),
        );
        let json = RawJson::from_serialize(&ext).map_err(|e| KppError::provider(e.to_string()))?;
        Ok((json, options))
    }

    fn asset_json(&self, asset: &Asset, multipart: bool) -> Result<RawJson, KppError> {
        let same = asset.extras.is_from(DIALECT);
        let media = asset.source.media_type().unwrap_or_default();
        let format = match asset.extras.form_str(DIALECT, FORM_FORMAT) {
            Some(f) if same => Some(f.to_owned()),
            _ => match media {
                "image/png" => Some("png".to_owned()),
                "image/jpeg" => Some("jpg".to_owned()),
                other => other.strip_prefix("image/").map(str::to_owned),
            },
        };
        let part = asset
            .extras
            .form_str(DIALECT, FORM_PART)
            .filter(|_| same && multipart);
        let data = match (&asset.source, part) {
            (_, Some(_)) => None,
            (Source::Base64(_), None) if self.dry => Some(B64::Text(String::new())),
            (Source::Base64(b), None) => Some(B64::Bytes(b.data.clone())),
            _ => {
                return Err(unsupported(
                    "image URL",
                    format!(
                        "{} takes images only as bytes, and Kaeso doesn't fetch image URLs; send the image as a data URL or base64.",
                        self.profile.name
                    ),
                ));
            }
        };
        let w = AssetWire {
            ty: (!(same && asset.extras.form_flag(DIALECT, FORM_NO_TYPE)))
                .then(|| "image".to_owned()),
            format: if part.is_some()
                && !(same && asset.extras.form_str(DIALECT, FORM_FORMAT).is_some())
            {
                None
            } else {
                format
            },
            data_base64: data,
            part: part.map(str::to_owned),
        };
        let extra = extra_for(&asset.extras, DIALECT)
            .into_iter()
            .filter(|(k, _)| part.is_some() || k != DATA_BASE64)
            .collect();
        let ext = Ext::with(w, extra, verbatim_from_form(&asset.extras, DIALECT, ""));
        RawJson::from_serialize(&ext).map_err(|e| KppError::provider(e.to_string()))
    }
}

fn place(
    state: String,
    questions: &[(String, RawJson)],
    w: &mut Writer<'_>,
) -> Result<(String, Option<RawJson>, Option<RawJson>, usize), KppError> {
    let profile = w.profile;
    if w.assets.is_empty() {
        return Ok((state, None, None, 0));
    }
    for (reference, asset) in &w.assets {
        check_image(&asset.source, &profile, &format!("assets.{reference}"))?;
    }
    let as_parts: Vec<String> = w
        .assets
        .iter()
        .filter(|(_, a)| match profile.media {
            MediaPlacement::State | MediaPlacement::Conversation => true,
            MediaPlacement::StateOrAssets => {
                a.extras.is_from(DIALECT) && a.extras.form_flag(DIALECT, STATE_PART)
            }
            _ => false,
        })
        .map(|(k, _)| k.clone())
        .collect();
    let elsewhere = |refs: &[String], state: &str| -> Result<(), KppError> {
        let state = raw(state.to_owned())?;
        let mut texts = Vec::new();
        strings(&state, &mut texts);
        for (_, q) in questions {
            strings(q, &mut texts);
        }
        if let Some(r) = refs
            .iter()
            .find(|r| texts.iter().any(|t| t.contains(r.as_str())))
        {
            return Err(unsupported(
                "image reference",
                format!(
                    "{} takes images only as whole values of the state, but the image `{r}` is referenced elsewhere; write it as a value of its own in the state.",
                    profile.name
                ),
            )
            .with_param(format!("assets.{r}")));
        }
        Ok(())
    };
    let count_in = |text: &str, refs: &[String]| -> Result<usize, KppError> {
        let mut texts = Vec::new();
        strings(&raw(text.to_owned())?, &mut texts);
        Ok(texts.iter().filter(|t| refs.contains(t)).count())
    };
    match profile.media {
        MediaPlacement::None => Err(unsupported(
            "images",
            format!(
                "{} takes no images in decision requests; send text only.",
                profile.name
            ),
        )
        .with_param("assets")),
        MediaPlacement::Assets => {
            let multipart =
                profile.multipart && w.request.extras.form_flag(DIALECT, FORM_MULTIPART);
            let mut map: IndexMap<String, RawJson> = IndexMap::new();
            for (k, a) in &w.assets {
                map.insert(k.clone(), w.asset_json(a, multipart)?);
            }
            let images = w.assets.len();
            Ok((
                state,
                Some(RawJson::from_serialize(&map).map_err(|e| KppError::provider(e.to_string()))?),
                None,
                images,
            ))
        }
        MediaPlacement::State | MediaPlacement::StateOrAssets | MediaPlacement::Conversation => {
            let parts_in = count_in(&state, &as_parts)?;
            let state_raw: Box<RawValue> = RawValue::from_string(state.clone())
                .map_err(|e| KppError::provider(e.to_string()))?;
            let conversation = profile.media == MediaPlacement::Conversation
                && !as_parts.is_empty()
                && crate::wire::field_of(&state_raw, "messages").is_none();
            let new_state = if conversation {
                conversation_state(&state_raw, &as_parts, w)?
            } else {
                let dry = w.dry;
                let assets = &w.assets;
                rewrite(&state_raw, &mut |v| {
                    let Ok(s) = serde_json::from_str::<String>(v.get()) else {
                        return Ok(None);
                    };
                    if !v.get().starts_with('"') || !as_parts.contains(&s) {
                        return Ok(None);
                    }
                    let Some(asset) = assets.get(&s) else {
                        return Ok(None);
                    };
                    image_part(asset, dry).map(Some)
                })?
                .unwrap_or(state)
            };
            elsewhere(&as_parts, &new_state)?;
            let rest: Vec<(&String, &Asset)> = w
                .assets
                .iter()
                .filter(|(k, _)| !as_parts.contains(k))
                .collect();
            let assets = if rest.is_empty() {
                None
            } else {
                let mut map: IndexMap<String, RawJson> = IndexMap::new();
                for (k, a) in &rest {
                    map.insert((*k).clone(), w.asset_json(a, false)?);
                }
                Some(RawJson::from_serialize(&map).map_err(|e| KppError::provider(e.to_string()))?)
            };
            Ok((new_state, assets, None, parts_in + rest.len()))
        }
        MediaPlacement::Root => {
            let refs: Vec<String> = w.assets.keys().cloned().collect();
            let items: Vec<&RawValue> = serde_json::from_str(&state).map_err(|_| {
                unsupported(
                    "images in the state",
                    format!("{} takes images in a list of its own, so each image must be a whole item of a state array.", profile.name),
                )
                .with_param("input")
            })?;
            let mut kept = Vec::new();
            let mut images = Vec::new();
            for item in items {
                let reference = serde_json::from_str::<String>(item.get())
                    .ok()
                    .filter(|s| item.get().starts_with('"') && refs.contains(s));
                match reference.and_then(|r| w.assets.get(&r)) {
                    Some(asset) => images.push(quoted(&data_url(asset, w.dry)?)),
                    None => kept.push(item.get().to_owned()),
                }
            }
            let new_state = format!("[{}]", kept.join(","));
            elsewhere(&refs, &new_state)?;
            let count = images.len();
            Ok((
                new_state,
                None,
                Some(raw(format!("[{}]", images.join(",")))?),
                count,
            ))
        }
    }
}

// Baseten reads images only in a user message under `state.messages`: a state of text and image
// references becomes that message, each string a text part and each image an image part.
fn conversation_state(
    state: &RawValue,
    refs: &[String],
    w: &Writer<'_>,
) -> Result<String, KppError> {
    let refuse = || {
        unsupported(
            "images in structured state",
            format!(
                "{} takes images only in a user message under `state.messages`; send the state as text and images, or as `{{\"messages\": …}}`.",
                w.profile.name
            ),
        )
        .with_param("input")
    };
    let items: Vec<String> = if state.get().starts_with('"') {
        vec![serde_json::from_str(state.get()).map_err(|_| refuse())?]
    } else {
        serde_json::from_str::<Vec<String>>(state.get()).map_err(|_| refuse())?
    };
    let mut parts = Vec::with_capacity(items.len());
    for item in &items {
        match w.assets.get(item).filter(|_| refs.contains(item)) {
            Some(asset) => parts.push(image_part(asset, w.dry)?),
            None => parts.push(format!("{{\"type\":\"text\",\"text\":{}}}", quoted(item))),
        }
    }
    Ok(format!(
        "{{\"messages\":[{{\"role\":\"user\",\"content\":[{}]}}]}}",
        parts.join(",")
    ))
}

fn build(r: &DecideRequest, ctx: &EgressCtx, dry: bool) -> Result<Built, KppError> {
    let profile = ctx.system_one_profile;
    if r.extras.is_from(DIALECT)
        && !profile.unknown_fields
        && let Some((key, _)) = r.extras.body_fields().next()
    {
        return Err(unsupported(
            key,
            format!(
                "{} refuses fields it doesn't document; remove `{key}`.",
                profile.name
            ),
        )
        .with_param(key));
    }
    if let Some(max) = profile.max_questions
        && r.questions.len() > max
    {
        return Err(unsupported(
            "questions",
            format!("{} takes at most {max} questions per request; this one has {}. Split them across requests.", profile.name, r.questions.len()),
        )
        .with_param("questions"));
    }
    let (keys, renamed) = question_keys(&r.questions, |k| profile.question_ids.allows(k));
    let mut haystack = String::new();
    let mut texts = Vec::new();
    match &r.input {
        DecisionInput::Text(t) => texts.push(t.clone()),
        DecisionInput::Json(j) => strings(j, &mut texts),
        _ => {}
    }
    for q in &r.questions {
        if let Some(Instructions::Json(j)) = &q.instructions {
            strings(j, &mut texts);
        } else if let Some(Instructions::Text(t)) = &q.instructions {
            texts.push(t.clone());
        }
    }
    for t in texts.iter().chain(r.assets.keys()) {
        haystack.push_str(t);
        haystack.push('\n');
    }
    let mut w = Writer {
        profile,
        dry,
        haystack,
        assets: r.assets.clone(),
        made: 0,
        adapted: Vec::new(),
        left_out: Vec::new(),
        request: r,
    };
    if renamed {
        Writer::note(&mut w.adapted, "names");
    }
    if r.cache.is_some() {
        Writer::note(&mut w.left_out, "cache");
    }
    if r.user.is_some() {
        Writer::note(&mut w.left_out, "user");
    }
    let state = w.state()?;
    let mut questions = Vec::with_capacity(r.questions.len());
    let mut sent = Vec::with_capacity(r.questions.len());
    for (i, (q, key)) in r.questions.iter().zip(&keys).enumerate() {
        let (json, options) = w.question(i, q)?;
        sent.push(SentQuestion {
            key: key.clone(),
            kind: sent_kind(q, options),
        });
        questions.push((key.clone(), json));
    }
    let (state, assets, images, count) = place(state, &questions, &mut w)?;
    if let Some(max) = profile.max_images
        && count > max
    {
        return Err(unsupported(
            "images",
            format!(
                "{} takes at most {max} images per request; this one shows {count}.",
                profile.name
            ),
        )
        .with_param("assets"));
    }
    let has_box = r
        .questions
        .iter()
        .any(|q| matches!(q.kind, QuestionKind::Box(_)));
    let images_header = profile.images_header && (assets.is_some() || has_box);
    let model = match profile.model_field {
        ModelField::ShortName(prefix) => {
            r.model.strip_prefix(prefix).unwrap_or(&r.model).to_owned()
        }
        _ => r.model.clone(),
    };
    let path = match &ctx.base_path {
        Some(base) => base.clone(),
        None => under_api_root(ctx, &profile.path.replace("{model}", &model)),
    };
    let questions_map: IndexMap<String, RawJson> = questions.into_iter().collect();
    let out = RequestOut {
        model,
        state: raw(state)?,
        questions: RawJson::from_serialize(&questions_map)
            .map_err(|e| KppError::provider(e.to_string()))?,
        assets,
        images,
    };
    let ext = Ext::with(
        out,
        extra_for(&r.extras, DIALECT),
        verbatim_from_form(&r.extras, DIALECT, ""),
    );
    let json = to_body(&ext)?;
    let multipart = profile.multipart
        && profile.media == MediaPlacement::Assets
        && r.extras.form_flag(DIALECT, FORM_MULTIPART);
    let (body, content_type) = if multipart && !dry {
        multipart_body(&json, &w.assets)?
    } else {
        (json, "application/json".to_owned())
    };
    Ok(Built {
        body,
        content_type,
        path,
        images_header,
        sent,
        adapted: w.adapted,
        left_out: w.left_out,
    })
}

fn multipart_body(
    json: &[u8],
    assets: &IndexMap<String, Asset>,
) -> Result<(Vec<u8>, String), KppError> {
    let files: Vec<(String, String, String, &[u8])> = assets
        .values()
        .filter(|a| a.extras.is_from(DIALECT))
        .filter_map(|a| {
            let name = a.extras.form_str(DIALECT, FORM_PART)?.to_owned();
            let Source::Base64(b) = &a.source else {
                return None;
            };
            let filename = a
                .extras
                .form_str(DIALECT, FORM_FILENAME)
                .unwrap_or(&name)
                .to_owned();
            let media = a
                .extras
                .form_str(DIALECT, FORM_PART_TYPE)
                .unwrap_or(&b.media_type)
                .to_owned();
            Some((name, filename, media, b.data.as_ref()))
        })
        .collect();
    let mut fields = vec![FormField::Typed("request", "application/json", json)];
    for (name, filename, media, bytes) in &files {
        fields.push(FormField::File(name, filename, media, bytes));
    }
    let form = FormWriter::write(&fields);
    let content_type = form.content_type();
    Ok((form.into_body(), content_type))
}
