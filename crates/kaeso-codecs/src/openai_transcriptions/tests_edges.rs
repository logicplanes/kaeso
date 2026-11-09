use kaeso_kpp::{
    AudioInput, ErrorKind, Origin, Reply, Request, Source, StreamEvent, StreamMode,
    TimestampGranularity, TranscriptFormat, TranscriptionOperation, TranscriptionRequest,
};
use serde_json::{Value, json};

use super::request::parts_of;
use super::*;
use crate::multipart::{FormField, FormWriter};

fn upload(fields: &[FormField<'_>]) -> Result<TranscriptionRequest, KppError> {
    upload_with(fields, HeaderList::new())
}

fn upload_with(
    fields: &[FormField<'_>],
    mut headers: HeaderList,
) -> Result<TranscriptionRequest, KppError> {
    let w = FormWriter::write(fields);
    headers.push("content-type", w.content_type());
    match crate::dispatch::decode_transcription_request(
        TranscriptionOperation::Transcribe,
        &w.into_body(),
        &headers,
    )? {
        Request::Transcription(r) => Ok(r),
        other => panic!("{other:?}"),
    }
}

const MODEL: FormField<'static> = FormField::Text("model", "whisper-1");
const FILE: FormField<'static> = FormField::File("file", "a.mp3", "audio/mpeg", b"ID3");

fn ctx_for(
    profile: TranscriptionsProfile,
    r: &TranscriptionRequest,
    base: Option<&str>,
) -> EgressCtx {
    let ctx = EgressCtx::new(&Request::Transcription(r.clone()))
        .with_provider("openai")
        .with_transcriptions_profile(profile);
    match base {
        Some(b) => ctx.with_base_path(b),
        None => ctx,
    }
}

fn sent(
    profile: TranscriptionsProfile,
    r: &TranscriptionRequest,
    base: Option<&str>,
) -> Result<(EncodedRequest, Vec<(String, Value)>), KppError> {
    let mut ctx = ctx_for(profile, r, base);
    let enc = OpenAiTranscriptions::encode_request(&Request::Transcription(r.clone()), &mut ctx)?;
    let parts = match enc.headers.get("content-type") {
        Some(t) if t.starts_with("multipart/") => parts_of(&enc.body, t),
        _ => Vec::new(),
    };
    Ok((enc, parts))
}

fn verbose_ctx() -> EgressCtx {
    let mut r = TranscriptionRequest::new(
        "whisper-1",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::base64("audio/mpeg", "SUQz").unwrap()),
    );
    r.format = Some(TranscriptFormat::VerboseJson);
    r.stream = StreamMode::on(true);
    let mut ctx = ctx_for(TranscriptionsProfile::OPENAI, &r, None);
    OpenAiTranscriptions::encode_request(&Request::Transcription(r), &mut ctx).unwrap();
    ctx
}

#[test]
fn uploads_that_cannot_be_one_request_are_refused_with_their_field() {
    let twice = upload(&[MODEL, FILE, FILE]).unwrap_err();
    assert_eq!(twice.param.as_deref(), Some("file"));
    assert!(twice.message.contains("more than once"), "{twice:?}");
    let both = upload(&[
        MODEL,
        FILE,
        FormField::Text("url", "https://example.com/a.mp3"),
    ])
    .unwrap_err();
    assert!(both.message.contains("not both"), "{both:?}");
    let r = upload_with(
        &[MODEL, FILE, FormField::Text("include", "logprobs")],
        HeaderList::new().with("openai-beta", "assistants=v2"),
    )
    .unwrap();
    assert_eq!(
        r.extras.fields["include"].to_value().unwrap(),
        json!(["logprobs"])
    );
    assert_eq!(r.extras.headers().count(), 1);
    let (enc, _) = sent(TranscriptionsProfile::OPENAI, &r, None).unwrap();
    assert_eq!(enc.headers.get("openai-beta"), Some("assistants=v2"));
}

#[test]
fn links_file_ids_and_timing_take_each_providers_own_field() {
    let linked = upload(&[
        MODEL,
        FormField::Text("file_url", "https://example.com/a.mp3"),
    ])
    .unwrap();
    let (_, parts) = sent(TranscriptionsProfile::MISTRAL, &linked, None).unwrap();
    assert!(parts.contains(&("file_url".into(), json!("https://example.com/a.mp3"))));
    let mut by_id = linked.clone();
    by_id.audio = AudioInput::new(Source::file_id(Origin::new(DIALECT), "file-1"));
    let (_, parts) = sent(TranscriptionsProfile::MISTRAL, &by_id, None).unwrap();
    assert!(parts.contains(&("file_id".into(), json!("file-1"))));
    let refused = sent(TranscriptionsProfile::OPENAI, &by_id, None).unwrap_err();
    assert_eq!(refused.param.as_deref(), Some("file"));
    let vercel = sent(TranscriptionsProfile::VERCEL, &linked, None).unwrap_err();
    assert_eq!(vercel.param.as_deref(), Some("file"));

    let unbracketed = upload(&[
        MODEL,
        FILE,
        FormField::Text("response_format", "verbose_json"),
        FormField::Text("timestamp_granularities", "word"),
    ])
    .unwrap();
    let (enc, parts) = sent(TranscriptionsProfile::OPENAI, &unbracketed, Some("/openai")).unwrap();
    assert_eq!(enc.path, "/openai/v1/audio/transcriptions");
    assert!(parts.contains(&("timestamp_granularities".into(), json!("word"))));

    let mut sentences = unbracketed.clone();
    sentences.timestamps = vec![TimestampGranularity::Segment];
    let (_, parts) = sent(TranscriptionsProfile::MINIMAX, &sentences, None).unwrap();
    assert!(parts.contains(&("timestamp_level".into(), json!("sentence"))));

    let mut text = unbracketed;
    text.timestamps.clear();
    text.format = Some(TranscriptFormat::Text);
    let (enc, _) = sent(TranscriptionsProfile::VERCEL, &text, Some("/vercel")).unwrap();
    assert_eq!(enc.path, "/vercel/v4/ai/transcription-model");
    let mut ctx = ctx_for(TranscriptionsProfile::ZAI, &text, None);
    OpenAiTranscriptions::encode_request(&Request::Transcription(text.clone()), &mut ctx).unwrap();
    assert_eq!(ctx.transcript_format, Some(TranscriptFormat::Json));
    let ctx = ctx_for(TranscriptionsProfile::ZAI, &text, None);
    assert!(OpenAiTranscriptions::check(&Request::Transcription(text), &ctx).is_ok());
}

#[test]
fn malformed_transcripts_are_provider_errors() {
    let ctx = verbose_ctx();
    for body in [
        "[]",
        "{}",
        r#"{"text":"a","segments":[1]}"#,
        r#"{"text":"a","segments":[{"end":1}]}"#,
        r#"{"text":"a","segments":[{"start":0}]}"#,
        r#"{"text":"a","words":[1]}"#,
        r#"{"text":"a","words":[{"word":"a"}]}"#,
    ] {
        let err =
            OpenAiTranscriptions::decode_response(200, &HeaderList::new(), body.as_bytes(), &ctx)
                .unwrap_err();
        assert_eq!(err.kind, ErrorKind::ProviderError, "{body}");
        assert!(err.message.contains("not a valid"), "{body}: {err:?}");
    }
    let odd_id = br#"{"text":"a","segments":[{"id":true,"start":0,"end":1,"text":"a"}]}"#;
    let reply =
        OpenAiTranscriptions::decode_response(200, &HeaderList::new(), odd_id, &ctx).unwrap();
    let Reply::Transcription(t) = &reply else {
        panic!("{reply:?}")
    };
    assert_eq!(t.segments[0].id, None);
    assert_eq!(
        t.segments[0].extras.fields["id"].to_value().unwrap(),
        json!(true)
    );
}

#[test]
fn broken_transcript_streams_end_with_one_error_and_keep_unknown_events() {
    let ctx = verbose_ctx();
    let run = |sse: &str| {
        let mut dec = OpenAiTranscriptions::stream_decoder(&ctx);
        let mut events = dec.feed(sse.as_bytes());
        events.extend(dec.finish());
        events
    };
    let errors = |events: &[StreamEvent]| {
        events
            .iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count()
    };
    let failed = run("data: {\"type\":\"error\",\"error\":{\"message\":\"Audio too long.\"}}\n\n");
    assert_eq!(errors(&failed), 1, "{failed:?}");
    let StreamEvent::Error(err) = &failed[0] else {
        panic!("{failed:?}")
    };
    assert!(err.message.contains("Audio too long."));
    assert_eq!(errors(&run("data: [1]\n\n")), 1);
    let cut = run("data: {\"type\":\"transcript.text.delta\",\"delta\":\"Hi\"}\n\n");
    assert_eq!(errors(&cut), 1, "{cut:?}");
    let unknown = run("event: ping\ndata: {\"type\":\"transcript.text.ping\"}\n\n");
    assert!(matches!(&unknown[0], StreamEvent::Extra(_)), "{unknown:?}");

    let req = Request::Transcription(TranscriptionRequest::new(
        "whisper-1",
        TranscriptionOperation::Transcribe,
        AudioInput::new(Source::base64("audio/mpeg", "SUQz").unwrap()),
    ));
    let mut enc = OpenAiTranscriptions::stream_encoder(&EncodeCtx::for_request(req));
    let error = enc
        .encode(&StreamEvent::Error(KppError::provider(
            "The audio is too long.",
        )))
        .unwrap();
    assert!(
        String::from_utf8(error)
            .unwrap()
            .contains("The audio is too long.")
    );
    assert!(
        String::from_utf8(enc.encode(&unknown[0]).unwrap())
            .unwrap()
            .contains("ping")
    );
    assert!(
        enc.encode(&StreamEvent::Speech(kaeso_kpp::SpeechEvent::audio("AA")))
            .unwrap()
            .is_empty()
    );
    assert!(enc.finish().unwrap().is_empty());
}
