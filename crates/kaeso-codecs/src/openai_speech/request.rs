use indexmap::IndexMap;
use kaeso_kpp::{
    AudioFormat, KppError, Origin, RawJson, Request, SpeechOperation, SpeechRequest, StreamMode,
    Voice,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::DIALECT;
use super::profile::{SpeechBody, SpeechEvents, SpeechProfile};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::check_extras;
use crate::wire::{
    Ext, extra_for, extras_from, note_integers, spelled_number, to_body, verbatim_from_form,
};

const FORM_STREAM_FORMAT: &str = "stream_format";
const FORM_DEFAULT_FORMAT: &str = "default_format";

// `CreateSpeechRequest` at openai-openapi 926a7b8, with the fields other providers' forms of
// it add (Together's `sample_rate` and `language`, Groq's and Mistral's `seed`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct SpeechRequestW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

const FIELDS: &[&str] = &[
    "model",
    "input",
    "instructions",
    "voice",
    "response_format",
    "speed",
    "stream_format",
    "sample_rate",
    "language",
    "seed",
];

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        DIALECT.display_name()
    ))
}

fn refused(p: &SpeechProfile, feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        DIALECT,
        format!("{} does not take {feature}; {hint}", p.provider),
    )
    .with_param(param.to_owned())
}

/// The KPP format of a `response_format` value: OpenAI's `pcm` is 16-bit PCM at 24 kHz.
pub(crate) fn format_of(wire: &str) -> AudioFormat {
    match wire {
        "pcm" => AudioFormat::Pcm16,
        other => AudioFormat::from_wire(other),
    }
}

/// The `response_format` value of a KPP format.
pub(crate) fn wire_format(format: &AudioFormat) -> &str {
    match format {
        AudioFormat::Pcm16 => "pcm",
        other => other.as_wire(),
    }
}

/// The media type OpenAI answers a `response_format` with.
pub(crate) fn media_type_of(format: Option<&AudioFormat>) -> &'static str {
    match format {
        None | Some(AudioFormat::Mp3) => "audio/mpeg",
        Some(AudioFormat::Opus) => "audio/opus",
        Some(AudioFormat::Aac) => "audio/aac",
        Some(AudioFormat::Flac) => "audio/flac",
        Some(AudioFormat::Wav) => "audio/wav",
        Some(AudioFormat::Pcm16) => "audio/pcm",
        Some(_) => "application/octet-stream",
    }
}

// serde names a missing or unknown field but not one of the wrong type, so each typed field
// is tried on its own.
fn field_of_wrong_type(body: &[u8]) -> Option<&'static str> {
    let Ok(Value::Object(m)) = serde_json::from_slice::<Value>(body) else {
        return None;
    };
    FIELDS.iter().copied().find(|f| {
        let Some(v) = m.get(*f) else { return false };
        let one = Value::Object(Map::from_iter([((*f).to_owned(), v.clone())]));
        serde_json::from_value::<SpeechRequestW>(one).is_err()
    })
}

pub(super) fn decode(body: &[u8], headers: &HeaderList) -> Result<Request, KppError> {
    let origin = Origin::new(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<SpeechRequestW>::from_slice(body).map_err(|e| {
        let text = crate::wire::without_position(&e);
        let field = FIELDS
            .iter()
            .find(|f| text.contains(&format!("`{f}`")))
            .copied()
            .or_else(|| field_of_wrong_type(body));
        let err = invalid(&text);
        match field {
            Some(f) => err.with_param(f),
            None => err,
        }
    })?;
    let mut req = SpeechRequest::new(w.model.clone().unwrap_or_default(), SpeechOperation::Speech);
    let mut extras = extras_from(&origin, extra, verbatim);
    req.input = w.input.clone();
    req.instructions = w.instructions.clone();
    req.voice = match &w.voice {
        None => None,
        Some(Value::String(name)) => Some(Voice::Named(name.clone())),
        Some(Value::Object(o)) if o.len() == 1 => match o.get("id") {
            Some(Value::String(id)) => Some(Voice::Id(id.clone())),
            _ => return Err(invalid("`voice.id` must be a string").with_param("voice")),
        },
        Some(_) => {
            return Err(
                invalid("`voice` must be a voice name or `{\"id\": …}`").with_param("voice")
            );
        }
    };
    // OpenAI answers mp3 when no format is named; other providers need to be asked for it.
    req.format = Some(match &w.response_format {
        Some(f) => format_of(f),
        None => {
            extras
                .form
                .insert(FORM_DEFAULT_FORMAT.to_owned(), true.into());
            AudioFormat::Mp3
        }
    });
    req.speed = w.speed.as_ref().and_then(serde_json::Number::as_f64);
    note_integers(&mut extras, &[("speed", w.speed.as_ref())]);
    req.stream = match w.stream_format.as_deref() {
        None => StreamMode::Off,
        Some("sse") => StreamMode::on(false),
        Some("audio") => {
            extras
                .form
                .insert(FORM_STREAM_FORMAT.to_owned(), "audio".into());
            StreamMode::Off
        }
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`stream_format` must be `sse` or `audio`, not `{other}`."
            ))
            .with_param("stream_format"));
        }
    };
    req.sample_rate = w.sample_rate;
    req.language = w.language.clone();
    req.seed = w.seed;
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            extras.insert_header(name, value);
        }
    }
    req.extras = extras;
    Ok(Request::Speech(req))
}

/// What the provider will be sent: whether it streams events, and the format it answers in.
pub(super) struct Plan {
    pub sse: bool,
}

pub(super) fn plan(r: &SpeechRequest, p: &SpeechProfile) -> Result<Plan, KppError> {
    if r.operation != SpeechOperation::Speech {
        return Err(refused(
            p,
            r.operation.as_str(),
            "model",
            "send this request to a provider that makes it.",
        ));
    }
    if r.dialogue.len() + usize::from(r.audio.is_some()) > 0 {
        return Err(refused(
            p,
            "dialogue or source audio",
            "input",
            "send text.",
        ));
    }
    match &r.voice {
        None => {}
        Some(Voice::Named(name)) => {
            if let Some(list) = p.voices
                && !crate::media::has_voice(list, name)
            {
                return Err(refused(
                    p,
                    &format!("the voice `{name}`"),
                    "voice",
                    "use one of its voices.",
                ));
            }
        }
        Some(Voice::Id(_)) if p.voice_ids => {}
        Some(Voice::Id(id)) => {
            return Err(refused(
                p,
                &format!("the voice id `{id}`"),
                "voice",
                "name one of its voices.",
            ));
        }
        Some(_) => return Err(refused(p, "this voice", "voice", "name one of its voices.")),
    }
    if let Some(f) = &r.format {
        let wire = wire_format(f);
        if !p.formats.contains(&wire) {
            return Err(refused(
                p,
                &format!("the format `{wire}`"),
                "response_format",
                &format!("use one of {}.", p.formats.join(", ")),
            ));
        }
    }
    let typed = [
        ("instructions", r.instructions.is_some(), p.instructions),
        ("speed", r.speed.is_some(), p.speed),
        (
            "sample_rate",
            r.sample_rate.is_some(),
            p.takes("sample_rate"),
        ),
        ("language", r.language.is_some(), p.takes("language")),
        ("seed", r.seed.is_some(), p.takes("seed")),
        ("bitrate", r.bitrate.is_some(), false),
        ("timestamps", r.timestamps, false),
        ("duration", r.duration.is_some(), false),
    ];
    for (field, set, takes) in typed {
        if set && !takes {
            return Err(refused(p, &format!("`{field}`"), field, "remove it."));
        }
    }
    check_extras(&r.extras, DIALECT, "", "request")?;
    if matches!(p.body, SpeechBody::Xai | SpeechBody::AiSdk)
        && let Some(key) = r.extras.fields.keys().next()
    {
        return Err(refused(p, &format!("`{key}`"), key, "remove it."));
    }
    // A provider that reports usage only in its events is asked for events even when the app
    // wants the audio itself; the app still gets the audio as it arrives.
    let events = p.events.is_some() && !p.sse_refused.iter().any(|m| r.model.starts_with(m));
    if !r.stream.is_off() && p.events.is_some() && !events {
        return Err(refused(
            p,
            &format!("`stream_format: sse` for `{}`", r.model),
            "stream_format",
            "ask for `audio`.",
        ));
    }
    Ok(Plan {
        sse: events && (!r.stream.is_off() || p.usage_in_events),
    })
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Speech(r) = req else {
        return Err(KppError::unsupported_by(
            format!("{:?} request", req.kind()),
            DIALECT,
            "Send speech requests to `/v1/audio/speech`.",
        ));
    };
    plan(r, &ctx.speech_profile).map(|_| ())
}

/// Encodes `req` for the provider `ctx` describes; events it can't stream are asked for as
/// audio, and `ctx.stream` turns off so that the reply is read as audio.
pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Speech(r) = req else {
        return Err(KppError::unsupported_by("generation request", DIALECT, ""));
    };
    let p = ctx.speech_profile;
    let plan = plan(r, &p)?;
    ctx.stream = if plan.sse {
        StreamMode::on(true)
    } else {
        StreamMode::Off
    };
    if ctx.audio_format.is_none() {
        ctx.audio_format.clone_from(&r.format);
    }
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if p.body == SpeechBody::AiSdk {
        // Vercel AI Gateway's speech protocol takes the model and its versions as headers.
        headers.push("ai-gateway-protocol-version", "0.0.1");
        headers.push("ai-speech-model-specification-version", "4");
        headers.push("ai-model-id", r.model.clone());
    }
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{}", p.path),
        None => p.path.to_owned(),
    };
    Ok(EncodedRequest::post_json(
        path,
        headers,
        to_body(&body(r, &p, plan.sse)?)?,
    ))
}

fn body(r: &SpeechRequest, p: &SpeechProfile, sse: bool) -> Result<Map<String, Value>, KppError> {
    match p.body {
        SpeechBody::Xai => return Ok(xai_body(r)),
        SpeechBody::AiSdk => return Ok(ai_sdk_body(r)),
        _ => {}
    }
    let mut out = Map::new();
    let mut put = |k: &str, v: Value| {
        out.insert(k.to_owned(), v);
    };
    put("model", r.model.clone().into());
    if let Some(input) = &r.input {
        put("input", input.clone().into());
    }
    if let Some(i) = &r.instructions {
        put("instructions", i.clone().into());
    }
    match (&r.voice, p.body) {
        (Some(v), SpeechBody::VoiceId) => put("voice_id", v.as_str().into()),
        (Some(Voice::Named(name)), _) => put("voice", name.clone().into()),
        (Some(Voice::Id(id)), _) => put("voice", serde_json::json!({ "id": id })),
        _ => {}
    }
    // An app that named no format expects OpenAI's mp3; a provider with another default is
    // asked for mp3 by name.
    let defaulted = r.extras.form_flag(DIALECT, FORM_DEFAULT_FORMAT) && p.default_format == "mp3";
    if let Some(f) = &r.format
        && !defaulted
    {
        put("response_format", wire_format(f).into());
    }
    if let Some(n) = spelled_number(r.speed, "speed", &r.extras, DIALECT) {
        put("speed", Value::Number(n));
    }
    match (sse, p.events) {
        (true, Some(SpeechEvents::StreamFlag(_))) => put("stream", true.into()),
        (true, _) => put("stream_format", "sse".into()),
        (false, Some(SpeechEvents::StreamFormat))
            if r.extras.form_str(DIALECT, FORM_STREAM_FORMAT) == Some("audio") =>
        {
            put("stream_format", "audio".into());
        }
        _ => {}
    }
    if let Some(rate) = r.sample_rate {
        put("sample_rate", rate.into());
    }
    if let Some(l) = &r.language {
        put("language", l.clone().into());
    }
    if let Some(seed) = r.seed {
        put("seed", seed.into());
    }
    let extra: IndexMap<String, RawJson> = extra_for(&r.extras, DIALECT);
    for (k, v) in extra {
        let value = v
            .to_value()
            .map_err(|_| invalid(format!("`{k}` cannot be written")))?;
        out.entry(k).or_insert(value);
    }
    for (k, v) in verbatim_from_form(&r.extras, DIALECT, "") {
        if let Ok(value) = v.to_value() {
            out.insert(k, value);
        }
    }
    Ok(out)
}

// SpaceX's `POST /v1/tts` requires `language`; `auto` is its documented value for detecting it.
fn xai_body(r: &SpeechRequest) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("text".into(), r.input.clone().unwrap_or_default().into());
    if let Some(v) = &r.voice {
        out.insert("voice_id".into(), v.as_str().into());
    }
    out.insert(
        "language".into(),
        r.language
            .clone()
            .unwrap_or_else(|| "auto".to_owned())
            .into(),
    );
    let mut format = Map::new();
    if let Some(f) = &r.format {
        format.insert("codec".into(), wire_format(f).into());
    }
    if let Some(rate) = r.sample_rate {
        format.insert("sample_rate".into(), rate.into());
    }
    if !format.is_empty() {
        out.insert("output_format".into(), Value::Object(format));
    }
    if let Some(n) = spelled_number(r.speed, "speed", &r.extras, DIALECT) {
        out.insert("speed".into(), Value::Number(n));
    }
    out
}

fn ai_sdk_body(r: &SpeechRequest) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("text".into(), r.input.clone().unwrap_or_default().into());
    if let Some(v) = &r.voice {
        out.insert("voice".into(), v.as_str().into());
    }
    if let Some(f) = &r.format {
        out.insert("outputFormat".into(), wire_format(f).into());
    }
    if let Some(i) = &r.instructions {
        out.insert("instructions".into(), i.clone().into());
    }
    if let Some(n) = spelled_number(r.speed, "speed", &r.extras, DIALECT) {
        out.insert("speed".into(), Value::Number(n));
    }
    if let Some(l) = &r.language {
        out.insert("language".into(), l.clone().into());
    }
    out
}
