use std::time::Duration;

use kaeso_kpp::{
    AudioFormat, AudioInput, CountTokensResponse, ErrorKind, KppError, Origin, Reply, Request,
    RequestKind, RerankDocument, RerankRequest, Source, SpeechEvent, SpeechOperation,
    SpeechRequest, SpeechResponse, StreamEvent, StreamMode, TranscriptEvent, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::codec::JSON_MEDIA_TYPE;

const VOICE: &str = "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4";

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn decode(status: u16, body: &Value) -> KppError {
    Cartesia::decode_error(
        status,
        &HeaderList::new(),
        &serde_json::to_vec(body).unwrap(),
    )
}

fn body_of(enc: &EncodedError) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn speech() -> SpeechRequest {
    let mut r = SpeechRequest::new("sonic-3", SpeechOperation::Speech);
    r.input = Some("Hello there.".into());
    r.voice = Some(Voice::Id(VOICE.into()));
    r
}

fn transcription() -> TranscriptionRequest {
    let mut r = TranscriptionRequest::new(
        "ink-whisper",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::bytes(
            "audio/wav",
            bytes::Bytes::from_static(b"RIFF"),
        )),
    );
    r.language = Some("en".into());
    r
}

fn speech_request(body: Value) -> Result<Request, KppError> {
    Cartesia::decode_request(
        RequestKind::Speech,
        &serde_json::to_vec(&body).unwrap(),
        &HeaderList::new(),
    )
}

#[test]
fn every_documented_error_code_reads_as_its_kind_and_field() {
    let cases = [
        ("voice_not_found", ErrorKind::NotFound, Some("voice")),
        (
            "voice_model_mismatch",
            ErrorKind::InvalidRequest,
            Some("voice"),
        ),
        ("model_not_found", ErrorKind::NotFound, Some("model")),
        ("model_sunsetted", ErrorKind::NotFound, Some("model")),
        (
            "language_not_supported",
            ErrorKind::InvalidRequest,
            Some("language"),
        ),
        (
            "unsupported_audio_format",
            ErrorKind::InvalidRequest,
            Some("file"),
        ),
        ("file_too_large", ErrorKind::RequestTooLarge, Some("file")),
        ("quota_exceeded", ErrorKind::Billing, None),
        ("concurrency_limited", ErrorKind::RateLimited, None),
        ("plan_upgrade_required", ErrorKind::PermissionDenied, None),
        (
            "a_code_kaeso_does_not_know",
            ErrorKind::InvalidRequest,
            None,
        ),
    ];
    for (code, kind, param) in cases {
        let err = decode(
            400,
            &json!({"error_code": code, "title": "Bad Request", "message": "Read the docs.", "request_id": "r1"}),
        );
        assert_eq!(
            (err.kind.clone(), err.param.as_deref(), err.message.as_str()),
            (kind, param, "Read the docs."),
            "{code}"
        );
        assert_eq!(err.status, Some(400), "{code}");
        assert!(err.origin.as_ref().is_some_and(|o| o.is(D)), "{code}");
    }
}

#[test]
fn an_error_without_a_known_code_reads_its_kind_from_the_status() {
    let cases = [
        (400, ErrorKind::InvalidRequest),
        (409, ErrorKind::InvalidRequest),
        (422, ErrorKind::InvalidRequest),
        (401, ErrorKind::Authentication),
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (413, ErrorKind::RequestTooLarge),
        (429, ErrorKind::RateLimited),
        (500, ErrorKind::ProviderError),
        (502, ErrorKind::ProviderError),
        (503, ErrorKind::Overloaded),
        (504, ErrorKind::Timeout),
    ];
    for (status, kind) in cases {
        let err = decode(status, &json!({"title": "Only a title", "message": ""}));
        assert_eq!(err.kind, kind, "{status}");
        assert_eq!(err.message, "Only a title", "{status}");
    }
}

#[test]
fn odd_error_bodies_still_say_what_happened() {
    let err = decode(500, &json!({"request_id": "r1"}));
    assert_eq!(err.message, "Cartesia returned HTTP 500.");
    assert_eq!(err.raw, Some(json!({"request_id": "r1"})));

    let empty = Cartesia::decode_error(502, &HeaderList::new(), b"  ");
    assert_eq!(
        (empty.kind, empty.message.as_str(), empty.raw),
        (
            ErrorKind::ProviderError,
            "Cartesia returned HTTP 502.",
            None
        )
    );

    let array = Cartesia::decode_error(400, &HeaderList::new(), b"[1, 2]");
    assert_eq!(
        (array.kind, array.message.as_str()),
        (ErrorKind::InvalidRequest, "[1, 2]")
    );
}

#[test]
fn retry_after_is_read_and_written_back() {
    let headers = HeaderList::new().with("retry-after", "7");
    let err = Cartesia::decode_error(
        429,
        &headers,
        br#"{"error_code": "concurrency_limited", "title": "Too Many Requests", "message": "Slow down."}"#,
    );
    assert_eq!(err.retry_after, Some(Duration::from_secs(7)));
    let enc = Cartesia::encode_error(&err);
    assert_eq!(enc.status, 429);
    assert_eq!(enc.headers.get("retry-after"), Some("7"));
    assert_eq!(body_of(&enc)["error_code"], "concurrency_limited");

    let ours = KppError::new(ErrorKind::RateLimited, "Wait a moment.")
        .with_retry_after(Duration::from_millis(1500));
    let enc = Cartesia::encode_error(&ours);
    assert_eq!(enc.headers.get("retry-after"), Some("2"));
    assert_eq!(enc.headers.get("retry-after-ms"), None);
}

#[test]
fn errors_from_other_formats_get_cartesias_status_code_and_title() {
    let cases = [
        (
            KppError::new(ErrorKind::Authentication, "m"),
            401,
            "Unauthorized",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::Billing, "m"),
            402,
            "Bad Request",
            json!("quota_exceeded"),
        ),
        (
            KppError::new(ErrorKind::PermissionDenied, "m"),
            403,
            "Bad Request",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::NotFound, "m").with_param("voice"),
            404,
            "Not Found",
            json!("voice_not_found"),
        ),
        (
            KppError::new(ErrorKind::NotFound, "m").with_param("model"),
            404,
            "Not Found",
            json!("model_not_found"),
        ),
        (
            KppError::new(ErrorKind::NotFound, "m"),
            404,
            "Not Found",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::RequestTooLarge, "m"),
            413,
            "Bad Request",
            json!("file_too_large"),
        ),
        (
            KppError::new(ErrorKind::RateLimited, "m"),
            429,
            "Too Many Requests",
            json!("concurrency_limited"),
        ),
        (
            KppError::new(ErrorKind::ProviderError, "m"),
            500,
            "Unexpected error",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::Overloaded, "m"),
            503,
            "Unexpected error",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::Timeout, "m"),
            504,
            "Unexpected error",
            Value::Null,
        ),
        (
            KppError::new(ErrorKind::ContextTooLong, "m"),
            400,
            "Bad Request",
            Value::Null,
        ),
        (
            KppError::provider("m").with_status(502),
            502,
            "Unexpected error",
            Value::Null,
        ),
    ];
    for (err, status, title, code) in cases {
        let enc = Cartesia::encode_error(&err);
        assert_eq!(enc.status, status, "{err:?}");
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
        assert_eq!(
            body_of(&enc),
            json!({"error_code": code, "title": title, "message": "m", "request_id": null}),
            "{err:?}"
        );
    }
}

#[test]
fn cartesias_own_error_without_a_message_is_written_anew() {
    let own = KppError::new(ErrorKind::InvalidRequest, "Kaeso's words.")
        .with_origin(Origin::new(D))
        .with_status(418)
        .with_raw(json!({"detail": "teapot"}));
    let enc = Cartesia::encode_error(&own);
    assert_eq!(enc.status, 418);
    assert_eq!(
        body_of(&enc),
        json!({"error_code": null, "title": "Bad Request", "message": "Kaeso's words.", "request_id": null})
    );
    let without_status = KppError::new(ErrorKind::InvalidRequest, "m")
        .with_origin(Origin::new(D))
        .with_raw(json!({"message": "As Cartesia sent it."}));
    let enc = Cartesia::encode_error(&without_status);
    assert_eq!(
        (enc.status, body_of(&enc)),
        (400, json!({"message": "As Cartesia sent it."}))
    );
}

#[test]
fn requests_and_replies_that_are_not_audio_are_refused() {
    let rerank = Request::Rerank(RerankRequest::new(
        "rerank-1",
        "q",
        vec![RerankDocument::new("d")],
    ));
    let err = Cartesia::check(&rerank, &ctx(RequestKind::Rerank)).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    assert!(Cartesia::encode_request(&rerank, &mut ctx(RequestKind::Rerank)).is_err());
    let reply = Reply::CountTokens(CountTokensResponse::new(3));
    let enc = EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off);
    assert!(matches!(
        Cartesia::encode_response(&reply, &enc).unwrap_err().kind,
        ErrorKind::Unsupported { .. }
    ));
    assert_eq!(Cartesia::response_media_type(&reply, &enc), JSON_MEDIA_TYPE);
    let err = Cartesia::decode_response(200, &HeaderList::new(), b"{}", &ctx(RequestKind::Rerank))
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    let failed =
        Cartesia::decode_response(503, &HeaderList::new(), b"busy", &ctx(RequestKind::Speech))
            .unwrap_err();
    assert_eq!(
        (failed.kind, failed.message.as_str()),
        (ErrorKind::Overloaded, "busy")
    );
}

#[test]
fn speech_replies_carry_their_audio_and_its_media_type() {
    let wav = Reply::Speech(SpeechResponse::new(
        String::new(),
        Source::bytes("audio/wav", bytes::Bytes::from_static(b"RIFF")),
    ));
    let enc = EncodeCtx::new(RequestKind::Speech, StreamMode::Off);
    assert_eq!(Cartesia::encode_response(&wav, &enc).unwrap(), b"RIFF");
    assert_eq!(Cartesia::response_media_type(&wav, &enc), "audio/wav");
    let link = Reply::Speech(SpeechResponse::new(
        String::new(),
        Source::Url("https://example.com/speech.mp3".into()),
    ));
    assert_eq!(Cartesia::response_media_type(&link, &enc), "audio/mpeg");
    assert_eq!(
        Cartesia::encode_response(&link, &enc).unwrap_err().kind,
        ErrorKind::ProviderError
    );

    let mut asked = speech();
    asked.format = Some(AudioFormat::Wav);
    assert_eq!(
        Cartesia::stream_media_type(&EncodeCtx::for_request(Request::Speech(asked.clone()))),
        "audio/wav"
    );
    assert_eq!(
        Cartesia::stream_media_type(&EncodeCtx::new(RequestKind::Transcription, StreamMode::Off)),
        JSON_MEDIA_TYPE
    );
    let mut c = ctx(RequestKind::Speech);
    Cartesia::encode_request(&Request::Speech(asked), &mut c).unwrap();
    for headers in [
        HeaderList::new(),
        HeaderList::new().with("content-type", "application/octet-stream"),
    ] {
        let Reply::Speech(s) = Cartesia::decode_response(200, &headers, b"RIFF", &c).unwrap()
        else {
            panic!()
        };
        assert!(matches!(&s.audio, Source::Base64(b) if b.media_type == "audio/wav"));
    }
}

#[test]
fn the_audio_stream_ends_once_and_ignores_empty_pieces() {
    let mut c = ctx(RequestKind::Speech);
    Cartesia::encode_request(&Request::Speech(speech()), &mut c).unwrap();
    let mut d = Cartesia::stream_decoder(&c);
    assert!(d.feed(b"").is_empty());
    let first = d.feed(b"\x01");
    let second = d.feed(b"\x02");
    let (StreamEvent::Speech(a), StreamEvent::Speech(b)) = (&first[0], &second[0]) else {
        panic!("{first:?} {second:?}")
    };
    assert_eq!(
        (a.media_type.as_deref(), b.media_type.as_deref()),
        (Some("audio/mpeg"), None)
    );
    assert_eq!(d.finish().len(), 1);
    assert!(d.finish().is_empty());
    assert!(d.feed(b"\x03").is_empty());
}

#[test]
fn the_stream_encoder_writes_audio_or_a_whole_transcript() {
    let mut speech_out =
        Cartesia::stream_encoder(&EncodeCtx::new(RequestKind::Speech, StreamMode::Off));
    assert_eq!(
        speech_out
            .encode(&StreamEvent::Speech(SpeechEvent::audio("AQI=")))
            .unwrap(),
        [1, 2]
    );
    assert!(
        speech_out
            .encode(&StreamEvent::Speech(SpeechEvent::done(None)))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        speech_out
            .encode(&StreamEvent::Speech(SpeechEvent::audio("%%")))
            .unwrap_err()
            .kind,
        ErrorKind::ProviderError
    );
    let failed = KppError::new(ErrorKind::Overloaded, "Busy.");
    assert_eq!(
        speech_out
            .encode(&StreamEvent::Error(failed.clone()))
            .unwrap_err(),
        failed
    );
    assert!(
        speech_out
            .encode(&StreamEvent::Transcript(TranscriptEvent::delta("Hel")))
            .unwrap()
            .is_empty()
    );
    assert!(speech_out.finish().unwrap().is_empty());

    let mut text_out =
        Cartesia::stream_encoder(&EncodeCtx::new(RequestKind::Transcription, StreamMode::Off));
    assert!(
        text_out
            .encode(&StreamEvent::Speech(SpeechEvent::audio("AQI=")))
            .unwrap()
            .is_empty()
    );
    let done = TranscriptionResponse::new(String::new(), "Hello.");
    let written = text_out
        .encode(&StreamEvent::Transcript(TranscriptEvent::done(done)))
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&written).unwrap(),
        json!({"type": "transcript", "text": "Hello."})
    );
}

#[test]
fn speech_bodies_kaeso_cannot_read_are_refused() {
    let base = json!({"model_id": "sonic-3", "transcript": "Hi."});
    let with = |k: &str, v: Value| {
        let mut b = base.clone();
        b[k] = v;
        b
    };
    let Request::Speech(r) = speech_request(with("voice", json!({"id": VOICE}))).unwrap() else {
        panic!()
    };
    assert_eq!(r.voice, Some(Voice::Id(VOICE.into())));
    let cases = [
        with("voice", json!({"mode": "id", "id": VOICE})),
        with("voice", json!(5)),
        with("output_format", json!("mp3")),
        with("output_format", json!({"container": "ogg"})),
        with(
            "output_format",
            json!({"container": "mp3", "sample_rate": 44100, "bit_rate": 128000, "padding": 1}),
        ),
        with(
            "output_format",
            json!({"container": "wav", "encoding": "pcm_f32le"}),
        ),
        with("generation_config", json!({"speed": "fast"})),
    ];
    let params = [
        "voice",
        "voice",
        "output_format",
        "output_format",
        "output_format",
        "output_format",
        "speed",
    ];
    for (body, param) in cases.into_iter().zip(params) {
        let err = speech_request(body.clone()).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{body}: {err:?}");
    }
}

/// A change to a speech request.
type SpeechEdit = fn(&mut SpeechRequest);

#[test]
fn speech_cartesia_cannot_make_is_refused_before_sending() {
    let cases: Vec<(SpeechEdit, &str)> = vec![
        (|r| r.operation = SpeechOperation::SoundEffect, "model"),
        (
            |r| {
                r.audio = Some(AudioInput::new(Source::bytes(
                    "audio/wav",
                    bytes::Bytes::from_static(b"RIFF"),
                )));
            },
            "input",
        ),
        (|r| r.input = Some(String::new()), "input"),
        (|r| r.input = None, "input"),
        (|r| r.seed = Some(7), "seed"),
        (|r| r.timestamps = true, "timestamps"),
        (|r| r.duration = Some(2.0), "duration"),
        (|r| r.bitrate = Some(100), "bitrate"),
        (|r| r.speed = Some(0.5), "speed"),
    ];
    for (edit, param) in cases {
        let mut r = speech();
        edit(&mut r);
        let err = Cartesia::check(&Request::Speech(r), &ctx(RequestKind::Speech)).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
    }
    let mut mp3 = speech();
    mp3.bitrate = Some(64);
    mp3.sample_rate = Some(22050);
    mp3.language = Some("de".into());
    let enc =
        Cartesia::encode_request(&Request::Speech(mp3), &mut ctx(RequestKind::Speech)).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        (&body["output_format"], &body["language"]),
        (
            &json!({"container": "mp3", "sample_rate": 22050, "bit_rate": 64000}),
            &json!("de")
        )
    );
}

/// A change to a transcription request.
type TranscriptionEdit = fn(&mut TranscriptionRequest);

#[test]
fn transcriptions_cartesia_cannot_make_are_refused_before_sending() {
    let cases: Vec<(TranscriptionEdit, &str)> = vec![
        (|r| r.stream = StreamMode::on(false), "stream"),
        (|r| r.diarize = Some(true), "diarize"),
        (|r| r.speakers = Some(2), "num_speakers"),
        (|r| r.language = Some(String::new()), "language"),
    ];
    for (edit, param) in cases {
        let mut r = transcription();
        edit(&mut r);
        let req = Request::Transcription(r);
        let err = Cartesia::check(&req, &ctx(RequestKind::Transcription)).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
        assert!(Cartesia::encode_request(&req, &mut ctx(RequestKind::Transcription)).is_err());
    }
    let mut unnamed = transcription();
    unnamed.audio.filename = None;
    let enc = Cartesia::encode_request(
        &Request::Transcription(unnamed),
        &mut ctx(RequestKind::Transcription),
    )
    .unwrap();
    assert!(
        enc.body
            .windows(b"filename=\"audio\"".len())
            .any(|w| w == b"filename=\"audio\""),
        "{}",
        String::from_utf8_lossy(&enc.body)
    );
}

#[test]
fn odd_transcripts_are_provider_errors_and_others_are_written_in_cartesias_shape() {
    let c = ctx(RequestKind::Transcription);
    for body in [
        br#"{"type": "transcript", "language": "en"}"#.as_slice(),
        br#"{"text": "Hi.", "words": [1]}"#,
        b"not json",
    ] {
        let err = Cartesia::decode_response(200, &HeaderList::new(), body, &c).unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::ProviderError,
            "{}",
            String::from_utf8_lossy(body)
        );
    }
    let mut elsewhere = TranscriptionResponse::new("whisper-1", "Hello world.");
    elsewhere.usage_seconds = Some(1.5);
    elsewhere.language = Some("en".into());
    elsewhere.words = vec![
        TranscriptWord::new("Hello", 0.0, 0.5),
        TranscriptWord::new("world.", 0.5, 1.5),
    ];
    let written = Cartesia::encode_response(
        &Reply::Transcription(elsewhere),
        &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&written).unwrap(),
        json!({
            "type": "transcript",
            "text": "Hello world.",
            "language": "en",
            "duration": 1.5,
            "words": [
                {"word": "Hello", "start": 0.0, "end": 0.5},
                {"word": "world.", "start": 0.5, "end": 1.5},
            ],
        })
    );
}

#[test]
fn the_voice_list_answers_the_model_check_and_names_no_model() {
    let req = Cartesia::encode_model_request("sonic-3");
    assert_eq!((req.method, req.path.as_str()), ("GET", VOICES_PATH));
    assert_eq!(req.headers.get("cartesia-version"), Some(VERSION));
    for body in [b"<html>".as_slice(), br#"{"voices": []}"#] {
        assert_eq!(
            Cartesia::decode_models(200, &HeaderList::new(), body)
                .unwrap_err()
                .kind,
            ErrorKind::ProviderError
        );
    }
    let err = Cartesia::decode_model(200, &HeaderList::new(), br#"{"data": []}"#).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    let err = Cartesia::decode_model(
        401,
        &HeaderList::new(),
        br#"{"title": "Unauthorized", "message": "Bad key."}"#,
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Authentication);
}
