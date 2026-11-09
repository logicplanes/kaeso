use kaeso_kpp::{
    AudioFormat, ErrorKind, Reply, Request, RequestKind, Source, SpeechEvent, SpeechResponse,
    StreamEvent, Voice,
};
use serde_json::Value;

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "minimax_speech";

fn one(folder: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json").remove(0).1
}

fn decode(body: &[u8]) -> SpeechRequest {
    match MinimaxSpeech::decode_request(RequestKind::Speech, body, &HeaderList::new()).unwrap() {
        Request::Speech(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode(r: &SpeechRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Speech(r.clone());
    let mut ctx = EgressCtx::new(&req).with_provider("minimax");
    let enc = MinimaxSpeech::encode_request(&req, &mut ctx)?;
    Ok((enc, ctx))
}

#[test]
fn the_request_fixture_round_trips_with_typed_settings() {
    let body = one("requests");
    let r = decode(&body);
    assert_eq!(
        r.voice,
        Some(Voice::Id("English_expressive_narrator".into()))
    );
    assert_eq!(
        (r.format.clone(), r.sample_rate, r.bitrate),
        (Some(AudioFormat::Mp3), Some(32000), Some(128))
    );
    assert_eq!(r.speed, Some(1.0));
    let (enc, _) = encode(&r).unwrap();
    assert_eq!(enc.path, T2A_PATH);
    assert_json_eq("speech", &body, &enc.body);
}

#[test]
fn an_openai_request_gets_t2a_settings_and_pcm_at_24_khz() {
    let mut r = SpeechRequest::new("speech-2.8-turbo", SpeechOperation::Speech)
        .with_input("Hi")
        .with_voice(Voice::Named("English_CalmWoman".into()));
    r.format = Some(AudioFormat::Pcm16);
    r.speed = Some(1.5);
    r.stream = StreamMode::on(false);
    let (enc, ctx) = encode(&r).unwrap();
    assert!(ctx.stream.is_off());
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body,
        json!({
            "model": "speech-2.8-turbo", "text": "Hi", "stream": false,
            "voice_setting": {"voice_id": "English_CalmWoman", "speed": 1.5},
            "audio_setting": {"sample_rate": 24000, "format": "pcm"},
            "output_format": "hex"
        })
    );
    for f in [
        |r: &mut SpeechRequest| r.instructions = Some("x".into()),
        |r: &mut SpeechRequest| r.format = Some(AudioFormat::Aac),
    ] {
        let mut bad = r.clone();
        f(&mut bad);
        let err = encode(&bad).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }
}

#[test]
fn the_hex_reply_becomes_audio_with_its_characters_and_round_trips() {
    let body = one("responses");
    let r = SpeechRequest::new("speech-2.8-hd", SpeechOperation::Speech).with_input("Hi");
    let (_, ctx) = encode(&r).unwrap();
    let reply = MinimaxSpeech::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap();
    let Reply::Speech(s) = &reply else { panic!() };
    assert_eq!(s.characters, Some(163));
    let Source::Base64(b) = &s.audio else {
        panic!()
    };
    assert_eq!(b.media_type, "audio/mpeg");
    assert_eq!(b.data.to_vec(), [0x49, 0x44, 0x33, 4, 0, 0, 0, 0, 0, 0x23]);
    let out =
        MinimaxSpeech::encode_response(&reply, &EncodeCtx::for_request(Request::Speech(r.clone())))
            .unwrap();
    assert_json_eq("speech", &body, &out);
    let mut whole = MinimaxSpeech::stream_decoder(&ctx);
    let mut events = whole.feed(&body[..10]);
    events.extend(whole.feed(&body[10..]));
    events.extend(whole.finish());
    assert!(matches!(&events[1], StreamEvent::Speech(e) if e.done && e.characters == Some(163)));
    assert!(
        MinimaxSpeech::decode_response(
            200,
            &HeaderList::new(),
            br#"{"data":{"audio":"zz"}}"#,
            &ctx
        )
        .is_err()
    );
}

#[test]
fn a_failing_base_resp_is_an_error_even_with_http_200() {
    let (name, body) = fixtures(DIR, "errors", "json").remove(0);
    let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
    assert_eq!(meta["status"], 200);
    let r = SpeechRequest::new("speech-2.8-hd", SpeechOperation::Speech).with_input("Hi");
    let (_, ctx) = encode(&r).unwrap();
    let err = MinimaxSpeech::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(err.message.contains("voice_id"));
    let enc = MinimaxSpeech::encode_error(&err);
    assert!(String::from_utf8(enc.body).unwrap().contains("2013"));
    assert!(
        MinimaxSpeech::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err()
    );
}

#[test]
fn each_audio_format_gets_its_media_type_and_odd_audio_is_refused() {
    let r = SpeechRequest::new("speech-2.8-hd", SpeechOperation::Speech).with_input("Hi");
    let (_, ctx) = encode(&r).unwrap();
    for (format, media_type) in [
        ("wav", "audio/wav"),
        ("flac", "audio/flac"),
        ("opus", "audio/opus"),
        ("pcm", "audio/pcm"),
        ("ogg", "application/octet-stream"),
    ] {
        let body =
            json!({"data": {"audio": "4944", "status": 2}, "extra_info": {"audio_format": format}});
        let reply = MinimaxSpeech::decode_response(
            200,
            &HeaderList::new(),
            body.to_string().as_bytes(),
            &ctx,
        )
        .unwrap();
        let Reply::Speech(s) = reply else { panic!() };
        assert_eq!(s.audio.media_type(), Some(media_type));
    }
    let odd = br#"{"data":{"audio":"494"}}"#;
    assert!(MinimaxSpeech::decode_response(200, &HeaderList::new(), odd, &ctx).is_err());
    let url = SpeechResponse::new("m", Source::Url("https://example.com/a.mp3".into()));
    let app = EncodeCtx::for_request(Request::Speech(r.clone()));
    assert!(MinimaxSpeech::encode_response(&Reply::Speech(url.clone()), &app).is_err());
    let mut whole = MinimaxSpeech::stream_encoder(&app);
    let err = whole
        .encode(&StreamEvent::Speech(SpeechEvent::audio("AA")))
        .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("stream"));
    assert!(whole.finish().unwrap().is_empty());
}

#[test]
fn streams_operations_and_base_paths_follow_t2a() {
    let streamed = decode(br#"{"model":"speech-2.8-hd","text":"Hi","stream":true}"#);
    assert!(!streamed.stream.is_off());
    let plain = decode(br#"{"model":"speech-2.8-hd","text":"Hi"}"#);
    let req = Request::Speech(plain.clone());
    let mut ctx = EgressCtx::new(&req).with_base_path("/minimax");
    assert_eq!(
        MinimaxSpeech::encode_request(&req, &mut ctx).unwrap().path,
        format!("/minimax{T2A_PATH}")
    );
    let mut sfx = plain.clone();
    sfx.operation = SpeechOperation::SoundEffect;
    let mut dialogue = plain;
    dialogue.audio = Some(kaeso_kpp::AudioInput::new(
        Source::base64("audio/mpeg", "SUQz").unwrap(),
    ));
    for bad in [sfx, dialogue] {
        let req = Request::Speech(bad);
        let err = MinimaxSpeech::check(&req, &EgressCtx::new(&req)).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }
}
