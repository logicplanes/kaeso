use kaeso_kpp::{
    Extras, KppError, Origin, RawItem, RawJson, RawUsage, Reply, Request, RequestKind, StreamEvent,
    TranscriptEvent, TranscriptFormat, TranscriptSegment, TranscriptWord, TranscriptionOperation,
    TranscriptionResponse, Usage,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use crate::codec::{EgressCtx, EncodeCtx, HeaderList, StreamDecoder, StreamEncoder};
use crate::sse::{SseItem, SseParser, frame_extra, write_event, write_frame_extra};

const DELTA: &str = "transcript.text.delta";
const SEGMENT: &str = "transcript.text.segment";
const DONE: &str = "transcript.text.done";
const FORM_NUMERIC_ID: &str = "numeric_id";
const PLAIN_TEXT: &str = "text/plain; charset=utf-8";

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a transcript that is not a valid {} reply: {detail}.",
        DIALECT.display_name()
    ))
}

fn take_f64(m: &mut Map<String, Value>, key: &str) -> Option<f64> {
    match m.get(key) {
        Some(Value::Number(n)) => {
            let v = n.as_f64();
            m.remove(key);
            v
        }
        _ => None,
    }
}

fn take_str(m: &mut Map<String, Value>, key: &str) -> Option<String> {
    match m.get(key) {
        Some(Value::String(_)) => m.remove(key).and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        }),
        _ => None,
    }
}

// A speaker as a label: SpaceX numbers speakers, Together and Mistral call the field
// `speaker_id`.
fn take_speaker(m: &mut Map<String, Value>) -> Option<String> {
    for key in ["speaker", "speaker_id"] {
        match m.get(key) {
            Some(Value::String(_)) => return take_str(m, key),
            Some(Value::Number(n)) => {
                let label = n.to_string();
                m.remove(key);
                return Some(label);
            }
            _ => {}
        }
    }
    None
}

fn extras_of(origin: &Origin, rest: Map<String, Value>) -> Extras {
    let mut extras = Extras::new(origin.clone());
    for (k, v) in rest {
        if let Ok(raw) = RawJson::from_serialize(&v) {
            extras.fields.insert(k, raw);
        }
    }
    extras
}

// Fields another provider's form of the API added are its own; an OpenAI app gets OpenAI's.
fn from_openai(extras: &Extras) -> bool {
    extras.is_from(DIALECT)
        && extras
            .origin
            .as_ref()
            .and_then(|o| o.provider.as_deref())
            .is_none_or(|p| p == "openai")
}

fn put_extras(out: &mut Map<String, Value>, extras: &Extras) {
    if !from_openai(extras) {
        return;
    }
    for (k, v) in &extras.fields {
        if let Ok(value) = v.to_value() {
            out.entry(k.clone()).or_insert(value);
        }
    }
}

fn segment_of(v: Value, origin: &Origin) -> Result<TranscriptSegment, KppError> {
    let Value::Object(mut m) = v else {
        return Err(bad("a segment is not an object"));
    };
    let mut numeric = false;
    let id = match m.remove("id") {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => {
            numeric = true;
            Some(n.to_string())
        }
        Some(other) => {
            m.insert("id".to_owned(), other);
            None
        }
        None => None,
    };
    let start = take_f64(&mut m, "start")
        .or_else(|| take_f64(&mut m, "startSecond"))
        .ok_or_else(|| bad("a segment has no `start`"))?;
    let end = take_f64(&mut m, "end")
        .or_else(|| take_f64(&mut m, "endSecond"))
        .ok_or_else(|| bad("a segment has no `end`"))?;
    let text = take_str(&mut m, "text").unwrap_or_default();
    let mut seg = TranscriptSegment::new(start, end, text);
    seg.id = id;
    seg.speaker = take_speaker(&mut m);
    seg.extras = extras_of(origin, m);
    if numeric {
        seg.extras
            .form
            .insert(FORM_NUMERIC_ID.to_owned(), true.into());
    }
    Ok(seg)
}

fn word_of(v: Value, origin: &Origin) -> Result<TranscriptWord, KppError> {
    let Value::Object(mut m) = v else {
        return Err(bad("a word is not an object"));
    };
    let start = take_f64(&mut m, "start").ok_or_else(|| bad("a word has no `start`"))?;
    let end = take_f64(&mut m, "end").ok_or_else(|| bad("a word has no `end`"))?;
    let text = take_str(&mut m, "word")
        .or_else(|| take_str(&mut m, "text"))
        .unwrap_or_default();
    let mut w = TranscriptWord::new(text, start, end);
    w.speaker = take_speaker(&mut m);
    w.extras = extras_of(origin, m);
    Ok(w)
}

/// The usage of a transcript: tokens, with the audio's share, or the seconds billed.
pub(crate) fn usage_of(raw: &Value, origin: &Origin, resp: &mut TranscriptionResponse) {
    let n = |a: &str, b: &str| {
        raw.get(a)
            .or_else(|| raw.get(b))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    // OpenRouter reports `seconds` beside its tokens, Mistral `prompt_audio_seconds`.
    resp.usage_seconds = raw
        .get("seconds")
        .or_else(|| raw.get("prompt_audio_seconds"))
        .and_then(Value::as_f64);
    if raw.get("type").and_then(Value::as_str) != Some("duration") {
        let mut u = Usage::new(
            n("input_tokens", "prompt_tokens"),
            n("output_tokens", "completion_tokens"),
        );
        u.input_audio_tokens = raw
            .get("input_token_details")
            .or_else(|| raw.get("prompt_tokens_details"))
            .and_then(|d| d.get("audio_tokens"))
            .and_then(Value::as_u64);
        resp.usage = u;
    }
    resp.usage.raw = Some(RawUsage::new(origin.clone(), raw.clone()));
}

fn wire_usage(r: &TranscriptionResponse) -> Option<Value> {
    // Other providers' forms of the API report usage in shapes of their own, which are
    // written in OpenAI's form.
    let openai = r
        .usage
        .raw
        .as_ref()
        .is_some_and(|raw| raw.origin.provider.as_deref().is_none_or(|p| p == "openai"));
    if let Some(raw) = r.usage.raw_for(DIALECT).filter(|_| openai) {
        return Some(raw.clone());
    }
    if let Some(seconds) = r.usage_seconds {
        return Some(json!({ "type": "duration", "seconds": seconds }));
    }
    let u = &r.usage;
    if u.input_tokens == 0 && u.output_tokens == 0 {
        return None;
    }
    let mut v = json!({
        "type": "tokens",
        "input_tokens": u.input_tokens,
        "output_tokens": u.output_tokens,
        "total_tokens": u.input_tokens.saturating_add(u.output_tokens),
    });
    if let (Some(audio), Value::Object(m)) = (u.input_audio_tokens, &mut v) {
        m.insert(
            "input_token_details".to_owned(),
            json!({
                "audio_tokens": audio,
                "text_tokens": u.input_tokens.saturating_sub(audio),
            }),
        );
    }
    Some(v)
}

/// Reads a JSON transcript in any of OpenAI's forms: `json`, `verbose_json` or
/// `diarized_json`.
pub(crate) fn from_json(
    value: Value,
    format: TranscriptFormat,
    origin: &Origin,
) -> Result<TranscriptionResponse, KppError> {
    let Value::Object(mut m) = value else {
        return Err(bad("the body is not an object"));
    };
    let text = take_str(&mut m, "text").ok_or_else(|| bad("`text` is missing"))?;
    let mut resp = TranscriptionResponse::new(String::new(), text);
    resp.format = Some(format);
    resp.language = take_str(&mut m, "language");
    resp.duration = take_f64(&mut m, "duration").or_else(|| take_f64(&mut m, "durationInSeconds"));
    if let Some(Value::Array(items)) = m.remove("segments") {
        resp.segments = items
            .into_iter()
            .map(|s| segment_of(s, origin))
            .collect::<Result<_, _>>()?;
    }
    if let Some(Value::Array(items)) = m.remove("words") {
        resp.words = items
            .into_iter()
            .map(|w| word_of(w, origin))
            .collect::<Result<_, _>>()?;
    }
    if let Some(u) = m.remove("usage") {
        usage_of(&u, origin, &mut resp);
    }
    if let Some(Value::String(model)) = m.get("model") {
        resp.model.clone_from(model);
    }
    resp.extras = extras_of(origin, m);
    Ok(resp)
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::decode_error(status, headers, body));
    }
    if ctx.kind != RequestKind::Transcription {
        return Err(KppError::unsupported_by("generation response", DIALECT, ""));
    }
    let origin = ctx.origin(DIALECT);
    let format = ctx
        .transcript_format
        .clone()
        .unwrap_or(TranscriptFormat::Json);
    if format.is_text() {
        let text = String::from_utf8(body.to_vec()).map_err(|_| bad("the text is not UTF-8"))?;
        let mut resp = TranscriptionResponse::new(String::new(), text);
        resp.format = Some(format);
        return Ok(Reply::Transcription(resp));
    }
    let value: Value =
        serde_json::from_slice(body).map_err(|e| bad(&crate::wire::without_position(&e)))?;
    Ok(Reply::Transcription(from_json(value, format, &origin)?))
}

fn app_format(ctx: &EncodeCtx) -> (TranscriptFormat, TranscriptionOperation) {
    match ctx.request.as_deref() {
        Some(Request::Transcription(r)) => (r.format_or_default(), r.operation),
        _ => (TranscriptFormat::Json, TranscriptionOperation::Transcribe),
    }
}

pub(super) fn app_media_type(ctx: &EncodeCtx) -> String {
    if !ctx.stream.is_off() {
        return crate::codec::EVENT_STREAM_MEDIA_TYPE.to_owned();
    }
    if app_format(ctx).0.is_text() {
        PLAIN_TEXT.to_owned()
    } else {
        crate::codec::JSON_MEDIA_TYPE.to_owned()
    }
}

fn seconds(s: f64) -> (u64, u64, u64, u64) {
    let ms = (s.max(0.0) * 1000.0).round() as u64;
    (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// Subtitles from timed segments, in SubRip or WebVTT form.
pub(crate) fn subtitles(segments: &[TranscriptSegment], vtt: bool) -> String {
    let mut out = String::new();
    if vtt {
        out.push_str("WEBVTT\n\n");
    }
    for (i, s) in segments.iter().enumerate() {
        let stamp = |t: f64| {
            let (h, m, sec, ms) = seconds(t);
            if vtt {
                format!("{h:02}:{m:02}:{sec:02}.{ms:03}")
            } else {
                format!("{h:02}:{m:02}:{sec:02},{ms:03}")
            }
        };
        if !vtt {
            out.push_str(&format!("{}\n", i + 1));
        }
        out.push_str(&format!(
            "{} --> {}\n{}\n\n",
            stamp(s.start),
            stamp(s.end),
            s.text.trim()
        ));
    }
    out
}

/// The segments of a reply, or one segment per run of words when it has only words.
fn timed(r: &TranscriptionResponse) -> Option<Vec<TranscriptSegment>> {
    if !r.segments.is_empty() {
        return Some(r.segments.clone());
    }
    let first = r.words.first()?;
    let last = r.words.last()?;
    Some(vec![TranscriptSegment::new(
        first.start,
        last.end,
        r.text.clone(),
    )])
}

fn wire_segment(s: &TranscriptSegment, diarized: bool) -> Value {
    let mut m = Map::new();
    if diarized {
        m.insert("type".to_owned(), SEGMENT.into());
    }
    match &s.id {
        Some(id) if s.extras.form_flag(DIALECT, FORM_NUMERIC_ID) => {
            m.insert(
                "id".to_owned(),
                id.parse::<u64>()
                    .map_or_else(|_| id.clone().into(), Value::from),
            );
        }
        Some(id) => {
            m.insert("id".to_owned(), id.clone().into());
        }
        None => {}
    }
    m.insert("start".to_owned(), json!(s.start));
    m.insert("end".to_owned(), json!(s.end));
    m.insert("text".to_owned(), s.text.clone().into());
    if let Some(sp) = &s.speaker {
        m.insert("speaker".to_owned(), sp.clone().into());
    }
    put_extras(&mut m, &s.extras);
    Value::Object(m)
}

fn wire_word(w: &TranscriptWord) -> Value {
    let mut m = Map::new();
    m.insert("word".to_owned(), w.text.clone().into());
    m.insert("start".to_owned(), json!(w.start));
    m.insert("end".to_owned(), json!(w.end));
    if let Some(sp) = &w.speaker {
        m.insert("speaker".to_owned(), sp.clone().into());
    }
    put_extras(&mut m, &w.extras);
    Value::Object(m)
}

/// The transcript in the form the app asked for.
pub(crate) fn body(
    r: &TranscriptionResponse,
    format: &TranscriptFormat,
    operation: TranscriptionOperation,
) -> Result<Vec<u8>, KppError> {
    let same = r.format.as_ref() == Some(format);
    let text = |vtt: bool| -> Result<Vec<u8>, KppError> {
        if same {
            return Ok(r.text.clone().into_bytes());
        }
        let segments = timed(r).ok_or_else(|| {
            KppError::provider("The provider's transcript has no timing to make subtitles from.")
        })?;
        Ok(subtitles(&segments, vtt).into_bytes())
    };
    match format {
        TranscriptFormat::Text => Ok(r.text.clone().into_bytes()),
        TranscriptFormat::Srt => text(false),
        TranscriptFormat::Vtt => text(true),
        _ => {
            let own = from_openai(&r.extras);
            let diarized = *format == TranscriptFormat::DiarizedJson;
            let verbose = *format == TranscriptFormat::VerboseJson;
            let mut m = Map::new();
            if diarized || (verbose && !own) {
                let task = match operation {
                    TranscriptionOperation::Translate => "translate",
                    _ => "transcribe",
                };
                m.insert("task".to_owned(), task.into());
            }
            if verbose && let Some(l) = &r.language {
                m.insert("language".to_owned(), l.clone().into());
            }
            if (verbose || diarized)
                && let Some(d) = r.duration.or_else(|| r.audio_seconds())
            {
                m.insert("duration".to_owned(), json!(d));
            }
            m.insert("text".to_owned(), r.text.clone().into());
            if verbose && !r.words.is_empty() {
                // ElevenLabs lists spacing and sound events among its words; they aren't words.
                let spoken = |w: &&TranscriptWord| {
                    w.extras
                        .fields
                        .get("type")
                        .and_then(|t| t.to_value().ok())
                        .is_none_or(|t| t == "word")
                };
                let words: Vec<Value> = r.words.iter().filter(spoken).map(wire_word).collect();
                m.insert("words".to_owned(), words.into());
            }
            if (verbose || diarized) && (!r.segments.is_empty() || diarized) {
                let segments: Vec<Value> = timed(r)
                    .unwrap_or_default()
                    .iter()
                    .map(|s| wire_segment(s, diarized))
                    .collect();
                m.insert("segments".to_owned(), segments.into());
            }
            if let Some(u) = wire_usage(r) {
                m.insert("usage".to_owned(), u);
            }
            put_extras(&mut m, &r.extras);
            crate::wire::to_body(&m)
        }
    }
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Transcription(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "",
        ));
    };
    if !ctx.stream.is_off() {
        let mut enc = Encoder;
        let mut out = enc.encode(&StreamEvent::Transcript(TranscriptEvent::delta(
            r.text.clone(),
        )))?;
        out.extend(enc.encode(&StreamEvent::Transcript(TranscriptEvent::done(r.clone())))?);
        return Ok(out);
    }
    let (format, operation) = app_format(ctx);
    body(r, &format, operation)
}

/// Reads `transcript.text.*` events.
pub(crate) struct Decoder {
    parser: SseParser,
    origin: Origin,
    text: String,
    language: Option<String>,
    done: bool,
    failed: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            parser: SseParser::new(),
            origin: ctx.origin(DIALECT),
            text: String::new(),
            language: None,
            done: false,
            failed: false,
        }
    }

    // The end of a transcript, with the text of the deltas when the event leaves it out, as
    // Z.ai's and MiniMax's may.
    fn finished(&mut self, mut resp: TranscriptionResponse) -> TranscriptEvent {
        self.done = true;
        if resp.text.is_empty() {
            resp.text = std::mem::take(&mut self.text);
        }
        if resp.language.is_none() {
            resp.language = self.language.take();
        }
        TranscriptEvent::done(resp)
    }

    fn event(&mut self, mut m: Map<String, Value>) -> Result<Option<StreamEvent>, KppError> {
        let ty = take_str(&mut m, "type");
        let event = match ty.as_deref() {
            Some(DELTA) => {
                let delta = take_str(&mut m, "delta").unwrap_or_default();
                self.text.push_str(&delta);
                let mut e = TranscriptEvent::delta(delta);
                e.segment_id = take_str(&mut m, "segment_id");
                e.extras = extras_of(&self.origin, m);
                e
            }
            Some(SEGMENT) | Some("transcription.segment") => {
                TranscriptEvent::segment(segment_of(Value::Object(m), &self.origin)?)
            }
            Some(DONE) => {
                let text = take_str(&mut m, "text").unwrap_or_default();
                let mut resp = TranscriptionResponse::new(String::new(), text);
                if let Some(u) = m.remove("usage") {
                    usage_of(&u, &self.origin, &mut resp);
                }
                resp.extras = extras_of(&self.origin, m);
                self.finished(resp)
            }
            // Mistral's events (OpenAPI `/v1/audio/transcriptions#stream`).
            Some("transcription.text.delta") => {
                let delta = take_str(&mut m, "text").unwrap_or_default();
                self.text.push_str(&delta);
                TranscriptEvent::delta(delta)
            }
            Some("transcription.done") => {
                let resp = from_json(Value::Object(m), TranscriptFormat::Json, &self.origin)?;
                self.finished(resp)
            }
            Some("transcription.language") => {
                self.language = take_str(&mut m, "audio_language");
                return Ok(Some(StreamEvent::Transcript(TranscriptEvent::default())));
            }
            Some("error") => {
                let body = serde_json::to_vec(&Value::Object(m)).unwrap_or_default();
                return Err(super::decode_error(500, &HeaderList::new(), &body));
            }
            // MiniMax's events: `{index, delta, finish, duration}`, without a type.
            None if m.contains_key("finish") => {
                let delta = take_str(&mut m, "delta").unwrap_or_default();
                self.text.push_str(&delta);
                if m.get("finish").and_then(Value::as_bool) == Some(true) {
                    let mut resp = TranscriptionResponse::new(String::new(), String::new());
                    resp.usage_seconds = m.get("duration").and_then(Value::as_f64);
                    resp.duration = resp.usage_seconds;
                    self.finished(resp)
                } else {
                    TranscriptEvent::delta(delta)
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(StreamEvent::Transcript(event)))
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        let SseItem::Event(ev) = item else {
            return;
        };
        if ev.data.trim() == "[DONE]" {
            return;
        }
        let parsed = match serde_json::from_str::<Value>(&ev.data) {
            Ok(Value::Object(m)) => self.event(m),
            _ => Err(bad("an event is not a JSON object")),
        };
        match parsed {
            Ok(Some(e)) => out.push(e),
            Ok(None) => {
                if let Ok(raw) = frame_extra(&ev) {
                    out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
                }
            }
            Err(err) => {
                self.failed = true;
                out.push(StreamEvent::Error(err.with_origin(self.origin.clone())));
            }
        }
    }
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        match self.parser.feed(bytes) {
            Ok(items) => {
                for item in items {
                    self.item(item, &mut out);
                }
            }
            Err(e) => {
                self.failed = true;
                out.push(StreamEvent::Error(e.into()));
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        match self.parser.finish() {
            Ok(items) => {
                for item in items {
                    self.item(item, &mut out);
                }
            }
            Err(e) => out.push(StreamEvent::Error(e.into())),
        }
        if !self.done && !self.failed {
            out.push(StreamEvent::Error(
                KppError::provider("The provider's stream ended before the transcript was done.")
                    .with_status(502),
            ));
        }
        out
    }
}

/// Writes `transcript.text.*` events.
pub(crate) struct Encoder;

impl Encoder {
    fn event(e: &TranscriptEvent) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(delta) = &e.delta {
            let mut m = Map::new();
            m.insert("type".to_owned(), DELTA.into());
            m.insert("delta".to_owned(), delta.clone().into());
            if let Some(id) = &e.segment_id {
                m.insert("segment_id".to_owned(), id.clone().into());
            }
            put_extras(&mut m, &e.extras);
            write_event(&mut out, None, &Value::Object(m).to_string());
        }
        if let Some(seg) = &e.segment {
            write_event(&mut out, None, &wire_segment(seg, true).to_string());
        }
        if let Some(done) = &e.done {
            let mut m = Map::new();
            m.insert("type".to_owned(), DONE.into());
            m.insert("text".to_owned(), done.text.clone().into());
            if let Some(u) = wire_usage(done) {
                m.insert("usage".to_owned(), u);
            }
            put_extras(&mut m, &done.extras);
            write_event(&mut out, None, &Value::Object(m).to_string());
        }
        out
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        match event {
            StreamEvent::Transcript(e) => out = Self::event(e),
            StreamEvent::Error(err) => {
                let body = crate::openai_responses::error::stream_error_event(err);
                write_event(&mut out, None, &body.to_string());
            }
            StreamEvent::Extra(item) if item.origin.is(DIALECT) => {
                write_frame_extra(&mut out, item.raw.as_raw()).map_err(|e| {
                    KppError::provider(format!("Kaeso could not write a streamed event ({e})."))
                })?;
            }
            _ => {}
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}
