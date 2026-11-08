use base64::Engine as _;
use indexmap::IndexMap;
use kaeso_kpp::{
    AudioInput, Extras, KppError, Origin, RawJson, Request, Source, StreamMode,
    TimestampGranularity, TranscriptFormat, TranscriptionOperation, TranscriptionRequest,
};
use serde_json::Map;

use super::DIALECT;
use super::profile::{LanguageField, TimestampsField, TranscriptionBody, TranscriptionsProfile};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::check_extras;
use crate::multipart::{FormError, FormField, FormLimits, FormPart, form_boundary, read_form};
use crate::wire::{extra_for, note_integers, spelled_number};

/// The most parts one upload may hold: the file and every field of `CreateTranscriptionRequest`,
/// with lists of speaker references, languages and keywords.
pub const MAX_FORM_PARTS: usize = 64;

/// The most bytes one uploaded audio file may hold. OpenAI takes 25 MB
/// (speech-to-text guide); Groq's paid tier 100 MB; the gateway's `body_limits.audio` bounds
/// the whole body below this.
pub const MAX_AUDIO_BYTES: usize = 100 * 1024 * 1024;

const FORM_TIMESTAMPS: &str = "timestamps_field";
const FORM_URL: &str = "url_field";
const FORM_STREAM_OFF: &str = "stream_off";

// Fields that take a list, written `name[]` by OpenAI's SDKs.
const LIST_FIELDS: &[&str] = &[
    "include",
    "known_speaker_names",
    "known_speaker_references",
    "languages",
    "keywords",
];

const TYPED: &[&str] = &[
    "model",
    "language",
    "prompt",
    "response_format",
    "temperature",
    "stream",
    "diarize",
    "file_url",
    "url",
];

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        DIALECT.display_name()
    ))
}

fn refused(p: &TranscriptionsProfile, feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        DIALECT,
        format!("{} does not take {feature}; {hint}", p.provider),
    )
    .with_param(param.to_owned())
}

fn form_error(e: FormError) -> KppError {
    match e {
        FormError::PartTooLarge { name, limit } => {
            KppError::invalid_request(format!("The upload `{name}` is larger than {limit} bytes."))
                .with_status(413)
                .with_param(name)
        }
        FormError::TooManyParts(n) => {
            KppError::invalid_request(format!("The upload has more than {n} parts."))
        }
        other => invalid(other),
    }
}

/// Decodes a transcription or translation upload; the path names the operation.
pub(super) fn decode(
    operation: TranscriptionOperation,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let content_type = headers.get("content-type").unwrap_or_default();
    if !content_type
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("multipart/")
    {
        return Err(invalid(
            "send the audio as a `multipart/form-data` upload with a `file` field",
        ));
    }
    let boundary = form_boundary(content_type).map_err(form_error)?;
    let parts = read_form(
        body,
        &boundary,
        FormLimits::new(MAX_FORM_PARTS, MAX_AUDIO_BYTES),
    )
    .map_err(form_error)?;
    let mut extras = Extras::new(Origin::new(DIALECT));
    let mut text: IndexMap<String, String> = IndexMap::new();
    let mut nested = Map::new();
    let mut audio: Option<AudioInput> = None;
    let mut timestamps = Vec::new();
    for part in &parts {
        let name = part.name.as_str();
        if name == "file" {
            if part.filename.is_none() {
                return Err(invalid("`file` must be a file").with_param("file"));
            }
            if audio.replace(uploaded(part)).is_some() {
                return Err(invalid("`file` is sent more than once").with_param("file"));
            }
            continue;
        }
        if part.filename.is_some() {
            return Err(KppError::invalid_request(format!(
                "Kaeso does not know the file field `{name}`; send the audio as `file`."
            ))
            .with_param(name.to_owned()));
        }
        let value = part
            .text()
            .ok_or_else(|| invalid(format!("`{name}` is not UTF-8 text")).with_param(name))?;
        if matches!(
            name,
            "timestamp_granularities[]" | "timestamp_granularities"
        ) {
            extras
                .form
                .entry(FORM_TIMESTAMPS.to_owned())
                .or_insert_with(|| name.into());
            timestamps.push(TimestampGranularity::from_wire(value));
        } else if TYPED.contains(&name) {
            if text.insert(name.to_owned(), value.to_owned()).is_some() {
                return Err(invalid(format!("`{name}` is sent more than once")).with_param(name));
            }
        } else {
            let key = match LIST_FIELDS.iter().find(|f| **f == name) {
                Some(f) => format!("{f}[]"),
                None => name.to_owned(),
            };
            crate::multipart::insert_nested(&mut nested, &key, value, &[]).map_err(invalid)?;
        }
    }
    let audio = match (audio, text.get("file_url"), text.get("url")) {
        (Some(a), None, None) => a,
        (None, Some(url), None) | (None, None, Some(url)) => {
            let field = if text.contains_key("file_url") {
                "file_url"
            } else {
                "url"
            };
            extras.form.insert(FORM_URL.to_owned(), field.into());
            AudioInput::new(Source::Url(url.clone()))
        }
        (None, None, None) => return Err(invalid("`file` is missing").with_param("file")),
        _ => {
            return Err(
                invalid("send either `file` or a link to the audio, not both").with_param("file"),
            );
        }
    };
    let mut req = TranscriptionRequest::new(
        text.get("model").cloned().unwrap_or_default(),
        operation,
        audio,
    );
    req.language = text.get("language").cloned();
    req.prompt = text.get("prompt").cloned();
    req.format = text
        .get("response_format")
        .map(|f| TranscriptFormat::from_wire(f));
    if let Some(t) = text.get("temperature") {
        let n: serde_json::Number = t.trim().parse().map_err(|_| {
            KppError::invalid_request(format!("`temperature` must be a number, not `{t}`."))
                .with_param("temperature")
        })?;
        req.temperature = n.as_f64();
        note_integers(&mut extras, &[("temperature", Some(&n))]);
    }
    req.timestamps = timestamps;
    req.stream = match text.get("stream").map(|s| s.trim()) {
        None => StreamMode::Off,
        Some("true") => StreamMode::on(true),
        Some("false") => {
            extras.form.insert(FORM_STREAM_OFF.to_owned(), true.into());
            StreamMode::Off
        }
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`stream` must be `true` or `false`, not `{other}`."
            ))
            .with_param("stream"));
        }
    };
    req.diarize = match text.get("diarize").map(|s| s.trim()) {
        None => None,
        Some("true") => Some(true),
        Some("false") => Some(false),
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`diarize` must be `true` or `false`, not `{other}`."
            ))
            .with_param("diarize"));
        }
    };
    for (key, value) in nested {
        let raw = RawJson::from_serialize(&value).map_err(|_| invalid("a field cannot be read"))?;
        extras.fields.insert(key, raw);
    }
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            extras.insert_header(name, value);
        }
    }
    req.extras = extras;
    Ok(Request::Transcription(req))
}

fn uploaded(part: &FormPart<'_>) -> AudioInput {
    let media_type = part
        .content_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let data = base64::engine::general_purpose::STANDARD.encode(part.body);
    let mut input = AudioInput::new(Source::base64(media_type, data));
    input.filename = part.filename.clone();
    input
}

/// What the provider will be sent.
pub(super) struct Plan {
    pub path: &'static str,
    /// The `response_format` to ask the provider for, when it differs from the app's or the
    /// app named one.
    pub format: Option<TranscriptFormat>,
    pub stream: bool,
}

/// The form a provider is asked for when it lacks the app's: subtitles and segments are made
/// from `verbose_json`, plain text from `json`.
pub(crate) fn provider_format(
    app: &TranscriptFormat,
    has: impl Fn(&str) -> bool,
) -> Option<TranscriptFormat> {
    if has(app.as_wire()) {
        return Some(app.clone());
    }
    match app {
        TranscriptFormat::Srt | TranscriptFormat::Vtt | TranscriptFormat::VerboseJson
            if has("verbose_json") =>
        {
            Some(TranscriptFormat::VerboseJson)
        }
        TranscriptFormat::Text | TranscriptFormat::Json if has("json") => {
            Some(TranscriptFormat::Json)
        }
        TranscriptFormat::Json if has("verbose_json") => Some(TranscriptFormat::VerboseJson),
        _ => None,
    }
}

pub(super) fn plan(r: &TranscriptionRequest, p: &TranscriptionsProfile) -> Result<Plan, KppError> {
    let path = match r.operation {
        TranscriptionOperation::Transcribe => p.transcriptions_path,
        TranscriptionOperation::Translate => p.translations_path.ok_or_else(|| {
            refused(
                p,
                "translations",
                "model",
                "transcribe the audio and translate the text instead.",
            )
        })?,
        _ => {
            return Err(refused(
                p,
                "this operation",
                "model",
                "transcribe the audio.",
            ));
        }
    };
    let app = r.format_or_default();
    let format = provider_format(&app, |f| p.has_format(f)).ok_or_else(|| {
        refused(
            p,
            &format!("`response_format: {}`", app.as_wire()),
            "response_format",
            &format!("use one of {}.", p.formats.join(", ")),
        )
    })?;
    match &r.audio.source {
        Source::Base64(_) => {}
        Source::Url(_) if p.url_field.is_some() => {}
        Source::FileId(_) if p.file_id_field.is_some() => {}
        Source::Url(_) => return Err(refused(p, "links to audio", "file", "upload the file.")),
        _ => return Err(refused(p, "file ids", "file", "upload the file.")),
    }
    if !r.timestamps.is_empty() && p.timestamps == TimestampsField::None {
        return Err(refused(
            p,
            "`timestamp_granularities`",
            "timestamp_granularities",
            "remove it.",
        ));
    }
    if r.timestamps.contains(&TimestampGranularity::Word) && !p.words {
        return Err(refused(
            p,
            "word timestamps",
            "timestamp_granularities",
            "ask for `segment` timestamps.",
        ));
    }
    match (p.language, &r.language) {
        (LanguageField::Required, None) => {
            return Err(refused(
                p,
                "requests without `language`",
                "language",
                "name the language spoken.",
            ));
        }
        (LanguageField::None, Some(_)) => {
            return Err(refused(p, "`language`", "language", "remove it."));
        }
        _ => {}
    }
    if r.temperature.is_some() && !p.temperature {
        return Err(refused(p, "`temperature`", "temperature", "remove it."));
    }
    if r.prompt.is_some() && !p.prompt {
        return Err(refused(p, "`prompt`", "prompt", "remove it."));
    }
    if r.diarize.is_some() && !p.takes("diarize") {
        return Err(refused(
            p,
            "`diarize`",
            "diarize",
            "use a diarizing model or remove it.",
        ));
    }
    if r.speakers.is_some() {
        return Err(refused(p, "a speaker count", "num_speakers", "remove it."));
    }
    if p.body == TranscriptionBody::AiSdk
        && let Some(key) = r.extras.fields.keys().next()
    {
        return Err(refused(p, &format!("`{key}`"), key, "remove it."));
    }
    check_extras(&r.extras, DIALECT, "", "request")?;
    let asked = (r.format.is_some() || format != app).then_some(format);
    Ok(Plan {
        path,
        format: asked,
        stream: !r.stream.is_off() && p.streams,
    })
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Transcription(r) = req else {
        return Err(KppError::unsupported_by(
            format!("{:?} request", req.kind()),
            DIALECT,
            "Send transcriptions to `/v1/audio/transcriptions`.",
        ));
    };
    plan(r, &ctx.transcriptions_profile).map(|_| ())
}

/// Encodes `req` as an upload for the provider `ctx` describes. A stream it can't send is
/// asked for whole and `ctx.stream` turns off; `ctx.transcript_format` is the form asked for.
pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Transcription(r) = req else {
        return Err(KppError::unsupported_by("generation request", DIALECT, ""));
    };
    let p = ctx.transcriptions_profile;
    let plan = plan(r, &p)?;
    if !plan.stream {
        ctx.stream = StreamMode::Off;
    }
    ctx.transcript_format = Some(plan.format.clone().unwrap_or(TranscriptFormat::Json));
    if p.body == TranscriptionBody::AiSdk {
        return ai_sdk(r, ctx, &plan);
    }
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, v: String| texts.push((k.to_owned(), v));
    push("model", r.model.clone());
    match &r.audio.source {
        Source::Url(url) => {
            let field = r
                .extras
                .form_str(DIALECT, FORM_URL)
                .filter(|f| Some(*f) == p.url_field)
                .or(p.url_field)
                .unwrap_or("url");
            push(field, url.clone());
        }
        Source::FileId(f) => push(p.file_id_field.unwrap_or("file_id"), f.id.clone()),
        _ => {}
    }
    if let Some(l) = &r.language
        && p.language != LanguageField::Header
    {
        push("language", l.clone());
    }
    if let Some(prompt) = &r.prompt {
        push("prompt", prompt.clone());
    }
    if let Some(f) = &plan.format
        && p.format_field
    {
        push("response_format", f.as_wire().to_owned());
    }
    if let Some(n) = spelled_number(r.temperature, "temperature", &r.extras, DIALECT) {
        push("temperature", n.to_string());
    }
    let mut timestamps = r.timestamps.clone();
    // Subtitles and segments from a provider that lacks the app's form need its timed segments.
    if plan.format == Some(TranscriptFormat::VerboseJson)
        && r.format != Some(TranscriptFormat::VerboseJson)
        && !timestamps.contains(&TimestampGranularity::Segment)
    {
        timestamps.push(TimestampGranularity::Segment);
    }
    match p.timestamps {
        TimestampsField::Granularities(own) => {
            let unbracketed = r.extras.form_str(DIALECT, FORM_TIMESTAMPS)
                == Some("timestamp_granularities")
                && own == "timestamp_granularities[]";
            let field = if unbracketed {
                "timestamp_granularities"
            } else {
                own
            };
            for t in &timestamps {
                push(field, t.as_wire().to_owned());
            }
        }
        TimestampsField::Level => {
            let level = if timestamps.contains(&TimestampGranularity::Word) {
                Some("word")
            } else if !timestamps.is_empty() {
                Some("sentence")
            } else {
                None
            };
            if let Some(level) = level {
                push("timestamp_level", level.to_owned());
            }
        }
        _ => {}
    }
    if plan.stream {
        push("stream", "true".to_owned());
    } else if r.extras.form_flag(DIALECT, FORM_STREAM_OFF) {
        push("stream", "false".to_owned());
    }
    if let Some(d) = r.diarize {
        push("diarize", d.to_string());
    }
    for (k, v) in extra_for(&r.extras, DIALECT) {
        let value = v
            .to_value()
            .map_err(|_| invalid(format!("`{k}` cannot be written")))?;
        crate::multipart::flatten(&k, &value, &mut texts);
    }
    let file = match &r.audio.source {
        Source::Base64(b) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&b.data)
                .map_err(|_| invalid("the audio is not valid base64").with_param("file"))?;
            let name = r
                .audio
                .filename
                .clone()
                .unwrap_or_else(|| default_name(&b.media_type));
            Some((name, b.media_type.clone(), bytes))
        }
        _ => None,
    };
    let mut fields: Vec<FormField<'_>> = texts.iter().map(|(k, v)| FormField::Text(k, v)).collect();
    if let Some((name, media, bytes)) = &file {
        fields.push(FormField::File("file", name, media, bytes));
    }
    let w = crate::multipart::FormWriter::write(&fields);
    let mut headers = HeaderList::new().with("content-type", w.content_type());
    if let (LanguageField::Header, Some(l)) = (p.language, &r.language) {
        headers.push("language", l.clone());
    }
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{}", plan.path),
        None => plan.path.to_owned(),
    };
    Ok(EncodedRequest::post_json(path, headers, w.into_body()))
}

// Vercel AI Gateway's transcription protocol: base64 audio in JSON, the model in a header.
fn ai_sdk(
    r: &TranscriptionRequest,
    ctx: &EgressCtx,
    plan: &Plan,
) -> Result<EncodedRequest, KppError> {
    let Source::Base64(b) = &r.audio.source else {
        return Err(invalid("the audio must be an upload").with_param("file"));
    };
    let body = serde_json::json!({ "audio": b.data, "mediaType": b.media_type });
    let headers = HeaderList::new()
        .with("content-type", "application/json")
        .with("ai-gateway-protocol-version", "0.0.1")
        .with("ai-transcription-model-specification-version", "4")
        .with("ai-model-id", r.model.clone());
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{}", plan.path),
        None => plan.path.to_owned(),
    };
    Ok(EncodedRequest::post_json(
        path,
        headers,
        crate::wire::to_body(&body)?,
    ))
}

/// A file name for audio that came without one, from its media type.
pub(crate) fn default_name(media_type: &str) -> String {
    let ext = match crate::media::essence(media_type).as_str() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/ogg" => "ogg",
        "audio/webm" | "video/webm" => "webm",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "video/mp4" => "mp4",
        "audio/opus" => "opus",
        _ => "bin",
    };
    format!("audio.{ext}")
}

#[cfg(test)]
pub(super) fn parts_of(body: &[u8], content_type: &str) -> Vec<(String, serde_json::Value)> {
    let boundary = form_boundary(content_type).unwrap();
    read_form(
        body,
        &boundary,
        FormLimits::new(MAX_FORM_PARTS, MAX_AUDIO_BYTES),
    )
    .unwrap()
    .iter()
    .map(|p| {
        let value = match &p.filename {
            Some(f) => serde_json::json!({
                "filename": f,
                "type": p.content_type,
                "bytes": p.body.len(),
            }),
            None => serde_json::Value::String(p.text().unwrap_or_default().to_owned()),
        };
        (p.name.clone(), value)
    })
    .collect()
}
