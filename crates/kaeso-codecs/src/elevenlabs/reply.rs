use base64::Engine as _;
use kaeso_kpp::{
    Extras, KppError, Origin, RawJson, Reply, Request, RequestKind, Source, SpeechAlignment,
    SpeechEvent, SpeechResponse, StreamEvent, TranscriptWord, TranscriptionResponse,
};
use serde_json::{Map, Value, json};

use super::{D, format};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList, StreamDecoder, StreamEncoder};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// The characters ElevenLabs billed a request, from its `character-cost` response header
/// (<https://elevenlabs.io/docs/api-reference/introduction>, "Tracking generation costs").
pub fn billed_characters(headers: &HeaderList) -> Option<u64> {
    headers.get("character-cost")?.trim().parse().ok()
}

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "ElevenLabs sent a reply Kaeso cannot read: {detail}."
    ))
}

fn alignment_of(v: Option<&Value>) -> Option<SpeechAlignment> {
    let v = v?;
    let list = |k: &str| {
        v.get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let numbers = |k: &str| list(k).iter().filter_map(Value::as_f64).collect::<Vec<_>>();
    Some(SpeechAlignment::new(
        list("characters")
            .iter()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect(),
        numbers("character_start_times_seconds"),
        numbers("character_end_times_seconds"),
    ))
}

fn wire_alignment(a: &SpeechAlignment) -> Value {
    json!({
        "characters": a.characters,
        "character_start_times_seconds": a.starts,
        "character_end_times_seconds": a.ends,
    })
}

// A piece of audio with timestamps, as `AudioWithTimestampsResponseModel` and each line of
// `/stream/with-timestamps` carry it; the normalized alignment stays an extra.
fn timed_piece(v: Value, origin: &Origin) -> SpeechEvent {
    let mut e = SpeechEvent::audio(
        v.get("audio_base64")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    e.alignment = alignment_of(v.get("alignment").filter(|a| !a.is_null()));
    if let Value::Object(mut m) = v {
        for k in ["audio_base64", "alignment"] {
            m.remove(k);
        }
        if !m.is_empty() {
            let mut extras = Extras::new(origin.clone());
            for (k, v) in m {
                if let Ok(raw) = RawJson::from_serialize(&v) {
                    extras.fields.insert(k, raw);
                }
            }
            e.extras = extras;
        }
    }
    e
}

fn media_of(headers: &HeaderList, ctx: &EgressCtx) -> String {
    match headers.get("content-type") {
        Some(t) if crate::media::is_audio(t) => t.to_owned(),
        _ => ctx
            .audio_format
            .as_ref()
            .and_then(|f| f.media_type())
            .unwrap_or("audio/mpeg")
            .to_owned(),
    }
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(status, headers, body));
    }
    let origin = ctx.origin(D);
    match ctx.kind {
        RequestKind::Transcription => {
            let v: Value =
                serde_json::from_slice(body).map_err(|_| bad("the transcript is not JSON"))?;
            Ok(Reply::Transcription(transcript_of(v, &origin)?))
        }
        RequestKind::Speech if ctx.speech_reply_json => {
            let v: Value =
                serde_json::from_slice(body).map_err(|_| bad("the audio is not JSON"))?;
            let piece = timed_piece(v, &origin);
            let source = Source::base64(
                media_of(&HeaderList::new(), ctx),
                piece.audio.unwrap_or_default(),
            )
            .map_err(|_| bad("the audio is not valid base64"))?;
            let mut resp = SpeechResponse::new(String::new(), source);
            resp.alignment = piece.alignment;
            resp.extras = piece.extras;
            resp.characters = billed_characters(headers);
            Ok(Reply::Speech(resp))
        }
        RequestKind::Speech => {
            let mut resp = SpeechResponse::new(
                String::new(),
                Source::bytes(media_of(headers, ctx), bytes::Bytes::copy_from_slice(body)),
            );
            resp.characters = billed_characters(headers);
            Ok(Reply::Speech(resp))
        }
        other => Err(KppError::unsupported_by(
            format!("{other:?} response"),
            D,
            "",
        )),
    }
}

/// A transcript from `SpeechToTextChunkResponseModel`: its text, language, timed words with
/// speakers, and the audio's length.
fn transcript_of(v: Value, origin: &Origin) -> Result<TranscriptionResponse, KppError> {
    let Value::Object(mut m) = v else {
        return Err(bad("the transcript is not an object"));
    };
    if m.contains_key("transcripts") {
        return Err(bad(
            "a transcript per channel has no place in one transcript",
        ));
    }
    let text = match m.remove("text") {
        Some(Value::String(s)) => s,
        _ => return Err(bad("the transcript has no `text`")),
    };
    let mut resp = TranscriptionResponse::new(String::new(), text);
    resp.language = match m.remove("language_code") {
        Some(Value::String(s)) => Some(s),
        _ => None,
    };
    resp.usage_seconds = m.get("audio_duration_secs").and_then(Value::as_f64);
    resp.duration = resp.usage_seconds;
    if let Some(Value::Array(words)) = m.remove("words") {
        for w in words {
            let Value::Object(mut wm) = w else { continue };
            let text = match wm.remove("text") {
                Some(Value::String(s)) => s,
                _ => String::new(),
            };
            let start = wm.remove("start").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let end = wm.remove("end").and_then(|v| v.as_f64()).unwrap_or(start);
            let mut word = TranscriptWord::new(text, start, end);
            word.speaker = match wm.remove("speaker_id") {
                Some(Value::String(s)) => Some(s),
                _ => None,
            };
            let mut extras = Extras::new(origin.clone());
            for (k, v) in wm {
                if let Ok(raw) = RawJson::from_serialize(&v) {
                    extras.fields.insert(k, raw);
                }
            }
            word.extras = extras;
            resp.words.push(word);
        }
    }
    let mut extras = Extras::new(origin.clone());
    for (k, v) in m {
        if let Ok(raw) = RawJson::from_serialize(&v) {
            extras.fields.insert(k, raw);
        }
    }
    resp.extras = extras;
    Ok(resp)
}

/// Reads speech as it arrives: lines of JSON with timestamps when Kaeso asked for them,
/// otherwise the audio itself.
pub(crate) struct Decoder {
    lines: bool,
    buffer: Vec<u8>,
    origin: Origin,
    media_type: Option<String>,
    done: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            lines: !ctx.stream.is_off(),
            buffer: Vec::new(),
            origin: ctx.origin(D),
            media_type: Some(media_of(&HeaderList::new(), ctx)),
            done: false,
        }
    }

    fn line(&mut self, line: &[u8], out: &mut Vec<StreamEvent>) {
        let text = String::from_utf8_lossy(line);
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        match serde_json::from_str::<Value>(text) {
            Ok(v) if v.get("detail").is_some() => {
                let err = super::error::decode(500, &HeaderList::new(), text.as_bytes());
                out.push(StreamEvent::Error(err));
            }
            Ok(v) => {
                let mut e = timed_piece(v, &self.origin);
                e.media_type = self.media_type.take();
                out.push(StreamEvent::Speech(e));
            }
            Err(_) => out.push(StreamEvent::Error(bad("a streamed line is not JSON"))),
        }
    }
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if !self.lines {
            if !bytes.is_empty() {
                let mut e = SpeechEvent::audio(B64.encode(bytes));
                e.media_type = self.media_type.take();
                out.push(StreamEvent::Speech(e));
            }
            return out;
        }
        self.buffer.extend_from_slice(bytes);
        while let Some(at) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=at).collect();
            self.line(&line, &mut out);
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        let rest = std::mem::take(&mut self.buffer);
        self.line(&rest, &mut out);
        if !self.done {
            self.done = true;
            out.push(StreamEvent::Speech(SpeechEvent::done(None)));
        }
        out
    }
}

fn speech_call(ctx: &EncodeCtx) -> (bool, bool) {
    match ctx.request.as_deref() {
        Some(Request::Speech(r)) => (r.timestamps, !r.stream.is_off()),
        _ => (false, false),
    }
}

pub(super) fn media_type(reply: Option<&Reply>, ctx: &EncodeCtx) -> String {
    let (timestamps, _) = speech_call(ctx);
    if ctx.kind == RequestKind::Transcription || timestamps {
        return crate::codec::JSON_MEDIA_TYPE.to_owned();
    }
    let asked = match ctx.request.as_deref() {
        Some(Request::Speech(r)) => r
            .extras
            .form_str(D, "output_format")
            .map(format::media_type),
        _ => None,
    };
    match (asked, reply) {
        (Some(m), _) => m.to_owned(),
        (None, Some(Reply::Speech(r))) => match &r.audio {
            Source::Base64(b) => b.media_type.clone(),
            _ => "audio/mpeg".to_owned(),
        },
        _ => "audio/mpeg".to_owned(),
    }
}

fn transcript_json(r: &TranscriptionResponse) -> Result<Vec<u8>, KppError> {
    let own = r.extras.is_from(D);
    let mut m = Map::new();
    m.insert(
        "language_code".into(),
        r.language.clone().unwrap_or_default().into(),
    );
    m.insert("text".into(), r.text.clone().into());
    let words: Vec<Value> = r
        .words
        .iter()
        .map(|w| {
            let mut o = Map::new();
            o.insert("text".into(), w.text.clone().into());
            o.insert("start".into(), json!(w.start));
            o.insert("end".into(), json!(w.end));
            if let Some(s) = &w.speaker {
                o.insert("speaker_id".into(), s.clone().into());
            }
            if w.extras.is_from(D) {
                for (k, v) in &w.extras.fields {
                    if let Ok(v) = v.to_value() {
                        o.insert(k.clone(), v);
                    }
                }
            } else {
                o.insert("type".into(), "word".into());
                o.insert("logprob".into(), json!(0.0));
            }
            Value::Object(o)
        })
        .collect();
    m.insert("words".into(), words.into());
    if own {
        for (k, v) in &r.extras.fields {
            if let Ok(v) = v.to_value() {
                m.entry(k.clone()).or_insert(v);
            }
        }
    } else {
        m.insert("language_probability".into(), json!(1.0));
        if let Some(s) = r.audio_seconds() {
            m.insert("audio_duration_secs".into(), json!(s));
        }
    }
    crate::wire::to_body(&m)
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    match reply {
        Reply::Transcription(r) => transcript_json(r),
        Reply::Speech(r) => {
            let Source::Base64(b) = &r.audio else {
                return Err(KppError::provider(
                    "The provider answered with a link to the audio, which ElevenLabs' API cannot return.",
                ));
            };
            let mut enc = Encoder::new(ctx);
            if !enc.lines && !enc.timestamps {
                return Ok(b.data.to_vec());
            }
            if enc.timestamps && !enc.lines {
                let mut m = Map::new();
                m.insert("audio_base64".into(), b.to_base64().into());
                m.insert(
                    "alignment".into(),
                    r.alignment.as_ref().map_or(Value::Null, wire_alignment),
                );
                put_timed_extras(&mut m, &r.extras);
                return crate::wire::to_body(&m);
            }
            let mut piece = SpeechEvent::audio(b.to_base64());
            piece.alignment.clone_from(&r.alignment);
            piece.extras.clone_from(&r.extras);
            let mut out = enc.encode(&StreamEvent::Speech(piece))?;
            out.extend(enc.encode(&StreamEvent::Speech(SpeechEvent::done(None)))?);
            Ok(out)
        }
        other => Err(KppError::unsupported_by(
            format!("{:?} response", other.kind()),
            D,
            "",
        )),
    }
}

fn put_timed_extras(m: &mut Map<String, Value>, extras: &Extras) {
    if extras.is_from(D) {
        for (k, v) in &extras.fields {
            if let Ok(v) = v.to_value() {
                m.entry(k.clone()).or_insert(v);
            }
        }
    } else {
        m.entry("normalized_alignment").or_insert(Value::Null);
    }
}

/// Writes speech for an ElevenLabs app: the audio bytes, or a line of JSON per piece on
/// `/stream/with-timestamps`.
pub(crate) struct Encoder {
    timestamps: bool,
    lines: bool,
}

impl Encoder {
    pub(super) fn new(ctx: &EncodeCtx) -> Encoder {
        let (timestamps, lines) = speech_call(ctx);
        Encoder { timestamps, lines }
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let StreamEvent::Speech(e) = event else {
            return Ok(Vec::new());
        };
        let Some(data) = &e.audio else {
            return Ok(Vec::new());
        };
        if !self.lines {
            return B64.decode(data).map_err(|_| {
                KppError::provider("The provider sent audio that is not valid base64.")
            });
        }
        let mut m = Map::new();
        m.insert("audio_base64".into(), data.clone().into());
        m.insert(
            "alignment".into(),
            e.alignment.as_ref().map_or(Value::Null, wire_alignment),
        );
        put_timed_extras(&mut m, &e.extras);
        let mut out = serde_json::to_vec(&m).unwrap_or_default();
        out.push(b'\n');
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}
