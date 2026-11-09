//! Speech on DashScope: Qwen-Audio-TTS on `POST /api/v1/services/audio/tts/SpeechSynthesizer`
//! (<https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api>) and
//! Qwen3-TTS on the multimodal generation path
//! (<https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts>), read 2026-11-09.
//! A whole reply is a link to the audio; with `X-DashScope-SSE: enable` the audio arrives as
//! base64 pieces in server-sent events, which is how Kaeso asks whenever the format allows.

use kaeso_kpp::{
    AudioFormat, Extras, KppError, Origin, RawUsage, Reply, Request, Source, SpeechEvent,
    SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent, StreamMode, Usage, Voice,
};
use serde_json::{Map, Value};

use super::{D, PROVIDER, voices};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, StreamDecoder};
use crate::sse::{SseItem, SseParser};
use crate::video_jobs::{Fields, body, provider_object, put, put_extras, refused};

/// The path of Qwen-Audio-TTS, from the root of the host.
pub const SYNTHESIZER_PATH: &str = "/api/v1/services/audio/tts/SpeechSynthesizer";

const QWEN_AUDIO_FORMATS: &[&str] = &["mp3", "pcm", "wav", "opus"];

// Qwen3-TTS's `language_type` names, by the language codes Kaeso's requests carry (the
// Qwen-TTS reference's `input.language_type`).
const LANGUAGE_TYPES: &[(&str, &str)] = &[
    ("zh", "Chinese"),
    ("en", "English"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("es", "Spanish"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("fr", "French"),
    ("ru", "Russian"),
];

/// How a QwenCloud speech model takes a request on DashScope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpeechFamily {
    /// Qwen-Audio-TTS 3.0 and 3.1: mp3, pcm, wav or opus, streamed in that format.
    QwenAudio,
    /// Qwen3-TTS Flash: a link to a wav file, or 24 kHz pcm when streamed.
    Qwen3,
}

/// The family of a QwenCloud speech model, or `None` for one Kaeso doesn't send, such as the
/// real-time models that take only WebSocket connections.
pub fn speech_family(model: &str) -> Option<SpeechFamily> {
    let m = model.to_ascii_lowercase();
    if m.contains("realtime") {
        None
    } else if m.starts_with("qwen-audio-") && m.contains("-tts-") {
        Some(SpeechFamily::QwenAudio)
    } else if m.starts_with("qwen3-tts-flash") {
        Some(SpeechFamily::Qwen3)
    } else {
        None
    }
}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn system_voices(model: &str) -> &'static [&'static str] {
    let m = model.to_ascii_lowercase();
    if m.starts_with("qwen-audio-3.1-tts-plus") {
        voices::AUDIO_31_PLUS
    } else if m.starts_with("qwen-audio-3.1-tts-flash") {
        voices::AUDIO_31_FLASH
    } else if m.starts_with("qwen-audio-3.0-tts-plus") {
        voices::AUDIO_30_PLUS
    } else if m.starts_with("qwen-audio-3.0-tts-flash") {
        voices::AUDIO_30_FLASH
    } else {
        voices::QWEN3_TTS
    }
}

// A named voice is a system voice of the model, or one of its base voices, which the voice list
// names `<model>-<suffix>`; an explicit voice id, such as a cloned voice, goes as it is.
fn known_voice(model: &str, name: &str) -> bool {
    system_voices(model).contains(&name)
        || (speech_family(model) == Some(SpeechFamily::QwenAudio)
            && name.starts_with(&format!("{model}-")))
}

fn wire_format(f: &AudioFormat) -> &str {
    match f {
        AudioFormat::Pcm16 => "pcm",
        other => other.as_wire(),
    }
}

fn format_of(wire: &str) -> AudioFormat {
    match wire {
        "pcm" => AudioFormat::Pcm16,
        other => AudioFormat::from_wire(other),
    }
}

/// The media type of audio in `format`, as Qwen-Audio-TTS writes it.
pub(super) fn media_type(format: Option<&AudioFormat>) -> String {
    match format {
        None => "audio/mpeg".to_owned(),
        Some(AudioFormat::Opus) => "audio/ogg".to_owned(),
        Some(AudioFormat::Pcm16) => kaeso_kpp::PCM16_MEDIA_TYPE.to_owned(),
        Some(f) => f
            .media_type()
            .unwrap_or("application/octet-stream")
            .to_owned(),
    }
}

pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut extras = Extras::new(Origin::new(D));
    let model = f.str("model")?.unwrap_or_default();
    let mut r = SpeechRequest::new(model, SpeechOperation::Speech);
    if let Some(mut input) = f.object("input")? {
        r.input = input.str("text")?;
        r.voice = input.str("voice")?.map(Voice::Named);
        r.format = input.str("format")?.as_deref().map(format_of);
        r.sample_rate = input.u32("sample_rate")?;
        r.bitrate = input.u32("bit_rate")?;
        r.instructions = input.str("instruction")?;
        r.seed = input.i64("seed")?;
        if let Some(Value::Number(n)) = input.peek("rate") {
            r.speed = n.as_f64();
            if n.is_i64() || n.is_u64() {
                extras.form.insert("rate_integer".into(), Value::Bool(true));
            }
            input.take("rate");
        }
        if let Some(Value::Array(list)) = input.peek("language_hints")
            && let [Value::String(l)] = list.as_slice()
        {
            r.language = Some(l.clone());
            input.take("language_hints");
        }
        if let Some(Value::String(name)) = input.peek("language_type")
            && let Some((code, _)) = LANGUAGE_TYPES.iter().find(|(_, n)| n == name)
        {
            r.language = Some((*code).to_owned());
            input.take("language_type");
        }
        input.rest_into(&mut extras, "input");
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Speech(r))
}

pub(super) fn check(r: &SpeechRequest) -> Result<SpeechFamily, KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    let Some(fam) = speech_family(&r.model) else {
        return Err(KppError::unsupported_by(
            format!("the model `{}`", r.model),
            D,
            "Kaeso sends Qwen-Audio-TTS and Qwen3-TTS Flash to DashScope; real-time models take \
             only WebSocket connections.",
        )
        .with_param("model"));
    };
    if r.operation != SpeechOperation::Speech || !r.dialogue.is_empty() || r.audio.is_some() {
        return Err(no(r.operation.as_str(), "model", "send text to speak."));
    }
    let unsupported = [
        ("timestamps", r.timestamps),
        ("duration", r.duration.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    match &r.voice {
        None => {
            return Err(no(
                "speech without a voice",
                "voice",
                "name a voice from QwenCloud's voice list.",
            ));
        }
        Some(Voice::Named(name)) if !known_voice(&r.model, name) => {
            return Err(no(
                &format!("the voice `{name}` on `{}`", r.model),
                "voice",
                "choose one from QwenCloud's voice list for this model.",
            ));
        }
        _ => {}
    }
    match fam {
        SpeechFamily::QwenAudio => {
            if let Some(f) = &r.format
                && !QWEN_AUDIO_FORMATS.contains(&wire_format(f))
            {
                return Err(no(
                    &format!("the format `{}`", f.as_wire()),
                    "response_format",
                    "use mp3, pcm, wav or opus.",
                ));
            }
            if let Some(s) = r.speed
                && !(0.5..=2.0).contains(&s)
            {
                return Err(no(
                    &format!("the speed {s}"),
                    "speed",
                    "choose a speed from 0.5 to 2.0.",
                ));
            }
            if r.bitrate.is_some() && r.format != Some(AudioFormat::Opus) {
                return Err(no(
                    "a bitrate for this format",
                    "bitrate",
                    "ask for opus, or remove it.",
                ));
            }
        }
        SpeechFamily::Qwen3 => {
            if let Some(f) = &r.format
                && !matches!(f, AudioFormat::Wav | AudioFormat::Pcm16)
            {
                return Err(no(
                    &format!("the format `{}` on Qwen3-TTS", f.as_wire()),
                    "response_format",
                    "use wav or pcm.",
                ));
            }
            if let Some(l) = &r.language
                && !LANGUAGE_TYPES.iter().any(|(code, _)| code == l)
            {
                return Err(no(
                    &format!("the language `{l}` on Qwen3-TTS"),
                    "language",
                    "use zh, en, de, it, pt, es, ja, ko, fr or ru, or leave it out.",
                ));
            }
            let unsupported = [
                ("speed", r.speed.is_some()),
                ("instructions", r.instructions.is_some()),
                ("seed", r.seed.is_some()),
                ("bitrate", r.bitrate.is_some()),
                ("sample_rate", r.sample_rate.is_some_and(|s| s != 24_000)),
            ];
            if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
                return Err(no(&format!("`{field}` on Qwen3-TTS"), field, "remove it."));
            }
        }
    }
    Ok(fam)
}

fn rate(r: &SpeechRequest) -> Option<Value> {
    let s = r.speed?;
    if r.extras.form_flag(D, "rate_integer")
        && let Ok(whole) = format!("{s}").parse::<i64>()
    {
        return Some(Value::from(whole));
    }
    serde_json::Number::from_f64(s).map(Value::Number)
}

pub(super) fn encode_request(
    r: &SpeechRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    let fam = check(r)?;
    let own = r.extras.is_from(D);
    let mut input = Map::new();
    input.insert("text".into(), r.input.clone().unwrap_or_default().into());
    put(
        &mut input,
        "voice",
        r.voice.as_ref().map(|v| v.as_str().to_owned()),
    );
    let mut headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    let path = match fam {
        SpeechFamily::QwenAudio => {
            put(
                &mut input,
                "format",
                r.format.as_ref().map(|f| wire_format(f).to_owned()),
            );
            let sample_rate = match (r.sample_rate, &r.format) {
                // Kaeso's `pcm16` is 24 kHz; DashScope's `pcm` defaults to 22,050 Hz.
                (None, Some(AudioFormat::Pcm16)) if !own => Some(24_000),
                (s, _) => s,
            };
            put(&mut input, "sample_rate", sample_rate);
            put(&mut input, "rate", rate(r));
            put(&mut input, "bit_rate", r.bitrate);
            put(&mut input, "instruction", r.instructions.clone());
            put(&mut input, "seed", r.seed);
            put(
                &mut input,
                "language_hints",
                r.language.clone().map(|l| Value::Array(vec![l.into()])),
            );
            ctx.audio_format = Some(r.format.clone().unwrap_or(AudioFormat::Mp3));
            ctx.stream = StreamMode::on(false);
            headers.push("x-dashscope-sse", "enable");
            SYNTHESIZER_PATH
        }
        SpeechFamily::Qwen3 => {
            let language = r.language.as_deref().and_then(|l| {
                LANGUAGE_TYPES
                    .iter()
                    .find(|(code, _)| *code == l)
                    .map(|(_, name)| (*name).to_owned())
            });
            put(&mut input, "language_type", language);
            // Streamed pieces are 24 kHz pcm; a whole reply is a link to a wav file.
            if r.format == Some(AudioFormat::Pcm16) {
                ctx.audio_format = Some(AudioFormat::Pcm16);
                ctx.stream = StreamMode::on(false);
                headers.push("x-dashscope-sse", "enable");
            } else {
                ctx.audio_format = Some(AudioFormat::Wav);
                ctx.stream = StreamMode::Off;
            }
            super::images::GENERATION_PATH
        }
    };
    put_extras(&mut input, &r.extras, D, "input");
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    out.insert("input".into(), Value::Object(input));
    put_extras(&mut out, &r.extras, D, "");
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn usage_of(value: &Value, origin: &Origin) -> (Usage, Option<u64>) {
    let n = |k: &str| value.get(k).and_then(Value::as_u64).unwrap_or(0);
    let mut usage = Usage::new(n("input_tokens"), n("output_tokens"));
    usage.raw = Some(RawUsage::new(origin.clone(), value.clone()));
    (usage, value.get("characters").and_then(Value::as_u64))
}

pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<Reply, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let mut output = f.object("output")?.ok_or_else(|| {
        KppError::provider("QwenCloud answered a speech request without its `output`.")
    })?;
    let mut audio = output.object("audio")?.ok_or_else(|| {
        KppError::provider("QwenCloud answered a speech request without its audio.")
    })?;
    let url = audio.str("url")?;
    let source = match (url, audio.peek("data").and_then(Value::as_str)) {
        (Some(url), _) => Source::Url(url),
        (None, Some(data)) if !data.is_empty() => {
            let s = Source::base64(media_type(ctx.audio_format.as_ref()), data).map_err(|_| {
                KppError::provider("QwenCloud sent audio that is not valid base64.")
            })?;
            audio.take("data");
            s
        }
        _ => {
            return Err(KppError::provider(
                "QwenCloud answered a speech request without its audio.",
            ));
        }
    };
    audio.rest_into(&mut extras, "output.audio");
    output.rest_into(&mut extras, "output");
    let mut resp = SpeechResponse::new(String::new(), source);
    if let Some(u) = f.take("usage") {
        let (usage, characters) = usage_of(&u, &ctx.origin(D));
        resp.usage = usage;
        resp.characters = characters;
    }
    f.rest_into(&mut extras, "");
    resp.extras = extras;
    Ok(Reply::Speech(resp))
}

pub(super) fn encode_response(r: &SpeechResponse) -> Result<Vec<u8>, KppError> {
    let mut audio = Map::new();
    match &r.audio {
        Source::Url(u) => {
            audio.insert("url".into(), u.clone().into());
        }
        Source::Base64(b) => {
            audio.insert("data".into(), b.to_base64().into());
        }
        _ => {
            return Err(KppError::provider(
                "A DashScope speech reply carries a link or the audio itself.",
            ));
        }
    }
    put_extras(&mut audio, &r.extras, D, "output.audio");
    let mut output = Map::new();
    output.insert("audio".into(), Value::Object(audio));
    put_extras(&mut output, &r.extras, D, "output");
    let mut out = Map::new();
    out.insert("output".into(), Value::Object(output));
    let usage = match &r.usage.raw {
        Some(raw) if raw.origin.is(D) => Some(raw.value.clone()),
        _ => None,
    };
    put(&mut out, "usage", usage);
    put_extras(&mut out, &r.extras, D, "");
    body(&out)
}

/// Reads DashScope's speech events: each event's `output.audio.data` is a base64 piece of the
/// audio, and the event whose `finish_reason` is `stop` ends it with the usage.
pub(super) struct Decoder {
    parser: SseParser,
    origin: Origin,
    media_type: Option<String>,
    status: u16,
    usage: Option<(Usage, Option<u64>)>,
    done: bool,
    failed: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            parser: SseParser::new(),
            origin: ctx.origin(D),
            media_type: Some(media_type(ctx.audio_format.as_ref())),
            status: 200,
            usage: None,
            done: false,
            failed: false,
        }
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        let ev = match item {
            SseItem::Comment(c) => {
                if let Some(s) = c.trim().strip_prefix("HTTP_STATUS/") {
                    self.status = s.trim().parse().unwrap_or(self.status);
                }
                return;
            }
            SseItem::Event(ev) => ev,
        };
        let value: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(e) => {
                self.failed = true;
                out.push(StreamEvent::Error(KppError::provider(format!(
                    "QwenCloud sent an event that is not JSON: {}.",
                    crate::wire::without_position(&e)
                ))));
                return;
            }
        };
        let errored = ev.event.as_deref() == Some("error")
            || !(200..300).contains(&self.status)
            || (value.get("output").is_none()
                && value
                    .get("code")
                    .and_then(Value::as_str)
                    .is_some_and(|c| !c.is_empty()));
        if errored {
            self.failed = true;
            let status = if (200..300).contains(&self.status) {
                500
            } else {
                self.status
            };
            let body = serde_json::to_vec(&value).unwrap_or_default();
            out.push(StreamEvent::Error(
                super::decode_error(status, &HeaderList::new(), &body)
                    .with_origin(self.origin.clone()),
            ));
            return;
        }
        if let Some(u) = value.get("usage") {
            self.usage = Some(usage_of(u, &self.origin));
        }
        let output = value.get("output");
        if let Some(data) = output
            .and_then(|o| o.pointer("/audio/data"))
            .and_then(Value::as_str)
            .filter(|d| !d.is_empty())
        {
            let mut piece = SpeechEvent::audio(data);
            piece.media_type = self.media_type.take();
            out.push(StreamEvent::Speech(piece));
        }
        if output
            .and_then(|o| o.get("finish_reason"))
            .and_then(Value::as_str)
            == Some("stop")
        {
            self.done = true;
            let (usage, characters) = self
                .usage
                .take()
                .map_or((None, None), |(u, c)| (Some(u), c));
            let mut end = SpeechEvent::done(usage);
            end.characters = characters;
            if let Some(audio) = output
                .and_then(|o| o.get("audio"))
                .and_then(Value::as_object)
            {
                let mut kept = audio.clone();
                kept.remove("data");
                if let Ok(raw) = kaeso_kpp::RawJson::from_serialize(&kept) {
                    end.extras = Extras::new(self.origin.clone()).with_field("audio", raw);
                }
            }
            out.push(StreamEvent::Speech(end));
        }
    }
}

impl crate::codec::sealed::Sealed for Decoder {}

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
            Err(e) => {
                self.failed = true;
                out.push(StreamEvent::Error(e.into()));
            }
        }
        if !self.done && !self.failed {
            out.push(StreamEvent::Error(
                KppError::provider("QwenCloud's stream ended before the audio was done.")
                    .with_status(502),
            ));
        }
        out
    }
}
