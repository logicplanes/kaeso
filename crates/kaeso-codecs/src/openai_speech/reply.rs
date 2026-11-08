use base64::Engine as _;
use kaeso_kpp::{
    KppError, Origin, RawItem, RawUsage, Reply, Request, RequestKind, Source, SpeechEvent,
    SpeechResponse, StreamEvent, Usage,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use super::profile::{SpeechEvents, SpeechReply};
use super::request::media_type_of;
use crate::codec::{EgressCtx, EncodeCtx, HeaderList, StreamDecoder, StreamEncoder};
use crate::sse::{SseItem, SseParser, frame_extra, write_event, write_frame_extra};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const DELTA: &str = "speech.audio.delta";
const DONE: &str = "speech.audio.done";

fn media_type(headers: &HeaderList, ctx: &EgressCtx) -> String {
    match headers.get("content-type") {
        Some(t) if crate::media::is_audio(t) => t.to_owned(),
        _ => media_type_of(ctx.audio_format.as_ref()).to_owned(),
    }
}

fn is_event_stream(headers: &HeaderList) -> bool {
    headers
        .get("content-type")
        .is_some_and(|t| crate::media::essence(t) == "text/event-stream")
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
    if ctx.kind != RequestKind::Speech {
        return Err(KppError::unsupported_by("generation response", DIALECT, ""));
    }
    if is_event_stream(headers) {
        let mut decoder = Decoder::new(ctx, true);
        let mut events = decoder.feed(body);
        events.extend(decoder.finish());
        return assemble(&events, &media_type(headers, ctx), &ctx.origin(DIALECT));
    }
    let data = match ctx.speech_profile.reply {
        SpeechReply::JsonBase64(key) => {
            let value: Value = serde_json::from_slice(body).map_err(|e| {
                KppError::provider(format!(
                    "The provider sent a speech reply that is not JSON: {}.",
                    crate::wire::without_position(&e)
                ))
            })?;
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| {
                    KppError::provider(format!("The provider's speech reply has no `{key}`."))
                })?
        }
        _ => B64.encode(body),
    };
    let media = match ctx.speech_profile.reply {
        SpeechReply::Bytes => media_type(headers, ctx),
        _ => media_type_of(ctx.audio_format.as_ref()).to_owned(),
    };
    let resp = SpeechResponse::new(String::new(), Source::base64(media, data));
    Ok(Reply::Speech(resp))
}

/// Whether the reply is read as it arrives: events, or audio a provider sends as it makes it.
pub(super) fn streams(ctx: &EgressCtx) -> bool {
    !ctx.stream.is_off() || ctx.speech_profile.reply == SpeechReply::Bytes
}

/// One reply from the events of a stream: the audio pieces joined, the usage of the end.
pub(crate) fn assemble(
    events: &[StreamEvent],
    media_type: &str,
    origin: &Origin,
) -> Result<Reply, KppError> {
    let mut bytes = Vec::new();
    let mut usage = Usage::default();
    let mut media = media_type.to_owned();
    for event in events {
        match event {
            StreamEvent::Speech(e) => {
                if let Some(data) = &e.audio {
                    bytes.extend(B64.decode(data).map_err(|_| {
                        KppError::provider("The provider sent audio that is not valid base64.")
                    })?);
                }
                if let Some(t) = &e.media_type {
                    media.clone_from(t);
                }
                if let Some(u) = &e.usage {
                    usage = u.clone();
                }
            }
            StreamEvent::Error(err) => return Err(err.clone().with_origin(origin.clone())),
            _ => {}
        }
    }
    let mut resp = SpeechResponse::new(String::new(), Source::base64(media, B64.encode(bytes)));
    resp.usage = usage;
    Ok(Reply::Speech(resp))
}

fn usage_of(raw: &Value, origin: &Origin) -> Usage {
    let n = |a: &str, b: &str| {
        raw.get(a)
            .or_else(|| raw.get(b))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let mut u = Usage::new(
        n("input_tokens", "prompt_tokens"),
        n("output_tokens", "completion_tokens"),
    );
    u.output_audio_tokens = Some(u.output_tokens);
    u.raw = Some(RawUsage::new(origin.clone(), raw.clone()));
    u
}

fn wire_usage(u: &Usage, dialect: kaeso_kpp::DialectId) -> Value {
    if let Some(raw) = u.raw_for(dialect) {
        return raw.clone();
    }
    json!({
        "input_tokens": u.input_tokens,
        "output_tokens": u.output_tokens,
        "total_tokens": u.input_tokens.saturating_add(u.output_tokens),
    })
}

/// Reads the provider's reply as it arrives: `speech.audio.*` events when Kaeso asked for
/// them, otherwise the audio itself, a piece per chunk.
pub(crate) struct Decoder {
    events: bool,
    audio_key: &'static str,
    parser: SseParser,
    origin: Origin,
    media_type: Option<String>,
    done: bool,
    failed: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx, events: bool) -> Decoder {
        Decoder {
            events,
            audio_key: match ctx.speech_profile.events {
                Some(SpeechEvents::StreamFlag(key)) => key,
                _ => "audio",
            },
            parser: SseParser::new(),
            origin: ctx.origin(DIALECT),
            media_type: Some(media_type_of(ctx.audio_format.as_ref()).to_owned()),
            done: false,
            failed: false,
        }
    }

    fn first(&mut self, mut e: SpeechEvent) -> SpeechEvent {
        e.media_type = self.media_type.take();
        e
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        let SseItem::Event(ev) = item else {
            return;
        };
        if ev.data.trim() == "[DONE]" {
            return;
        }
        let value: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(e) => {
                self.failed = true;
                out.push(StreamEvent::Error(KppError::provider(format!(
                    "The provider sent an event that is not JSON: {}.",
                    crate::wire::without_position(&e)
                ))));
                return;
            }
        };
        match value.get("type").and_then(Value::as_str) {
            Some(DELTA) => {
                let audio = value
                    .get(self.audio_key)
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let e = self.first(SpeechEvent::audio(audio));
                out.push(StreamEvent::Speech(e));
            }
            Some(DONE) => {
                self.done = true;
                let usage = value.get("usage").map(|u| usage_of(u, &self.origin));
                out.push(StreamEvent::Speech(SpeechEvent::done(usage)));
            }
            Some("error") => {
                self.failed = true;
                let body = serde_json::to_vec(&value).unwrap_or_default();
                out.push(StreamEvent::Error(
                    super::decode_error(500, &HeaderList::new(), &body)
                        .with_origin(self.origin.clone()),
                ));
            }
            _ => match frame_extra(&ev) {
                Ok(raw) => out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw))),
                Err(_) => self.failed = true,
            },
        }
    }
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if !self.events {
            if !bytes.is_empty() {
                let e = self.first(SpeechEvent::audio(B64.encode(bytes)));
                out.push(StreamEvent::Speech(e));
            }
            return out;
        }
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
        if self.events {
            match self.parser.finish() {
                Ok(items) => {
                    for item in items {
                        self.item(item, &mut out);
                    }
                }
                Err(e) => out.push(StreamEvent::Error(e.into())),
            }
        }
        if !self.done && !self.failed {
            if self.events {
                out.push(StreamEvent::Error(
                    KppError::provider("The provider's stream ended before the audio was done.")
                        .with_status(502),
                ));
            } else {
                self.done = true;
                out.push(StreamEvent::Speech(SpeechEvent::done(None)));
            }
        }
        out
    }
}

fn wants_events(ctx: &EncodeCtx) -> bool {
    !ctx.stream.is_off()
}

/// The `content-type` of the app's reply: events, or the audio in the format it asked for.
pub(super) fn app_media_type(ctx: &EncodeCtx, audio: Option<&Source>) -> String {
    if wants_events(ctx) {
        return crate::codec::EVENT_STREAM_MEDIA_TYPE.to_owned();
    }
    match ctx.request.as_deref() {
        Some(Request::Speech(r)) if r.format.is_some() => {
            media_type_of(r.format.as_ref()).to_owned()
        }
        _ => match audio {
            Some(Source::Base64(b)) => b.media_type.clone(),
            _ => media_type_of(None).to_owned(),
        },
    }
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Speech(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "",
        ));
    };
    let Source::Base64(b) = &r.audio else {
        return Err(KppError::provider(
            "The provider answered with a link to the audio, which this endpoint cannot return.",
        ));
    };
    let mut enc = Encoder::new(wants_events(ctx));
    let mut out = enc.encode(&StreamEvent::Speech(SpeechEvent::audio(b.data.clone())))?;
    let mut done = SpeechEvent::done(Some(r.usage.clone()));
    done.extras.clone_from(&r.extras);
    out.extend(enc.encode(&StreamEvent::Speech(done))?);
    Ok(out)
}

/// Writes `speech.audio.*` events for an app that asked for them, otherwise the audio bytes.
pub(crate) struct Encoder {
    events: bool,
}

impl Encoder {
    pub(super) fn new(events: bool) -> Encoder {
        Encoder { events }
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        let mut out = Vec::new();
        match event {
            StreamEvent::Speech(e) if !self.events => {
                if let Some(data) = &e.audio {
                    out = B64.decode(data).map_err(|_| {
                        KppError::provider("The provider sent audio that is not valid base64.")
                    })?;
                }
            }
            StreamEvent::Speech(e) => {
                if let Some(data) = &e.audio {
                    let body = json!({ "type": DELTA, "audio": data });
                    write_event(&mut out, None, &body.to_string());
                }
                if e.done {
                    let mut m = Map::new();
                    m.insert("type".to_owned(), DONE.into());
                    let usage = e.usage.clone().unwrap_or_default();
                    m.insert("usage".to_owned(), wire_usage(&usage, DIALECT));
                    write_event(&mut out, None, &Value::Object(m).to_string());
                }
            }
            StreamEvent::Error(err) if self.events => {
                let body = crate::openai_responses::error::stream_error_event(err);
                write_event(&mut out, None, &body.to_string());
            }
            StreamEvent::Extra(item) if self.events && item.origin.is(DIALECT) => {
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
