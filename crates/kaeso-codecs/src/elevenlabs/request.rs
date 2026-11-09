use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, AudioInput, DialogueLine, Extras, KppError, Origin, RawJson, Request, Source,
    SpeechOperation, SpeechRequest, StreamMode, TimestampGranularity, TranscriptFormat,
    TranscriptionOperation, TranscriptionRequest, Voice,
};
use serde_json::{Map, Value};

use super::format;
use super::{D, ElevenLabsCall, ElevenLabsOperation};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_extras;
use crate::multipart::{FormError, FormField, FormLimits, FormPart, form_boundary, read_form};
use crate::wire::{note_integers, spelled_number, to_body};

/// The most bytes one uploaded audio file may hold here: the gateway's body limit bounds a
/// request well below ElevenLabs' own 5 GB.
const MAX_UPLOAD: usize = 512 * 1024 * 1024;

// The models ElevenLabs uses when a request names none, from the OpenAPI document's defaults.
const DEFAULT_MODELS: [(ElevenLabsOperation, &str); 5] = [
    (ElevenLabsOperation::TextToSpeech, "eleven_multilingual_v2"),
    (ElevenLabsOperation::TextToDialogue, "eleven_v3"),
    (
        ElevenLabsOperation::SoundGeneration,
        "eleven_text_to_sound_v2",
    ),
    (ElevenLabsOperation::Music, "music_v1"),
    (ElevenLabsOperation::VoiceChanger, "eleven_english_sts_v2"),
];

const FORM_DEFAULT_MODEL: &str = "default_model";
const FORM_OUTPUT_FORMAT: &str = "output_format";
const FORM_STREAM_PATH: &str = "stream_path";
const FORM_GRANULARITY: &str = "timestamps_granularity";
const FORM_URL_FIELD: &str = "url_field";
const QUERY: &str = "query";

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        D.display_name()
    ))
}

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        D,
        format!("ElevenLabs does not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (k.to_owned(), v.to_owned()),
            None => (p.to_owned(), String::new()),
        })
        .collect()
}

fn operation_of(call: &ElevenLabsCall) -> SpeechOperation {
    match call.operation {
        ElevenLabsOperation::TextToDialogue => SpeechOperation::Dialogue,
        ElevenLabsOperation::SoundGeneration => SpeechOperation::SoundEffect,
        ElevenLabsOperation::Music => SpeechOperation::Music,
        ElevenLabsOperation::VoiceChanger => SpeechOperation::VoiceChange,
        _ => SpeechOperation::Speech,
    }
}

fn take_str(m: &mut Map<String, Value>, key: &str) -> Result<Option<String>, KppError> {
    match m.remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(invalid(format!("`{key}` must be a string")).with_param(key)),
    }
}

fn into_extras(extras: &mut Extras, rest: Map<String, Value>) -> Result<(), KppError> {
    for (k, v) in rest {
        let raw = RawJson::from_serialize(&v).map_err(|_| invalid("a field cannot be read"))?;
        extras.fields.insert(k, raw);
    }
    Ok(())
}

/// Decodes an ElevenLabs-format request: JSON for speech, dialogue, sound effects and music,
/// uploads for the voice changer and speech to text.
pub(super) fn decode(
    call: &ElevenLabsCall,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let mut extras = Extras::new(Origin::new(D));
    let mut query = Map::new();
    let mut output_format = None;
    for (k, v) in query_pairs(&call.query) {
        if k == "output_format" {
            output_format = Some(v);
        } else {
            query.insert(k, Value::String(v));
        }
    }
    if !query.is_empty() {
        let raw =
            RawJson::from_serialize(&query).map_err(|_| invalid("the query cannot be read"))?;
        extras.fields.insert(QUERY.to_owned(), raw);
    }
    if call.operation == ElevenLabsOperation::SpeechToText {
        return decode_transcription(body, headers, extras);
    }
    let mut req = SpeechRequest::new(String::new(), operation_of(call));
    if let Some(of) = &output_format {
        let (f, rate, kbps) = format::parse(of).ok_or_else(|| {
            KppError::invalid_request(format!(
                "`output_format` `{of}` is not one ElevenLabs lists."
            ))
            .with_param("output_format")
        })?;
        req.format = Some(f);
        req.sample_rate = rate;
        req.bitrate = kbps;
        extras
            .form
            .insert(FORM_OUTPUT_FORMAT.to_owned(), of.clone().into());
    }
    req.voice = call.voice_id.clone().map(Voice::Id);
    req.timestamps = call.timestamps;
    if call.stream {
        extras.form.insert(FORM_STREAM_PATH.to_owned(), true.into());
    }
    // Streamed timestamps come as a line of JSON per piece: events, in KPP's terms.
    if call.stream && call.timestamps {
        req.stream = StreamMode::on(false);
    }
    let mut m = if call.operation == ElevenLabsOperation::VoiceChanger {
        decode_voice_change_form(body, headers, &mut req)?
    } else {
        match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(invalid("the body must be a JSON object")),
            Err(e) => return Err(invalid(crate::wire::without_position(&e))),
        }
    };
    req.model = match take_str(&mut m, "model_id")? {
        Some(model) => model,
        None => {
            extras
                .form
                .insert(FORM_DEFAULT_MODEL.to_owned(), true.into());
            DEFAULT_MODELS
                .iter()
                .find(|(op, _)| *op == call.operation)
                .map_or("eleven_multilingual_v2", |(_, m)| m)
                .to_owned()
        }
    };
    match call.operation {
        ElevenLabsOperation::TextToSpeech | ElevenLabsOperation::SoundGeneration => {
            req.input = take_str(&mut m, "text")?;
        }
        ElevenLabsOperation::Music => req.input = take_str(&mut m, "prompt")?,
        ElevenLabsOperation::TextToDialogue => {
            let Some(Value::Array(inputs)) = m.remove("inputs") else {
                return Err(invalid("`inputs` must be a list").with_param("inputs"));
            };
            for (i, item) in inputs.into_iter().enumerate() {
                let Value::Object(mut line) = item else {
                    return Err(invalid("a dialogue line must be an object")
                        .with_param(format!("inputs[{i}]")));
                };
                let text = take_str(&mut line, "text")?.unwrap_or_default();
                let voice = take_str(&mut line, "voice_id")?.map(Voice::Id);
                let mut l = DialogueLine::new(text, voice);
                if !line.is_empty() {
                    let mut e = Extras::new(Origin::new(D));
                    into_extras(&mut e, line)?;
                    l.extras = e;
                }
                req.dialogue.push(l);
            }
        }
        _ => {}
    }
    req.language = take_str(&mut m, "language_code")?;
    match m.remove("seed") {
        None | Some(Value::Null) => {}
        Some(Value::Number(n)) => req.seed = n.as_i64(),
        Some(_) => return Err(invalid("`seed` must be a whole number").with_param("seed")),
    }
    match m.remove("duration_seconds") {
        None => {}
        Some(Value::Number(n)) => {
            req.duration = n.as_f64();
            note_integers(&mut extras, &[("duration_seconds", Some(&n))]);
        }
        Some(Value::Null) => {
            extras.form.insert("duration_null".into(), true.into());
        }
        Some(_) => {
            return Err(
                invalid("`duration_seconds` must be a number").with_param("duration_seconds")
            );
        }
    }
    if let Some(Value::Number(ms)) = m.get("music_length_ms") {
        req.duration = ms.as_f64().map(|ms| ms / 1000.0);
        m.remove("music_length_ms");
    }
    if let Some(Value::Object(settings)) = m.get_mut("voice_settings")
        && let Some(Value::Number(speed)) = settings.remove("speed")
    {
        req.speed = speed.as_f64();
        note_integers(&mut extras, &[("speed", Some(&speed))]);
    }
    into_extras(&mut extras, m)?;
    req.extras = extras;
    Ok(Request::Speech(req))
}

fn form_error(e: FormError) -> KppError {
    match e {
        FormError::PartTooLarge { name, limit } => {
            KppError::invalid_request(format!("The upload `{name}` is larger than {limit} bytes."))
                .with_status(413)
                .with_param(name)
        }
        other => invalid(other),
    }
}

fn form<'a>(body: &'a [u8], headers: &HeaderList) -> Result<Vec<FormPart<'a>>, KppError> {
    let content_type = headers.get("content-type").unwrap_or_default();
    let boundary = form_boundary(content_type).map_err(form_error)?;
    read_form(body, &boundary, FormLimits::new(64, MAX_UPLOAD)).map_err(form_error)
}

fn uploaded(part: &FormPart<'_>) -> AudioInput {
    let media = part
        .content_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let mut input = AudioInput::new(Source::base64(
        media,
        base64::engine::general_purpose::STANDARD.encode(part.body),
    ));
    input.filename = part.filename.clone();
    input
}

// Text fields of a form as JSON values: numbers and booleans as ElevenLabs reads them, and the
// JSON strings it documents (`voice_settings`, `additional_formats`, `webhook_metadata`).
fn form_value(name: &str, text: &str) -> Value {
    const JSON_FIELDS: &[&str] = &["voice_settings", "additional_formats", "webhook_metadata"];
    if JSON_FIELDS.contains(&name)
        && let Ok(v) = serde_json::from_str::<Value>(text)
    {
        return v;
    }
    Value::String(text.to_owned())
}

fn decode_voice_change_form(
    body: &[u8],
    headers: &HeaderList,
    req: &mut SpeechRequest,
) -> Result<Map<String, Value>, KppError> {
    let mut m = Map::new();
    for part in form(body, headers)? {
        if part.name == "audio" && part.filename.is_some() {
            req.audio = Some(uploaded(&part));
            continue;
        }
        let text = part
            .text()
            .ok_or_else(|| invalid(format!("`{}` is not UTF-8 text", part.name)))?;
        match part.name.as_str() {
            "seed" => {
                req.seed =
                    Some(text.trim().parse().map_err(|_| {
                        invalid("`seed` must be a whole number").with_param("seed")
                    })?);
            }
            name => {
                m.insert(name.to_owned(), form_value(name, text));
            }
        }
    }
    if req.audio.is_none() {
        return Err(invalid("`audio` is missing").with_param("audio"));
    }
    Ok(m)
}

fn decode_transcription(
    body: &[u8],
    headers: &HeaderList,
    mut extras: Extras,
) -> Result<Request, KppError> {
    let mut text_fields = Map::new();
    let mut audio = None;
    for part in form(body, headers)? {
        if part.name == "file" && part.filename.is_some() {
            audio = Some(uploaded(&part));
            continue;
        }
        let value = part
            .text()
            .ok_or_else(|| invalid(format!("`{}` is not UTF-8 text", part.name)))?;
        text_fields.insert(part.name.clone(), form_value(&part.name, value));
    }
    let mut m = text_fields;
    let mut text = |k: &str| match m.remove(k) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    };
    let model =
        text("model_id").ok_or_else(|| invalid("`model_id` is missing").with_param("model_id"))?;
    let url = text("source_url")
        .map(|u| ("source_url", u))
        .or_else(|| text("cloud_storage_url").map(|u| ("cloud_storage_url", u)));
    let audio = match (audio, url) {
        (Some(a), None) => a,
        (None, Some((field, u))) => {
            extras.form.insert(FORM_URL_FIELD.to_owned(), field.into());
            AudioInput::new(Source::Url(u))
        }
        (None, None) => return Err(invalid("send `file` or `source_url`").with_param("file")),
        _ => return Err(invalid("send either `file` or `source_url`").with_param("file")),
    };
    let mut r = TranscriptionRequest::new(model, TranscriptionOperation::Transcribe, audio);
    r.language = text("language_code");
    let flag = |v: Option<String>, k: &str| -> Result<Option<bool>, KppError> {
        match v.as_deref() {
            None => Ok(None),
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            Some(o) => {
                Err(invalid(format!("`{k}` must be `true` or `false`, not `{o}`")).with_param(k))
            }
        }
    };
    r.diarize = flag(text("diarize"), "diarize")?;
    if let Some(n) = text("num_speakers") {
        r.speakers = Some(n.trim().parse().map_err(|_| {
            invalid("`num_speakers` must be a whole number").with_param("num_speakers")
        })?);
    }
    if let Some(t) = text("temperature") {
        let n: serde_json::Number = t
            .trim()
            .parse()
            .map_err(|_| invalid("`temperature` must be a number").with_param("temperature"))?;
        r.temperature = n.as_f64();
        note_integers(&mut extras, &[("temperature", Some(&n))]);
    }
    // Words are timed unless the app asks for `none`; that timing is what `verbose_json` is
    // to the other formats.
    let granularity = text("timestamps_granularity");
    r.format = Some(match granularity.as_deref() {
        Some("none") => TranscriptFormat::Json,
        _ => TranscriptFormat::VerboseJson,
    });
    match granularity.as_deref() {
        None => {}
        Some("word") => r.timestamps = vec![TimestampGranularity::Word],
        Some(g) => {
            extras.form.insert(FORM_GRANULARITY.to_owned(), g.into());
        }
    }
    into_extras(&mut extras, m)?;
    r.extras = extras;
    Ok(Request::Transcription(r))
}

pub(super) fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
    match req {
        Request::Speech(r) => plan_speech(r).map(|_| ()),
        Request::Transcription(r) => check_transcription(r),
        other => Err(KppError::unsupported_by(
            format!("{:?} request", other.kind()),
            D,
            "ElevenLabs makes and transcribes audio.",
        )),
    }
}

/// An ElevenLabs voice id: twenty letters and digits, as `GET /v1/voices` lists them. A voice
/// an app names any other way has no ElevenLabs id Kaeso could know without guessing.
fn voice_id(voice: &Voice) -> Option<&str> {
    let s = voice.as_str();
    let id_like = s.len() == 20 && s.bytes().all(|b| b.is_ascii_alphanumeric());
    match voice {
        // The id becomes a path segment, so it is the listing's alphabet and nothing else.
        Voice::Id(id) if is_voice_id(id) => Some(id),
        Voice::Named(_) if id_like => Some(s),
        _ => None,
    }
}

/// Whether `id` has the shape of an ElevenLabs voice id: letters and digits, at most 64.
pub(super) fn is_voice_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

struct SpeechPlan {
    path: String,
    output_format: Option<String>,
    stream: bool,
    timestamps: bool,
}

fn plan_speech(r: &SpeechRequest) -> Result<SpeechPlan, KppError> {
    check_extras(&r.extras, D, "", "request")?;
    let own = r.extras.is_from(D);
    let voice = |v: Option<&Voice>| -> Result<String, KppError> {
        let Some(v) = v else {
            return Err(refused(
                "speech without a voice",
                "voice",
                "name a voice by its id.",
            ));
        };
        voice_id(v).map(str::to_owned).ok_or_else(|| {
            refused(
                &format!("the voice `{}`", v.as_str()),
                "voice",
                "name an ElevenLabs voice by its id, as `GET /v1/voices` lists them.",
            )
        })
    };
    let typed = [
        ("instructions", r.instructions.is_some()),
        (
            "speed",
            r.speed.is_some() && !matches!(r.operation, SpeechOperation::Speech),
        ),
    ];
    if let Some((field, _)) = typed.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{field}`"), field, "remove it."));
    }
    let output_format = match (
        &r.format,
        r.extras.form_str(D, FORM_OUTPUT_FORMAT).filter(|_| own),
    ) {
        (_, Some(raw)) => Some(raw.to_owned()),
        (None, None) => None,
        (Some(f), None) => Some(format::value(f, r.sample_rate, r.bitrate).ok_or_else(|| {
            refused(
                &format!("the format `{}` at that rate", f.as_wire()),
                "response_format",
                "use mp3, opus, wav, pcm, ulaw or alaw at a rate ElevenLabs lists.",
            )
        })?),
    };
    let wav = output_format
        .as_deref()
        .is_some_and(|f| f.starts_with("wav_"));
    // The `/stream` paths send audio as it is made, but take no WAV.
    let stream_path = if own {
        r.extras.form_flag(D, FORM_STREAM_PATH)
    } else {
        !wav
    };
    if stream_path && wav {
        return Err(refused(
            "WAV on its streaming paths",
            "output_format",
            "ask for mp3 or pcm, or use the non-streaming path.",
        ));
    }
    let suffix = |base: String, stream: bool, timestamps: bool| {
        let mut p = base;
        if stream {
            p.push_str("/stream");
        }
        if timestamps {
            p.push_str("/with-timestamps");
        }
        p
    };
    let path = match r.operation {
        SpeechOperation::Speech => {
            if r.dialogue.len() + usize::from(r.audio.is_some()) > 0 {
                return Err(refused(
                    "dialogue or source audio here",
                    "input",
                    "send text.",
                ));
            }
            let v = voice(r.voice.as_ref())?;
            suffix(format!("/v1/text-to-speech/{v}"), stream_path, r.timestamps)
        }
        SpeechOperation::Dialogue => {
            for (i, line) in r.dialogue.iter().enumerate() {
                voice(line.voice.as_ref())
                    .map_err(|e| e.with_param(format!("inputs[{i}].voice_id")))?;
            }
            suffix("/v1/text-to-dialogue".to_owned(), stream_path, r.timestamps)
        }
        SpeechOperation::SoundEffect | SpeechOperation::Music | SpeechOperation::VoiceChange
            if r.timestamps =>
        {
            return Err(refused("timestamps here", "timestamps", "remove them."));
        }
        SpeechOperation::SoundEffect => "/v1/sound-generation".to_owned(),
        SpeechOperation::Music => suffix("/v1/music".to_owned(), stream_path && own, false),
        SpeechOperation::VoiceChange => {
            let v = voice(r.voice.as_ref())?;
            suffix(format!("/v1/voice-changer/{v}"), stream_path, false)
        }
        _ => return Err(refused("this operation", "model", "send another request.")),
    };
    Ok(SpeechPlan {
        path,
        output_format,
        stream: stream_path,
        timestamps: r.timestamps,
    })
}

fn check_transcription(r: &TranscriptionRequest) -> Result<(), KppError> {
    check_extras(&r.extras, D, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return Err(refused(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    if r.prompt.is_some() {
        return Err(refused(
            "`prompt`",
            "prompt",
            "send key terms as `keyterms` or remove it.",
        ));
    }
    if matches!(r.audio.source, Source::FileId(_) | Source::Text(_)) {
        return Err(refused(
            "file ids",
            "file",
            "upload the file or send a link.",
        ));
    }
    if let TranscriptFormat::Other(f) = r.format_or_default() {
        return Err(refused(
            &format!("`response_format: {f}`"),
            "response_format",
            "use json, text, srt, vtt, verbose_json or diarized_json.",
        ));
    }
    Ok(())
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    match req {
        Request::Speech(r) => encode_speech(r, ctx),
        Request::Transcription(r) => encode_transcription(r, ctx),
        other => Err(KppError::unsupported_by(
            format!("{:?} request", other.kind()),
            D,
            "ElevenLabs makes and transcribes audio.",
        )),
    }
}

fn query_string(r: &Extras, output_format: Option<&str>) -> String {
    let mut pairs: Vec<String> = Vec::new();
    if let Some(of) = output_format {
        pairs.push(format!("output_format={of}"));
    }
    if r.is_from(D)
        && let Some(Ok(Value::Object(q))) = r.fields.get(QUERY).map(RawJson::to_value)
    {
        for (k, v) in q {
            if let Value::String(v) = v {
                pairs.push(format!("{k}={v}"));
            }
        }
    }
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

fn put_extras(out: &mut Map<String, Value>, extras: &Extras) -> Result<(), KppError> {
    if !extras.is_from(D) {
        return Ok(());
    }
    for (k, v) in &extras.fields {
        if k == QUERY {
            continue;
        }
        let value = v
            .to_value()
            .map_err(|_| invalid(format!("`{k}` cannot be written")))?;
        out.entry(k.clone()).or_insert(value);
    }
    Ok(())
}

fn encode_speech(r: &SpeechRequest, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let plan = plan_speech(r)?;
    let own = r.extras.is_from(D);
    ctx.speech_reply_json = plan.timestamps && !plan.stream;
    ctx.stream = if plan.timestamps && plan.stream {
        StreamMode::on(false)
    } else {
        StreamMode::Off
    };
    ctx.audio_format = plan
        .output_format
        .as_deref()
        .and_then(format::parse)
        .map(|(f, _, _)| f)
        .or(Some(AudioFormat::Mp3));
    let mut out = Map::new();
    let default_model = own && r.extras.form_flag(D, FORM_DEFAULT_MODEL);
    if !default_model {
        out.insert("model_id".into(), r.model.clone().into());
    }
    match r.operation {
        SpeechOperation::Speech | SpeechOperation::SoundEffect => {
            out.insert("text".into(), r.input.clone().unwrap_or_default().into());
        }
        SpeechOperation::Music => {
            if let Some(p) = &r.input {
                out.insert("prompt".into(), p.clone().into());
            }
        }
        SpeechOperation::Dialogue => {
            let lines: Vec<Value> = r
                .dialogue
                .iter()
                .map(|l| {
                    let mut m = Map::new();
                    m.insert("text".into(), l.text.clone().into());
                    if let Some(id) = l.voice.as_ref().and_then(voice_id) {
                        m.insert("voice_id".into(), id.into());
                    }
                    if l.extras.is_from(D) {
                        for (k, v) in &l.extras.fields {
                            if let Ok(v) = v.to_value() {
                                m.insert(k.clone(), v);
                            }
                        }
                    }
                    Value::Object(m)
                })
                .collect();
            out.insert("inputs".into(), lines.into());
        }
        _ => {}
    }
    if let Some(l) = &r.language {
        out.insert("language_code".into(), l.clone().into());
    }
    if let Some(seed) = r.seed {
        out.insert("seed".into(), seed.into());
    }
    match (r.operation, r.duration) {
        (SpeechOperation::Music, Some(d)) => {
            out.insert(
                "music_length_ms".into(),
                ((d * 1000.0).round() as u64).into(),
            );
        }
        (_, Some(d)) => {
            if let Some(n) = spelled_number(Some(d), "duration_seconds", &r.extras, D) {
                out.insert("duration_seconds".into(), Value::Number(n));
            }
        }
        (_, None) if r.extras.form_flag(D, "duration_null") => {
            out.insert("duration_seconds".into(), Value::Null);
        }
        _ => {}
    }
    put_extras(&mut out, &r.extras)?;
    if let Some(n) = spelled_number(r.speed, "speed", &r.extras, D) {
        let settings = out
            .entry("voice_settings")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(s) = settings {
            s.insert("speed".into(), Value::Number(n));
        }
    }
    let path = format!(
        "{}{}",
        plan.path,
        query_string(&r.extras, plan.output_format.as_deref())
    );
    if r.operation == SpeechOperation::VoiceChange {
        return voice_change_form(r, out, path);
    }
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&out)?,
    ))
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn voice_change_form(
    r: &SpeechRequest,
    out: Map<String, Value>,
    path: String,
) -> Result<EncodedRequest, KppError> {
    let Some(Source::Base64(b)) = r.audio.as_ref().map(|a| &a.source) else {
        return Err(refused("audio by link", "audio", "upload the audio."));
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&b.data)
        .map_err(|_| invalid("the audio is not valid base64").with_param("audio"))?;
    let name = r
        .audio
        .as_ref()
        .and_then(|a| a.filename.clone())
        .unwrap_or_else(|| crate::openai_transcriptions::default_name(&b.media_type));
    let texts: Vec<(String, String)> = out.iter().map(|(k, v)| (k.clone(), text_of(v))).collect();
    let mut fields: Vec<FormField<'_>> = texts.iter().map(|(k, v)| FormField::Text(k, v)).collect();
    fields.push(FormField::File("audio", &name, &b.media_type, &bytes));
    let w = crate::multipart::FormWriter::write(&fields);
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", w.content_type()),
        w.into_body(),
    ))
}

fn encode_transcription(
    r: &TranscriptionRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check_transcription(r)?;
    ctx.stream = StreamMode::Off;
    ctx.transcript_format = Some(r.format_or_default());
    let own = r.extras.is_from(D);
    let mut texts: Vec<(String, String)> = vec![("model_id".into(), r.model.clone())];
    if let Source::Url(u) = &r.audio.source {
        let field = r.extras.form_str(D, FORM_URL_FIELD).unwrap_or("source_url");
        texts.push((field.to_owned(), u.clone()));
    }
    if let Some(l) = &r.language {
        texts.push(("language_code".into(), l.clone()));
    }
    let diarize = r
        .diarize
        .or_else(|| (r.format_or_default() == TranscriptFormat::DiarizedJson).then_some(true));
    if let Some(d) = diarize {
        texts.push(("diarize".into(), d.to_string()));
    }
    if let Some(n) = r.speakers {
        texts.push(("num_speakers".into(), n.to_string()));
    }
    if let Some(n) = spelled_number(r.temperature, "temperature", &r.extras, D) {
        texts.push(("temperature".into(), n.to_string()));
    }
    let granularity = match r.extras.form_str(D, FORM_GRANULARITY).filter(|_| own) {
        Some(g) => Some(g.to_owned()),
        None if r.timestamps.contains(&TimestampGranularity::Word) && own => Some("word".into()),
        None if !own && !r.format_or_default().needs_timing() && r.timestamps.is_empty() => {
            Some("none".into())
        }
        None => None,
    };
    if let Some(g) = granularity {
        texts.push(("timestamps_granularity".into(), g));
    }
    if own {
        for (k, v) in &r.extras.fields {
            if k == QUERY {
                continue;
            }
            if let Ok(v) = v.to_value() {
                texts.push((k.clone(), text_of(&v)));
            }
        }
    }
    let file = match &r.audio.source {
        Source::Base64(b) => Some((
            r.audio
                .filename
                .clone()
                .unwrap_or_else(|| crate::openai_transcriptions::default_name(&b.media_type)),
            b.media_type.clone(),
            base64::engine::general_purpose::STANDARD
                .decode(&b.data)
                .map_err(|_| invalid("the audio is not valid base64").with_param("file"))?,
        )),
        _ => None,
    };
    let mut fields: Vec<FormField<'_>> = texts.iter().map(|(k, v)| FormField::Text(k, v)).collect();
    if let Some((name, media, bytes)) = &file {
        fields.push(FormField::File("file", name, media, bytes));
    }
    let w = crate::multipart::FormWriter::write(&fields);
    let query = query_string(&r.extras, None);
    Ok(EncodedRequest::post_json(
        format!("/v1/speech-to-text{query}"),
        HeaderList::new().with("content-type", w.content_type()),
        w.into_body(),
    ))
}
