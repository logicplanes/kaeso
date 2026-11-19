//! Sonic text to speech, `POST /tts/bytes`
//! (<https://docs.cartesia.ai/api-reference/tts/bytes>): `{model_id, transcript, voice,
//! output_format, language, generation_config}` in, the audio out, sent as it is made.

use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, KppError, Origin, Reply, Request, RequestKind, Source, SpeechEvent,
    SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent, StreamMode, Voice,
};
use serde_json::{Map, Value, json};

use super::{D, PROVIDER, TTS_PATH, versioned};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, StreamDecoder, StreamEncoder};
use crate::video_jobs::{Fields, put_extras, refused};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const GENERATION_CONFIG: &str = "generation_config";

// `generation_config.speed` (the 2026-08-14 reference: 0.6 to 1.5, Sonic 3 and newer).
const SPEED: std::ops::RangeInclusive<f64> = 0.6..=1.5;
// `output_format.sample_rate` and the mp3 `bit_rate` of the reference.
const SAMPLE_RATES: [u32; 6] = [8000, 16000, 22050, 24000, 44100, 48000];
const MP3_KBPS: [u32; 5] = [32, 64, 96, 128, 192];
// OpenAI's `wav` and `pcm` are 16-bit PCM at 24 kHz; Cartesia's mp3 defaults are 44.1 kHz at
// 128 kbps (https://docs.cartesia.ai/build-with-cartesia/capability-guides/tts-output-audio-format).
const PCM_RATE: u32 = 24_000;
const MP3_RATE: u32 = 44_100;
const MP3_KBPS_DEFAULT: u32 = 128;

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn invalid(message: &str, param: &str) -> KppError {
    KppError::invalid_request(message).with_param(param.to_owned())
}

fn u32_of(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

/// The format, sample rate and mp3 bit rate in kbps of an `output_format` object; only the
/// forms Kaeso writes are read.
fn format_of(v: Value) -> Result<(AudioFormat, Option<u32>, Option<u32>), KppError> {
    let Value::Object(m) = v else {
        return Err(invalid("`output_format` is an object.", "output_format"));
    };
    let text = |k: &str| m.get(k).and_then(Value::as_str);
    let rate = u32_of(m.get("sample_rate"));
    let keys = |allowed: &[&str]| m.keys().all(|k| allowed.contains(&k.as_str()));
    match (text("container"), text("encoding")) {
        (Some("mp3"), None) if keys(&["container", "sample_rate", "bit_rate"]) => Ok((
            AudioFormat::Mp3,
            rate,
            u32_of(m.get("bit_rate")).map(|b| b / 1000),
        )),
        (Some("wav"), Some("pcm_s16le")) if keys(&["container", "encoding", "sample_rate"]) => {
            Ok((AudioFormat::Wav, rate, None))
        }
        (Some("raw"), Some("pcm_s16le")) if keys(&["container", "encoding", "sample_rate"]) => {
            Ok((AudioFormat::Pcm16, rate, None))
        }
        _ => Err(no(
            "this `output_format`",
            "output_format",
            "use mp3, or wav or raw with pcm_s16le.",
        )),
    }
}

/// A Cartesia-format speech request, as the dialect's fixtures write it.
pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut r = SpeechRequest::new(
        f.str("model_id")?.unwrap_or_default(),
        SpeechOperation::Speech,
    );
    r.input = f.str("transcript")?;
    r.voice = match f.take("voice") {
        None => None,
        Some(Value::String(id)) => Some(Voice::Id(id)),
        Some(Value::Object(m)) => match (m.get("id").and_then(Value::as_str), m.len()) {
            (Some(id), 1) => Some(Voice::Id(id.to_owned())),
            _ => return Err(no("this `voice` object", "voice", "send the voice's id.")),
        },
        Some(_) => return Err(invalid("`voice` is a voice id.", "voice")),
    };
    if let Some(v) = f.take("output_format") {
        let (format, rate, kbps) = format_of(v)?;
        r.format = Some(format);
        r.sample_rate = rate;
        r.bitrate = kbps;
    }
    r.language = f.str("language")?;
    let config = f.object(GENERATION_CONFIG)?;
    let mut extras = kaeso_kpp::Extras::new(Origin::new(D));
    if let Some(mut config) = config {
        r.speed = match config.take("speed") {
            None => None,
            Some(v) => Some(
                v.as_f64()
                    .ok_or_else(|| invalid("`speed` is a number.", "speed"))?,
            ),
        };
        config.rest_into(&mut extras, GENERATION_CONFIG);
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Speech(r))
}

/// Checks that Cartesia can speak `r`, before anything is sent.
pub(super) fn check(r: &SpeechRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if r.operation != SpeechOperation::Speech {
        return Err(no(r.operation.as_str(), "model", "send text to speak."));
    }
    if !r.dialogue.is_empty() || r.audio.is_some() {
        return Err(no("dialogue or source audio", "input", "send text."));
    }
    if r.input.as_deref().is_none_or(str::is_empty) {
        return Err(invalid("Send the text to speak as `input`.", "input"));
    }
    if r.voice.is_none() {
        return Err(invalid(
            "Name a Cartesia voice by its id, such as db6b0ed5-d5d3-463d-ae85-518a07d3c2b4.",
            "voice",
        ));
    }
    let unsupported = [
        ("instructions", r.instructions.is_some()),
        ("seed", r.seed.is_some()),
        ("timestamps", r.timestamps),
        ("duration", r.duration.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    match &r.format {
        None | Some(AudioFormat::Mp3 | AudioFormat::Wav | AudioFormat::Pcm16) => {}
        Some(other) => {
            return Err(no(
                &format!("the format `{}`", other.as_wire()),
                "response_format",
                "use mp3, wav or pcm.",
            ));
        }
    }
    if let Some(rate) = r.sample_rate
        && !SAMPLE_RATES.contains(&rate)
    {
        return Err(no(
            &format!("a sample rate of {rate}"),
            "sample_rate",
            "use 8000, 16000, 22050, 24000, 44100 or 48000.",
        ));
    }
    if let Some(kbps) = r.bitrate
        && (!MP3_KBPS.contains(&kbps) || !matches!(r.format, None | Some(AudioFormat::Mp3)))
    {
        return Err(no(
            &format!("a bit rate of {kbps} kbps here"),
            "bitrate",
            "use 32, 64, 96, 128 or 192 with mp3.",
        ));
    }
    if let Some(s) = r.speed
        && !SPEED.contains(&s)
    {
        return Err(invalid(
            "Cartesia takes a `speed` from 0.6 to 1.5.",
            "speed",
        ));
    }
    Ok(())
}

fn output_format(r: &SpeechRequest, format: &AudioFormat) -> Value {
    match format {
        AudioFormat::Wav => json!({
            "container": "wav",
            "encoding": "pcm_s16le",
            "sample_rate": r.sample_rate.unwrap_or(PCM_RATE),
        }),
        AudioFormat::Pcm16 => json!({
            "container": "raw",
            "encoding": "pcm_s16le",
            "sample_rate": r.sample_rate.unwrap_or(PCM_RATE),
        }),
        _ => json!({
            "container": "mp3",
            "sample_rate": r.sample_rate.unwrap_or(MP3_RATE),
            "bit_rate": r.bitrate.unwrap_or(MP3_KBPS_DEFAULT).saturating_mul(1000),
        }),
    }
}

pub(super) fn encode_request(
    r: &SpeechRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check(r)?;
    ctx.stream = StreamMode::Off;
    let format = r.format.clone().unwrap_or(AudioFormat::Mp3);
    let mut body = Map::new();
    body.insert("model_id".into(), r.model.clone().into());
    body.insert(
        "transcript".into(),
        r.input.clone().unwrap_or_default().into(),
    );
    if let Some(v) = &r.voice {
        body.insert("voice".into(), v.as_str().into());
    }
    body.insert("output_format".into(), output_format(r, &format));
    if let Some(l) = &r.language {
        body.insert("language".into(), l.clone().into());
    }
    let mut config = Map::new();
    if let Some(s) = r.speed {
        config.insert("speed".into(), s.into());
    }
    put_extras(&mut config, &r.extras, D, GENERATION_CONFIG);
    if !config.is_empty() {
        body.insert(GENERATION_CONFIG.into(), Value::Object(config));
    }
    put_extras(&mut body, &r.extras, D, "");
    ctx.audio_format = Some(format);
    Ok(EncodedRequest::post_json(
        TTS_PATH,
        versioned(HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE)),
        crate::wire::to_body(&body)?,
    ))
}

/// The content type of the audio Kaeso asks Cartesia for.
pub(super) fn media_type_of(format: Option<&AudioFormat>) -> String {
    match format {
        Some(AudioFormat::Pcm16) => format!("audio/l16;rate={PCM_RATE}"),
        Some(AudioFormat::Wav) => "audio/wav".to_owned(),
        _ => "audio/mpeg".to_owned(),
    }
}

fn media_of(headers: &HeaderList, ctx: &EgressCtx) -> String {
    match headers.get("content-type") {
        Some(t) if crate::media::is_audio(t) => t.to_owned(),
        _ => media_type_of(ctx.audio_format.as_ref()),
    }
}

pub(super) fn decode_response(headers: &HeaderList, body: &[u8], ctx: &EgressCtx) -> Reply {
    Reply::Speech(SpeechResponse::new(
        String::new(),
        Source::bytes(media_of(headers, ctx), bytes::Bytes::copy_from_slice(body)),
    ))
}

pub(super) fn encode_response(r: &SpeechResponse) -> Result<Vec<u8>, KppError> {
    match &r.audio {
        Source::Base64(b) => Ok(b.data.to_vec()),
        _ => Err(KppError::provider(
            "The provider answered with a link to the audio, which Cartesia's format cannot carry.",
        )),
    }
}

pub(super) fn media_type_of_reply(r: &SpeechResponse) -> String {
    match &r.audio {
        Source::Base64(b) => b.media_type.clone(),
        _ => media_type_of(None),
    }
}

/// Reads the audio as it arrives, a piece per chunk.
pub(crate) struct Decoder {
    media_type: Option<String>,
    done: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            media_type: Some(media_type_of(ctx.audio_format.as_ref())),
            done: false,
        }
    }
}

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        if bytes.is_empty() || self.done {
            return Vec::new();
        }
        let mut e = SpeechEvent::audio(B64.encode(bytes));
        e.media_type = self.media_type.take();
        vec![StreamEvent::Speech(e)]
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }
        self.done = true;
        vec![StreamEvent::Speech(SpeechEvent::done(None))]
    }
}

/// Writes Cartesia's replies as they arrive: the audio bytes, or a whole transcript at its end.
pub(crate) struct Encoder {
    kind: RequestKind,
}

impl Encoder {
    pub(super) fn new(kind: RequestKind) -> Encoder {
        Encoder { kind }
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        match event {
            StreamEvent::Speech(e) if self.kind != RequestKind::Transcription => match &e.audio {
                Some(data) => B64.decode(data).map_err(|_| {
                    KppError::provider("The provider sent audio that is not valid base64.")
                }),
                None => Ok(Vec::new()),
            },
            StreamEvent::Transcript(t) => match &t.done {
                Some(done) => super::stt::encode_response(done),
                None => Ok(Vec::new()),
            },
            StreamEvent::Error(err) => Err(err.clone()),
            _ => Ok(Vec::new()),
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}
