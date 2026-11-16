//! AssemblyAI's Sync API (<https://www.assemblyai.com/docs/api-reference/sync-api/transcribe>,
//! its OpenAPI document read 2026-11-14): `POST /v1/transcribe` with a `multipart/form-data`
//! upload of the `audio` and an optional JSON `config` part, the model in the `X-AAI-Model`
//! header, answered with the whole transcript in the same request. It takes audio of up to 20
//! minutes and 256 MB and answers within 30 seconds or with 504 `Inference Timeout`.

mod error;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;

use kaeso_kpp::{
    Base64Source, DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request,
    RequestKind, Source, TimestampGranularity, TranscriptFormat, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse,
};
use serde_json::{Map, Value};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::multipart::{FormField, FormWriter};
use crate::video_jobs::{Fields, no_stream, put_extras, refused};

const D: DialectId = DialectId::AssemblyAi;
const PROVIDER: &str = "AssemblyAI";

/// The path of the Sync API's transcription.
pub const TRANSCRIBE_PATH: &str = "/v1/transcribe";

/// The header that names the model, which the Sync API routes by.
pub const MODEL_HEADER: &str = "x-aai-model";

/// The languages the Sync API steers toward (`config.language_codes`); it detects none and,
/// without one, transcribes without steering.
pub const LANGUAGES: [&str; 32] = [
    "en", "es", "de", "fr", "it", "pt", "tr", "nl", "sv", "no", "da", "fi", "hi", "vi", "ar", "he",
    "ja", "ur", "zh", "ko", "ca", "gl", "ru", "ro", "et", "fa", "yue", "af", "mr", "zu", "xh",
    "nn",
];

// `config.prompt` takes at most 6000 characters (the OpenAPI document).
const MAX_PROMPT: usize = 6000;

/// AssemblyAI's Sync API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AssemblyAi;

impl crate::codec::sealed::Sealed for AssemblyAi {}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn not_transcription(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "Kaeso sends AssemblyAI transcriptions only.",
    )
}

/// The Sync API's code of a language, as `en-US` is `en`.
fn language_code(language: &str) -> Option<&'static str> {
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    LANGUAGES.into_iter().find(|l| *l == primary)
}

fn upload(r: &TranscriptionRequest) -> Result<&Base64Source, KppError> {
    match &r.audio.source {
        Source::Base64(audio) => Ok(audio),
        _ => Err(no(
            "audio by link or file id",
            "file",
            "upload the audio as `file`.",
        )),
    }
}

/// Checks that the Sync API can transcribe `r`, before anything is sent.
fn check(r: &TranscriptionRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return Err(no(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    upload(r)?;
    if let Some(l) = &r.language
        && language_code(l).is_none()
    {
        return Err(no(
            &format!("the language `{l}`"),
            "language",
            "leave `language` out to transcribe without steering.",
        ));
    }
    if r.format == Some(TranscriptFormat::DiarizedJson) {
        return Err(no(
            "speaker labels",
            "response_format",
            "use json, text, srt, vtt or verbose_json.",
        ));
    }
    if r.timestamps
        .iter()
        .any(|t| *t != TimestampGranularity::Word)
    {
        return Err(no(
            "segment timestamps",
            "timestamp_granularities",
            "ask for word timestamps.",
        ));
    }
    if r.prompt
        .as_deref()
        .is_some_and(|p| p.chars().count() > MAX_PROMPT)
    {
        return Err(KppError::invalid_request(
            "AssemblyAI takes a `prompt` of at most 6000 characters.",
        )
        .with_param("prompt"));
    }
    let unsupported = [
        ("temperature", r.temperature.is_some()),
        ("diarize", r.diarize.is_some()),
        ("num_speakers", r.speakers.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    if !r.stream.is_off() {
        return Err(no(
            "streamed transcripts",
            "stream",
            "ask without `stream`.",
        ));
    }
    Ok(())
}

// Kaeso writes `verbose_json`, `srt` and `vtt` from the timed words, the only timing the Sync
// API gives.
fn wants_words(r: &TranscriptionRequest) -> bool {
    matches!(
        r.format,
        Some(TranscriptFormat::VerboseJson | TranscriptFormat::Srt | TranscriptFormat::Vtt)
    ) || !r.timestamps.is_empty()
}

fn encode_request(r: &TranscriptionRequest) -> Result<EncodedRequest, KppError> {
    check(r)?;
    let audio = upload(r)?;
    let mut config = Map::new();
    if let Some(code) = r.language.as_deref().and_then(language_code) {
        config.insert("language_codes".into(), Value::from(vec![code]));
    }
    if let Some(p) = &r.prompt {
        config.insert("prompt".into(), p.clone().into());
    }
    if wants_words(r) {
        config.insert("timestamps".into(), true.into());
    }
    let config = crate::wire::to_body(&config)?;
    let name = r
        .audio
        .filename
        .clone()
        .unwrap_or_else(|| "audio".to_owned());
    let mut fields = vec![FormField::File(
        "audio",
        &name,
        &audio.media_type,
        &audio.data,
    )];
    if config.len() > 2 {
        fields.push(FormField::Typed(
            "config",
            crate::codec::JSON_MEDIA_TYPE,
            &config,
        ));
    }
    let w = FormWriter::write(&fields);
    let headers = HeaderList::new()
        .with("content-type", w.content_type())
        .with(MODEL_HEADER, r.model.clone());
    Ok(EncodedRequest::post_json(
        TRANSCRIBE_PATH,
        headers,
        w.into_body(),
    ))
}

fn ms(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).map(|n| n / 1000.0)
}

fn ms_value(seconds: f64) -> Value {
    // Seconds back to the whole milliseconds the Sync API writes.
    let ms = (seconds * 1000.0).round();
    if ms.fract() == 0.0 && ms.abs() < 9.0e15 {
        Value::from(ms as i64)
    } else {
        Value::from(seconds * 1000.0)
    }
}

/// The words as KPP's timed words, when every one of them has its times; the Sync API leaves
/// them out of words it can't align, and then the words stay as AssemblyAI wrote them.
fn timed_words(words: &[Value]) -> Option<Vec<TranscriptWord>> {
    words
        .iter()
        .map(|w| {
            let m = w.as_object()?;
            let text = m.get("text")?.as_str()?;
            let start = ms(m.get("start"))?;
            let end = ms(m.get("end"))?;
            let mut word = TranscriptWord::new(text, start, end);
            let mut f = Fields::from_map(m.clone(), D, "reply");
            for k in ["text", "start", "end"] {
                f.take(k);
            }
            word.extras = f.rest(&Origin::new(D));
            Some(word)
        })
        .collect()
}

fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<TranscriptionResponse, KppError> {
    let mut f = crate::video_jobs::provider_object(body, D)?;
    let text = f.str("text")?.ok_or_else(|| {
        KppError::provider(format!("{PROVIDER} sent a transcript without `text`."))
    })?;
    let mut r = TranscriptionResponse::new(String::new(), text);
    r.duration = ms(f.peek("audio_duration_ms"));
    if r.duration.is_some() {
        f.take("audio_duration_ms");
    }
    r.usage_seconds = r.duration;
    if let Some(Value::Array(words)) = f.peek("words")
        && let Some(timed) = timed_words(words)
    {
        r.words = timed;
        f.take("words");
    }
    r.extras = f.rest(&ctx.origin(D));
    Ok(r)
}

fn encode_response(r: &TranscriptionResponse) -> Result<Vec<u8>, KppError> {
    let mut out = Map::new();
    out.insert("text".into(), r.text.clone().into());
    if !r.words.is_empty() {
        let words = r
            .words
            .iter()
            .map(|w| {
                let mut m = Map::new();
                m.insert("text".into(), w.text.clone().into());
                m.insert("start".into(), ms_value(w.start));
                m.insert("end".into(), ms_value(w.end));
                put_extras(&mut m, &w.extras, D, "");
                Value::Object(m)
            })
            .collect();
        out.insert("words".into(), Value::Array(words));
    }
    if let Some(d) = r.duration.or(r.usage_seconds) {
        out.insert("audio_duration_ms".into(), ms_value(d));
    }
    put_extras(&mut out, &r.extras, D, "");
    crate::wire::to_body(&out)
}

impl Codec for AssemblyAi {
    const DIALECT: DialectId = D;

    /// Kaeso writes the Sync API's uploads but doesn't read them: no app sends AssemblyAI's
    /// own form to Kaeso.
    fn decode_request(
        kind: RequestKind,
        _body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, D)?;
        Err(KppError::unsupported_by(
            "AssemblyAI-format requests",
            D,
            "Send transcriptions in OpenAI's format.",
        ))
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, D)?;
        match reply {
            Reply::Transcription(r) => encode_response(r),
            other => Err(not_transcription(other.kind())),
        }
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => check(r),
            other => Err(not_transcription(other.kind())),
        }
    }

    fn encode_request(req: &Request, _ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, D)?;
        match req {
            Request::Transcription(r) => encode_request(r),
            other => Err(not_transcription(other.kind())),
        }
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        match ctx.kind {
            RequestKind::Transcription => decode_response(body, ctx).map(Reply::Transcription),
            other => Err(not_transcription(other)),
        }
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(D, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        Err(error::decode(status, headers, body))
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        Err(error::decode(status, headers, body))
    }

    fn encode_model_request(_model: &str) -> EncodedRequest {
        EncodedRequest::get(TRANSCRIBE_PATH, HeaderList::new())
    }

    /// The Sync API lists no models and has no request that checks a key without
    /// transcribing (`GET /v1/warm` takes none), so a model check can't say anything.
    fn encode_models_request(_query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        Err(no_check())
    }

    fn encode_model_request_with(
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Err(no_check())
    }
}

fn no_check() -> KppError {
    KppError::unsupported_by(
        "a model check",
        D,
        "AssemblyAI can't check a key without a transcription.",
    )
}
