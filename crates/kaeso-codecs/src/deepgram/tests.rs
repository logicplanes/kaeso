use kaeso_kpp::{
    AudioFormat, AudioInput, ErrorKind, Extras, Origin, RawJson, Reply, Request, RequestKind,
    Source, SpeechOperation, SpeechRequest, StreamEvent, StreamMode, TimestampGranularity,
    TranscriptFormat, TranscriptionOperation, TranscriptionRequest, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, fixtures_dir, sidecar};

const DIR: &str = "deepgram";

fn pairs(path: &str) -> (String, Vec<(String, String)>) {
    let (p, q) = path.split_once('?').unwrap_or((path, ""));
    let mut pairs = crate::models::query_pairs(q).unwrap();
    pairs.sort();
    (p.to_owned(), pairs)
}

fn query_of(path: &str) -> Vec<(String, String)> {
    let (_, q) = path.split_once('?').unwrap_or((path, ""));
    crate::models::query_pairs(q).unwrap()
}

fn has(path: &str, k: &str, v: &str) -> bool {
    query_of(path).iter().any(|(a, b)| a == k && b == v)
}

fn named(path: &str, k: &str) -> bool {
    query_of(path).iter().any(|(a, _)| a == k)
}

fn upload(model: &str) -> TranscriptionRequest {
    TranscriptionRequest::new(
        model,
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::bytes("audio/mpeg", b"ID3fake".to_vec())).with_filename("a.mp3"),
    )
}

fn encode(req: Request) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let mut ctx = EgressCtx::new(&req).with_provider("deepgram");
    let enc = Deepgram::encode_request(&req, &mut ctx)?;
    Ok((enc, ctx))
}

fn speech(model: &str, voice: Option<&str>) -> SpeechRequest {
    let r = SpeechRequest::new(model, SpeechOperation::Speech).with_input("Hello there.");
    match voice {
        Some(v) => r.with_voice(Voice::Named(v.to_owned())),
        None => r,
    }
}

fn refused_param(err: &KppError) -> Option<&str> {
    err.param.as_deref()
}

#[test]
fn every_request_fixture_round_trips_with_its_query_and_body() {
    let dir = fixtures_dir().join(DIR).join("requests");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if name.ends_with(".meta.json") || name.ends_with(".md") {
            continue;
        }
        let stem = name.rsplit_once('.').unwrap().0;
        let meta: Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{stem}.meta.json"))).unwrap())
                .unwrap();
        let kind = match meta["kind"].as_str().unwrap() {
            "speech" => RequestKind::Speech,
            _ => RequestKind::Transcription,
        };
        let query = meta["query"].as_str().unwrap();
        let mut headers = HeaderList::new();
        if let Some(ct) = meta["content_type"].as_str() {
            headers.push("content-type", ct);
        }
        let body = std::fs::read(&path).unwrap();
        let req = Deepgram::decode_call(kind, query, &body, &headers).unwrap();
        let (enc, _) = encode(req).unwrap();
        let want_path = match kind {
            RequestKind::Speech => SPEAK_PATH,
            _ => LISTEN_PATH,
        };
        assert_eq!(
            pairs(&enc.path),
            pairs(&format!("{want_path}?{query}")),
            "{name}"
        );
        if name.ends_with(".json") {
            assert_json_eq(&name, &body, &enc.body);
        } else {
            assert_eq!(enc.body, body, "{name}");
        }
        if let Some(ct) = meta["content_type"].as_str() {
            assert_eq!(enc.headers.get("content-type"), Some(ct), "{name}");
        }
        seen += 1;
    }
    assert_eq!(seen, 3);
}

#[test]
fn an_openai_upload_is_sent_as_the_body_with_language_detection_and_smart_formatting() {
    let (enc, ctx) = encode(Request::Transcription(upload("nova-3"))).unwrap();
    assert_eq!(
        enc.path,
        "/v1/listen?model=nova-3&detect_language=true&smart_format=true"
    );
    assert_eq!(enc.body, b"ID3fake");
    assert_eq!(enc.headers.get("content-type"), Some("audio/mpeg"));
    assert_eq!(ctx.transcript_format, Some(TranscriptFormat::Json));
    assert!(ctx.stream.is_off());

    let mut r = upload("nova-3");
    r.language = Some("de".into());
    r.format = Some(TranscriptFormat::Srt);
    r.stream = StreamMode::on(false);
    let (enc, ctx) = encode(Request::Transcription(r)).unwrap();
    assert!(has(&enc.path, "language", "de") && !named(&enc.path, "detect_language"));
    assert!(has(&enc.path, "utterances", "true"));
    assert!(!named(&enc.path, "diarize_model"));
    assert!(ctx.stream.is_off(), "Deepgram answers a transcript whole");

    let mut r = upload("nova-3");
    r.format = Some(TranscriptFormat::DiarizedJson);
    let (enc, _) = encode(Request::Transcription(r.clone())).unwrap();
    assert!(has(&enc.path, "diarize_model", "latest") && has(&enc.path, "utterances", "true"));
    r.diarize = Some(false);
    let (enc, _) = encode(Request::Transcription(r)).unwrap();
    assert!(!named(&enc.path, "diarize_model"));

    let mut r = upload("nova-3");
    r.timestamps = vec![TimestampGranularity::Segment];
    r.diarize = Some(true);
    let (enc, _) = encode(Request::Transcription(r)).unwrap();
    assert!(has(&enc.path, "utterances", "true") && has(&enc.path, "diarize_model", "latest"));

    let mut r = upload("whisper-large");
    r.audio = AudioInput::new(Source::Url("https://example.com/a.wav".into()));
    let (enc, _) = encode(Request::Transcription(r)).unwrap();
    assert_eq!(enc.headers.get("content-type"), Some("application/json"));
    assert_json_eq("url", br#"{"url":"https://example.com/a.wav"}"#, &enc.body);

    let mut r = upload("nova-3");
    r.audio = AudioInput::new(Source::bytes("", b"x".to_vec()));
    let (enc, _) = encode(Request::Transcription(r)).unwrap();
    assert_eq!(
        enc.headers.get("content-type"),
        Some("application/octet-stream")
    );

    let mut ctx = EgressCtx::new(&Request::Transcription(upload("nova-3"))).with_base_path("/dg");
    let enc =
        Deepgram::encode_request(&Request::Transcription(upload("nova-3")), &mut ctx).unwrap();
    assert!(enc.path.starts_with("/dg/v1/listen?"), "{}", enc.path);
}

/// A change to a transcription request.
type Edit = fn(&mut TranscriptionRequest);

#[test]
fn transcription_fields_deepgram_lacks_are_refused_before_sending() {
    let cases: Vec<(Edit, &str)> = vec![
        (|r| r.prompt = Some("Kaeso".into()), "prompt"),
        (|r| r.temperature = Some(0.2), "temperature"),
        (|r| r.speakers = Some(2), "num_speakers"),
        (|r| r.operation = TranscriptionOperation::Translate, "model"),
        (
            |r| {
                r.audio =
                    AudioInput::new(Source::file_id(DialectId::OpenAiTranscriptions, "file-1"))
            },
            "file",
        ),
        (
            |r| r.format = Some(TranscriptFormat::Other("tsv".into())),
            "response_format",
        ),
    ];
    for (change, param) in cases {
        let mut r = upload("nova-3");
        change(&mut r);
        let req = Request::Transcription(r);
        let err = Deepgram::check(&req, &EgressCtx::new(&req)).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::Unsupported { .. }),
            "{param}: {err:?}"
        );
        assert_eq!(refused_param(&err), Some(param));
        assert!(encode(req).is_err());
    }
    let mut r = upload("nova-3");
    r.extras = Extras::new(Origin::new(DialectId::OpenAiTranscriptions)).with_field(
        "chunking_strategy",
        RawJson::from_serialize(&"auto").unwrap(),
    );
    assert!(encode(Request::Transcription(r)).is_err());
}

fn reply(name: &str) -> (Vec<u8>, kaeso_kpp::TranscriptionResponse) {
    let body = fixtures(DIR, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1;
    let ctx = EgressCtx::for_kind(RequestKind::Transcription, StreamMode::Off);
    match Deepgram::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap() {
        Reply::Transcription(t) => (body, t),
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_transcript_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let ctx = EgressCtx::for_kind(RequestKind::Transcription, StreamMode::Off);
        let decoded = Deepgram::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap();
        let out = Deepgram::encode_response(
            &decoded,
            &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
        let mut enc =
            Deepgram::stream_encoder(&EncodeCtx::new(RequestKind::Transcription, StreamMode::Off));
        let Reply::Transcription(t) = decoded else {
            panic!()
        };
        let streamed = enc
            .encode(&StreamEvent::Transcript(kaeso_kpp::TranscriptEvent::done(
                t,
            )))
            .unwrap();
        assert_json_eq(&name, &body, &streamed);
        assert!(enc.finish().unwrap().is_empty());
    }
}

#[test]
fn a_transcript_reads_its_text_words_utterances_language_and_billed_length() {
    let (_, t) = reply("listen-nova3-doc");
    assert!(t.text.starts_with("Yeah. As as much as"));
    assert_eq!(t.duration, Some(25.933313));
    assert_eq!(t.usage_seconds, Some(25.933313));
    assert_eq!(t.words[0].text, "Yeah.", "the punctuated word");
    assert_eq!((t.words[0].start, t.words[0].end), (0.08, 0.32));
    assert!(t.segments.is_empty() && t.language.is_none());

    let (_, t) = reply("listen-utterances-schema");
    assert_eq!(t.language.as_deref(), Some("en"));
    assert_eq!(t.segments.len(), 2);
    assert_eq!(t.segments[1].text, "Yes.");
    assert_eq!(t.segments[1].speaker.as_deref(), Some("1"));
    assert_eq!(
        t.segments[0].id.as_deref(),
        Some("2d8211a4-3a5b-4053-8939-edf2b2b389fa")
    );
    assert_eq!(t.words[6].speaker.as_deref(), Some("1"));
}

fn openai(t: &kaeso_kpp::TranscriptionResponse, format: TranscriptFormat) -> Vec<u8> {
    let mut r = upload("nova-3");
    r.format = Some(format);
    crate::dispatch::encode_response(
        DialectId::OpenAiTranscriptions,
        &Reply::Transcription(t.clone()),
        &EncodeCtx::for_request(Request::Transcription(r)),
    )
    .unwrap()
}

#[test]
fn an_openai_app_gets_each_transcript_form_from_a_deepgram_reply() {
    let (_, t) = reply("listen-utterances-schema");
    let v: Value = serde_json::from_slice(&openai(&t, TranscriptFormat::Json)).unwrap();
    assert_eq!(
        v,
        json!({"text": "Four score and seven years ago. Yes.", "usage": {"type": "duration", "seconds": 4.1}})
    );
    let v: Value = serde_json::from_slice(&openai(&t, TranscriptFormat::VerboseJson)).unwrap();
    assert_eq!(v["language"], "en");
    assert_eq!(v["duration"], 4.1);
    assert_eq!(
        v["words"][0],
        json!({"word": "Four", "start": 0.41915998, "end": 0.85827994, "speaker": "0"})
    );
    assert_eq!(v["segments"][1]["text"], "Yes.");
    assert!(v.get("metadata").is_none(), "Deepgram's fields stay out");
    let v: Value = serde_json::from_slice(&openai(&t, TranscriptFormat::DiarizedJson)).unwrap();
    assert_eq!(v["segments"][1]["speaker"], "1");
    assert_eq!(v["segments"][0]["type"], "transcript.text.segment");
    let srt = String::from_utf8(openai(&t, TranscriptFormat::Srt)).unwrap();
    assert_eq!(
        srt,
        "1\n00:00:00,419 --> 00:00:02,914\nFour score and seven years ago.\n\n2\n00:00:03,513 --> 00:00:03,952\nYes.\n\n"
    );
    let vtt = String::from_utf8(openai(&t, TranscriptFormat::Vtt)).unwrap();
    assert!(vtt.starts_with("WEBVTT\n\n00:00:00.419 --> 00:00:02.914\nFour score"));
    let text = String::from_utf8(openai(&t, TranscriptFormat::Text)).unwrap();
    assert_eq!(text, "Four score and seven years ago. Yes.");

    let (_, t) = reply("listen-nova3-doc");
    let srt = String::from_utf8(openai(&t, TranscriptFormat::Srt)).unwrap();
    assert!(srt.starts_with("1\n00:00:00,080 --> 00:00:00,800\nYeah. As as much as"));
}

#[test]
fn a_reply_with_more_channels_or_alternatives_than_asked_is_an_error() {
    let ctx = EgressCtx::for_kind(RequestKind::Transcription, StreamMode::Off);
    let alt = json!({"transcript": "a", "words": []});
    for body in [
        json!({"results": {"channels": [{"alternatives": [alt]}, {"alternatives": [alt]}]}}),
        json!({"results": {"channels": [{"alternatives": [alt, alt]}]}}),
        json!({"results": {"channels": []}}),
        json!({"results": {"channels": [{"alternatives": [{"words": []}]}]}}),
        json!({"metadata": {}}),
        json!({"results": {"channels": [{"alternatives": [{"transcript": "a", "words": [1]}]}]}}),
        json!({"results": {"channels": [{"alternatives": [alt]}], "utterances": ["x"]}}),
        json!({"results": {"channels": ["x"]}}),
        json!([]),
    ] {
        let err = Deepgram::decode_response(
            200,
            &HeaderList::new(),
            &serde_json::to_vec(&body).unwrap(),
            &ctx,
        )
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::ProviderError, "{body}");
    }
    let two = json!({"metadata": {"duration": 2.5, "channels": 2}, "results": {"channels": [{"alternatives": [alt]}]}});
    let Reply::Transcription(t) = Deepgram::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&two).unwrap(),
        &ctx,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(t.usage_seconds, Some(5.0), "each channel is billed");
}

#[test]
fn a_transcript_from_another_format_is_written_in_deepgrams_shape() {
    let mut t = kaeso_kpp::TranscriptionResponse::new("whisper-1", "Hi there.");
    t.duration = Some(1.5);
    t.language = Some("en".into());
    let mut w = kaeso_kpp::TranscriptWord::new("Hi", 0.1, 0.4);
    w.speaker = Some("A".into());
    t.words.push(w);
    let mut s = kaeso_kpp::TranscriptSegment::new(0.1, 1.2, "Hi there.");
    s.speaker = Some("0".into());
    t.segments.push(s);
    let out = Deepgram::encode_response(
        &Reply::Transcription(t),
        &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v,
        json!({
            "metadata": {"duration": 1.5, "channels": 1},
            "results": {
                "channels": [{"alternatives": [{"transcript": "Hi there.", "words": [{"word": "Hi", "start": 0.1, "end": 0.4, "speaker": "A"}]}], "detected_language": "en"}],
                "utterances": [{"start": 0.1, "end": 1.2, "transcript": "Hi there.", "speaker": 0}]
            }
        })
    );
}

#[test]
fn an_openai_speech_request_names_the_voice_in_the_model_and_the_format_in_the_query() {
    let (enc, ctx) = encode(Request::Speech(speech("aura-2", Some("thalia")))).unwrap();
    assert_eq!(enc.path, "/v1/speak?model=aura-2-thalia-en&encoding=mp3");
    assert_json_eq("speech", br#"{"text":"Hello there."}"#, &enc.body);
    assert_eq!(enc.headers.get("content-type"), Some("application/json"));
    assert_eq!(ctx.audio_format, Some(AudioFormat::Mp3));
    assert!(ctx.stream.is_off());

    for (format, query) in [
        (AudioFormat::Wav, "encoding=linear16&container=wav"),
        (
            AudioFormat::Pcm16,
            "encoding=linear16&container=none&sample_rate=24000",
        ),
        (AudioFormat::Opus, "encoding=opus"),
        (AudioFormat::Aac, "encoding=aac"),
        (AudioFormat::Flac, "encoding=flac"),
    ] {
        let mut r = speech("aura-2", Some("thalia"));
        r.format = Some(format.clone());
        let (enc, ctx) = encode(Request::Speech(r)).unwrap();
        assert_eq!(
            enc.path,
            format!("/v1/speak?model=aura-2-thalia-en&{query}")
        );
        assert_eq!(ctx.audio_format, Some(format));
    }

    for (model, voice, language, sent) in [
        ("aura-2", "Agustina-ES", None, "aura-2-agustina-es"),
        ("aura-2", "aura-2-thalia-en", None, "aura-2-thalia-en"),
        ("aura-2", "aurelia", Some("de-DE"), "aura-2-aurelia-de"),
        ("aura", "asteria", None, "aura-asteria-en"),
        ("aura", "aura-asteria-en", None, "aura-asteria-en"),
        (
            "aura-2-thalia-en",
            "aura-2-thalia-en",
            None,
            "aura-2-thalia-en",
        ),
    ] {
        let mut r = speech(model, Some(voice));
        r.language = language.map(str::to_owned);
        let (enc, _) = encode(Request::Speech(r)).unwrap();
        assert!(has(&enc.path, "model", sent), "{voice}: {}", enc.path);
    }

    let mut r = speech("aura-2", Some("thalia"));
    r.speed = Some(1.25);
    r.sample_rate = Some(48000);
    r.bitrate = Some(48);
    let (enc, _) = encode(Request::Speech(r.clone())).unwrap();
    assert!(has(&enc.path, "speed", "1.25"));
    assert!(has(&enc.path, "sample_rate", "48000") && has(&enc.path, "bit_rate", "48000"));
    r.format = Some(AudioFormat::Pcm16);
    let (enc, _) = encode(Request::Speech(r)).unwrap();
    assert_eq!(
        query_of(&enc.path)
            .iter()
            .filter(|(k, _)| k == "sample_rate")
            .count(),
        1
    );
}

#[test]
fn speech_deepgram_cannot_make_is_refused_before_sending() {
    let long = "a".repeat(MAX_SPEECH_CHARACTERS + 1);
    let cases: Vec<(SpeechRequest, &str)> = vec![
        (speech("aura-2", None), "voice"),
        (speech("aura-2", Some("thalia voice")), "voice"),
        (speech("aura", Some("aura-2-thalia-en")), "voice"),
        (speech("aura-2", Some("aura-asteria-en")), "voice"),
        (speech("aura-2-thalia-en", Some("alloy")), "voice"),
        (
            SpeechRequest::new("aura-2", SpeechOperation::Speech)
                .with_input(long.as_str())
                .with_voice(Voice::Named("thalia".into())),
            "input",
        ),
        (
            {
                let mut r = speech("aura-2", Some("thalia"));
                r.speed = Some(2.0);
                r
            },
            "speed",
        ),
        (
            {
                let mut r = speech("aura-2", Some("thalia"));
                r.instructions = Some("Cheerfully".into());
                r
            },
            "instructions",
        ),
        (
            {
                let mut r = speech("aura-2", Some("thalia"));
                r.format = Some(AudioFormat::Other("pcm_ulaw".into()));
                r
            },
            "response_format",
        ),
        (
            SpeechRequest::new("aura-2", SpeechOperation::SoundEffect).with_input("Rain"),
            "model",
        ),
    ];
    for (r, param) in cases {
        let req = Request::Speech(r);
        let err = Deepgram::check(&req, &EgressCtx::new(&req)).unwrap_err();
        assert_eq!(refused_param(&err), Some(param), "{err:?}");
        assert!(encode(req).is_err());
    }
    let at_limit = SpeechRequest::new("aura-2", SpeechOperation::Speech)
        .with_input("é".repeat(MAX_SPEECH_CHARACTERS))
        .with_voice(Voice::Named("thalia".into()));
    assert!(encode(Request::Speech(at_limit)).is_ok());
}

#[test]
fn the_audio_reply_streams_through_with_the_billed_characters() {
    let body = std::fs::read(fixtures_dir().join(DIR).join("responses/speak.mp3")).unwrap();
    let meta = sidecar(DIR, "responses", "speak.mp3", "meta").unwrap();
    let mut headers =
        HeaderList::new().with("content-type", meta["content_type"].as_str().unwrap());
    for (k, v) in meta["headers"].as_object().unwrap() {
        headers.push(k.clone(), v.as_str().unwrap());
    }
    assert_eq!(billed_characters(&headers), Some(32));
    assert_eq!(
        request_id(&headers),
        Some("00000000-0000-0000-0000-000000000000")
    );
    let req = Request::Speech(speech("aura-2", Some("thalia")));
    let (_, ctx) = encode(req.clone()).unwrap();
    assert!(Deepgram::reads_reply_as_stream(&ctx));
    let reply = Deepgram::decode_response(200, &headers, &body, &ctx).unwrap();
    let Reply::Speech(s) = &reply else { panic!() };
    assert_eq!(s.characters, Some(32));
    assert_eq!(s.audio.media_type(), Some("audio/mpeg"));
    let ectx = EncodeCtx::for_request(req);
    assert_eq!(Deepgram::encode_response(&reply, &ectx).unwrap(), body);
    assert_eq!(Deepgram::response_media_type(&reply, &ectx), "audio/mpeg");
    assert_eq!(Deepgram::stream_media_type(&ectx), "audio/mpeg");

    let mut dec = Deepgram::stream_decoder(&ctx);
    let mut events = dec.feed(&body[..4]);
    events.extend(dec.feed(&[]));
    events.extend(dec.feed(&body[4..]));
    events.extend(dec.finish());
    events.extend(dec.finish());
    assert!(dec.feed(b"late").is_empty());
    assert_eq!(events.len(), 3);
    let mut enc = Deepgram::stream_encoder(&ectx);
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    assert_eq!(out, body);
    let StreamEvent::Speech(first) = &events[0] else {
        panic!()
    };
    assert_eq!(first.media_type.as_deref(), Some("audio/mpeg"));
    assert!(
        enc.encode(&StreamEvent::Error(KppError::provider("x")))
            .is_err()
    );

    let mut pcm = speech("aura-2", Some("thalia"));
    pcm.format = Some(AudioFormat::Pcm16);
    let (_, ctx) = encode(Request::Speech(pcm.clone())).unwrap();
    let Reply::Speech(s) =
        Deepgram::decode_response(200, &HeaderList::new(), b"\0\0", &ctx).unwrap()
    else {
        panic!()
    };
    assert_eq!(s.audio.media_type(), Some("audio/l16;rate=24000"));
    assert_eq!(
        Deepgram::stream_media_type(&EncodeCtx::for_request(Request::Speech(pcm))),
        "audio/l16;rate=24000"
    );
    let link = Reply::Speech(kaeso_kpp::SpeechResponse::new(
        "m",
        Source::Url("https://example.com/a.mp3".into()),
    ));
    assert!(Deepgram::encode_response(&link, &ectx).is_err());
}

#[test]
fn every_error_fixture_reads_as_its_kind_and_passes_back_as_it_came() {
    let kinds = [
        ("corrupt-audio-doc-400", ErrorKind::InvalidRequest),
        ("invalid-json-doc-400", ErrorKind::InvalidRequest),
        ("model-access-doc-403", ErrorKind::PermissionDenied),
        ("payment-required-doc-402", ErrorKind::Billing),
        ("recorded-invalid-auth-401", ErrorKind::Authentication),
        ("recorded-invalid-query-400", ErrorKind::InvalidRequest),
        (
            "recorded-unsupported-audio-format-400",
            ErrorKind::InvalidRequest,
        ),
        ("text-too-long-doc-413", ErrorKind::RequestTooLarge),
        ("too-many-requests-doc-429", ErrorKind::RateLimited),
    ];
    let all = fixtures(DIR, "errors", "json");
    assert_eq!(all.len(), kinds.len());
    for (name, body) in all {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = Deepgram::decode_error(status, &HeaderList::new(), &body);
        let kind = &kinds.iter().find(|(n, _)| *n == name).unwrap().1;
        assert_eq!(&err.kind, kind, "{name}");
        assert_eq!(err.status, Some(status));
        assert!(!err.message.is_empty());
        let enc = Deepgram::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    let corrupt = Deepgram::decode_error(
        400,
        &HeaderList::new(),
        &std::fs::read(
            fixtures_dir()
                .join(DIR)
                .join("errors/corrupt-audio-doc-400.json"),
        )
        .unwrap(),
    );
    assert_eq!(
        corrupt.message,
        "Bad Request: failed to process audio: corrupt or unsupported data"
    );
    let invalid = Deepgram::decode_error(
        400,
        &HeaderList::new(),
        &std::fs::read(
            fixtures_dir()
                .join(DIR)
                .join("errors/invalid-json-doc-400.json"),
        )
        .unwrap(),
    );
    assert_eq!(
        invalid.message,
        "Invalid JSON submitted. Json deserialize error: expected `:` at line 3 column 13"
    );
    let text = std::fs::read(
        fixtures_dir()
            .join(DIR)
            .join("errors/recorded-model-uuid-404.txt"),
    )
    .unwrap();
    let err = Deepgram::decode_error(404, &HeaderList::new(), &text);
    assert_eq!(err.kind, ErrorKind::NotFound);
    assert_eq!(
        err.message,
        "UUID parsing failed: invalid character: found `u` at 1"
    );
    let enc = Deepgram::encode_error(&err);
    assert_eq!(enc.status, 404);
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap()["err_code"],
        "Not Found"
    );
}

#[test]
fn errors_without_a_body_or_from_elsewhere_get_deepgrams_shape() {
    for (status, kind) in [
        (401, ErrorKind::Authentication),
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (413, ErrorKind::RequestTooLarge),
        (415, ErrorKind::InvalidRequest),
        (429, ErrorKind::RateLimited),
        (503, ErrorKind::Overloaded),
        (504, ErrorKind::Timeout),
        (500, ErrorKind::ProviderError),
    ] {
        let err = Deepgram::decode_error(status, &HeaderList::new().with("retry-after", "2"), b"");
        assert_eq!(err.kind, kind, "{status}");
        assert_eq!(
            err.message,
            format!("Deepgram returned HTTP {status} without a message.")
        );
        assert!(err.retry_after.is_some());
        let enc = Deepgram::encode_error(&err);
        assert!(enc.headers.get("retry-after").is_some());
    }
    let unavailable = Deepgram::decode_error(
        503,
        &HeaderList::new(),
        br#"{"error_code":"Service Unavailable","err_msg":"Please try again later","request_id":"x"}"#,
    );
    assert_eq!(unavailable.kind, ErrorKind::Overloaded);
    for (err, status, code) in [
        (KppError::invalid_request("Bad."), 400, "Bad Request"),
        (
            KppError::unsupported_by("x", DialectId::Deepgram, "No."),
            400,
            "Bad Request",
        ),
        (
            KppError::new(ErrorKind::Authentication, "No key."),
            401,
            "INVALID_AUTH",
        ),
        (
            KppError::new(ErrorKind::RequestTooLarge, "Big."),
            413,
            "PAYLOAD_TOO_LARGE",
        ),
        (
            KppError::new(ErrorKind::ContentFiltered, "No."),
            500,
            "Internal Server Error",
        ),
        (
            KppError::provider("Down.").with_status(502),
            502,
            "Internal Server Error",
        ),
    ] {
        let enc = Deepgram::encode_error(&err);
        assert_eq!(enc.status, status);
        let v: Value = serde_json::from_slice(&enc.body).unwrap();
        assert_eq!(v["err_code"], code);
        assert_eq!(v["err_msg"], err.message);
    }
}

#[test]
fn the_model_listing_lists_every_canonical_name_and_round_trips() {
    let (name, body) = fixtures(DIR, "models", "json").remove(0);
    let list = Deepgram::decode_models(200, &HeaderList::new(), &body).unwrap();
    let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "nova-3-general",
            "nova-3-general",
            "nova-2-general",
            "whisper-large",
            "aura-2-thalia-en",
            "aura-2-agustina-es",
            "aura-asteria-en"
        ]
    );
    assert_json_eq(&name, &body, &Deepgram::encode_models(&list).unwrap());
    let one =
        serde_json::to_vec(&serde_json::from_slice::<Value>(&body).unwrap()["tts"][0]).unwrap();
    let model = Deepgram::decode_model(200, &HeaderList::new(), &one).unwrap();
    assert_eq!(model.id, "aura-2-thalia-en");
    assert_json_eq("one", &one, &Deepgram::encode_model(&model).unwrap());
    let mut named = kaeso_kpp::ModelInfo::new("nova-3");
    named.display_name = Some("Nova-3".into());
    let v: Value = serde_json::from_slice(&Deepgram::encode_model(&named).unwrap()).unwrap();
    assert_eq!(v, json!({"name": "Nova-3", "canonical_name": "nova-3"}));
    for bad in [
        &b"x"[..],
        b"[]",
        br#"{"stt": 1}"#,
        br#"{"stt": [1]}"#,
        br#"{"stt": [{}]}"#,
    ] {
        assert!(Deepgram::decode_models(200, &HeaderList::new(), bad).is_err());
    }
    for empty in [&b"{}"[..], br#"{"stt": null, "tts": []}"#] {
        let list = Deepgram::decode_models(200, &HeaderList::new(), empty).unwrap();
        assert!(list.models.is_empty());
    }
    assert!(Deepgram::decode_model(200, &HeaderList::new(), b"[]").is_err());
    let err = Deepgram::decode_model(404, &HeaderList::new(), b"UUID parsing failed").unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound);
    assert!(Deepgram::decode_models(401, &HeaderList::new(), b"{}").is_err());
    assert_eq!(
        Deepgram::encode_model_request("nova-3").path,
        "/v1/models/nova-3"
    );
    assert_eq!(
        Deepgram::encode_models_request(&kaeso_kpp::ModelsQuery::default())
            .unwrap()
            .path,
        "/v1/models"
    );
    assert!(Deepgram::decode_models_query("").is_ok());
}

#[test]
fn only_audio_requests_and_replies_cross_deepgram() {
    let chat = crate::dispatch::decode_request(
        DialectId::OpenAiChat,
        RequestKind::Generate,
        br#"{"model":"m","messages":[{"role":"user","content":"Hi"}]}"#,
        &HeaderList::new(),
    )
    .unwrap();
    assert!(Deepgram::check(&chat, &EgressCtx::new(&chat)).is_err());
    assert!(encode(chat).is_err());
    assert!(Deepgram::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err());
    let gen_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(Deepgram::decode_response(200, &HeaderList::new(), b"{}", &gen_ctx).is_err());
    assert!(!Deepgram::reads_reply_as_stream(&gen_ctx));
    let ectx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    let embed = Reply::Embed(kaeso_kpp::EmbedResponse::default());
    assert!(Deepgram::encode_response(&embed, &ectx).is_err());
    assert_eq!(
        Deepgram::response_media_type(&embed, &ectx),
        "application/json"
    );
    assert_eq!(Deepgram::stream_media_type(&ectx), "application/json");

    let req = Deepgram::decode_request(
        RequestKind::Speech,
        br#"{"text":"Hi","url":"https://example.com/t.txt"}"#,
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Speech(r) = &req else { panic!() };
    assert_eq!(r.model, "");
    let req = Deepgram::decode_call(
        RequestKind::Speech,
        "model=aura-2-thalia-en&speed=1&tag=a&tag=b",
        br#"{"text":"Hi"}"#,
        &HeaderList::new(),
    )
    .unwrap();
    let (enc, _) = encode(req).unwrap();
    assert_eq!(
        enc.path,
        "/v1/speak?model=aura-2-thalia-en&speed=1&tag=a&tag=b"
    );
    assert!(
        Deepgram::decode_call(RequestKind::Speech, "speed=fast", b"{}", &HeaderList::new())
            .is_err()
    );
    let json = HeaderList::new().with("content-type", "application/json; charset=utf-8");
    assert!(Deepgram::decode_call(RequestKind::Transcription, "", b"{}", &json).is_err());
    let req = Deepgram::decode_call(
        RequestKind::Transcription,
        "model=nova-3",
        b"RIFF",
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Transcription(t) = &req else {
        panic!()
    };
    assert_eq!(
        t.audio.source.media_type(),
        Some("application/octet-stream")
    );
}
