use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, ErrorKind, Reply, Request, RequestKind, Source, SpeechOperation, SpeechRequest,
    StreamEvent, StreamMode, TranscriptFormat, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::multipart::{FormLimits, form_boundary, read_form};
use crate::openai_speech::OpenAiSpeech;
use crate::openai_transcriptions::OpenAiTranscriptions;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "elevenlabs";
const VOICE: &str = "hpp4J3VqNfWAUOO0d1Us";

fn call_of(name: &str) -> (ElevenLabsCall, HeaderList) {
    let meta = sidecar(DIR, "requests", name, "meta").unwrap();
    let call = ElevenLabsCall::from_path(
        meta["path"].as_str().unwrap(),
        meta["query"].as_str().unwrap(),
    )
    .unwrap();
    let mut headers = HeaderList::new();
    if let Some(ct) = meta["content_type"].as_str() {
        headers.push("content-type", ct);
    }
    (call, headers)
}

fn decode(name: &str, body: &[u8]) -> Request {
    let (call, headers) = call_of(name);
    crate::dispatch::decode_elevenlabs_request(&call, body, &headers).unwrap()
}

fn encode(req: &Request) -> (EncodedRequest, EgressCtx) {
    let mut ctx = EgressCtx::new(req).with_provider("elevenlabs");
    let enc = ElevenLabs::encode_request(req, &mut ctx).unwrap();
    (enc, ctx)
}

fn parts(body: &[u8], ct: &str) -> Vec<(String, Value)> {
    let boundary = form_boundary(ct).unwrap();
    let mut out: Vec<(String, Value)> = read_form(body, &boundary, FormLimits::new(64, 1 << 24))
        .unwrap()
        .iter()
        .map(|p| {
            let v = match &p.filename {
                Some(f) => json!({"file": f, "bytes": p.body.len()}),
                None => Value::String(p.text().unwrap().to_owned()),
            };
            (p.name.clone(), v)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn reply(name: &str, ext: &str) -> Vec<u8> {
    fixtures(DIR, "responses", ext)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b)
        .unwrap()
}

#[test]
fn every_request_fixture_round_trips_with_its_path_and_query() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = decode(&name, &body);
        req.validate().unwrap();
        let (enc, _) = encode(&req);
        let meta = sidecar(DIR, "requests", &name, "meta").unwrap();
        let query = meta["query"].as_str().unwrap();
        let expected = if query.is_empty() {
            meta["path"].as_str().unwrap().to_owned()
        } else {
            format!("{}?{query}", meta["path"].as_str().unwrap())
        };
        assert_eq!(enc.path, expected, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    for (name, body) in fixtures(DIR, "requests", "multipart") {
        let req = decode(&name, &body);
        let (enc, _) = encode(&req);
        let meta = sidecar(DIR, "requests", &name, "meta").unwrap();
        assert!(
            enc.path.starts_with(meta["path"].as_str().unwrap()),
            "{name}: {}",
            enc.path
        );
        let ct = meta["content_type"].as_str().unwrap();
        assert_eq!(
            parts(&enc.body, enc.headers.get("content-type").unwrap()),
            parts(&body, ct),
            "{name}"
        );
    }
}

#[test]
fn requests_decode_to_typed_fields() {
    let body = fixtures(DIR, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == "speech-every-field")
        .unwrap()
        .1;
    let Request::Speech(r) = decode("speech-every-field", &body) else {
        panic!()
    };
    assert_eq!(r.voice, Some(Voice::Id(VOICE.into())));
    assert_eq!(
        (r.format, r.sample_rate, r.bitrate),
        (Some(AudioFormat::Opus), Some(48_000), Some(64))
    );
    assert_eq!(
        (r.speed, r.seed, r.language.as_deref()),
        (Some(1.1), Some(42), Some("en"))
    );
    let Request::Speech(d) = decode("dialogue", br#"{"inputs":[{"text":"Hi.","voice_id":"a"}]}"#)
    else {
        panic!()
    };
    assert_eq!(
        (d.operation, d.model.as_str(), d.dialogue.len()),
        (SpeechOperation::Dialogue, "eleven_v3", 1)
    );
    let Request::Speech(m) = decode("music", br#"{"prompt":"x","music_length_ms":3000}"#) else {
        panic!()
    };
    assert_eq!(
        (m.operation, m.duration),
        (SpeechOperation::Music, Some(3.0))
    );
    let (_, body) = fixtures(DIR, "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe-every-field")
        .unwrap();
    let Request::Transcription(t) = decode("transcribe-every-field", &body) else {
        panic!()
    };
    assert_eq!(
        (t.diarize, t.speakers, t.language.as_deref()),
        (Some(true), Some(2), Some("en"))
    );
    assert_eq!(t.format, Some(TranscriptFormat::VerboseJson));
    let (call, headers) = call_of("speech");
    let err = crate::dispatch::decode_elevenlabs_request(&call, b"[1]", &headers).unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(ElevenLabsCall::from_path("/v1/music/detailed", "").is_none());
    assert!(ElevenLabsCall::from_path("/v1/voice-changer/x/with-timestamps", "").is_none());
    assert_eq!(
        ElevenLabsCall::from_path("/v1/speech-to-text", "")
            .unwrap()
            .kind(),
        RequestKind::Transcription
    );
}

#[test]
fn recorded_audio_replies_come_back_as_they_were_with_their_characters() {
    for (name, ext) in [
        ("recorded-speech", "mp3"),
        ("recorded-sound", "mp3"),
        ("recorded-dialogue", "mp3"),
        ("recorded-voice-change", "mp3"),
    ] {
        let audio = reply(name, ext);
        let meta = sidecar(DIR, "responses", &format!("{name}.{ext}"), "meta").unwrap();
        let headers = HeaderList::new()
            .with("content-type", meta["content_type"].as_str().unwrap())
            .with(
                "character-cost",
                meta["headers"]["character-cost"].as_str().unwrap(),
            );
        let req = decode(
            "speech",
            br#"{"text":"Hi.","model_id":"eleven_flash_v2_5"}"#,
        );
        let (_, mut ctx) = encode(&req);
        ctx.speech_reply_json = false;
        let r = ElevenLabs::decode_response(200, &headers, &audio, &ctx).unwrap();
        let Reply::Speech(s) = &r else { panic!() };
        assert!(s.characters.is_some(), "{name}");
        let out = ElevenLabs::encode_response(&r, &EncodeCtx::for_request(req.clone())).unwrap();
        assert_eq!(out, audio, "{name}");
        assert_eq!(billed_characters(&headers), s.characters);
    }
}

#[test]
fn raw_streams_and_timestamped_lines_flow_through() {
    let pcm = reply("recorded-speech-stream", "pcm");
    let body = br#"{"text":"Hi.","model_id":"eleven_flash_v2_5"}"#;
    let req = decode("speech-stream", body);
    let (enc, ctx) = encode(&req);
    assert!(ElevenLabs::reads_reply_as_stream(&ctx));
    assert!(
        enc.path.contains("/stream?output_format=pcm_24000"),
        "{}",
        enc.path
    );
    let mut dec = ElevenLabs::stream_decoder(&ctx);
    let mut events = Vec::new();
    for chunk in pcm.chunks(4096) {
        events.extend(dec.feed(chunk));
    }
    events.extend(dec.finish());
    let mut enc = ElevenLabs::stream_encoder(&EncodeCtx::for_request(req.clone()));
    let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
    assert_eq!(out, pcm);
    assert_eq!(
        ElevenLabs::stream_media_type(&EncodeCtx::for_request(req)),
        "audio/pcm"
    );

    let (_, lines) = fixtures(DIR, "streams", "ndjson").remove(0);
    let req = decode("speech-stream-with-timestamps", body);
    let (_, ctx) = encode(&req);
    let mut dec = ElevenLabs::stream_decoder(&ctx);
    let mut events = dec.feed(&lines);
    events.extend(dec.finish());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Speech(s) if s.alignment.is_some()))
    );
    let mut enc = ElevenLabs::stream_encoder(&EncodeCtx::for_request(req));
    let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
    let as_values = |b: &[u8]| -> Vec<Value> {
        b.split(|c| *c == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_slice(l).unwrap())
            .collect()
    };
    assert_eq!(as_values(&out), as_values(&lines));
}

#[test]
fn the_timestamped_reply_and_the_transcript_round_trip() {
    let ts = reply("recorded-speech-with-timestamps", "json");
    let req = decode(
        "speech-with-timestamps",
        br#"{"text":"Hi.","model_id":"eleven_flash_v2_5"}"#,
    );
    let (_, ctx) = encode(&req);
    assert!(ctx.speech_reply_json && !ElevenLabs::reads_reply_as_stream(&ctx));
    let r = ElevenLabs::decode_response(200, &HeaderList::new(), &ts, &ctx).unwrap();
    let Reply::Speech(s) = &r else { panic!() };
    assert_eq!(s.alignment.as_ref().unwrap().characters, ["H", "i", "."]);
    let out = ElevenLabs::encode_response(&r, &EncodeCtx::for_request(req.clone())).unwrap();
    assert_json_eq("with-timestamps", &ts, &out);

    let stt = reply("recorded-transcribe", "json");
    let (_, body) = fixtures(DIR, "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe")
        .unwrap();
    let treq = decode("transcribe", &body);
    let (_, ctx) = encode(&treq);
    let r = ElevenLabs::decode_response(200, &HeaderList::new(), &stt, &ctx).unwrap();
    let Reply::Transcription(t) = &r else {
        panic!()
    };
    assert_eq!(t.text, "Hello, Kizo");
    assert_eq!(t.words.len(), 3);
    let out = ElevenLabs::encode_response(&r, &EncodeCtx::for_request(treq)).unwrap();
    assert_json_eq("transcribe", &stt, &out);
    assert_eq!(
        ElevenLabs::response_media_type(
            &r,
            &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off)
        ),
        crate::JSON_MEDIA_TYPE
    );
}

#[test]
fn errors_round_trip_in_elevenlabs_shape_and_kaesos_own_take_it() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = meta["status"].as_u64().unwrap() as u16;
        let err = ElevenLabs::decode_error(status, &HeaderList::new(), &body);
        let enc = ElevenLabs::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    let err = ElevenLabs::decode_error(
        404,
        &HeaderList::new(),
        &fixtures(DIR, "errors", "json").remove(1).1,
    );
    assert_eq!(err.kind, ErrorKind::NotFound);
    let own = kaeso_kpp::KppError::invalid_request("Send `text`.").with_param("text");
    let enc = ElevenLabs::encode_error(&own);
    let v: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        (
            enc.status,
            v["detail"]["message"].as_str(),
            v["detail"]["param"].as_str()
        ),
        (400, Some("Send `text`."), Some("text"))
    );
    let validation =
        br#"{"detail":[{"loc":["body","text"],"msg":"field required","type":"missing"}]}"#;
    let err = ElevenLabs::decode_error(422, &HeaderList::new(), validation);
    assert_eq!(
        (err.param.as_deref(), err.message.as_str()),
        (Some("text"), "field required")
    );
    assert_eq!(
        ElevenLabs::decode_error(
            402,
            &HeaderList::new(),
            &fixtures(DIR, "errors", "json").remove(0).1
        )
        .kind,
        ErrorKind::Billing
    );
}

#[test]
fn an_openai_app_reaches_elevenlabs_by_voice_id_and_gets_openais_forms_back() {
    let mut r = SpeechRequest::new("eleven_flash_v2_5", SpeechOperation::Speech)
        .with_input("Hi.")
        .with_voice(Voice::Named(VOICE.into()));
    r.format = Some(AudioFormat::Pcm16);
    r.speed = Some(1.1);
    let req = Request::Speech(r.clone());
    let (enc, ctx) = encode(&req);
    assert_eq!(
        enc.path,
        format!("/v1/text-to-speech/{VOICE}/stream?output_format=pcm_24000")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap(),
        json!({"model_id": "eleven_flash_v2_5", "text": "Hi.", "voice_settings": {"speed": 1.1}})
    );
    assert!(ElevenLabs::reads_reply_as_stream(&ctx));
    let mut wav = r.clone();
    wav.format = Some(AudioFormat::Wav);
    let (enc, _) = encode(&Request::Speech(wav));
    assert!(
        enc.path
            .ends_with(&format!("{VOICE}?output_format=wav_24000")),
        "{}",
        enc.path
    );
    for (voice, format) in [
        (Voice::Named("alloy".into()), AudioFormat::Mp3),
        (Voice::Id(VOICE.into()), AudioFormat::Aac),
    ] {
        let mut bad = r.clone();
        bad.voice = Some(voice);
        bad.format = Some(format);
        let req = Request::Speech(bad);
        let err = ElevenLabs::check(&req, &EgressCtx::new(&req)).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }

    let stt = reply("recorded-transcribe", "json");
    let (_, body) = fixtures("openai_transcriptions", "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe-srt")
        .unwrap();
    let headers = HeaderList::new().with(
        "content-type",
        "multipart/form-data; boundary=----kaesofixture",
    );
    let Request::Transcription(mut t) =
        OpenAiTranscriptions::decode_request(RequestKind::Transcription, &body, &headers).unwrap()
    else {
        panic!()
    };
    t.model = "scribe_v2".into();
    let treq = Request::Transcription(t);
    let (enc, ctx) = encode(&treq);
    let sent = parts(&enc.body, enc.headers.get("content-type").unwrap());
    assert!(
        sent.iter()
            .any(|(k, v)| k == "model_id" && v == "scribe_v2")
    );
    assert!(
        !sent.iter().any(|(k, _)| k == "timestamps_granularity"),
        "words are timed by default"
    );
    let r = ElevenLabs::decode_response(200, &HeaderList::new(), &stt, &ctx).unwrap();
    let srt = OpenAiTranscriptions::encode_response(&r, &EncodeCtx::for_request(treq)).unwrap();
    assert_eq!(
        String::from_utf8(srt).unwrap(),
        "1\n00:00:00,040 --> 00:00:00,860\nHello, Kizo\n\n"
    );
    let mut verbose = TranscriptFormatCtx::new();
    let v: Value =
        serde_json::from_slice(&OpenAiTranscriptions::encode_response(&r, &verbose.ctx()).unwrap())
            .unwrap();
    assert_eq!(
        v["words"].as_array().unwrap().len(),
        2,
        "spacing is not a word"
    );
}

struct TranscriptFormatCtx(kaeso_kpp::TranscriptionRequest);

impl TranscriptFormatCtx {
    fn new() -> Self {
        let mut r = kaeso_kpp::TranscriptionRequest::new(
            "scribe_v2",
            kaeso_kpp::TranscriptionOperation::Transcribe,
            kaeso_kpp::AudioInput::new(Source::base64("audio/mpeg", "SUQz")),
        );
        r.format = Some(TranscriptFormat::VerboseJson);
        TranscriptFormatCtx(r)
    }

    fn ctx(&mut self) -> EncodeCtx {
        EncodeCtx::for_request(Request::Transcription(self.0.clone()))
    }
}

#[test]
fn elevenlabs_audio_reaches_an_openai_app_as_bytes_and_events() {
    let mp3 = reply("recorded-speech", "mp3");
    let mut r = SpeechRequest::new("eleven_flash_v2_5", SpeechOperation::Speech)
        .with_input("Hi.")
        .with_voice(Voice::Id(VOICE.into()));
    r.format = Some(AudioFormat::Mp3);
    r.stream = StreamMode::on(false);
    let req = Request::Speech(r);
    let (_, ctx) = encode(&req);
    let mut dec = ElevenLabs::stream_decoder(&ctx);
    let mut events = dec.feed(&mp3);
    events.extend(dec.finish());
    let mut enc = OpenAiSpeech::stream_encoder(&EncodeCtx::for_request(req));
    let out =
        String::from_utf8(events.iter().flat_map(|e| enc.encode(e).unwrap()).collect()).unwrap();
    assert!(out.contains("speech.audio.delta") && out.contains("speech.audio.done"));
    let first = out
        .split("\"audio\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(first)
            .unwrap(),
        mp3
    );
}

#[test]
fn listings_keep_elevenlabs_shapes() {
    let (_, list) = fixtures(DIR, "models", "json").remove(0);
    let models = ElevenLabs::decode_models(200, &HeaderList::new(), &list).unwrap();
    assert_eq!(models.models[0].id, "eleven_v4");
    let out = ElevenLabs::encode_models(&models).unwrap();
    assert_json_eq("models", &list, &out);
    assert!(voice_listing_request("/v2/voices", "page_size=3&search=bella").is_ok());
    assert!(voice_listing_request(&format!("/v1/voices/{VOICE}"), "").is_ok());
    assert!(voice_listing_request("/v1/voices/add", "x=\n").is_err());
    assert!(voice_listing_request("/v1/history", "").is_err());
    assert!(voices_reply(200, &HeaderList::new(), br#"{"voices":[]}"#).is_ok());
    assert!(voices_reply(200, &HeaderList::new(), br#"{"x":1}"#).is_err());
    assert_eq!(
        voices_reply(
            401,
            &HeaderList::new(),
            br#"{"detail":{"status":"unauthorized","message":"No."}}"#
        )
        .unwrap_err()
        .kind,
        ErrorKind::Authentication
    );
}
