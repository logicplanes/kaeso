use kaeso_kpp::{
    AudioFormat, AudioInput, ErrorKind, Reply, Request, Source, SpeechOperation, SpeechRequest,
    StreamEvent, StreamMode, TimestampGranularity, TranscriptFormat, TranscriptionOperation,
    TranscriptionRequest, Voice,
};
use serde_json::{Value, json};

use super::*;
use crate::openai_speech::OpenAiSpeech;
use crate::openai_transcriptions::OpenAiTranscriptions;
use crate::test_support::fixtures;

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures("gemini", folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b)
        .unwrap()
}

fn egress(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("google")
        .with_gemini_profile(GeminiProfile::GOOGLE_AI_STUDIO)
}

fn speech() -> SpeechRequest {
    let mut r = SpeechRequest::new("gemini-3.8-flash-lite-tts", SpeechOperation::Speech)
        .with_input("Hi.")
        .with_voice(Voice::Named("Kore".into()));
    r.format = Some(AudioFormat::Mp3);
    r
}

fn param_of(req: Request) -> String {
    let err = Gemini::encode_request(&req, &mut egress(&req)).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    err.param.unwrap_or_default()
}

#[test]
fn an_openai_speech_request_becomes_the_recorded_generate_content_call() {
    let req = Request::Speech(speech());
    let mut ctx = egress(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(
        enc.path,
        "/v1beta/models/gemini-3.8-flash-lite-tts:generateContent"
    );
    assert!(ctx.stream.is_off());
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    let recorded: Value = serde_json::from_slice(&fixture("requests", "speech-mp3")).unwrap();
    assert_eq!(sent, recorded);

    let reply = Gemini::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "recorded-speech-mp3"),
        &ctx,
    )
    .unwrap();
    let Reply::Speech(r) = &reply else { panic!() };
    let Source::Base64(b) = &r.audio else {
        panic!()
    };
    assert_eq!(b.media_type, "audio/mp3; rate=44100; channels=1");
    assert_eq!(r.usage.output_audio_tokens, Some(26));
    let app = EncodeCtx::for_request(req.clone());
    let bytes = OpenAiSpeech::encode_response(&reply, &app).unwrap();
    assert_eq!(bytes, b.data.to_vec());
    assert_eq!(
        OpenAiSpeech::response_media_type(&reply, &app),
        "audio/mpeg"
    );
}

#[test]
fn pcm_speech_streams_and_reaches_an_events_app() {
    let mut r = speech();
    r.format = Some(AudioFormat::Pcm16);
    r.language = Some("en-US".into());
    r.seed = Some(7);
    r.stream = StreamMode::on(false);
    let req = Request::Speech(r);
    let mut ctx = egress(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap();
    assert!(
        enc.path.ends_with(":streamGenerateContent?alt=sse"),
        "{}",
        enc.path
    );
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["generationConfig"]["speechConfig"]["languageCode"],
        "en-US"
    );
    assert_eq!(body["generationConfig"]["seed"], 7);
    let (_, sse) = fixtures("gemini", "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == "recorded-stream-speech")
        .unwrap();
    let mut dec = Gemini::stream_decoder(&ctx);
    let mut events = Vec::new();
    for chunk in sse.chunks(512) {
        events.extend(dec.feed(chunk));
    }
    events.extend(dec.finish());
    events.extend(dec.flush());
    let pieces = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Speech(s) if s.audio.is_some()))
        .count();
    assert_eq!(pieces, 16);
    let StreamEvent::Speech(first) = &events[0] else {
        panic!("{events:?}")
    };
    assert!(
        first
            .media_type
            .as_deref()
            .unwrap()
            .starts_with("audio/l16")
    );
    assert!(matches!(events.last(), Some(StreamEvent::Speech(e)) if e.done && e.usage.is_some()));
    let mut enc = OpenAiSpeech::stream_encoder(&EncodeCtx::for_request(req.clone()));
    let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
    let items = crate::testing::sse_items(&out).unwrap();
    assert_eq!(items.len(), 17);
}

#[test]
fn speech_settings_gemini_lacks_are_refused() {
    let with = |f: fn(&mut SpeechRequest)| {
        let mut r = speech();
        f(&mut r);
        param_of(Request::Speech(r))
    };
    assert_eq!(
        with(|r| r.voice = Some(Voice::Named("alloy".into()))),
        "voice"
    );
    assert_eq!(with(|r| r.voice = Some(Voice::Id("v".into()))), "voice");
    assert_eq!(
        with(|r| r.format = Some(AudioFormat::Opus)),
        "response_format"
    );
    assert_eq!(
        with(|r| r.instructions = Some("cheerful".into())),
        "instructions"
    );
    assert_eq!(with(|r| r.speed = Some(1.5)), "speed");
    assert_eq!(with(|r| r.operation = SpeechOperation::Music), "model");
    let req = Request::Speech(speech());
    Gemini::check(&req, &egress(&req)).unwrap();
}

fn openai_transcription(format: TranscriptFormat) -> TranscriptionRequest {
    let (_, body) = fixtures("openai_transcriptions", "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe")
        .unwrap();
    let headers = HeaderList::new().with(
        "content-type",
        "multipart/form-data; boundary=----kaesofixture",
    );
    let Request::Transcription(mut r) = OpenAiTranscriptions::decode_request(
        kaeso_kpp::RequestKind::Transcription,
        &body,
        &headers,
    )
    .unwrap() else {
        panic!()
    };
    r.model = "gemini-3.5-transcribe".into();
    r.format = Some(format);
    r
}

#[test]
fn an_openai_transcription_becomes_the_recorded_transcribe_call() {
    let req = Request::Transcription(openai_transcription(TranscriptFormat::DiarizedJson));
    let mut ctx = egress(&req);
    let enc = Gemini::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(
        enc.path,
        "/v1beta/models/gemini-3.5-transcribe:generateContent"
    );
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    let recorded: Value =
        serde_json::from_slice(&fixture("requests", "transcribe-diarized")).unwrap();
    assert_eq!(sent, recorded);

    let reply = Gemini::decode_response(
        200,
        &HeaderList::new(),
        &fixture("responses", "recorded-transcribe-diarized"),
        &ctx,
    )
    .unwrap();
    let Reply::Transcription(t) = &reply else {
        panic!()
    };
    assert_eq!(t.text, "Hello, Kizo.");
    assert_eq!(t.segments.len(), 1);
    assert_eq!(t.segments[0].speaker.as_deref(), Some("spk:0"));
    assert_eq!((t.segments[0].start, t.segments[0].end), (0.0, 0.9));
    assert_eq!(t.words.len(), 2);
    assert_eq!(t.usage.input_audio_tokens, Some(28));
    let app = |format| EncodeCtx::for_request(Request::Transcription(openai_transcription(format)));
    let diarized: Value = serde_json::from_slice(
        &OpenAiTranscriptions::encode_response(&reply, &app(TranscriptFormat::DiarizedJson))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(diarized["segments"][0]["speaker"], "spk:0");
    assert_eq!(diarized["duration"], json!(0.9));
    let srt = OpenAiTranscriptions::encode_response(&reply, &app(TranscriptFormat::Srt)).unwrap();
    assert_eq!(
        String::from_utf8(srt).unwrap(),
        "1\n00:00:00,000 --> 00:00:00,900\nHello, Kizo.\n\n"
    );
}

#[test]
fn plain_transcripts_ask_for_no_timing_and_settings_gemini_lacks_are_refused() {
    let mut r = openai_transcription(TranscriptFormat::Text);
    r.language = Some("en-US".into());
    let req = Request::Transcription(r);
    let enc = Gemini::encode_request(&req, &mut egress(&req)).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["generationConfig"]["audioTranscriptionConfig"],
        json!({"languageCodes": ["en-US"]})
    );
    let mut words = openai_transcription(TranscriptFormat::Json);
    words.timestamps = vec![TimestampGranularity::Word];
    let req = Request::Transcription(words);
    let enc = Gemini::encode_request(&req, &mut egress(&req)).unwrap();
    assert!(
        String::from_utf8(enc.body)
            .unwrap()
            .contains("wordTimestamp")
    );
    let with = |f: fn(&mut TranscriptionRequest)| {
        let mut r = openai_transcription(TranscriptFormat::Json);
        f(&mut r);
        param_of(Request::Transcription(r))
    };
    assert_eq!(
        with(|r| r.operation = TranscriptionOperation::Translate),
        "model"
    );
    assert_eq!(with(|r| r.prompt = Some("p".into())), "prompt");
    assert_eq!(with(|r| r.temperature = Some(0.1)), "temperature");
    assert_eq!(with(|r| r.speakers = Some(2)), "num_speakers");
    assert_eq!(
        with(|r| r.format = Some(TranscriptFormat::Other("tsv".into()))),
        "response_format"
    );
    assert_eq!(
        with(|r| r.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()))),
        "file"
    );
}

#[test]
fn a_reply_without_audio_or_transcript_is_an_error() {
    let empty =
        br#"{"candidates":[{"content":{"parts":[],"role":"model"},"finishReason":"SAFETY"}]}"#;
    let req = Request::Speech(speech());
    let err = Gemini::decode_response(200, &HeaderList::new(), empty, &egress(&req)).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ContentFiltered);
    let req = Request::Transcription(openai_transcription(TranscriptFormat::Json));
    let err = Gemini::decode_response(200, &HeaderList::new(), empty, &egress(&req)).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ContentFiltered);
}
