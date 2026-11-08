//! OpenAI-style speech and transcription requests as `generateContent` calls: speech through
//! Gemini's speech models (`responseModalities: ["AUDIO"]`, speech generation guide), and
//! transcription through Gemini 3.5 Transcribe (`generationConfig.audioTranscriptionConfig`,
//! audio transcription guide; discovery document `AudioTranscriptionConfig`,
//! `AudioTranscription`, `WordInfo` at revision 20261106).

use kaeso_kpp::{
    AudioFormat, Block, BlockHeader, Delta, ErrorKind, KppError, Origin, Reply, Source,
    SpeechEvent, SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent, StreamMode,
    TimestampGranularity, TranscriptFormat, TranscriptSegment, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse, Usage, Voice,
};
use serde_json::{Map, Value, json};

use super::D;
use super::path::GeminiAction;
use super::response::decode_generate;
use super::wire::ResponseW;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, StreamDecoder};
use crate::cross::check_extras;
use crate::wire::{Ext, to_body};

fn refused(models: &str, feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        D,
        format!("Gemini {models} models do not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

fn speech_format(f: &AudioFormat) -> Option<&'static str> {
    match f {
        AudioFormat::Wav => Some("AUDIO_WAV"),
        AudioFormat::Mp3 => Some("AUDIO_MP3"),
        AudioFormat::Pcm16 => Some("AUDIO_L16"),
        _ => None,
    }
}

pub(super) fn check_speech(r: &SpeechRequest) -> Result<(), KppError> {
    let no = |feature: &str, param: &str, hint: &str| Err(refused("speech", feature, param, hint));
    check_extras(&r.extras, D, "", "request")?;
    if r.operation != SpeechOperation::Speech {
        return no(r.operation.as_str(), "model", "send text to speak.");
    }
    match &r.voice {
        Some(Voice::Named(v)) if !crate::media::has_voice(crate::media::GEMINI_VOICES, v) => {
            return no(
                &format!("the voice `{v}`"),
                "voice",
                "name a prebuilt voice such as Kore, Puck or Zephyr.",
            );
        }
        Some(Voice::Id(id)) => {
            return no(
                &format!("the voice id `{id}`"),
                "voice",
                "name a prebuilt voice such as Kore.",
            );
        }
        _ => {}
    }
    if let Some(f) = &r.format
        && speech_format(f).is_none()
    {
        return no(
            &format!("the format `{}`", f.as_wire()),
            "response_format",
            "ask for wav, mp3 or pcm.",
        );
    }
    let typed = [
        (
            "instructions",
            r.instructions.is_some(),
            "write the style into `input`, as Google's speech guide does.",
        ),
        ("speed", r.speed.is_some(), "describe the pace in `input`."),
        ("sample_rate", r.sample_rate.is_some(), "remove it."),
        ("bitrate", r.bitrate.is_some(), "remove it."),
        ("timestamps", r.timestamps, "remove it."),
        ("duration", r.duration.is_some(), "remove it."),
    ];
    if let Some((field, _, hint)) = typed.iter().find(|(_, set, _)| *set) {
        return no(&format!("`{field}`"), field, hint);
    }
    if r.dialogue.len() + usize::from(r.audio.is_some()) > 0 {
        return no("dialogue or source audio", "input", "send text.");
    }
    Ok(())
}

/// A speech request as a `generateContent` call that asks only for audio. Gemini streams 16-bit
/// PCM only, so a stream is asked for when the app wants `pcm`, and other formats come whole.
pub(super) fn encode_speech(
    r: &SpeechRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check_speech(r)?;
    let streams = r.format == Some(AudioFormat::Pcm16);
    ctx.stream = if streams {
        StreamMode::on(true)
    } else {
        StreamMode::Off
    };
    let mut config = Map::new();
    config.insert("responseModalities".into(), json!(["AUDIO"]));
    let mut speech = Map::new();
    if let Some(Voice::Named(v)) = &r.voice {
        speech.insert(
            "voiceConfig".into(),
            json!({"prebuiltVoiceConfig": {"voiceName": v}}),
        );
    }
    if let Some(l) = &r.language {
        speech.insert("languageCode".into(), l.clone().into());
    }
    if !speech.is_empty() {
        config.insert("speechConfig".into(), Value::Object(speech));
    }
    if let Some(mime) = r.format.as_ref().and_then(speech_format) {
        let format = json!({"audio": {"mimeType": mime}});
        let format = if ctx.gemini_profile.response_format_list {
            json!([format])
        } else {
            format
        };
        config.insert("responseFormat".into(), format);
    }
    if let Some(seed) = r.seed {
        config.insert("seed".into(), seed.into());
    }
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": r.input.clone().unwrap_or_default()}]}],
        "generationConfig": config,
    });
    let action = if streams {
        GeminiAction::StreamGenerateContent
    } else {
        GeminiAction::GenerateContent
    };
    Ok(EncodedRequest::post_json(
        ctx.gemini_profile.path(&r.model, action)?,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&body)?,
    ))
}

fn generated(body: &[u8], origin: &Origin) -> Result<kaeso_kpp::Response, KppError> {
    let w: Ext<ResponseW> = Ext::from_slice(body).map_err(|e| {
        KppError::provider(format!(
            "The provider sent a response that is not a valid Gemini API response: {}.",
            crate::wire::without_position(&e)
        ))
    })?;
    decode_generate(w, origin)
}

fn without(what: &str, reply: &kaeso_kpp::Response, origin: &Origin) -> KppError {
    let kind = match reply.stop {
        kaeso_kpp::StopReason::ContentFilter | kaeso_kpp::StopReason::Refusal => {
            ErrorKind::ContentFiltered
        }
        _ => ErrorKind::ProviderError,
    };
    KppError::new(kind, format!("The model answered without {what}.")).with_origin(origin.clone())
}

/// The audio of a speech model's reply; 16-bit PCM pieces join into one.
pub(super) fn decode_speech(body: &[u8], origin: &Origin) -> Result<Reply, KppError> {
    let reply = generated(body, origin)?;
    let audio: Vec<&kaeso_kpp::Base64Source> = reply
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Audio(a) => match &a.source {
                Source::Base64(b) => Some(b),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let Some(first) = audio.first() else {
        return Err(without("audio", &reply, origin));
    };
    let data = crate::media::join_base64(audio.iter().map(|b| b.data.as_str()))
        .unwrap_or_else(|| first.data.clone());
    let mut resp = SpeechResponse::new(reply.model, Source::base64(first.media_type.clone(), data));
    resp.usage = reply.usage;
    Ok(Reply::Speech(resp))
}

/// Speech events from the events of a `streamGenerateContent` stream.
pub(crate) struct SpeechStream {
    inner: super::stream::Decoder,
    media_type: Option<String>,
    usage: Option<Usage>,
}

impl SpeechStream {
    pub(super) fn new(ctx: &EgressCtx) -> SpeechStream {
        SpeechStream {
            inner: super::stream::Decoder::new(ctx),
            media_type: None,
            usage: None,
        }
    }

    fn map(&mut self, events: Vec<StreamEvent>) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        for event in events {
            match event {
                StreamEvent::BlockStart(s) => {
                    if let BlockHeader::Audio(h) = &s.block {
                        self.media_type = Some(h.media_type.clone());
                    }
                }
                StreamEvent::BlockDelta(d) => {
                    if let Delta::Audio(a) = &d.delta
                        && let Some(data) = &a.data
                    {
                        let mut e = SpeechEvent::audio(data.clone());
                        e.media_type = self.media_type.take();
                        out.push(StreamEvent::Speech(e));
                    }
                }
                StreamEvent::ResponseDelta(d) => {
                    if let Some(u) = d.usage {
                        self.usage = Some(u);
                    }
                }
                StreamEvent::ResponseStart(s) => {
                    if let Some(u) = s.usage {
                        self.usage = Some(u);
                    }
                }
                StreamEvent::ResponseStop(_) => {
                    out.push(StreamEvent::Speech(SpeechEvent::done(self.usage.take())));
                }
                StreamEvent::Error(e) => out.push(StreamEvent::Error(e)),
                _ => {}
            }
        }
        out
    }
}

impl crate::codec::sealed::Sealed for SpeechStream {}

impl StreamDecoder for SpeechStream {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let events = self.inner.feed(bytes);
        self.map(events)
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let events = self.inner.finish();
        self.map(events)
    }

    fn flush(&mut self) -> Vec<StreamEvent> {
        let events = self.inner.flush();
        self.map(events)
    }
}

pub(super) fn check_transcription(r: &TranscriptionRequest) -> Result<(), KppError> {
    let no = |feature: &str, param: &str, hint: &str| {
        Err(refused("transcription", feature, param, hint))
    };
    check_extras(&r.extras, D, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return no(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        );
    }
    if !matches!(r.audio.source, Source::Base64(_)) {
        return no("links to audio or file ids", "file", "upload the file.");
    }
    if r.prompt.is_some() {
        return no("`prompt`", "prompt", "remove it.");
    }
    if r.temperature.is_some() {
        return no("`temperature`", "temperature", "remove it.");
    }
    if r.speakers.is_some() {
        return no("a speaker count", "num_speakers", "remove it.");
    }
    if let TranscriptFormat::Other(f) = r.format_or_default() {
        return no(
            &format!("`response_format: {f}`"),
            "response_format",
            "use json, text, srt, vtt, verbose_json or diarized_json.",
        );
    }
    Ok(())
}

/// A transcription request as a `generateContent` call to a transcribe model. Subtitles and
/// segments need times, so word timestamps are asked for whenever the app's form has times.
pub(super) fn encode_transcription(
    r: &TranscriptionRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check_transcription(r)?;
    ctx.stream = StreamMode::Off;
    let format = r.format_or_default();
    ctx.transcript_format = Some(format.clone());
    let mut config = Map::new();
    if let Some(l) = &r.language {
        config.insert("languageCodes".into(), json!([l]));
    }
    let diarize = r.diarize.unwrap_or(false) || format == TranscriptFormat::DiarizedJson;
    if diarize {
        config.insert("diarization".into(), true.into());
    }
    if format.needs_timing() || r.timestamps.contains(&TimestampGranularity::Word) {
        config.insert("wordTimestamp".into(), true.into());
    }
    let Source::Base64(b) = &r.audio.source else {
        return Err(KppError::invalid_request("The audio must be an upload."));
    };
    let mut generation = Map::new();
    generation.insert("audioTranscriptionConfig".into(), Value::Object(config));
    let body = json!({
        "contents": [{"role": "user", "parts": [{"inlineData": {"mimeType": b.media_type, "data": b.data}}]}],
        "generationConfig": generation,
    });
    Ok(EncodedRequest::post_json(
        ctx.gemini_profile
            .path(&r.model, GeminiAction::GenerateContent)?,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&body)?,
    ))
}

fn offset(v: Option<&Value>) -> Option<f64> {
    v?.as_str()?.strip_suffix('s')?.parse().ok()
}

/// The transcript of a transcribe model's reply: its text parts, and a segment per
/// `audioTranscription` part with its words and speaker.
pub(super) fn decode_transcription(body: &[u8], origin: &Origin) -> Result<Reply, KppError> {
    let reply = generated(body, origin)?;
    let value: Value = serde_json::from_slice(body).unwrap_or_default();
    let parts = value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut text = String::new();
    let mut segments = Vec::new();
    let mut words = Vec::new();
    for part in &parts {
        if let Some(t) = part.get("text").and_then(Value::as_str)
            && part.get("thought").and_then(Value::as_bool) != Some(true)
        {
            text.push_str(t);
        }
        let Some(at) = part.get("audioTranscription") else {
            continue;
        };
        let speaker = at
            .get("speakerLabel")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut seg_words = Vec::new();
        for w in at
            .get("words")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(start), Some(end)) =
                (offset(w.get("startOffset")), offset(w.get("endOffset")))
            else {
                continue;
            };
            let mut word = TranscriptWord::new(
                w.get("word").and_then(Value::as_str).unwrap_or_default(),
                start,
                end,
            );
            word.speaker.clone_from(&speaker);
            seg_words.push(word);
        }
        let seg_text = at
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                seg_words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        let start = seg_words.first().map_or(0.0, |w| w.start);
        let end = seg_words.last().map_or(0.0, |w| w.end);
        let mut seg = TranscriptSegment::new(start, end, seg_text);
        seg.id = Some(segments.len().to_string());
        seg.speaker = speaker;
        segments.push(seg);
        words.extend(seg_words);
    }
    if text.is_empty() {
        text = segments
            .iter()
            .map(|s| s.text.trim())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if text.is_empty() && segments.is_empty() {
        return Err(without("a transcript", &reply, origin));
    }
    let mut resp = TranscriptionResponse::new(reply.model, text);
    if !words.is_empty() || segments.iter().any(|s| s.speaker.is_some()) {
        resp.segments = segments;
    }
    resp.words = words;
    resp.usage = reply.usage;
    Ok(Reply::Transcription(resp))
}
