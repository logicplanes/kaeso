//! Aura text to speech, `POST /v1/speak`
//! (<https://developers.deepgram.com/reference/text-to-speech/speak-request>): `{"text"}` in,
//! the model (which is the voice, `aura-2-thalia-en`) and the audio format in the query
//! string, and the audio itself out, sent as it is made.

use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, Extras, KppError, Origin, Reply, Request, RequestKind, Source, SpeechEvent,
    SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent, StreamMode,
};
use serde_json::Map;

use super::listen::{keep_query, path_under, query_extras, with_query};
use super::{D, SPEAK_PATH};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, StreamDecoder, StreamEncoder};
use crate::video_jobs::{Fields, put_extras, refused};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const PROVIDER: &str = "Deepgram";
const BODY: &str = "body";
const FORM_SPEED: &str = "speed";

/// The model families Kaeso offers for Deepgram's speech, each a set of voices named
/// `{family}-{voice}-{language}` (<https://developers.deepgram.com/docs/tts-models>).
pub const SPEECH_FAMILIES: [&str; 2] = ["aura-2", "aura"];

/// The most characters Aura and Aura-2 speak in one request; Deepgram answers 413 above it
/// (<https://developers.deepgram.com/docs/text-to-speech>, "Input text limit").
pub const MAX_SPEECH_CHARACTERS: usize = 2000;

// `speed` of `/v1/speak` (OpenAPI `V1SpeakPost` parameters: minimum 0.7, maximum 1.5).
const SPEED: std::ops::RangeInclusive<f64> = 0.7..=1.5;

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

pub(super) fn decode_request(
    pairs: Vec<(String, String)>,
    body: &[u8],
) -> Result<Request, KppError> {
    let mut extras = Extras::new(Origin::new(D));
    let mut f = Fields::parse(body, D, "request")?;
    let text = f.str("text")?;
    f.rest_into(&mut extras, BODY);
    let mut model = None;
    let mut speed = None;
    let mut rest = Vec::new();
    for (k, v) in pairs {
        match k.as_str() {
            "model" if model.is_none() => model = Some(v),
            "speed" if speed.is_none() => {
                let n = v.parse::<f64>().map_err(|_| {
                    KppError::invalid_request("`speed` must be a number.").with_param("speed")
                })?;
                extras.form.insert(FORM_SPEED.to_owned(), v.into());
                speed = Some(n);
            }
            _ => rest.push((k, v)),
        }
    }
    keep_query(&mut extras, rest);
    let mut r = SpeechRequest::new(model.unwrap_or_default(), SpeechOperation::Speech);
    r.input = text;
    r.speed = speed;
    r.extras = extras;
    Ok(Request::Speech(r))
}

fn family_of(model: &str) -> Option<&'static str> {
    SPEECH_FAMILIES.into_iter().find(|f| *f == model)
}

fn voice_part(v: &str) -> bool {
    !v.is_empty()
        && v.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// The language suffix of a voice named without one: the primary subtag of the request's
// language (`en-US` → `en`), English otherwise, as the first Aura voices all are.
fn language_suffix(language: Option<&str>) -> String {
    language
        .and_then(|l| l.split(['-', '_']).next())
        .filter(|l| !l.is_empty())
        .map_or_else(|| "en".to_owned(), str::to_ascii_lowercase)
}

/// The Deepgram model a request speaks with: the model itself when it names a voice, or the
/// family `aura-2` or `aura` with the request's voice, `thalia` (English unless the request
/// names a language), `thalia-en` or the whole id `aura-2-thalia-en`.
fn voice_model(r: &SpeechRequest) -> Result<String, KppError> {
    let voice = r.voice.as_ref().map(|v| v.as_str().to_ascii_lowercase());
    let Some(family) = family_of(&r.model) else {
        return match voice {
            Some(v) if v != r.model => Err(no(
                "a voice beside a model that names one",
                "voice",
                &format!(
                    "send `voice: {}` or the model `aura-2` with the voice.",
                    r.model
                ),
            )),
            _ => Ok(r.model.clone()),
        };
    };
    let Some(v) = voice else {
        return Err(KppError::invalid_request(format!(
            "Name a voice of `{family}`, such as `thalia`."
        ))
        .with_param("voice"));
    };
    if !voice_part(&v) {
        return Err(KppError::invalid_request(
            "A Deepgram voice has lowercase letters, digits and hyphens only.",
        )
        .with_param("voice"));
    }
    let own_family = |id: &str| {
        id.strip_prefix(family)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|rest| family != "aura" || !rest.starts_with("2-"))
    };
    if v.starts_with("aura-") {
        return if own_family(&v) {
            Ok(v)
        } else {
            Err(no(
                &format!("the voice `{v}` with `{family}`"),
                "voice",
                "send the voice with the model of its family.",
            ))
        };
    }
    if v.contains('-') {
        return Ok(format!("{family}-{v}"));
    }
    Ok(format!(
        "{family}-{v}-{}",
        language_suffix(r.language.as_deref())
    ))
}

/// Checks a speech request and answers the model, which names the voice, Kaeso sends it as.
pub(super) fn check(r: &SpeechRequest) -> Result<String, KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if r.operation != SpeechOperation::Speech {
        return Err(no(r.operation.as_str(), "model", "send text to speak."));
    }
    if !r.dialogue.is_empty() || r.audio.is_some() {
        return Err(no("dialogue or source audio", "input", "send text."));
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
    if let Some(AudioFormat::Other(f)) = &r.format {
        return Err(no(
            &format!("the format `{f}`"),
            "response_format",
            "use mp3, opus, aac, flac, wav or pcm.",
        ));
    }
    if let Some(s) = r.speed
        && !SPEED.contains(&s)
    {
        return Err(
            KppError::invalid_request("Deepgram takes a `speed` from 0.7 to 1.5.")
                .with_param("speed"),
        );
    }
    if r.characters() > MAX_SPEECH_CHARACTERS as u64 {
        return Err(KppError::invalid_request(format!(
            "Deepgram speaks at most {MAX_SPEECH_CHARACTERS} characters a request; split the text."
        ))
        .with_param("input"));
    }
    voice_model(r)
}

// The query of each format (https://developers.deepgram.com/docs/tts-media-output-settings):
// `linear16` is 16-bit little-endian signed PCM (https://developers.deepgram.com/docs/tts-encoding),
// so OpenAI's `pcm` is `linear16` at 24 kHz without a container.
fn format_query(format: &AudioFormat) -> &'static [(&'static str, &'static str)] {
    match format {
        AudioFormat::Wav => &[("encoding", "linear16"), ("container", "wav")],
        AudioFormat::Pcm16 => &[
            ("encoding", "linear16"),
            ("container", "none"),
            ("sample_rate", "24000"),
        ],
        AudioFormat::Opus => &[("encoding", "opus")],
        AudioFormat::Aac => &[("encoding", "aac")],
        AudioFormat::Flac => &[("encoding", "flac")],
        _ => &[("encoding", "mp3")],
    }
}

fn spelled_speed(r: &SpeechRequest) -> Option<String> {
    let speed = r.speed?;
    match r.extras.form_str(D, FORM_SPEED) {
        Some(s) if s.parse::<f64>().ok() == Some(speed) => Some(s.to_owned()),
        _ => Some(speed.to_string()),
    }
}

pub(super) fn encode_request(
    r: &SpeechRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    let model = check(r)?;
    let own = r.extras.is_from(D);
    ctx.stream = StreamMode::Off;
    let mut q = vec![("model".to_owned(), model)];
    if r.format.is_some() || !own {
        // The OpenAPI document's default is mp3, the media output guide's linear16 in wav, so
        // the encoding is always named.
        let format = r.format.clone().unwrap_or(AudioFormat::Mp3);
        let pcm = format == AudioFormat::Pcm16;
        for (k, v) in format_query(&format) {
            if !(pcm && *k == "sample_rate" && r.sample_rate.is_some()) {
                q.push(((*k).to_owned(), (*v).to_owned()));
            }
        }
        ctx.audio_format = Some(format);
    }
    if let Some(rate) = r.sample_rate {
        q.push(("sample_rate".to_owned(), rate.to_string()));
    }
    if let Some(kbps) = r.bitrate {
        q.push(("bit_rate".to_owned(), kbps.saturating_mul(1000).to_string()));
    }
    if let Some(s) = spelled_speed(r) {
        q.push(("speed".to_owned(), s));
    }
    q.extend(query_extras(&r.extras));
    let mut body = Map::new();
    body.insert(
        "text".to_owned(),
        r.input.clone().unwrap_or_default().into(),
    );
    put_extras(&mut body, &r.extras, D, BODY);
    Ok(EncodedRequest::post_json(
        with_query(&path_under(ctx, SPEAK_PATH), &q),
        HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE),
        crate::wire::to_body(&body)?,
    ))
}

fn media_of(headers: &HeaderList, ctx: &EgressCtx) -> String {
    match headers.get("content-type") {
        Some(t) if crate::media::is_audio(t) => t.to_owned(),
        _ => media_of_format(ctx.audio_format.as_ref()),
    }
}

// The content type Deepgram answers each format with (media output settings, "Content-Type").
fn media_of_format(format: Option<&AudioFormat>) -> String {
    match format {
        Some(AudioFormat::Pcm16) => "audio/l16;rate=24000".to_owned(),
        Some(AudioFormat::Opus) => "audio/ogg;codecs=opus".to_owned(),
        Some(f) => f.media_type().unwrap_or("audio/mpeg").to_owned(),
        None => "audio/mpeg".to_owned(),
    }
}

pub(super) fn decode_response(headers: &HeaderList, body: &[u8], ctx: &EgressCtx) -> Reply {
    let mut resp = SpeechResponse::new(
        String::new(),
        Source::bytes(media_of(headers, ctx), bytes::Bytes::copy_from_slice(body)),
    );
    resp.characters = super::billed_characters(headers);
    Reply::Speech(resp)
}

pub(super) fn encode_response(r: &SpeechResponse) -> Result<Vec<u8>, KppError> {
    match &r.audio {
        Source::Base64(b) => Ok(b.data.to_vec()),
        _ => Err(KppError::provider(
            "The provider answered with a link to the audio, which Deepgram's format cannot carry.",
        )),
    }
}

pub(super) fn media_type_of_reply(r: &SpeechResponse) -> String {
    match &r.audio {
        Source::Base64(b) => b.media_type.clone(),
        _ => media_of_format(None),
    }
}

pub(super) fn media_type_of_request(r: &SpeechRequest) -> String {
    media_of_format(r.format.as_ref())
}

/// Reads the audio as it arrives, a piece per chunk.
pub(crate) struct Decoder {
    media_type: Option<String>,
    done: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            media_type: Some(media_of_format(ctx.audio_format.as_ref())),
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

/// Writes Deepgram's replies as they arrive: the audio bytes, or a whole transcript at its end.
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
                Some(done) => super::listen::encode_response(done),
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
