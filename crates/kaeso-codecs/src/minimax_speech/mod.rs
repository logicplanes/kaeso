#[cfg(test)]
mod tests;

use kaeso_kpp::{
    AudioFormat, DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, RawJson, Reply,
    Request, RequestKind, Source, SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent,
    StreamMode, Voice,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::cross::check_extras;
use crate::wire::{
    Ext, extra_for, extras_from, note_integers, parse_provider, spelled_number, to_body,
};

const D: DialectId = DialectId::MinimaxSpeech;

/// The path of MiniMax's synchronous text-to-speech API, from the root of its host
/// (<https://platform.minimax.io/docs/api-reference/speech-t2a-http>).
pub const T2A_PATH: &str = "/v1/t2a_v2";

const FORMATS: &[&str] = &["mp3", "pcm", "flac", "wav", "opus"];

/// MiniMax's text-to-speech API, `POST /v1/t2a_v2` (T2A v2): text, a voice and audio settings
/// in, hex-encoded audio out, with errors in `base_resp`. Kaeso asks for the whole reply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MinimaxSpeech;

impl crate::codec::sealed::Sealed for MinimaxSpeech {}
impl crate::codec::sealed::Sealed for Whole {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RequestW {
    model: String,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice_setting: Option<Ext<VoiceSettingW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audio_setting: Option<Ext<AudioSettingW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_format: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct VoiceSettingW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    speed: Option<serde_json::Number>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct AudioSettingW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    format: Option<String>,
}

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        D.display_name()
    ))
}

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        D,
        format!("MiniMax's speech models do not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

fn not_speech(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "MiniMax's speech API only makes speech from text.",
    )
}

fn format_of(wire: &str) -> AudioFormat {
    match wire {
        "pcm" => AudioFormat::Pcm16,
        other => AudioFormat::from_wire(other),
    }
}

fn wire_format(f: &AudioFormat) -> &str {
    match f {
        AudioFormat::Pcm16 => "pcm",
        other => other.as_wire(),
    }
}

fn media_type(format: &str) -> &'static str {
    match format {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "opus" => "audio/opus",
        "pcm" => "audio/pcm",
        _ => "application/octet-stream",
    }
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<RequestW>::from_slice(body)
        .map_err(|e| invalid(crate::wire::without_position(&e)))?;
    let mut r = SpeechRequest::new(w.model, SpeechOperation::Speech).with_input(w.text);
    let mut extras = extras_from(&Origin::new(D), extra, verbatim);
    if let Some(v) = w.voice_setting {
        r.voice = v.known.voice_id.clone().map(Voice::Id);
        r.speed = v.known.speed.as_ref().and_then(serde_json::Number::as_f64);
        note_integers(&mut extras, &[("speed", v.known.speed.as_ref())]);
        crate::wire::add_nested_extra(&mut extras, "voice_setting", v.extra);
        extras.form.insert("voice_setting".into(), true.into());
    }
    if let Some(a) = w.audio_setting {
        r.format = a.known.format.as_deref().map(format_of);
        r.sample_rate = a.known.sample_rate;
        r.bitrate = a.known.bitrate.map(|b| b / 1000);
        crate::wire::add_nested_extra(&mut extras, "audio_setting", a.extra);
        extras.form.insert("audio_setting".into(), true.into());
    }
    match w.stream {
        Some(true) => r.stream = StreamMode::on(false),
        Some(false) => {
            extras.form.insert("stream".into(), false.into());
        }
        None => {}
    }
    if let Some(f) = w.output_format {
        extras.form.insert("output_format".into(), f.into());
    }
    r.extras = extras;
    Ok(Request::Speech(r))
}

fn check_request(r: &SpeechRequest) -> Result<(), KppError> {
    check_extras(&r.extras, D, "", "request")?;
    if r.operation != SpeechOperation::Speech {
        return Err(refused(
            r.operation.as_str(),
            "model",
            "send text to speak.",
        ));
    }
    if r.dialogue.len() + usize::from(r.audio.is_some()) > 0 {
        return Err(refused("dialogue or source audio", "input", "send text."));
    }
    if let Some(f) = &r.format
        && !FORMATS.contains(&wire_format(f))
    {
        return Err(refused(
            &format!("the format `{}`", wire_format(f)),
            "response_format",
            "use mp3, pcm, flac, wav or opus.",
        ));
    }
    let unsupported = [
        ("instructions", r.instructions.is_some()),
        ("language", r.language.is_some()),
        ("seed", r.seed.is_some()),
        ("timestamps", r.timestamps),
        ("duration", r.duration.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{field}`"), field, "remove it."));
    }
    Ok(())
}

fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Speech(r) = req else {
        return Err(not_speech(req.kind()));
    };
    check_request(r)?;
    ctx.stream = StreamMode::Off;
    let own = r.extras.is_from(D);
    let mut voice = Map::new();
    if let Some(v) = &r.voice {
        voice.insert("voice_id".into(), v.as_str().into());
    }
    if let Some(n) = spelled_number(r.speed, "speed", &r.extras, D) {
        voice.insert("speed".into(), Value::Number(n));
    }
    let mut audio = Map::new();
    if let Some(rate) = r.sample_rate {
        audio.insert("sample_rate".into(), rate.into());
    } else if r.format == Some(AudioFormat::Pcm16) && !own {
        // KPP's `pcm16` is 24 kHz; MiniMax's `pcm` takes the rate of `sample_rate`.
        audio.insert("sample_rate".into(), 24_000.into());
    }
    if let Some(b) = r.bitrate {
        audio.insert("bitrate".into(), (b.saturating_mul(1000)).into());
    }
    if let Some(f) = &r.format {
        audio.insert("format".into(), wire_format(f).into());
    }
    let nested = |prefix: &str, map: &mut Map<String, Value>| {
        if own {
            for (k, v) in r.extras.nested_fields(prefix) {
                if let Ok(value) = v.to_value() {
                    map.insert(k.to_owned(), value);
                }
            }
        }
    };
    nested("voice_setting", &mut voice);
    nested("audio_setting", &mut audio);
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    out.insert("text".into(), r.input.clone().unwrap_or_default().into());
    let stream_named = r.extras.form_value(D, "stream") == Some(&Value::Bool(false));
    if stream_named || !own {
        out.insert("stream".into(), false.into());
    }
    if !voice.is_empty() || r.extras.form_flag(D, "voice_setting") {
        out.insert("voice_setting".into(), Value::Object(voice));
    }
    if !audio.is_empty() || r.extras.form_flag(D, "audio_setting") {
        out.insert("audio_setting".into(), Value::Object(audio));
    }
    out.insert(
        "output_format".into(),
        r.extras
            .form_str(D, "output_format")
            .unwrap_or("hex")
            .into(),
    );
    for (k, v) in extra_for(&r.extras, D) {
        if let Ok(value) = v.to_value() {
            out.entry(k).or_insert(value);
        }
    }
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{T2A_PATH}"),
        None => T2A_PATH.to_owned(),
    };
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&out)?,
    ))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ResponseW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extra_info: Option<RawJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_resp: Option<RawJson>,
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let digits = hex.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    let value = |c: u8| (c as char).to_digit(16);
    digits
        .chunks(2)
        .map(|pair| match pair {
            [a, b] => Some(u8::try_from(value(*a)? * 16 + value(*b)?).ok()?),
            _ => None,
        })
        .collect()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn decode_response(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(decode_error(status, headers, body));
    }
    if ctx.kind != RequestKind::Speech {
        return Err(not_speech(ctx.kind));
    }
    let origin = ctx.origin(D);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ResponseW>(body, D)?;
    let failed = w
        .base_resp
        .as_ref()
        .and_then(|b| b.to_value().ok())
        .and_then(|b| b.get("status_code").and_then(Value::as_i64))
        .is_some_and(|c| c != 0);
    // MiniMax answers HTTP 200 with the failure in `base_resp`.
    if failed {
        return Err(decode_error(status, headers, body));
    }
    let data = w
        .data
        .as_ref()
        .and_then(|d| d.to_value().ok())
        .unwrap_or_default();
    let info = w
        .extra_info
        .as_ref()
        .and_then(|d| d.to_value().ok())
        .unwrap_or_default();
    let hex = data
        .get("audio")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let bytes = hex_to_bytes(hex)
        .ok_or_else(|| KppError::provider("MiniMax sent audio that is not valid hex."))?;
    let format = info
        .get("audio_format")
        .and_then(Value::as_str)
        .unwrap_or("mp3");
    let mut resp = SpeechResponse::new(String::new(), Source::bytes(media_type(format), bytes));
    resp.characters = info.get("usage_characters").and_then(Value::as_u64);
    let mut extras = extras_from(&origin, extra, verbatim);
    let mut rest = data;
    if let Value::Object(m) = &mut rest {
        m.remove("audio");
    }
    let kept = [
        ("data", RawJson::from_serialize(&rest).ok()),
        ("extra_info", w.extra_info.clone()),
        ("base_resp", w.base_resp.clone()),
    ];
    for (key, raw) in kept {
        if let Some(raw) = raw.filter(|r| r.get() != "null") {
            extras.fields.insert(key.to_owned(), raw);
        }
    }
    resp.extras = extras;
    Ok(Reply::Speech(resp))
}

fn encode_response(reply: &Reply) -> Result<Vec<u8>, KppError> {
    let Reply::Speech(r) = reply else {
        return Err(not_speech(reply.kind()));
    };
    let Source::Base64(b) = &r.audio else {
        return Err(KppError::provider(
            "MiniMax's reply carries the audio itself.",
        ));
    };
    let bytes = &b.data;
    let mut out = Map::new();
    if r.extras.is_from(D) {
        for (k, v) in &r.extras.fields {
            if let Ok(value) = v.to_value() {
                out.insert(k.clone(), value);
            }
        }
    }
    let data = out.entry("data").or_insert_with(|| json!({"status": 2}));
    if let Value::Object(d) = data {
        d.insert("audio".into(), bytes_to_hex(bytes).into());
    }
    out.entry("base_resp")
        .or_insert_with(|| json!({"status_code": 0, "status_msg": "success"}));
    to_body(&out)
}

fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    crate::minimax_images::base_resp_error(D, status, headers, body)
}

/// Reads a whole reply as it arrives: Kaeso never asks MiniMax to stream speech, so a stream
/// is the whole reply in pieces.
struct Whole {
    body: Vec<u8>,
    ctx: EgressCtx,
}

impl StreamDecoder for Whole {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        self.body.extend_from_slice(bytes);
        Vec::new()
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let body = std::mem::take(&mut self.body);
        match decode_response(200, &HeaderList::new(), &body, &self.ctx) {
            Ok(Reply::Speech(r)) => {
                let Source::Base64(b) = r.audio else {
                    return Vec::new();
                };
                let mut piece = kaeso_kpp::SpeechEvent::audio(b.to_base64());
                piece.media_type = Some(b.media_type);
                let mut done = kaeso_kpp::SpeechEvent::done(Some(r.usage));
                done.characters = r.characters;
                vec![StreamEvent::Speech(piece), StreamEvent::Speech(done)]
            }
            Ok(_) => Vec::new(),
            Err(e) => vec![StreamEvent::Error(e)],
        }
    }
}

impl StreamEncoder for Whole {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(refused(
            "streaming",
            "stream",
            "send the request without `stream`.",
        ))
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

impl Codec for MinimaxSpeech {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        if kind != RequestKind::Speech {
            return Err(not_speech(kind));
        }
        crate::limits::request_shape(D, body)?;
        decode_request(body)
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        encode_response(reply)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        let mut egress = EgressCtx::for_kind(ctx.kind, StreamMode::Off);
        egress.provider = None;
        Box::new(Whole {
            body: Vec::new(),
            ctx: egress,
        })
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::minimax_images::encode_error(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        match req {
            Request::Speech(r) => check_request(r),
            other => Err(not_speech(other.kind())),
        }
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        encode_request(req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        decode_response(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(Whole {
            body: Vec::new(),
            ctx: ctx.clone(),
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error(status, headers, body)
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
        if !(200..300).contains(&status) {
            return Err(decode_error(status, headers, body));
        }
        crate::openai_models::decode_models(D, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(D, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(D, query)
    }
}
