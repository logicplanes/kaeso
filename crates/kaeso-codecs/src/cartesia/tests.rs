use kaeso_kpp::{
    AudioFormat, AudioInput, ErrorKind, Reply, Request, RequestKind, Source, SpeechOperation,
    SpeechRequest, StreamEvent, StreamMode, TimestampGranularity, TranscriptFormat,
    TranscriptionOperation, TranscriptionRequest, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "cartesia";
const VOICE: &str = "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4";

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn sent(req: &Request, kind: RequestKind) -> EncodedRequest {
    Cartesia::check(req, &ctx(kind)).unwrap_or_else(|e| panic!("{e:?}"));
    Cartesia::encode_request(req, &mut ctx(kind)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn speech(model: &str) -> SpeechRequest {
    let mut r = SpeechRequest::new(model, SpeechOperation::Speech);
    r.input = Some("Hello there.".into());
    r.voice = Some(Voice::Id(VOICE.into()));
    r
}

#[test]
fn every_speech_request_fixture_round_trips_with_the_version_header() {
    for (name, body) in fixtures(DIR, "requests/speech", "json") {
        let req = Cartesia::decode_request(RequestKind::Speech, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let enc = sent(&req, RequestKind::Speech);
        assert_eq!(
            (enc.method, enc.path.as_str()),
            ("POST", TTS_PATH),
            "{name}"
        );
        assert_eq!(enc.headers.get("cartesia-version"), Some(VERSION), "{name}");
        assert_json_eq(&name, &body, &enc.body);
        let Request::Speech(r) = req else { panic!() };
        match name.as_str() {
            "wav-generation-config-built" => {
                assert_eq!((r.format, r.speed), (Some(AudioFormat::Wav), Some(1.2)));
                assert!(r.extras.fields.contains_key("generation_config.emotion"));
            }
            "raw-pcm-built" => assert_eq!(r.format, Some(AudioFormat::Pcm16)),
            _ => assert_eq!((r.format, r.bitrate), (Some(AudioFormat::Mp3), Some(128))),
        }
    }
}

#[test]
fn an_openai_speech_request_asks_for_its_format_and_streams_the_audio_back() {
    let mut r = speech("sonic-3.6");
    r.format = Some(AudioFormat::Pcm16);
    r.speed = Some(1.5);
    let mut c = ctx(RequestKind::Speech);
    let enc = Cartesia::encode_request(&Request::Speech(r), &mut c).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap(),
        json!({
            "model_id": "sonic-3.6",
            "transcript": "Hello there.",
            "voice": VOICE,
            "output_format": {"container": "raw", "encoding": "pcm_s16le", "sample_rate": 24000},
            "generation_config": {"speed": 1.5},
        })
    );
    assert!(Cartesia::reads_reply_as_stream(&c));
    let mut d = Cartesia::stream_decoder(&c);
    let mut events = d.feed(b"\x01\x02");
    events.extend(d.finish());
    let StreamEvent::Speech(first) = &events[0] else {
        panic!("{events:?}")
    };
    assert_eq!(first.media_type.as_deref(), Some("audio/l16;rate=24000"));
    let enc = Cartesia::encode_request(
        &Request::Speech(speech("sonic-3")),
        &mut ctx(RequestKind::Speech),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap()["output_format"],
        json!({"container": "mp3", "sample_rate": 44100, "bit_rate": 128000})
    );
    let reply = Cartesia::decode_response(
        200,
        &HeaderList::new().with("content-type", "audio/mpeg"),
        b"ID3",
        &ctx(RequestKind::Speech),
    )
    .unwrap();
    let Reply::Speech(s) = reply else { panic!() };
    assert!(matches!(&s.audio, Source::Base64(b) if b.media_type == "audio/mpeg"));
}

/// A change to a speech request.
type SpeechEdit = fn(&mut SpeechRequest);

#[test]
fn speech_fields_cartesia_lacks_are_refused_before_sending() {
    let cases: Vec<(SpeechEdit, &str)> = vec![
        (|r| r.format = Some(AudioFormat::Opus), "response_format"),
        (|r| r.format = Some(AudioFormat::Flac), "response_format"),
        (|r| r.speed = Some(2.0), "speed"),
        (|r| r.instructions = Some("calm".into()), "instructions"),
        (|r| r.voice = None, "voice"),
        (|r| r.sample_rate = Some(11025), "sample_rate"),
        (
            |r| {
                r.format = Some(AudioFormat::Wav);
                r.bitrate = Some(128);
            },
            "bitrate",
        ),
    ];
    for (edit, param) in cases {
        let mut r = speech("sonic-3.6");
        edit(&mut r);
        let err = Cartesia::check(&Request::Speech(r), &ctx(RequestKind::Speech)).unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
    }
}

fn transcription(format: Option<TranscriptFormat>) -> TranscriptionRequest {
    let mut r = TranscriptionRequest::new(
        "ink-whisper",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::bytes(
            "audio/wav",
            bytes::Bytes::from_static(b"RIFF"),
        )),
    );
    r.language = Some("en".into());
    r.format = format;
    r
}

fn form_fields(enc: &EncodedRequest) -> Vec<(String, String)> {
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
            (p.name.clone(), v)
        })
        .collect()
}

#[test]
fn a_transcription_is_an_stt_upload_that_asks_for_words_when_the_app_wants_them() {
    let enc = sent(
        &Request::Transcription(transcription(Some(TranscriptFormat::VerboseJson))),
        RequestKind::Transcription,
    );
    assert_eq!((enc.method, enc.path.as_str()), ("POST", STT_PATH));
    assert_eq!(enc.headers.get("cartesia-version"), Some(VERSION));
    assert_eq!(
        form_fields(&enc),
        [
            ("model".to_owned(), "ink-whisper".to_owned()),
            ("language".to_owned(), "en".to_owned()),
            ("timestamp_granularities[]".to_owned(), "word".to_owned()),
            ("file".to_owned(), "<4 bytes>".to_owned()),
        ]
    );
    let mut words = transcription(None);
    words.timestamps = vec![TimestampGranularity::Word];
    assert!(
        form_fields(&sent(
            &Request::Transcription(words),
            RequestKind::Transcription
        ))
        .iter()
        .any(|(k, _)| k == "timestamp_granularities[]")
    );
    let plain = sent(
        &Request::Transcription(transcription(Some(TranscriptFormat::Text))),
        RequestKind::Transcription,
    );
    assert_eq!(form_fields(&plain).len(), 3);
    let srt = sent(
        &Request::Transcription(transcription(Some(TranscriptFormat::Srt))),
        RequestKind::Transcription,
    );
    assert_eq!(form_fields(&srt).len(), 4);
}

/// A change to a transcription request.
type TranscriptionEdit = fn(&mut TranscriptionRequest);

#[test]
fn transcription_fields_cartesia_lacks_are_refused_before_sending() {
    let cases: Vec<(TranscriptionEdit, &str)> = vec![
        (|r| r.language = None, "language"),
        (
            |r| r.format = Some(TranscriptFormat::DiarizedJson),
            "response_format",
        ),
        (|r| r.prompt = Some("Kaeso".into()), "prompt"),
        (|r| r.temperature = Some(0.2), "temperature"),
        (
            |r| r.timestamps = vec![TimestampGranularity::Segment],
            "timestamp_granularities",
        ),
        (|r| r.operation = TranscriptionOperation::Translate, "model"),
        (
            |r| r.audio = AudioInput::new(Source::Url("https://example.com/a.wav".into())),
            "file",
        ),
    ];
    for (edit, param) in cases {
        let mut r = transcription(None);
        edit(&mut r);
        let err = Cartesia::check(&Request::Transcription(r), &ctx(RequestKind::Transcription))
            .unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{param}: {err:?}");
    }
    assert!(
        Cartesia::decode_request(RequestKind::Transcription, b"x", &HeaderList::new()).is_err()
    );
}

#[test]
fn transcripts_decode_with_their_words_and_length_and_write_back() {
    for (name, body) in fixtures(DIR, "responses/stt", "json") {
        let reply = Cartesia::decode_response(
            200,
            &HeaderList::new(),
            &body,
            &ctx(RequestKind::Transcription),
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back = Cartesia::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Transcription, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &back);
        let Reply::Transcription(t) = reply else {
            panic!()
        };
        if name == "words-built" {
            assert_eq!(
                (t.words.len(), t.words[0].text.as_str(), t.usage_seconds),
                (2, " Hello", Some(1.2))
            );
            assert_eq!(t.language.as_deref(), Some("en"));
        }
    }
}

#[test]
fn errors_decode_with_their_kind_and_write_back() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = Cartesia::decode_error(status, &HeaderList::new(), &body);
        let (kind, param) = match status {
            404 => (ErrorKind::NotFound, Some("voice")),
            429 => (ErrorKind::RateLimited, None),
            _ => (ErrorKind::Authentication, None),
        };
        assert_eq!(
            (err.kind.clone(), err.param.as_deref()),
            (kind, param),
            "{name}"
        );
        let enc = Cartesia::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    let text = &fixtures(DIR, "plain", "txt")[0].1;
    let err = Cartesia::decode_error(400, &HeaderList::new(), text);
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(err.message.contains("Cartesia-Version"), "{}", err.message);
    let ours = Cartesia::encode_error(&KppError::invalid_request("No.").with_param("speed"));
    assert_eq!(
        serde_json::from_slice::<Value>(&ours.body).unwrap(),
        json!({"error_code": null, "title": "Bad Request", "message": "No.", "request_id": null})
    );
}

#[test]
fn the_model_check_reads_one_voice_with_the_version_header() {
    let req = Cartesia::encode_models_request(&ModelsQuery::default()).unwrap();
    assert_eq!((req.method, req.path.as_str()), ("GET", VOICES_PATH));
    assert_eq!(req.headers.get("cartesia-version"), Some(VERSION));
    let list = Cartesia::decode_models(
        200,
        &HeaderList::new(),
        br#"{"data": [], "has_more": false, "next_page": null}"#,
    )
    .unwrap();
    assert!(list.models.is_empty());
    let err = Cartesia::decode_models(
        401,
        &HeaderList::new(),
        &fixtures(DIR, "errors", "json")
            .into_iter()
            .find(|(n, _)| n == "recorded-voices-not-logged-in-401")
            .unwrap()
            .1,
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Authentication);
}
