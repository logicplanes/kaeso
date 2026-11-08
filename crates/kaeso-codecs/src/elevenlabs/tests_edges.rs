use kaeso_kpp::{
    AudioInput, ErrorKind, KppError, Origin, Reply, Request, Source, SpeechEvent, SpeechOperation,
    SpeechRequest, SpeechResponse, StreamEvent, TranscriptFormat, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::multipart::{FormField, FormWriter};

const VOICE: &str = "hpp4J3VqNfWAUOO0d1Us";

fn call(path: &str, query: &str) -> ElevenLabsCall {
    ElevenLabsCall::from_path(path, query).unwrap()
}

fn json_call(path: &str, query: &str, body: &str) -> Result<Request, KppError> {
    crate::dispatch::decode_elevenlabs_request(
        &call(path, query),
        body.as_bytes(),
        &HeaderList::new().with("content-type", "application/json"),
    )
}

fn upload(path: &str, fields: &[FormField<'_>]) -> Result<Request, KppError> {
    let w = FormWriter::write(fields);
    let headers = HeaderList::new().with("content-type", w.content_type());
    crate::dispatch::decode_elevenlabs_request(&call(path, ""), &w.into_body(), &headers)
}

fn param(r: Result<Request, KppError>) -> Option<String> {
    r.unwrap_err().param
}

fn encode(req: &Request) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let mut ctx = EgressCtx::new(req).with_provider("elevenlabs");
    let enc = ElevenLabs::encode_request(req, &mut ctx)?;
    Ok((enc, ctx))
}

fn speech_with(f: impl FnOnce(&mut SpeechRequest)) -> Request {
    let mut r = SpeechRequest::new("eleven_flash_v2_5", SpeechOperation::Speech)
        .with_input("Hi")
        .with_voice(Voice::Id(VOICE.into()));
    f(&mut r);
    Request::Speech(r)
}

fn transcription_with(f: impl FnOnce(&mut TranscriptionRequest)) -> Request {
    let mut r = TranscriptionRequest::new(
        "scribe_v2",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::base64("audio/mpeg", "SUQz")).with_filename("a.mp3"),
    );
    f(&mut r);
    Request::Transcription(r)
}

fn refused(req: &Request) -> String {
    let err = ElevenLabs::check(req, &EgressCtx::new(req)).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    err.param.unwrap_or_default()
}

#[test]
fn json_bodies_with_wrong_types_are_refused_with_their_field() {
    let tts = format!("/v1/text-to-speech/{VOICE}");
    assert_eq!(
        param(json_call(
            &tts,
            "output_format=flac_44100",
            r#"{"text":"Hi"}"#
        ))
        .as_deref(),
        Some("output_format")
    );
    assert_eq!(
        param(json_call(&tts, "", r#"{"text":1}"#)).as_deref(),
        Some("text")
    );
    assert_eq!(
        param(json_call(&tts, "", r#"{"text":"Hi","seed":"7"}"#)).as_deref(),
        Some("seed")
    );
    let dialogue = "/v1/text-to-dialogue";
    assert_eq!(
        param(json_call(dialogue, "", r#"{"inputs":"Hi"}"#)).as_deref(),
        Some("inputs")
    );
    assert_eq!(
        param(json_call(dialogue, "", r#"{"inputs":[1]}"#)).as_deref(),
        Some("inputs[0]")
    );
    let sfx = "/v1/sound-generation";
    assert_eq!(
        param(json_call(
            sfx,
            "",
            r#"{"text":"Rain","duration_seconds":"5"}"#
        ))
        .as_deref(),
        Some("duration_seconds")
    );
}

#[test]
fn dialogue_line_fields_null_durations_and_bare_query_flags_round_trip() {
    let body = format!(
        r#"{{"inputs":[{{"text":"Hi","voice_id":"{VOICE}","pause":true}}],"model_id":"eleven_v3"}}"#
    );
    let req = json_call("/v1/text-to-dialogue", "enable_logging", &body).unwrap();
    let (enc, _) = encode(&req).unwrap();
    assert!(enc.path.ends_with("?enable_logging="), "{}", enc.path);
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(sent["inputs"][0]["pause"], json!(true));
    let req = json_call(
        "/v1/sound-generation",
        "",
        r#"{"text":"Rain","duration_seconds":null}"#,
    )
    .unwrap();
    let (enc, _) = encode(&req).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(sent["duration_seconds"], Value::Null);
}

#[test]
fn uploads_name_the_field_that_is_wrong_or_missing() {
    let changer = format!("/v1/voice-changer/{VOICE}");
    let audio = FormField::File("audio", "a.mp3", "audio/mpeg", b"ID3");
    assert_eq!(
        param(upload(
            &changer,
            &[FormField::Text("model_id", "eleven_multilingual_sts_v2")]
        ))
        .as_deref(),
        Some("audio")
    );
    assert_eq!(
        param(upload(&changer, &[audio, FormField::Text("seed", "seven")])).as_deref(),
        Some("seed")
    );
    let settings = upload(
        &changer,
        &[
            audio,
            FormField::Text("voice_settings", r#"{"stability":0.5}"#),
        ],
    )
    .unwrap();
    let (enc, _) = encode(&settings).unwrap();
    assert!(String::from_utf8_lossy(&enc.body).contains(r#"{"stability":0.5}"#));

    let stt = "/v1/speech-to-text";
    let model = FormField::Text("model_id", "scribe_v2");
    let file = FormField::File("file", "a.mp3", "audio/mpeg", b"ID3");
    let link = FormField::Text("source_url", "https://example.com/a.mp3");
    assert_eq!(param(upload(stt, &[model])).as_deref(), Some("file"));
    assert_eq!(
        param(upload(stt, &[model, file, link])).as_deref(),
        Some("file")
    );
    assert_eq!(
        param(upload(
            stt,
            &[model, file, FormField::Text("diarize", "maybe")]
        ))
        .as_deref(),
        Some("diarize")
    );
    assert_eq!(
        param(upload(
            stt,
            &[model, file, FormField::Text("num_speakers", "two")]
        ))
        .as_deref(),
        Some("num_speakers")
    );
    let linked = upload(stt, &[model, link, FormField::Text("diarize", "false")]).unwrap();
    let (enc, _) = encode(&linked).unwrap();
    let body = String::from_utf8_lossy(&enc.body).into_owned();
    assert!(body.contains("https://example.com/a.mp3") && body.contains("source_url"));
}

#[test]
fn what_elevenlabs_cannot_make_is_refused_before_sending() {
    let generate = crate::dispatch::decode_request(
        kaeso_kpp::DialectId::OpenAiChat,
        kaeso_kpp::RequestKind::Generate,
        br#"{"model":"m","messages":[{"role":"user","content":"Hi"}]}"#,
        &HeaderList::new(),
    )
    .unwrap();
    refused(&generate);
    assert!(encode(&generate).is_err());
    assert_eq!(refused(&speech_with(|r| r.voice = None)), "voice");
    assert_eq!(
        refused(&speech_with(|r| r.instructions = Some("Calm.".into()))),
        "instructions"
    );
    let wav_stream = json_call(
        &format!("/v1/text-to-speech/{VOICE}/stream"),
        "output_format=wav_24000",
        r#"{"text":"Hi"}"#,
    )
    .unwrap();
    assert_eq!(refused(&wav_stream), "output_format");
    assert_eq!(
        refused(&speech_with(
            |r| r.audio = Some(AudioInput::new(Source::base64("audio/mpeg", "SUQz")))
        )),
        "input"
    );
    assert_eq!(
        refused(&speech_with(|r| {
            r.operation = SpeechOperation::SoundEffect;
            r.timestamps = true;
        })),
        "timestamps"
    );
    let by_link = speech_with(|r| {
        r.operation = SpeechOperation::VoiceChange;
        r.input = None;
        r.audio = Some(AudioInput::new(Source::Url(
            "https://example.com/a.mp3".into(),
        )));
    });
    assert_eq!(
        encode(&by_link).unwrap_err().param.as_deref(),
        Some("audio")
    );
    assert_eq!(
        refused(&transcription_with(
            |r| r.operation = TranscriptionOperation::Translate
        )),
        "model"
    );
    assert_eq!(
        refused(&transcription_with(|r| r.prompt = Some("Kaeso".into()))),
        "prompt"
    );
    assert_eq!(
        refused(&transcription_with(|r| {
            r.audio = AudioInput::new(Source::file_id(Origin::new(D), "file-1"));
        })),
        "file"
    );
    assert_eq!(
        refused(&transcription_with(
            |r| r.format = Some(TranscriptFormat::from_wire("tsv"))
        )),
        "response_format"
    );
}

#[test]
fn broken_replies_and_lines_are_provider_errors() {
    let treq = transcription_with(|_| {});
    let (_, ctx) = encode(&treq).unwrap();
    for body in ["[]", r#"{"transcripts":[]}"#, r#"{"language_code":"en"}"#] {
        let err = ElevenLabs::decode_response(200, &HeaderList::new(), body.as_bytes(), &ctx)
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::ProviderError, "{body}");
    }
    let failed = br#"{"detail":{"status":"voice_not_found","message":"No voice."}}"#;
    let err = ElevenLabs::decode_response(404, &HeaderList::new(), failed, &ctx).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound);

    let sreq = speech_with(|r| {
        r.timestamps = true;
        r.stream = kaeso_kpp::StreamMode::on(false);
    });
    let (_, ctx) = encode(&sreq).unwrap();
    let mut dec = ElevenLabs::stream_decoder(&ctx);
    let mut events =
        dec.feed(b"{\"detail\":{\"message\":\"Busy.\",\"status\":\"system_busy\"}}\nnot json\n");
    events.extend(dec.finish());
    let errors: Vec<&KppError> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::Error(err) => Some(err),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 2, "{events:?}");
    assert_eq!(errors[0].message, "Busy.");
}

#[test]
fn foreign_transcripts_and_audio_get_elevenlabs_forms() {
    let treq = transcription_with(|_| {});
    let mut t = TranscriptionResponse::new("whisper-1", "Hi there");
    let mut word = TranscriptWord::new("Hi", 0.0, 0.4);
    word.speaker = Some("speaker_0".into());
    t.words = vec![word];
    t.usage_seconds = Some(1.5);
    let out = ElevenLabs::encode_response(&Reply::Transcription(t), &EncodeCtx::for_request(treq))
        .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        (
            &v["words"][0]["type"],
            &v["words"][0]["speaker_id"],
            &v["language_probability"],
            &v["audio_duration_secs"]
        ),
        (
            &json!("word"),
            &json!("speaker_0"),
            &json!(1.0),
            &json!(1.5)
        )
    );

    let sreq = speech_with(|_| {});
    let app = EncodeCtx::for_request(sreq);
    let linked = SpeechResponse::new("m", Source::Url("https://example.com/a.mp3".into()));
    assert!(ElevenLabs::encode_response(&Reply::Speech(linked.clone()), &app).is_err());
    let plain = EncodeCtx::new(kaeso_kpp::RequestKind::Speech, kaeso_kpp::StreamMode::Off);
    let wav = SpeechResponse::new("m", Source::base64("audio/wav", "UklG"));
    assert_eq!(
        ElevenLabs::response_media_type(&Reply::Speech(wav), &plain),
        "audio/wav"
    );
    assert_eq!(
        ElevenLabs::response_media_type(&Reply::Speech(linked), &plain),
        "audio/mpeg"
    );
    let mut enc = ElevenLabs::stream_encoder(&app);
    assert!(
        enc.encode(&StreamEvent::Speech(SpeechEvent::audio("not base64!")))
            .is_err()
    );
    assert!(
        enc.encode(&StreamEvent::Transcript(kaeso_kpp::TranscriptEvent::delta(
            "Hi"
        )))
        .unwrap()
        .is_empty()
    );
    assert!(enc.finish().unwrap().is_empty());
}

#[test]
fn every_error_kind_round_trips_through_elevenlabs_shape() {
    for kind in [
        ErrorKind::InvalidRequest,
        ErrorKind::unsupported("speed"),
        ErrorKind::RequestTooLarge,
        ErrorKind::Authentication,
        ErrorKind::PermissionDenied,
        ErrorKind::Billing,
        ErrorKind::NotFound,
        ErrorKind::RateLimited,
        ErrorKind::Overloaded,
        ErrorKind::ProviderError,
    ] {
        let err = KppError::new(kind.clone(), "No.")
            .with_param("text")
            .with_retry_after(std::time::Duration::from_secs(3));
        let enc = ElevenLabs::encode_error(&err);
        let back = ElevenLabs::decode_error(enc.status, &enc.headers, &enc.body);
        let expected = match kind {
            ErrorKind::Unsupported { .. } => ErrorKind::InvalidRequest,
            ErrorKind::RequestTooLarge => ErrorKind::InvalidRequest,
            other => other,
        };
        assert_eq!(back.kind, expected, "{enc:?}");
        assert_eq!(back.retry_after, Some(std::time::Duration::from_secs(3)));
    }
    for (status, kind) in [
        (400, ErrorKind::InvalidRequest),
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (429, ErrorKind::RateLimited),
        (503, ErrorKind::Overloaded),
        (500, ErrorKind::ProviderError),
    ] {
        let err = ElevenLabs::decode_error(status, &HeaderList::new(), b"<html>busy</html>");
        assert_eq!(err.kind, kind, "{status}");
        assert!(err.message.contains("<html>busy</html>"));
    }
    let text = ElevenLabs::decode_error(401, &HeaderList::new(), br#"{"detail":"Bad key."}"#);
    assert_eq!(
        (text.kind, text.message.as_str()),
        (ErrorKind::Authentication, "Bad key.")
    );
    let unnamed =
        ElevenLabs::decode_error(422, &HeaderList::new(), br#"{"detail":[{"msg":"No."}]}"#);
    assert_eq!(unnamed.param, None);
}
