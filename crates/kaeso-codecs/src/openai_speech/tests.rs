use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, ErrorKind, Reply, Request, RequestKind, Source, SpeechEvent, SpeechOperation,
    SpeechRequest, SpeechResponse, StreamEvent, StreamMode, Usage, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures, sidecar};

const DIR: &str = "openai_speech";

fn own(dir: &str, folder: &str, ext: &str) -> (String, Vec<u8>) {
    fixtures(dir, folder, ext)
        .into_iter()
        .find(|(n, _)| !n.starts_with("providers/"))
        .unwrap()
}

fn decode(body: &[u8]) -> SpeechRequest {
    match OpenAiSpeech::decode_request(RequestKind::Speech, body, &HeaderList::new()).unwrap() {
        Request::Speech(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode(
    profile: SpeechProfile,
    req: &SpeechRequest,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let r = Request::Speech(req.clone());
    let mut ctx = EgressCtx::new(&r)
        .with_provider("openai")
        .with_speech_profile(profile);
    let enc = OpenAiSpeech::encode_request(&r, &mut ctx)?;
    Ok((enc, ctx))
}

fn body_of(enc: &EncodedRequest) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn refusal(profile: SpeechProfile, req: &SpeechRequest) -> (String, String) {
    let err = encode(profile, req).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    (err.param.unwrap_or_default(), err.message)
}

fn speech(input: &str) -> SpeechRequest {
    SpeechRequest::new("gpt-4o-mini-tts", SpeechOperation::Speech)
        .with_input(input)
        .with_voice(Voice::Named("alloy".into()))
}

fn app_ctx(req: &SpeechRequest) -> EncodeCtx {
    EncodeCtx::for_request(Request::Speech(req.clone()))
}

#[test]
fn every_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        if name.starts_with("providers/") {
            continue;
        }
        let req = decode(&body);
        req.validate().unwrap();
        let (enc, _) = encode(SpeechProfile::OPENAI, &req).unwrap();
        assert_eq!(enc.path, "/v1/audio/speech", "{name}");
        let mut sent = body_of(&enc);
        let mut original: Value = serde_json::from_slice(&body).unwrap();
        // Kaeso asks OpenAI for events where it reports usage only there.
        if original.get("stream_format") != Some(&json!("sse")) && !req.model.starts_with("tts-1") {
            assert_eq!(sent["stream_format"], "sse", "{name}");
            sent.as_object_mut().unwrap().remove("stream_format");
            original.as_object_mut().unwrap().remove("stream_format");
        }
        let original = serde_json::to_vec(&original).unwrap();
        assert_json_eq(&name, &original, &serde_json::to_vec(&sent).unwrap());
    }
}

#[test]
fn voices_formats_and_speeds_decode_to_typed_fields() {
    let req = decode(
        br#"{"model":"m","input":"Hi","voice":{"id":"voice_1"},"response_format":"pcm","speed":1}"#,
    );
    assert_eq!(req.voice, Some(Voice::Id("voice_1".into())));
    assert_eq!(req.format, Some(AudioFormat::Pcm16));
    assert_eq!(req.speed, Some(1.0));
    let req = decode(br#"{"model":"m","input":"Hi","voice":"alloy"}"#);
    assert_eq!(req.format, Some(AudioFormat::Mp3), "OpenAI's default");
    assert_eq!(req.characters(), 2);
    for (body, param) in [
        (&br#"{"model":"m","input":"Hi","voice":3}"#[..], "voice"),
        (br#"{"model":"m","input":"Hi","voice":{"id":3}}"#, "voice"),
        (
            br#"{"model":"m","input":"Hi","stream_format":"ws"}"#,
            "stream_format",
        ),
        (br#"{"model":"m","input":"Hi","speed":"fast"}"#, "speed"),
    ] {
        let err = OpenAiSpeech::decode_request(RequestKind::Speech, body, &HeaderList::new())
            .unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{err:?}");
    }
}

#[test]
fn the_audio_reply_becomes_bytes_with_the_type_the_app_asked_for() {
    let (name, audio) = own(DIR, "responses", "mp3");
    let meta = sidecar(DIR, "responses", &format!("{name}.mp3"), "meta").unwrap();
    let headers = HeaderList::new().with("content-type", meta["content_type"].as_str().unwrap());
    let req = speech("Hello");
    let ctx = EgressCtx::new(&Request::Speech(req.clone()));
    let Reply::Speech(r) = OpenAiSpeech::decode_response(200, &headers, &audio, &ctx).unwrap()
    else {
        panic!()
    };
    let Source::Base64(b) = &r.audio else {
        panic!()
    };
    assert_eq!(b.media_type, "audio/mpeg");
    let reply = Reply::Speech(r);
    let out = OpenAiSpeech::encode_response(&reply, &app_ctx(&req)).unwrap();
    assert_eq!(out, audio);
    assert_eq!(
        OpenAiSpeech::response_media_type(&reply, &app_ctx(&req)),
        "audio/mpeg"
    );
}

#[test]
fn the_event_stream_round_trips_and_feeds_an_audio_app() {
    let (name, sse) = own(DIR, "streams", "sse");
    let mut req = speech("Hi");
    req.stream = StreamMode::on(false);
    let r = Request::Speech(req.clone());
    let mut ctx = EgressCtx::new(&r).with_speech_profile(SpeechProfile::OPENAI);
    OpenAiSpeech::encode_request(&r, &mut ctx).unwrap();
    assert!(OpenAiSpeech::reads_reply_as_stream(&ctx));
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = Vec::new();
    for chunk in sse.chunks(37) {
        events.extend(dec.feed(chunk));
    }
    events.extend(dec.finish());
    assert!(
        events.iter().all(|e| matches!(e, StreamEvent::Speech(_))),
        "{events:?}"
    );
    let mut enc = OpenAiSpeech::stream_encoder(&app_ctx(&req));
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    assert_sse_eq(&name, &sse, &out);
    assert_eq!(
        OpenAiSpeech::stream_media_type(&app_ctx(&req)),
        crate::EVENT_STREAM_MEDIA_TYPE
    );

    let plain = speech("Hi");
    let mut enc = OpenAiSpeech::stream_encoder(&app_ctx(&plain));
    let mut bytes = Vec::new();
    for e in &events {
        bytes.extend(enc.encode(e).unwrap());
    }
    let (_, mp3) = own(DIR, "responses", "mp3");
    assert_eq!(bytes, mp3, "an audio app gets the bytes of the events");
    let Reply::Speech(whole) =
        reply::assemble(&events, "audio/mpeg", &ctx.origin(DIALECT)).unwrap()
    else {
        panic!()
    };
    assert_eq!(whole.usage.output_tokens, 101);
    assert_eq!(whole.usage.output_audio_tokens, Some(101));
}

#[test]
fn raw_audio_streams_through_piece_by_piece() {
    let req = speech("Hi");
    let mut ctx = EgressCtx::new(&Request::Speech(req.clone()));
    ctx.stream = StreamMode::Off;
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = dec.feed(b"ID3");
    events.extend(dec.feed(b"abc"));
    events.extend(dec.finish());
    let StreamEvent::Speech(first) = &events[0] else {
        panic!()
    };
    assert_eq!(first.media_type.as_deref(), Some("audio/mpeg"));
    assert!(matches!(&events[2], StreamEvent::Speech(e) if e.done));
    let mut enc = OpenAiSpeech::stream_encoder(&app_ctx(&req));
    let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
    assert_eq!(out, b"ID3abc");
    assert!(enc.finish().unwrap().is_empty());
}

#[test]
fn a_whole_reply_to_an_events_app_is_one_delta_and_the_end() {
    let mut req = speech("Hi");
    req.stream = StreamMode::on(false);
    let mut r = SpeechResponse::new("gpt-4o-mini-tts", Source::base64("audio/mpeg", "SUQz"));
    r.usage = Usage::new(3, 4);
    let out = OpenAiSpeech::encode_response(&Reply::Speech(r), &app_ctx(&req)).unwrap();
    let items = crate::testing::sse_items(&out).unwrap();
    assert_eq!(items.len(), 2);
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains(r#""audio":"SUQz""#) && text.contains("speech.audio.delta"));
    assert!(text.contains(r#""total_tokens":7"#), "{text}");
    let link = SpeechResponse::new("m", Source::Url("https://example.com/a.mp3".into()));
    assert!(OpenAiSpeech::encode_response(&Reply::Speech(link), &app_ctx(&req)).is_err());
}

#[test]
fn events_are_asked_for_usage_except_where_the_model_has_none() {
    let (enc, ctx) = encode(SpeechProfile::OPENAI, &speech("Hi")).unwrap();
    assert_eq!(body_of(&enc)["stream_format"], "sse");
    assert!(!ctx.stream.is_off());
    let mut old = speech("Hi");
    old.model = "tts-1".into();
    let (enc, ctx) = encode(SpeechProfile::OPENAI, &old).unwrap();
    assert!(body_of(&enc).get("stream_format").is_none());
    assert!(ctx.stream.is_off());
    old.stream = StreamMode::on(false);
    assert_eq!(refusal(SpeechProfile::OPENAI, &old).0, "stream_format");
}

#[test]
fn what_the_provider_lacks_is_refused_before_sending() {
    let p = SpeechProfile::OPENAI;
    let mut r = speech("Hi");
    r.voice = Some(Voice::Named("Kore".into()));
    assert_eq!(refusal(p, &r).0, "voice");
    let mut r = speech("Hi");
    r.format = Some(AudioFormat::Other("ulaw".into()));
    assert_eq!(refusal(p, &r).0, "response_format");
    for (field, set) in [
        (
            "sample_rate",
            (|r: &mut SpeechRequest| r.sample_rate = Some(8000)) as fn(&mut _),
        ),
        ("language", |r| r.language = Some("de".into())),
        ("seed", |r| r.seed = Some(1)),
        ("bitrate", |r| r.bitrate = Some(128)),
        ("timestamps", |r| r.timestamps = true),
        ("duration", |r| r.duration = Some(2.0)),
    ] {
        let mut r = speech("Hi");
        set(&mut r);
        assert_eq!(refusal(p, &r).0, field);
    }
    let mut music = SpeechRequest::new("m", SpeechOperation::Music).with_input("jazz");
    music.voice = None;
    assert_eq!(refusal(p, &music).0, "model");
    let mut no_ids = p;
    no_ids.voice_ids = false;
    let mut r = speech("Hi");
    r.voice = Some(Voice::Id("v".into()));
    assert_eq!(refusal(no_ids, &r).0, "voice");
    let mut gemini_app = speech("Hi");
    gemini_app.extras = kaeso_kpp::Extras::new(DialectId::Gemini)
        .with_field("x", kaeso_kpp::RawJson::from_serialize(&json!(1)).unwrap());
    assert!(encode(p, &gemini_app).is_err());
}

#[test]
fn errors_round_trip_in_openais_shape() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = meta["status"].as_u64().unwrap() as u16;
        let err = OpenAiSpeech::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.param.as_deref(), Some("voice"));
        let enc = OpenAiSpeech::encode_error(&err);
        assert_eq!(enc.status, status);
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn a_streamed_error_reaches_an_events_app_and_an_unfinished_stream_fails() {
    let mut req = speech("Hi");
    req.stream = StreamMode::on(false);
    let ctx = EgressCtx::new(&Request::Speech(req.clone()));
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = dec.feed(
        b"data: {\"type\":\"error\",\"error\":{\"message\":\"busy\",\"type\":\"server_error\"}}\n\n",
    );
    events.extend(dec.finish());
    assert!(matches!(&events[0], StreamEvent::Error(e) if e.message == "busy"));
    let mut enc = OpenAiSpeech::stream_encoder(&app_ctx(&req));
    let out = enc.encode(&events[0]).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("busy"));
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = dec.feed(b"data: {\"type\":\"speech.audio.delta\",\"audio\":\"AA==\"}\n\n");
    events.extend(dec.finish());
    assert!(matches!(events.last(), Some(StreamEvent::Error(_))));
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    assert!(matches!(
        dec.feed(b"data: nope\n\n")[0],
        StreamEvent::Error(_)
    ));
}

#[test]
fn an_event_stream_answered_whole_is_assembled() {
    let (_, sse) = own(DIR, "streams", "sse");
    let req = speech("Hi");
    let mut ctx = EgressCtx::new(&Request::Speech(req));
    ctx.stream = StreamMode::on(true);
    let headers = HeaderList::new().with("content-type", "text/event-stream");
    let Reply::Speech(r) = OpenAiSpeech::decode_response(200, &headers, &sse, &ctx).unwrap() else {
        panic!()
    };
    let Source::Base64(b) = r.audio else { panic!() };
    let (_, mp3) = own(DIR, "responses", "mp3");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(b.data)
            .unwrap(),
        mp3
    );
    let event = SpeechEvent::done(None);
    assert!(event.done);
}

fn provider_fixture(id: &str, folder: &str, name: &str, ext: &str) -> Vec<u8> {
    let path = format!("providers/{id}/{name}");
    fixtures(DIR, folder, ext)
        .into_iter()
        .find(|(n, _)| *n == path)
        .map(|(_, b)| b)
        .unwrap_or_else(|| panic!("no {path}.{ext}"))
}

fn sent(profile: SpeechProfile, req: &SpeechRequest) -> (EncodedRequest, EgressCtx, Value) {
    let (enc, ctx) = encode(profile, req).unwrap();
    let body = body_of(&enc);
    (enc, ctx, body)
}

#[test]
fn each_provider_gets_its_documented_body() {
    let mut groq = SpeechRequest::new("canopylabs/orpheus-v1-english", SpeechOperation::Speech)
        .with_input("Welcome to Orpheus text-to-speech. [cheerful] This is an example of high-quality English audio generation with vocal directions support.")
        .with_voice(Voice::Named("austin".into()));
    groq.format = Some(AudioFormat::Wav);
    let (enc, ctx, body) = sent(SpeechProfile::GROQ, &groq);
    assert_eq!(enc.path, "/v1/audio/speech");
    assert!(ctx.stream.is_off());
    let expected: Value =
        serde_json::from_slice(&provider_fixture("groq", "requests", "speech", "json")).unwrap();
    assert_eq!(body, expected);

    let mut together = SpeechRequest::new("canopylabs/orpheus-3b-0.1-ft", SpeechOperation::Speech)
        .with_input("Today is a wonderful day to build something people love!")
        .with_voice(Voice::Named("tara".into()));
    together.format = Some(AudioFormat::Mp3);
    let (_, _, body) = sent(SpeechProfile::TOGETHER, &together);
    let expected: Value = serde_json::from_slice(&provider_fixture(
        "togetherai",
        "requests",
        "speech",
        "json",
    ))
    .unwrap();
    assert_eq!(body, expected);

    let mut mistral = SpeechRequest::new("voxtral-mini-tts-3-0", SpeechOperation::Speech)
        .with_input("Your order shipped this morning and should arrive on Thursday.")
        .with_voice(Voice::Named("gb_jane_neutral".into()));
    mistral.format = Some(AudioFormat::Mp3);
    let (_, ctx, body) = sent(SpeechProfile::MISTRAL, &mistral);
    assert!(
        !OpenAiSpeech::reads_reply_as_stream(&ctx),
        "Mistral answers JSON"
    );
    let expected: Value =
        serde_json::from_slice(&provider_fixture("mistral", "requests", "speech", "json")).unwrap();
    assert_eq!(body, expected);

    let mut openrouter =
        SpeechRequest::new("mistralai/voxtral-mini-tts-2603", SpeechOperation::Speech)
            .with_input("Hello world")
            .with_voice(Voice::Named("en_paul_neutral".into()));
    openrouter.format = Some(AudioFormat::Pcm16);
    openrouter.speed = Some(1.0);
    openrouter.extras = kaeso_kpp::Extras::new(DIALECT);
    crate::wire::note_integers(
        &mut openrouter.extras,
        &[("speed", Some(&serde_json::Number::from(1)))],
    );
    let (_, _, body) = sent(SpeechProfile::OPENROUTER, &openrouter);
    let expected: Value = serde_json::from_slice(&provider_fixture(
        "openrouter",
        "requests",
        "speech",
        "json",
    ))
    .unwrap();
    assert_eq!(body, expected);

    let mut xai = SpeechRequest::new("grok-tts", SpeechOperation::Speech)
        .with_input("Hello! This is a high-fidelity text to speech example.")
        .with_voice(Voice::Named("ara".into()));
    xai.format = Some(AudioFormat::Mp3);
    xai.sample_rate = Some(44100);
    xai.language = Some("en".into());
    xai.speed = Some(1.2);
    let (enc, _, body) = sent(SpeechProfile::XAI, &xai);
    assert_eq!(enc.path, "/v1/tts");
    let expected: Value =
        serde_json::from_slice(&provider_fixture("xai", "requests", "speech", "json")).unwrap();
    assert_eq!(body, expected);
    xai.language = None;
    assert_eq!(sent(SpeechProfile::XAI, &xai).2["language"], "auto");

    let mut vercel = SpeechRequest::new("openai/tts-1", SpeechOperation::Speech)
        .with_input("Hello! Thanks for trying out AI Gateway.")
        .with_voice(Voice::Named("alloy".into()));
    vercel.format = Some(AudioFormat::Mp3);
    let (enc, ctx, body) = sent(SpeechProfile::VERCEL, &vercel);
    assert_eq!(enc.path, "/v4/ai/speech-model");
    assert_eq!(enc.headers.get("ai-model-id"), Some("openai/tts-1"));
    assert_eq!(
        enc.headers.get("ai-speech-model-specification-version"),
        Some("4")
    );
    let expected: Value =
        serde_json::from_slice(&provider_fixture("vercel", "requests", "speech", "json")).unwrap();
    assert_eq!(body, expected);
    let reply = OpenAiSpeech::decode_response(
        200,
        &HeaderList::new(),
        &provider_fixture("vercel", "responses", "speech", "json"),
        &ctx,
    )
    .unwrap();
    let Reply::Speech(r) = reply else { panic!() };
    assert!(
        matches!(&r.audio, Source::Base64(b) if b.data == "SUQzBAAAAAAA" && b.media_type == "audio/mpeg")
    );
}

#[test]
fn an_app_without_a_format_asks_other_providers_for_mp3() {
    let req = decode(br#"{"model":"canopylabs/orpheus-3b-0.1-ft","input":"Hi","voice":"tara"}"#);
    assert_eq!(
        sent(SpeechProfile::TOGETHER, &req).2["response_format"],
        "mp3"
    );
    let openai = decode(br#"{"model":"tts-1","input":"Hi","voice":"alloy"}"#);
    let (enc, _) = encode(SpeechProfile::OPENAI, &openai).unwrap();
    assert!(body_of(&enc).get("response_format").is_none());
    let (param, message) = refusal(SpeechProfile::GROQ, &req);
    assert_eq!(param, "response_format");
    assert!(
        message.contains("Groq") && message.contains("wav"),
        "{message}"
    );
}

#[test]
fn mistral_replies_and_events_carry_audio_data() {
    let mut req = SpeechRequest::new("voxtral-mini-tts-3-0", SpeechOperation::Speech)
        .with_input("Hi")
        .with_voice(Voice::Id("gb_jane_neutral".into()));
    req.format = Some(AudioFormat::Mp3);
    let (_, ctx, _) = sent(SpeechProfile::MISTRAL, &req);
    let reply = OpenAiSpeech::decode_response(
        200,
        &HeaderList::new(),
        &provider_fixture("mistral", "responses", "speech", "json"),
        &ctx,
    )
    .unwrap();
    let Reply::Speech(r) = reply else { panic!() };
    assert!(matches!(&r.audio, Source::Base64(b) if b.data.starts_with("SUQz")));
    assert!(OpenAiSpeech::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
    assert!(OpenAiSpeech::decode_response(200, &HeaderList::new(), b"nope", &ctx).is_err());

    req.stream = StreamMode::on(false);
    let (_, ctx, body) = sent(SpeechProfile::MISTRAL, &req);
    assert_eq!(body["stream"], true);
    assert!(body.get("stream_format").is_none());
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = dec.feed(&provider_fixture(
        "mistral",
        "streams",
        "stream-speech",
        "sse",
    ));
    events.extend(dec.finish());
    let pieces: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::Speech(s) => s.audio.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(pieces, ["SUQzBAAAAAAAI1RTU0UAAAAP", "AAADTGF2ZjYwLjMuMTAw"]);
    let Some(StreamEvent::Speech(done)) = events.last() else {
        panic!()
    };
    assert_eq!(
        done.usage
            .as_ref()
            .map(|u| (u.input_tokens, u.output_tokens)),
        Some((14, 120))
    );
}

#[test]
fn provider_bodies_take_no_foreign_fields() {
    let mut req = speech("Hi");
    req.voice = Some(Voice::Named("eve".into()));
    req.extras = kaeso_kpp::Extras::new(DIALECT)
        .with_field("x", kaeso_kpp::RawJson::from_serialize(&json!(1)).unwrap());
    assert_eq!(refusal(SpeechProfile::XAI, &req).0, "x");
    req.voice = Some(Voice::Named("alloy".into()));
    req.extras = kaeso_kpp::Extras::default();
    assert_eq!(refusal(SpeechProfile::XAI, &req).0, "voice");
    req.voice = Some(Voice::Id("custom-1".into()));
    assert_eq!(sent(SpeechProfile::XAI, &req).2["voice_id"], "custom-1");
}

#[test]
fn unknown_fields_headers_and_settings_reach_a_provider_that_takes_them() {
    let headers = HeaderList::new().with("openai-beta", "realtime=v1");
    let body = br#"{"model":"tts-1","input":"Hi","voice":"alloy","stream_format":"audio","metadata":{"order":"7"}}"#;
    let Request::Speech(req) =
        OpenAiSpeech::decode_request(RequestKind::Speech, body, &headers).unwrap()
    else {
        panic!()
    };
    let r = Request::Speech(req.clone());
    let mut ctx = EgressCtx::new(&r)
        .with_provider("openai")
        .with_speech_profile(SpeechProfile::OPENAI)
        .with_base_path("/openai");
    let enc = OpenAiSpeech::encode_request(&r, &mut ctx).unwrap();
    assert_eq!(enc.path, "/openai/v1/audio/speech");
    assert_eq!(enc.headers.get("openai-beta"), Some("realtime=v1"));
    let sent = body_of(&enc);
    assert_eq!(sent["stream_format"], "audio");
    assert_eq!(sent["metadata"], json!({"order": "7"}));

    let mut settings = speech("Hi");
    settings.voice = None;
    settings.sample_rate = Some(24000);
    settings.language = Some("de".into());
    settings.seed = Some(7);
    let takes = SpeechProfile {
        extensions: &["sample_rate", "language", "seed"],
        ..SpeechProfile::TOGETHER
    };
    let (enc, _) = encode(takes, &settings).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        (&sent["sample_rate"], &sent["language"], &sent["seed"]),
        (&json!(24000), &json!("de"), &json!(7))
    );
    assert!(sent.get("voice").is_none());
    let mut vercel = speech("Hi");
    vercel.instructions = Some("Calm.".into());
    vercel.speed = Some(1.25);
    vercel.language = Some("de".into());
    let takes = SpeechProfile {
        instructions: true,
        speed: true,
        extensions: &["language"],
        ..SpeechProfile::VERCEL
    };
    let (enc, _) = encode(takes, &vercel).unwrap();
    let sent = body_of(&enc);
    assert_eq!(
        (&sent["instructions"], &sent["speed"], &sent["language"]),
        (&json!("Calm."), &json!(1.25), &json!("de"))
    );
    let mut dialogue = speech("Hi");
    dialogue.audio = Some(kaeso_kpp::AudioInput::new(Source::base64(
        "audio/mpeg",
        "SUQz",
    )));
    assert_eq!(refusal(SpeechProfile::OPENAI, &dialogue).0, "input");
    let r = Request::Speech(dialogue);
    let ctx = EgressCtx::new(&r).with_speech_profile(SpeechProfile::OPENAI);
    assert!(OpenAiSpeech::check(&r, &ctx).is_err());
}

#[test]
fn broken_event_replies_fail_and_unknown_events_pass_to_events_apps() {
    let req = speech("Hi");
    let mut ctx = EgressCtx::new(&Request::Speech(req.clone()));
    ctx.stream = StreamMode::on(true);
    let headers = HeaderList::new().with("content-type", "text/event-stream");
    for body in [
        "data: {\"type\":\"error\",\"error\":{\"message\":\"busy\"}}\n\n",
        "data: {\"type\":\"speech.audio.delta\",\"audio\":\"not base64!\"}\n\n",
    ] {
        assert!(
            OpenAiSpeech::decode_response(200, &headers, body.as_bytes(), &ctx).is_err(),
            "{body}"
        );
    }
    let mut dec = OpenAiSpeech::stream_decoder(&ctx);
    let mut events = dec
        .feed(b": keepalive\n\ndata: [DONE]\n\nevent: ping\ndata: {\"type\":\"speech.ping\"}\n\n");
    events.extend(dec.finish());
    assert!(matches!(&events[0], StreamEvent::Extra(_)), "{events:?}");
    let mut events_app = req.clone();
    events_app.stream = StreamMode::on(false);
    let mut enc = OpenAiSpeech::stream_encoder(&app_ctx(&events_app));
    assert!(
        String::from_utf8(enc.encode(&events[0]).unwrap())
            .unwrap()
            .contains("speech.ping")
    );
    assert!(
        enc.encode(&StreamEvent::Transcript(kaeso_kpp::TranscriptEvent::delta(
            "Hi"
        )))
        .unwrap()
        .is_empty()
    );
    let mut audio_app = OpenAiSpeech::stream_encoder(&app_ctx(&req));
    assert!(
        audio_app
            .encode(&StreamEvent::Speech(SpeechEvent::audio("not base64!")))
            .is_err()
    );
}
