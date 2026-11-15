//! Ink batch speech to text, `POST /stt` (<https://docs.cartesia.ai/api-reference/stt/transcribe>):
//! a `multipart/form-data` upload of `file`, `model`, `language` and
//! `timestamp_granularities[]`, answered with `{type, request_id, text, language, duration,
//! words}`. The batch API doesn't detect the language and takes no prompt, temperature or
//! response format (Cartesia's Whisper migration guide,
//! <https://www.cartesia.ai/migrate/openai-whisper>).

use kaeso_kpp::{
    Base64Source, KppError, Source, TimestampGranularity, TranscriptFormat, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse,
};
use serde_json::{Map, Value};

use super::{D, PROVIDER, STT_PATH, versioned};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::multipart::{FormField, FormWriter};
use crate::video_jobs::{Fields, put_extras, refused};

const WORDS: &str = "timestamp_granularities[]";

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
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

/// Checks that Cartesia can transcribe `r`, before anything is sent.
pub(super) fn check(r: &TranscriptionRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return Err(no(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    upload(r)?;
    if r.language.as_deref().is_none_or(str::is_empty) {
        return Err(KppError::invalid_request(
            "Cartesia doesn't detect the language; send `language`, such as `en`.",
        )
        .with_param("language"));
    }
    if let Some(f) = &r.format
        && *f == TranscriptFormat::DiarizedJson
    {
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
    let unsupported = [
        ("prompt", r.prompt.is_some()),
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

// Kaeso writes `verbose_json`, `srt` and `vtt` from the timed words, the only timing
// Cartesia's batch API gives.
fn wants_words(r: &TranscriptionRequest) -> bool {
    matches!(
        r.format,
        Some(TranscriptFormat::VerboseJson | TranscriptFormat::Srt | TranscriptFormat::Vtt)
    ) || !r.timestamps.is_empty()
}

pub(super) fn encode_request(
    r: &TranscriptionRequest,
    _ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check(r)?;
    let audio = upload(r)?;
    let language = r.language.clone().unwrap_or_default();
    let name = r
        .audio
        .filename
        .clone()
        .unwrap_or_else(|| "audio".to_owned());
    let mut fields = vec![
        FormField::Text("model", &r.model),
        FormField::Text("language", &language),
    ];
    if wants_words(r) {
        fields.push(FormField::Text(WORDS, "word"));
    }
    fields.push(FormField::File(
        "file",
        &name,
        &audio.media_type,
        &audio.data,
    ));
    let w = FormWriter::write(&fields);
    let headers = versioned(HeaderList::new().with("content-type", w.content_type()));
    Ok(EncodedRequest::post_json(STT_PATH, headers, w.into_body()))
}

fn number(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
}

fn word_of(v: Value) -> Result<TranscriptWord, KppError> {
    let Value::Object(m) = v else {
        return Err(KppError::provider(format!(
            "{PROVIDER} listed a word that is not an object."
        )));
    };
    let mut f = Fields::from_map(m, D, "reply");
    let text = f.str("word")?.unwrap_or_default();
    let start = number(f.take("start").as_ref()).unwrap_or_default();
    let end = number(f.take("end").as_ref()).unwrap_or_default();
    let mut word = TranscriptWord::new(text, start, end);
    word.extras = f.rest(&kaeso_kpp::Origin::new(D));
    Ok(word)
}

/// A transcript as Cartesia answers it; `type` and `request_id` stay among the extras.
pub(super) fn decode_response(
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<TranscriptionResponse, KppError> {
    let mut f = crate::video_jobs::provider_object(body, D)?;
    let text = f.str("text")?.ok_or_else(|| {
        KppError::provider(format!("{PROVIDER} sent a transcript without `text`."))
    })?;
    let mut r = TranscriptionResponse::new(String::new(), text);
    r.language = f.str("language")?;
    r.duration = number(f.take("duration").as_ref());
    r.usage_seconds = r.duration;
    for w in f.array("words")?.unwrap_or_default() {
        r.words.push(word_of(w)?);
    }
    r.extras = f.rest(&ctx.origin(D));
    Ok(r)
}

/// The transcript as Cartesia writes it.
pub(super) fn encode_response(r: &TranscriptionResponse) -> Result<Vec<u8>, KppError> {
    let mut out = Map::new();
    if !r.extras.is_from(D) {
        out.insert("type".into(), "transcript".into());
    }
    out.insert("text".into(), r.text.clone().into());
    if let Some(l) = &r.language {
        out.insert("language".into(), l.clone().into());
    }
    if let Some(d) = r.duration.or(r.usage_seconds) {
        out.insert("duration".into(), d.into());
    }
    if !r.words.is_empty() {
        let words = r
            .words
            .iter()
            .map(|w| {
                let mut m = Map::new();
                m.insert("word".into(), w.text.clone().into());
                m.insert("start".into(), w.start.into());
                m.insert("end".into(), w.end.into());
                put_extras(&mut m, &w.extras, D, "");
                Value::Object(m)
            })
            .collect();
        out.insert("words".into(), Value::Array(words));
    }
    put_extras(&mut out, &r.extras, D, "");
    crate::wire::to_body(&out)
}
