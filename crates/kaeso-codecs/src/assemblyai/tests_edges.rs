use std::time::Duration;

use kaeso_kpp::{
    AudioInput, CountTokensResponse, ErrorKind, KppError, RerankDocument, RerankRequest,
    StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::*;

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn decode(status: u16, body: &Value) -> KppError {
    AssemblyAi::decode_error(
        status,
        &HeaderList::new(),
        &serde_json::to_vec(body).unwrap(),
    )
}

fn body_of(enc: &EncodedError) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn transcription() -> TranscriptionRequest {
    TranscriptionRequest::new(
        "universal-3-6-pro",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::bytes(
            "audio/wav",
            bytes::Bytes::from_static(b"RIFF"),
        )),
    )
}

fn transcript(body: &Value) -> Result<TranscriptionResponse, KppError> {
    match AssemblyAi::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(body).unwrap(),
        &ctx(RequestKind::Transcription),
    )? {
        Reply::Transcription(t) => Ok(t),
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_status_reads_as_its_kind_with_the_detail_as_the_message() {
    let cases = [
        (400, ErrorKind::InvalidRequest),
        (415, ErrorKind::InvalidRequest),
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
        let err = decode(
            status,
            &json!({"status": status, "title": "Some Title", "detail": "What went wrong."}),
        );
        assert_eq!(
            (err.kind, err.message.as_str(), err.status, err.param),
            (kind, "What went wrong.", Some(status), None),
            "{status}"
        );
    }
}

#[test]
fn audio_problems_name_the_file_and_a_refused_key_is_an_authentication_error() {
    for title in [
        "Bad Audio",
        "Audio Too Short",
        "Audio Too Large",
        "Unsupported Media Type",
    ] {
        let err = decode(400, &json!({"status": 400, "title": title, "detail": "d"}));
        assert_eq!(err.param.as_deref(), Some("file"), "{title}");
    }
    let refused = decode(
        404,
        &json!({"status": 404, "title": "Not Found", "detail": "Invalid API key"}),
    );
    assert_eq!(refused.kind, ErrorKind::Authentication);
}

#[test]
fn odd_error_bodies_still_say_what_happened() {
    let title_only = decode(503, &json!({"title": "Service Unavailable", "detail": ""}));
    assert_eq!(title_only.message, "Service Unavailable");
    let neither = decode(500, &json!({"status": 500}));
    assert_eq!(neither.message, "AssemblyAI returned HTTP 500.");
    assert_eq!(neither.raw, Some(json!({"status": 500})));
    let empty = AssemblyAi::decode_error(502, &HeaderList::new(), b"");
    assert_eq!(
        (empty.kind, empty.message.as_str(), empty.raw),
        (
            ErrorKind::ProviderError,
            "AssemblyAI returned HTTP 502.",
            None
        )
    );
    let array = AssemblyAi::decode_error(400, &HeaderList::new(), b"[\"no\"]");
    assert_eq!(array.message, "[\"no\"]");
}

#[test]
fn retry_after_is_read_and_written_back() {
    let headers = HeaderList::new().with("retry-after-ms", "2500");
    let err = AssemblyAi::decode_error(
        429,
        &headers,
        br#"{"status": 429, "title": "Too Many Requests", "detail": "Slow down."}"#,
    );
    assert_eq!(err.retry_after, Some(Duration::from_millis(2500)));
    let enc = AssemblyAi::encode_error(&err);
    assert_eq!(enc.status, 429);
    assert_eq!(enc.headers.get("retry-after"), Some("3"));
    assert_eq!(
        enc.headers.get("content-type"),
        Some("application/problem+json")
    );
}

#[test]
fn errors_from_other_formats_become_problem_details() {
    let cases = [
        (
            KppError::new(ErrorKind::Authentication, "m"),
            401,
            "Unauthorized",
        ),
        (
            KppError::new(ErrorKind::RequestTooLarge, "m"),
            413,
            "Audio Too Large",
        ),
        (
            KppError::new(ErrorKind::RateLimited, "m"),
            429,
            "Too Many Requests",
        ),
        (
            KppError::new(ErrorKind::Overloaded, "m"),
            503,
            "Service Unavailable",
        ),
        (
            KppError::new(ErrorKind::Timeout, "m"),
            504,
            "Inference Timeout",
        ),
        (
            KppError::new(ErrorKind::ProviderError, "m"),
            500,
            "Inference Error",
        ),
        (KppError::new(ErrorKind::NotFound, "m"), 400, "Bad Request"),
        (KppError::new(ErrorKind::Billing, "m"), 400, "Bad Request"),
        (
            KppError::new(ErrorKind::InvalidRequest, "m").with_status(415),
            415,
            "Unsupported Media Type",
        ),
        (
            KppError::provider("m").with_status(502),
            502,
            "Inference Error",
        ),
    ];
    for (err, status, title) in cases {
        let enc = AssemblyAi::encode_error(&err);
        assert_eq!(
            (enc.status, body_of(&enc)),
            (
                status,
                json!({"status": status, "title": title, "detail": "m"})
            ),
            "{err:?}"
        );
    }
    let own_without_problem = KppError::new(ErrorKind::InvalidRequest, "Kaeso's words.")
        .with_origin(Origin::new(D))
        .with_raw(json!({"error": "x"}));
    assert_eq!(
        body_of(&AssemblyAi::encode_error(&own_without_problem)),
        json!({"status": 400, "title": "Bad Request", "detail": "Kaeso's words."})
    );
    let own = KppError::new(ErrorKind::InvalidRequest, "m")
        .with_origin(Origin::new(D))
        .with_raw(json!({"detail": "As sent."}));
    let enc = AssemblyAi::encode_error(&own);
    assert_eq!(
        (enc.status, body_of(&enc)),
        (400, json!({"detail": "As sent."}))
    );
}

#[test]
fn requests_and_replies_that_are_not_transcriptions_are_refused() {
    let rerank = Request::Rerank(RerankRequest::new(
        "rerank-1",
        "q",
        vec![RerankDocument::new("d")],
    ));
    for err in [
        AssemblyAi::check(&rerank, &ctx(RequestKind::Rerank)).unwrap_err(),
        AssemblyAi::encode_request(&rerank, &mut ctx(RequestKind::Rerank)).unwrap_err(),
        AssemblyAi::encode_response(
            &Reply::CountTokens(CountTokensResponse::new(3)),
            &EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off),
        )
        .unwrap_err(),
        AssemblyAi::decode_response(200, &HeaderList::new(), b"{}", &ctx(RequestKind::Rerank))
            .unwrap_err(),
    ] {
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    }
    let failed = AssemblyAi::decode_response(
        504,
        &HeaderList::new(),
        br#"{"status": 504, "title": "Inference Timeout", "detail": "Too slow."}"#,
        &ctx(RequestKind::Transcription),
    )
    .unwrap_err();
    assert_eq!(failed.kind, ErrorKind::Timeout);
}

#[test]
fn nothing_is_streamed_and_a_stream_says_so() {
    let mut out =
        AssemblyAi::stream_encoder(&EncodeCtx::new(RequestKind::Transcription, StreamMode::Off));
    assert!(matches!(
        out.encode(&StreamEvent::Error(KppError::provider("x")))
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported { .. }
    ));
    assert!(out.finish().unwrap().is_empty());
    let mut input = AssemblyAi::stream_decoder(&ctx(RequestKind::Transcription));
    let events = input.feed(b"{}");
    assert!(
        matches!(events.as_slice(), [StreamEvent::Error(e)] if matches!(e.kind, ErrorKind::Unsupported { .. })),
        "{events:?}"
    );
    assert!(input.finish().is_empty());
}

/// A change to a transcription request.
type Edit = fn(&mut TranscriptionRequest);

#[test]
fn streams_and_speaker_counts_are_refused_and_a_language_is_steered_by_its_code() {
    let cases: Vec<(Edit, &str)> = vec![
        (|r| r.stream = StreamMode::on(false), "stream"),
        (|r| r.speakers = Some(2), "num_speakers"),
    ];
    for (edit, param) in cases {
        let mut r = transcription();
        edit(&mut r);
        let req = Request::Transcription(r);
        let err = AssemblyAi::check(&req, &ctx(RequestKind::Transcription)).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
        assert!(AssemblyAi::encode_request(&req, &mut ctx(RequestKind::Transcription)).is_err());
    }
    let mut r = transcription();
    r.language = Some("PT_br".into());
    r.prompt = Some("Kaeso".into());
    r.audio.filename = Some("call.wav".into());
    let enc = AssemblyAi::encode_request(
        &Request::Transcription(r),
        &mut ctx(RequestKind::Transcription),
    )
    .unwrap();
    let body = String::from_utf8_lossy(&enc.body);
    assert!(body.contains("filename=\"call.wav\""), "{body}");
    assert!(body.contains(r#""language_codes":["pt"]"#), "{body}");
    assert!(body.contains(r#""prompt":"Kaeso""#), "{body}");
    assert!(!body.contains("timestamps"), "{body}");
}

#[test]
fn odd_transcripts_keep_what_kaeso_cannot_read_and_a_missing_text_is_an_error() {
    let err = transcript(&json!({"words": []})).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
    let t =
        transcript(&json!({"text": "Hi.", "audio_duration_ms": "long", "words": "none"})).unwrap();
    assert_eq!((t.duration, t.usage_seconds), (None, None));
    assert!(t.words.is_empty());
    assert!(t.extras.fields.contains_key("audio_duration_ms"));
    assert!(t.extras.fields.contains_key("words"));
    let mut elsewhere = TranscriptionResponse::new("whisper-1", "Hello.");
    elsewhere.usage_seconds = Some(2.0);
    let written = AssemblyAi::encode_response(
        &Reply::Transcription(elsewhere),
        &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&written).unwrap(),
        json!({"text": "Hello.", "audio_duration_ms": 2000})
    );
}

#[test]
fn milliseconds_that_are_not_whole_numbers_stay_as_they_are() {
    assert_eq!(ms_value(f64::NAN), Value::Null);
    assert_eq!(ms_value(1.0e13), json!(1.0e16));
}

#[test]
fn the_model_request_is_the_transcription_path() {
    let req = AssemblyAi::encode_model_request("universal-3-6-pro");
    assert_eq!((req.method, req.path.as_str()), ("GET", TRANSCRIBE_PATH));
    for err in [
        AssemblyAi::decode_models(401, &HeaderList::new(), b"Unauthorized").unwrap_err(),
        AssemblyAi::decode_model(401, &HeaderList::new(), b"Unauthorized").unwrap_err(),
    ] {
        assert_eq!(err.kind, ErrorKind::Authentication);
    }
}
