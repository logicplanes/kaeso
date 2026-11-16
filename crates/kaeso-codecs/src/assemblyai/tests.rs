use kaeso_kpp::{AudioInput, ErrorKind, StreamMode};
use serde_json::json;

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "assemblyai";

fn ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Transcription, StreamMode::Off)
}

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

fn transcription(format: Option<TranscriptFormat>) -> TranscriptionRequest {
    let mut r = TranscriptionRequest::new(
        "universal-3-6-pro",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::bytes(
            "audio/wav",
            bytes::Bytes::from_static(b"RIFF"),
        )),
    );
    r.format = format;
    r
}

fn sent(r: TranscriptionRequest) -> EncodedRequest {
    let req = Request::Transcription(r);
    AssemblyAi::check(&req, &ctx()).unwrap_or_else(|e| panic!("{e:?}"));
    AssemblyAi::encode_request(&req, &mut ctx()).unwrap()
}

/// The parts of an upload: name, content type and body as text (a file's as its length).
fn parts(enc: &EncodedRequest) -> Vec<(String, Option<String>, String)> {
    let ct = enc.headers.get("content-type").unwrap();
    let boundary = crate::multipart::form_boundary(ct).unwrap();
    let limits = crate::multipart::FormLimits::new(16, 1 << 20);
    crate::multipart::read_form(&enc.body, &boundary, limits)
        .unwrap()
        .into_iter()
        .map(|p| {
            let v = if p.filename.is_some() {
                format!("<{} bytes>", p.body.len())
            } else {
                p.text().unwrap_or_default().to_owned()
            };
            (p.name.clone(), p.content_type.clone(), v)
        })
        .collect()
}

#[test]
fn a_transcription_is_one_upload_with_the_model_in_its_header() {
    let mut r = transcription(Some(TranscriptFormat::VerboseJson));
    r.language = Some("de-DE".into());
    r.prompt = Some("A call about an order.".into());
    let enc = sent(r);
    assert_eq!((enc.method, enc.path.as_str()), ("POST", TRANSCRIBE_PATH));
    assert_eq!(enc.headers.get(MODEL_HEADER), Some("universal-3-6-pro"));
    let p = parts(&enc);
    assert_eq!(p[0].0, "audio");
    assert_eq!(p[0].1.as_deref(), Some("audio/wav"));
    assert_eq!(p[0].2, "<4 bytes>");
    assert_eq!(p[1].0, "config");
    assert_eq!(p[1].1.as_deref(), Some("application/json"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&p[1].2).unwrap(),
        json!({"language_codes": ["de"], "prompt": "A call about an order.", "timestamps": true})
    );
    // Without a language, prompt or timing the upload carries the audio alone.
    let plain = sent(transcription(Some(TranscriptFormat::Text)));
    assert_eq!(parts(&plain).len(), 1);
    let mut words = transcription(None);
    words.timestamps = vec![TimestampGranularity::Word];
    assert!(parts(&sent(words))[1].2.contains("\"timestamps\":true"));
    assert!(
        parts(&sent(transcription(Some(TranscriptFormat::Srt))))[1]
            .2
            .contains("\"timestamps\":true")
    );
}

/// A change to a transcription request.
type Edit = fn(&mut TranscriptionRequest);

#[test]
fn fields_the_sync_api_lacks_are_refused_before_sending() {
    let cases: Vec<(Edit, &str)> = vec![
        (|r| r.language = Some("pl".into()), "language"),
        (
            |r| r.format = Some(TranscriptFormat::DiarizedJson),
            "response_format",
        ),
        (|r| r.temperature = Some(0.2), "temperature"),
        (|r| r.diarize = Some(true), "diarize"),
        (
            |r| r.timestamps = vec![TimestampGranularity::Segment],
            "timestamp_granularities",
        ),
        (|r| r.operation = TranscriptionOperation::Translate, "model"),
        (|r| r.prompt = Some("x".repeat(6001)), "prompt"),
        (
            |r| r.audio = AudioInput::new(Source::Url("https://example.com/a.wav".into())),
            "file",
        ),
    ];
    for (edit, param) in cases {
        let mut r = transcription(None);
        edit(&mut r);
        let err = AssemblyAi::check(&Request::Transcription(r), &ctx()).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
    }
    assert!(
        AssemblyAi::decode_request(RequestKind::Transcription, b"x", &HeaderList::new()).is_err()
    );
}

#[test]
fn transcripts_decode_with_their_length_and_timed_words_and_write_back() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let reply = AssemblyAi::decode_response(200, &HeaderList::new(), &body, &ctx())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back = AssemblyAi::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &back);
        let Reply::Transcription(t) = reply else {
            panic!()
        };
        assert_eq!(t.usage_seconds, t.duration, "{name}");
        match name.as_str() {
            "timestamps-built" => {
                assert_eq!(t.words.len(), 2);
                assert_eq!(
                    (t.words[1].text.as_str(), t.words[1].start, t.words[1].end),
                    ("world.", 0.7, 1.18)
                );
                assert_eq!(t.duration, Some(1.5));
            }
            // Words without times stay as AssemblyAI wrote them.
            _ => {
                assert!(t.words.is_empty(), "{name}");
                assert!(t.extras.fields.contains_key("words"), "{name}");
            }
        }
    }
    assert_eq!(ms_value(0.7), json!(700));
    assert_eq!(ms_value(0.0005), json!(1));
}

#[test]
fn errors_decode_with_their_kind_and_write_back() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = AssemblyAi::decode_error(status, &HeaderList::new(), &body);
        let kind = match status {
            404 => ErrorKind::Authentication,
            413 => ErrorKind::RequestTooLarge,
            429 => ErrorKind::RateLimited,
            _ => ErrorKind::Timeout,
        };
        assert_eq!(err.kind, kind, "{name}");
        assert_eq!(err.param.as_deref(), (status == 413).then_some("file"));
        let enc = AssemblyAi::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    let text = &fixtures(DIR, "plain", "txt")[0].1;
    let err = AssemblyAi::decode_error(404, &HeaderList::new(), text);
    assert_eq!(
        (err.kind, err.message.as_str()),
        (ErrorKind::NotFound, "Not found")
    );
    let ours = AssemblyAi::encode_error(&KppError::invalid_request("No.").with_param("language"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&ours.body).unwrap(),
        json!({"status": 400, "title": "Bad Request", "detail": "No."})
    );
    let _ = fixture("errors", "recorded-invalid-key-404");
}

#[test]
fn the_model_check_says_a_key_cant_be_checked() {
    let err = AssemblyAi::encode_models_request(&ModelsQuery::default()).unwrap_err();
    assert!(err.message.contains("can't check a key"), "{}", err.message);
    assert!(AssemblyAi::encode_model_request_with("universal-3-6-pro", &ctx()).is_err());
}
