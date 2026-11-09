//! Transcription with Qwen-Audio-ASR on DashScope's synchronous multimodal generation path
//! (<https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api>, read
//! 2026-11-09): the audio as a link or a data URL in an `input_audio` part, an optional context
//! message before it, the audio's format in `parameters`, and the transcript in `output.text`.

use kaeso_kpp::{
    AudioInput, Extras, KppError, Origin, RawUsage, Reply, Request, Source, StreamMode,
    TranscriptFormat, TranscriptionOperation, TranscriptionRequest, TranscriptionResponse, Usage,
};
use serde_json::{Map, Value, json};

use super::{D, PROVIDER};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::video_jobs::{Fields, body, provider_object, put, put_extras, refused};

const FORMAT: &str = "format";

/// Whether Kaeso sends a QwenCloud speech recognition model to DashScope's synchronous API:
/// Qwen-Audio-ASR Flash 3.0 and 3.1. The file transcription models answer only asynchronous
/// tasks, and the streaming, real-time and message models only WebSocket connections.
pub fn transcribes(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("qwen-audio-") && m.ends_with("-asr-flash")
}

fn only(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|m| m.keys().all(|k| keys.contains(&k.as_str())))
}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

// The formats Qwen-Audio-ASR names in `parameters.format`, by the media types an upload carries.
fn format_of(media_type: &str) -> Option<&'static str> {
    let essence = media_type.split(';').next()?.trim().to_ascii_lowercase();
    match essence.as_str() {
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/opus" | "audio/ogg" => Some("opus"),
        _ => None,
    }
}

fn format_of_name(name: &str) -> Option<&'static str> {
    let path = name.split(['?', '#']).next()?;
    let (_, ext) = path.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "wav" => Some("wav"),
        "mp3" => Some("mp3"),
        "opus" | "ogg" => Some("opus"),
        _ => None,
    }
}

fn audio_format(r: &TranscriptionRequest) -> Option<String> {
    if let Some(f) = r.extras.form_str(D, FORMAT) {
        return Some(f.to_owned());
    }
    let by_name = r.audio.filename.as_deref().and_then(format_of_name);
    match &r.audio.source {
        Source::Base64(b) => format_of(&b.media_type).or(by_name),
        Source::Url(u) => by_name.or_else(|| format_of_name(u)),
        _ => None,
    }
    .map(str::to_owned)
}

pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut extras = Extras::new(Origin::new(D));
    let model = f.str("model")?.unwrap_or_default();
    let mut audio = None;
    let mut prompt = None;
    if let Some(mut input) = f.object("input")? {
        for message in input.array("messages")?.unwrap_or_default() {
            let parts = message
                .get("content")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let role = message.get("role").and_then(Value::as_str);
            for part in parts {
                let shaped = only(&message, &["role", "content"])
                    && only(&part, &["type", "input_audio", "text"])
                    && part.get("input_audio").is_none_or(|a| only(a, &["data"]));
                match (role, part.get("type").and_then(Value::as_str)) {
                    _ if !shaped => {
                        return Err(KppError::unsupported_by(
                            "fields beside the audio and the prompt",
                            D,
                            "Kaeso maps the audio and a prompt before it only.",
                        )
                        .with_param("messages"));
                    }
                    (Some("user"), Some("input_audio")) if audio.is_none() => {
                        let data = part
                            .pointer("/input_audio/data")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        audio = Some(AudioInput::new(crate::video_jobs::link_source(
                            data.to_owned(),
                        )));
                    }
                    (Some("user"), Some("input_text")) if audio.is_none() && prompt.is_none() => {
                        prompt = part.get("text").and_then(Value::as_str).map(str::to_owned);
                    }
                    _ => {
                        return Err(KppError::unsupported_by(
                            "context other than one text before the audio",
                            D,
                            "Kaeso maps the audio and a prompt before it only.",
                        )
                        .with_param("messages"));
                    }
                }
            }
        }
        input.rest_into(&mut extras, "input");
    }
    let audio = audio.ok_or_else(|| {
        KppError::invalid_request("Send the audio in an `input_audio` part.").with_param("input")
    })?;
    let mut r = TranscriptionRequest::new(model, TranscriptionOperation::Transcribe, audio);
    r.prompt = prompt;
    if let Some(mut p) = f.object("parameters")? {
        if let Some(format) = p.str("format")? {
            extras.form.insert(FORMAT.into(), format.into());
        }
        if let Some(Value::Array(list)) = p.peek("language_hints")
            && let [Value::String(l)] = list.as_slice()
        {
            r.language = Some(l.clone());
            p.take("language_hints");
        }
        p.rest_into(&mut extras, "parameters");
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Transcription(r))
}

pub(super) fn check(r: &TranscriptionRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if !transcribes(&r.model) {
        return Err(KppError::unsupported_by(
            format!("the model `{}`", r.model),
            D,
            "Kaeso sends Qwen-Audio-ASR Flash to DashScope; file transcription models answer only \
             asynchronous tasks, and streaming models only WebSocket connections.",
        )
        .with_param("model"));
    }
    if r.operation == TranscriptionOperation::Translate {
        return Err(no(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    if !matches!(r.audio.source, Source::Base64(_) | Source::Url(_)) {
        return Err(no("file ids", "file", "upload the file."));
    }
    if audio_format(r).is_none() {
        return Err(no(
            "audio other than wav, mp3 or opus",
            "file",
            "upload a wav, mp3 or opus file.",
        ));
    }
    if !matches!(
        r.format_or_default(),
        TranscriptFormat::Json | TranscriptFormat::Text
    ) {
        return Err(no(
            &format!("`response_format: {}`", r.format_or_default().as_wire()),
            "response_format",
            "ask for `json` or `text`.",
        ));
    }
    let unsupported = [
        ("temperature", r.temperature.is_some()),
        ("timestamp_granularities", !r.timestamps.is_empty()),
        ("diarize", r.diarize.is_some()),
        ("num_speakers", r.speakers.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    Ok(())
}

pub(super) fn encode_request(
    r: &TranscriptionRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check(r)?;
    ctx.stream = StreamMode::Off;
    ctx.transcript_format = Some(r.format_or_default());
    let data = match &r.audio.source {
        Source::Base64(b) => b.data_url(),
        Source::Url(u) => u.clone(),
        _ => String::new(),
    };
    let mut messages = Vec::new();
    if let Some(p) = &r.prompt {
        messages.push(json!({"role": "user", "content": [{"type": "input_text", "text": p}]}));
    }
    messages.push(json!({
        "role": "user",
        "content": [{"type": "input_audio", "input_audio": {"data": data}}],
    }));
    let mut input = Map::new();
    input.insert("messages".into(), Value::Array(messages));
    put_extras(&mut input, &r.extras, D, "input");
    let mut params = Map::new();
    put(&mut params, "format", audio_format(r));
    put(
        &mut params,
        "language_hints",
        r.language.clone().map(|l| Value::Array(vec![l.into()])),
    );
    put_extras(&mut params, &r.extras, D, "parameters");
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    out.insert("input".into(), Value::Object(input));
    out.insert("parameters".into(), Value::Object(params));
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(
        super::images::GENERATION_PATH,
        headers,
        body(&out)?,
    ))
}

pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<Reply, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let mut output = f.object("output")?.ok_or_else(|| {
        KppError::provider("QwenCloud answered a transcription without its `output`.")
    })?;
    let text = output
        .str("text")?
        .ok_or_else(|| KppError::provider("QwenCloud's transcript has no text."))?;
    output.rest_into(&mut extras, "output");
    let mut resp = TranscriptionResponse::new(String::new(), text);
    resp.format = ctx.transcript_format.clone();
    if let Some(u) = f.take("usage") {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        let mut usage = Usage::new(n("input_tokens"), n("output_tokens"));
        // `duration` is the processed audio in seconds, which QwenCloud bills.
        resp.usage_seconds = u.get("duration").and_then(Value::as_f64);
        usage.raw = Some(RawUsage::new(ctx.origin(D), u));
        resp.usage = usage;
    }
    f.rest_into(&mut extras, "");
    resp.extras = extras;
    Ok(Reply::Transcription(resp))
}

pub(super) fn encode_response(r: &TranscriptionResponse) -> Result<Vec<u8>, KppError> {
    let mut output = Map::new();
    put_extras(&mut output, &r.extras, D, "output");
    output.insert("text".into(), r.text.clone().into());
    let mut out = Map::new();
    out.insert("output".into(), Value::Object(output));
    let usage = match &r.usage.raw {
        Some(raw) if raw.origin.is(D) => Some(raw.value.clone()),
        _ => r.usage_seconds.map(|s| json!({ "duration": s })),
    };
    put(&mut out, "usage", usage);
    put_extras(&mut out, &r.extras, D, "");
    body(&out)
}
