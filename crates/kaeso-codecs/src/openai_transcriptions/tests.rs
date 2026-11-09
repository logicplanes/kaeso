use kaeso_kpp::{
    AudioInput, ErrorKind, Reply, Request, Source, StreamEvent, StreamMode, TimestampGranularity,
    TranscriptFormat, TranscriptSegment, TranscriptWord, TranscriptionOperation,
    TranscriptionRequest, TranscriptionResponse,
};
use serde_json::{Value, json};

use super::request::{self, parts_of};
use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures, sidecar};

const DIR: &str = "openai_transcriptions";

fn operation_of(name: &str) -> TranscriptionOperation {
    if name.starts_with("translate") {
        TranscriptionOperation::Translate
    } else {
        TranscriptionOperation::Transcribe
    }
}

fn upload_headers(name: &str) -> HeaderList {
    let meta = sidecar(DIR, "requests", name, "meta").unwrap();
    HeaderList::new().with("content-type", meta["content_type"].as_str().unwrap())
}

fn decode(name: &str, body: &[u8]) -> TranscriptionRequest {
    match crate::dispatch::decode_transcription_request(
        operation_of(name),
        body,
        &upload_headers(name),
    )
    .unwrap()
    {
        Request::Transcription(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode(
    profile: TranscriptionsProfile,
    req: &TranscriptionRequest,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let r = Request::Transcription(req.clone());
    let provider = if profile == TranscriptionsProfile::OPENAI {
        "openai"
    } else {
        "other"
    };
    let mut ctx = EgressCtx::new(&r)
        .with_provider(provider)
        .with_transcriptions_profile(profile);
    let enc = OpenAiTranscriptions::encode_request(&r, &mut ctx)?;
    Ok((enc, ctx))
}

fn sorted(mut parts: Vec<(String, Value)>) -> Vec<(String, Value)> {
    parts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.to_string().cmp(&b.1.to_string())));
    parts
}

fn mp3() -> AudioInput {
    AudioInput::new(Source::base64("audio/mpeg", "SUQz").unwrap()).with_filename("a.mp3")
}

fn transcribe(format: Option<TranscriptFormat>) -> TranscriptionRequest {
    let mut r = TranscriptionRequest::new("whisper-1", TranscriptionOperation::Transcribe, mp3());
    r.format = format;
    r
}

fn app_ctx(req: &TranscriptionRequest) -> EncodeCtx {
    EncodeCtx::for_request(Request::Transcription(req.clone()))
}

fn refusal(profile: TranscriptionsProfile, req: &TranscriptionRequest) -> String {
    let err = encode(profile, req).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    err.param.unwrap_or_default()
}

#[test]
fn every_upload_round_trips_part_for_part() {
    for (name, body) in fixtures(DIR, "requests", "multipart") {
        if name.starts_with("providers/") {
            continue;
        }
        let req = decode(&name, &body);
        req.validate().unwrap();
        let (enc, _) = encode(TranscriptionsProfile::OPENAI, &req).unwrap();
        let expected = if name.starts_with("translate") {
            "/v1/audio/translations"
        } else {
            "/v1/audio/transcriptions"
        };
        assert_eq!(enc.path, expected, "{name}");
        let ct = enc.headers.get("content-type").unwrap();
        let meta = sidecar(DIR, "requests", &name, "meta").unwrap();
        assert_eq!(
            sorted(parts_of(&enc.body, ct)),
            sorted(parts_of(&body, meta["content_type"].as_str().unwrap())),
            "{name}"
        );
    }
}

#[test]
fn uploads_decode_to_typed_fields_and_extras() {
    let (name, body) = fixtures(DIR, "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe-every-field")
        .unwrap();
    let r = decode(&name, &body);
    assert_eq!(r.language.as_deref(), Some("en"));
    assert_eq!(r.prompt.as_deref(), Some("Kaeso, KPP"));
    assert_eq!(r.format, Some(TranscriptFormat::VerboseJson));
    assert_eq!(r.temperature, Some(0.2));
    assert_eq!(
        r.timestamps,
        [TimestampGranularity::Word, TimestampGranularity::Segment]
    );
    assert_eq!(r.audio.filename.as_deref(), Some("speech.mp3"));
    let (name, body) = fixtures(DIR, "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "transcribe-diarize")
        .unwrap();
    let r = decode(&name, &body);
    assert!(r.extras.fields.contains_key("known_speaker_names"));
    assert!(r.extras.fields.contains_key("chunking_strategy"));
    let (name, body) = fixtures(DIR, "requests", "multipart")
        .into_iter()
        .find(|(n, _)| n == "stream-transcribe")
        .unwrap();
    assert!(!decode(&name, &body).stream.is_off());
}

#[test]
fn broken_uploads_are_refused_with_their_field() {
    let ct = "multipart/form-data; boundary=b";
    let form = |parts: &str| parts.replace('\n', "\r\n").into_bytes();
    let cases = [
        (
            "--b\nContent-Disposition: form-data; name=\"model\"\n\nm\n--b--\n",
            "file",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"file\"\n\nx\n--b--\n",
            "file",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"x\"; filename=\"a\"\n\nx\n--b--\n",
            "x",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"url\"\n\nhttps://example.com/a.mp3\n--b\nContent-Disposition: form-data; name=\"temperature\"\n\nhot\n--b--\n",
            "temperature",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"url\"\n\nu\n--b\nContent-Disposition: form-data; name=\"stream\"\n\nyes\n--b--\n",
            "stream",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"url\"\n\nu\n--b\nContent-Disposition: form-data; name=\"diarize\"\n\n1\n--b--\n",
            "diarize",
        ),
        (
            "--b\nContent-Disposition: form-data; name=\"url\"\n\nu\n--b\nContent-Disposition: form-data; name=\"model\"\n\na\n--b\nContent-Disposition: form-data; name=\"model\"\n\nb\n--b--\n",
            "model",
        ),
    ];
    for (body, param) in cases {
        let err = OpenAiTranscriptions::decode_call(
            TranscriptionOperation::Transcribe,
            &form(body),
            &HeaderList::new().with("content-type", ct),
        )
        .unwrap_err();
        assert_eq!(err.param.as_deref(), Some(param), "{err:?}");
    }
    let err = OpenAiTranscriptions::decode_request(
        kaeso_kpp::RequestKind::Transcription,
        b"{}",
        &HeaderList::new().with("content-type", "application/json"),
    )
    .unwrap_err();
    assert!(err.message.contains("multipart"), "{err:?}");
}

#[test]
fn every_json_reply_round_trips_in_its_own_form() {
    let cases = [
        ("json", TranscriptFormat::Json),
        ("json-logprobs", TranscriptFormat::Json),
        ("verbose-words", TranscriptFormat::VerboseJson),
        ("verbose-segments", TranscriptFormat::VerboseJson),
        ("diarized", TranscriptFormat::DiarizedJson),
        ("translation", TranscriptFormat::Json),
    ];
    let replies: Vec<_> = fixtures(DIR, "responses", "json")
        .into_iter()
        .filter(|(n, _)| !n.starts_with("providers/"))
        .collect();
    assert_eq!(replies.len(), cases.len());
    for (name, format) in cases {
        let body = &replies.iter().find(|(n, _)| n == name).unwrap().1;
        let mut req = transcribe(Some(format.clone()));
        if name == "translation" {
            req.operation = TranscriptionOperation::Translate;
        }
        let (_, ctx) = encode(TranscriptionsProfile::OPENAI, &req).unwrap();
        let reply =
            OpenAiTranscriptions::decode_response(200, &HeaderList::new(), body, &ctx).unwrap();
        let out = OpenAiTranscriptions::encode_response(&reply, &app_ctx(&req)).unwrap();
        assert_json_eq(name, body, &out);
        assert_eq!(
            OpenAiTranscriptions::response_media_type(&reply, &app_ctx(&req)),
            crate::JSON_MEDIA_TYPE
        );
    }
}

#[test]
fn text_replies_keep_their_bytes() {
    for (ext, format) in [
        ("txt", TranscriptFormat::Text),
        ("srt", TranscriptFormat::Srt),
        ("vtt", TranscriptFormat::Vtt),
    ] {
        let (name, body) = fixtures(DIR, "responses", ext).remove(0);
        let req = transcribe(Some(format));
        let (_, ctx) = encode(TranscriptionsProfile::OPENAI, &req).unwrap();
        let reply =
            OpenAiTranscriptions::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap();
        let out = OpenAiTranscriptions::encode_response(&reply, &app_ctx(&req)).unwrap();
        assert_eq!(out, body, "{name}");
        assert_eq!(
            OpenAiTranscriptions::response_media_type(&reply, &app_ctx(&req)),
            "text/plain; charset=utf-8"
        );
    }
}

#[test]
fn subtitles_are_made_from_segments_for_a_provider_without_them() {
    let mut p = TranscriptionsProfile::OPENAI;
    p.formats = &["json", "verbose_json", "text"];
    let req = transcribe(Some(TranscriptFormat::Srt));
    let (enc, ctx) = encode(p, &req).unwrap();
    let ct = enc.headers.get("content-type").unwrap();
    let parts = parts_of(&enc.body, ct);
    assert!(parts.contains(&("response_format".into(), json!("verbose_json"))));
    assert!(parts.contains(&("timestamp_granularities[]".into(), json!("segment"))));
    assert_eq!(ctx.transcript_format, Some(TranscriptFormat::VerboseJson));
    let verbose = &fixtures(DIR, "responses", "json")
        .into_iter()
        .find(|(n, _)| n == "verbose-segments")
        .unwrap()
        .1;
    let reply =
        OpenAiTranscriptions::decode_response(200, &HeaderList::new(), verbose, &ctx).unwrap();
    let srt = OpenAiTranscriptions::encode_response(&reply, &app_ctx(&req)).unwrap();
    let (_, expected) = fixtures(DIR, "responses", "srt").remove(0);
    assert_eq!(
        String::from_utf8(srt).unwrap(),
        String::from_utf8(expected).unwrap()
    );
    let vtt_req = transcribe(Some(TranscriptFormat::Vtt));
    let vtt = OpenAiTranscriptions::encode_response(&reply, &app_ctx(&vtt_req)).unwrap();
    let (_, expected) = fixtures(DIR, "responses", "vtt").remove(0);
    assert_eq!(vtt, expected);
    let mut words_only = TranscriptionResponse::new("m", "Hi there");
    words_only.words = vec![
        TranscriptWord::new("Hi", 0.0, 0.4),
        TranscriptWord::new("there", 0.5, 1.25),
    ];
    let out = String::from_utf8(
        OpenAiTranscriptions::encode_response(&Reply::Transcription(words_only), &app_ctx(&req))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(out, "1\n00:00:00,000 --> 00:00:01,250\nHi there\n\n");
    let untimed = TranscriptionResponse::new("m", "Hi");
    assert!(
        OpenAiTranscriptions::encode_response(&Reply::Transcription(untimed), &app_ctx(&req))
            .is_err()
    );
}

#[test]
fn foreign_replies_take_each_form() {
    let mut r = TranscriptionResponse::new("m", "Hi there.");
    r.language = Some("en".into());
    r.duration = Some(1.5);
    let mut seg = TranscriptSegment::new(0.0, 1.5, "Hi there.");
    seg.speaker = Some("speaker_0".into());
    seg.id = Some("0".into());
    r.segments.push(seg);
    r.words.push(TranscriptWord::new("Hi", 0.0, 0.5));
    r.usage_seconds = Some(2.0);
    let reply = Reply::Transcription(r);
    let as_json = |format: TranscriptFormat| -> Value {
        let out =
            OpenAiTranscriptions::encode_response(&reply, &app_ctx(&transcribe(Some(format))))
                .unwrap();
        serde_json::from_slice(&out).unwrap()
    };
    assert_eq!(
        as_json(TranscriptFormat::Json),
        json!({"text": "Hi there.", "usage": {"type": "duration", "seconds": 2.0}})
    );
    let verbose = as_json(TranscriptFormat::VerboseJson);
    assert_eq!(verbose["task"], "transcribe");
    assert_eq!(verbose["language"], "en");
    assert_eq!(verbose["words"][0]["word"], "Hi");
    let diarized = as_json(TranscriptFormat::DiarizedJson);
    assert_eq!(diarized["segments"][0]["type"], "transcript.text.segment");
    assert_eq!(diarized["segments"][0]["speaker"], "speaker_0");
    let text = OpenAiTranscriptions::encode_response(
        &reply,
        &app_ctx(&transcribe(Some(TranscriptFormat::Text))),
    )
    .unwrap();
    assert_eq!(text, b"Hi there.");
}

#[test]
fn streams_round_trip_and_feed_whole_replies_to_streaming_apps() {
    for (name, sse) in fixtures(DIR, "streams", "sse")
        .into_iter()
        .filter(|(n, _)| !n.starts_with("providers/"))
    {
        let mut req = transcribe(None);
        req.stream = StreamMode::on(true);
        let (enc, ctx) = encode(TranscriptionsProfile::OPENAI, &req).unwrap();
        let parts = parts_of(&enc.body, enc.headers.get("content-type").unwrap());
        assert!(parts.contains(&("stream".into(), json!("true"))));
        let mut dec = OpenAiTranscriptions::stream_decoder(&ctx);
        let mut events = Vec::new();
        for chunk in sse.chunks(29) {
            events.extend(dec.feed(chunk));
        }
        events.extend(dec.finish());
        assert!(
            events
                .iter()
                .all(|e| matches!(e, StreamEvent::Transcript(_))),
            "{events:?}"
        );
        let mut enc = OpenAiTranscriptions::stream_encoder(&app_ctx(&req));
        let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
        assert_sse_eq(&name, &sse, &out);
    }
    let mut req = transcribe(None);
    req.stream = StreamMode::on(true);
    let out = OpenAiTranscriptions::encode_response(
        &Reply::Transcription(TranscriptionResponse::new("m", "Hi")),
        &app_ctx(&req),
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("transcript.text.delta") && text.contains("transcript.text.done"));
    assert_eq!(
        OpenAiTranscriptions::response_media_type(
            &Reply::Transcription(TranscriptionResponse::new("m", "Hi")),
            &app_ctx(&req)
        ),
        crate::EVENT_STREAM_MEDIA_TYPE
    );
}

#[test]
fn a_provider_without_streams_is_asked_whole() {
    let mut p = TranscriptionsProfile::OPENAI;
    p.streams = false;
    let mut req = transcribe(None);
    req.stream = StreamMode::on(true);
    let (enc, ctx) = encode(p, &req).unwrap();
    assert!(ctx.stream.is_off());
    let parts = parts_of(&enc.body, enc.headers.get("content-type").unwrap());
    assert!(!parts.iter().any(|(k, _)| k == "stream"));
}

#[test]
fn what_the_provider_lacks_is_refused_before_sending() {
    let mut p = TranscriptionsProfile::OPENAI;
    p.translations_path = None;
    p.formats = &["json"];
    p.timestamps = TimestampsField::None;
    p.prompt = false;
    let mut r = transcribe(None);
    r.operation = TranscriptionOperation::Translate;
    assert_eq!(refusal(p, &r), "model");
    assert_eq!(
        refusal(p, &transcribe(Some(TranscriptFormat::Srt))),
        "response_format"
    );
    let mut r = transcribe(None);
    r.timestamps = vec![TimestampGranularity::Word];
    assert_eq!(refusal(p, &r), "timestamp_granularities");
    let mut r = transcribe(None);
    r.prompt = Some("p".into());
    assert_eq!(refusal(p, &r), "prompt");
    let mut r = transcribe(None);
    r.diarize = Some(true);
    assert_eq!(refusal(p, &r), "diarize");
    let mut r = transcribe(None);
    r.speakers = Some(2);
    assert_eq!(refusal(p, &r), "num_speakers");
    let mut r = transcribe(None);
    r.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()));
    assert_eq!(refusal(p, &r), "file");
    let (enc, _) = encode(
        TranscriptionsProfile::OPENAI,
        &transcribe(Some(TranscriptFormat::Text)),
    )
    .unwrap();
    assert!(
        enc.headers
            .get("content-type")
            .unwrap()
            .starts_with("multipart/")
    );
    let mut links = TranscriptionsProfile::OPENAI;
    links.url_field = Some("file_url");
    let mut r = transcribe(None);
    r.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()));
    let (enc, _) = encode(links, &r).unwrap();
    let parts = parts_of(&enc.body, enc.headers.get("content-type").unwrap());
    assert!(parts.contains(&("file_url".into(), json!("https://example.com/a.mp3"))));
    assert!(!parts.iter().any(|(k, _)| k == "file"));
}

#[test]
fn errors_decode_in_openais_shape() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = meta["status"].as_u64().unwrap() as u16;
        let err = OpenAiTranscriptions::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.status, Some(status));
        let enc = OpenAiTranscriptions::encode_error(&err);
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn default_names_follow_the_media_type() {
    assert_eq!(request::default_name("audio/mpeg"), "audio.mp3");
    assert_eq!(
        request::default_name("audio/webm; codecs=opus"),
        "audio.webm"
    );
    assert_eq!(
        request::default_name("application/octet-stream"),
        "audio.bin"
    );
    assert_eq!(
        request::provider_format(&TranscriptFormat::DiarizedJson, |f| f == "json"),
        None
    );
}

fn provider_reply(id: &str, folder: &str, name: &str, ext: &str) -> Vec<u8> {
    let path = format!("providers/{id}/{name}");
    fixtures(DIR, folder, ext)
        .into_iter()
        .find(|(n, _)| *n == path)
        .map(|(_, b)| b)
        .unwrap_or_else(|| panic!("no {path}.{ext}"))
}

fn sent_parts(
    profile: TranscriptionsProfile,
    req: &TranscriptionRequest,
) -> (EncodedRequest, EgressCtx, Vec<(String, Value)>) {
    let (enc, ctx) = encode(profile, req).unwrap();
    let parts = match enc.headers.get("content-type") {
        Some(ct) if ct.starts_with("multipart/") => parts_of(&enc.body, ct),
        _ => Vec::new(),
    };
    (enc, ctx, parts)
}

fn decoded(
    profile: TranscriptionsProfile,
    req: &TranscriptionRequest,
    body: &[u8],
) -> TranscriptionResponse {
    let (_, ctx, _) = sent_parts(profile, req);
    match OpenAiTranscriptions::decode_response(200, &HeaderList::new(), body, &ctx).unwrap() {
        Reply::Transcription(t) => t,
        other => panic!("{other:?}"),
    }
}

fn has(parts: &[(String, Value)], key: &str, value: &str) -> bool {
    parts.contains(&(key.to_owned(), json!(value)))
}

#[test]
fn groq_and_together_take_openais_form_with_their_limits() {
    let mut r = transcribe(Some(TranscriptFormat::VerboseJson));
    r.model = "whisper-large-v3".into();
    r.timestamps = vec![TimestampGranularity::Word];
    let (enc, _, parts) = sent_parts(TranscriptionsProfile::GROQ, &r);
    assert_eq!(enc.path, "/v1/audio/transcriptions");
    assert!(has(&parts, "timestamp_granularities[]", "word"));
    let t = decoded(
        TranscriptionsProfile::GROQ,
        &r,
        &provider_reply("groq", "responses", "verbose", "json"),
    );
    assert_eq!(t.segments[0].id.as_deref(), Some("8"));
    let j = decoded(
        TranscriptionsProfile::GROQ,
        &transcribe(None),
        &provider_reply("groq", "responses", "json", "json"),
    );
    assert_eq!(j.text, "Your transcribed text appears here...");
    let mut link = transcribe(None);
    link.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()));
    let (_, _, parts) = sent_parts(TranscriptionsProfile::GROQ, &link);
    assert!(has(&parts, "url", "https://example.com/a.mp3"));
    let (_, _, parts) = sent_parts(TranscriptionsProfile::TOGETHER, &link);
    assert!(has(&parts, "file", "https://example.com/a.mp3"));

    let mut diarized = transcribe(Some(TranscriptFormat::VerboseJson));
    diarized.diarize = Some(true);
    diarized.timestamps = vec![TimestampGranularity::Word];
    let (_, _, parts) = sent_parts(TranscriptionsProfile::TOGETHER, &diarized);
    assert!(has(&parts, "diarize", "true") && has(&parts, "timestamp_granularities", "word"));
    let t = decoded(
        TranscriptionsProfile::TOGETHER,
        &diarized,
        &provider_reply("togetherai", "responses", "verbose", "json"),
    );
    assert_eq!(t.words[1].speaker.as_deref(), Some("SPEAKER_00"));
    let srt = OpenAiTranscriptions::encode_response(
        &Reply::Transcription(t),
        &app_ctx(&transcribe(Some(TranscriptFormat::Srt))),
    )
    .unwrap();
    assert!(String::from_utf8(srt).unwrap().contains("00:00:02,500"));
    assert_eq!(
        refusal(
            TranscriptionsProfile::TOGETHER,
            &transcribe(Some(TranscriptFormat::DiarizedJson))
        ),
        "response_format"
    );
}

#[test]
fn mistral_answers_one_form_and_streams_its_own_events() {
    let mut r = transcribe(Some(TranscriptFormat::Srt));
    r.model = "voxtral-mini-transcribe-3".into();
    let (_, ctx, parts) = sent_parts(TranscriptionsProfile::MISTRAL, &r);
    assert!(!parts.iter().any(|(k, _)| k == "response_format"));
    assert!(has(&parts, "timestamp_granularities", "segment"));
    assert_eq!(ctx.transcript_format, Some(TranscriptFormat::VerboseJson));
    let t = decoded(
        TranscriptionsProfile::MISTRAL,
        &r,
        &provider_reply("mistral", "responses", "json", "json"),
    );
    assert_eq!(t.usage_seconds, Some(34.0));
    assert_eq!(
        (
            t.usage.input_tokens,
            t.usage.output_tokens,
            t.usage.input_audio_tokens
        ),
        (3, 126, Some(750))
    );
    let srt =
        OpenAiTranscriptions::encode_response(&Reply::Transcription(t), &app_ctx(&r)).unwrap();
    assert_eq!(
        String::from_utf8(srt).unwrap(),
        "1\n00:00:00,000 --> 00:00:01,600\nThanks for calling Northwind Support.\n\n"
    );
    let mut words = r.clone();
    words.timestamps = vec![TimestampGranularity::Word];
    assert_eq!(
        refusal(TranscriptionsProfile::MISTRAL, &words),
        "timestamp_granularities"
    );
    let mut p = r.clone();
    p.prompt = Some("x".into());
    assert_eq!(refusal(TranscriptionsProfile::MISTRAL, &p), "prompt");

    let mut stream = transcribe(None);
    stream.stream = StreamMode::on(true);
    let (_, ctx, parts) = sent_parts(TranscriptionsProfile::MISTRAL, &stream);
    assert!(has(&parts, "stream", "true"));
    let mut dec = OpenAiTranscriptions::stream_decoder(&ctx);
    let mut events = dec.feed(&provider_reply(
        "mistral",
        "streams",
        "stream-transcribe",
        "sse",
    ));
    events.extend(dec.finish());
    let mut enc = OpenAiTranscriptions::stream_encoder(&app_ctx(&stream));
    let out: Vec<u8> = events.iter().flat_map(|e| enc.encode(e).unwrap()).collect();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.matches("transcript.text.delta").count(), 2, "{text}");
    assert!(
        text.contains(r#""text":"Thanks for calling.""#) && text.contains(r#""seconds":2.0"#),
        "{text}"
    );
}

#[test]
fn zai_minimax_and_their_streams_fill_the_end_from_the_deltas() {
    let mut r = transcribe(None);
    r.model = "glm-asr-2512".into();
    r.prompt = Some("Kaeso".into());
    let (enc, _, parts) = sent_parts(TranscriptionsProfile::ZAI, &r);
    assert_eq!(enc.path, "/api/paas/v4/audio/transcriptions");
    assert!(has(&parts, "prompt", "Kaeso"));
    let mut lang = r.clone();
    lang.language = Some("en".into());
    assert_eq!(refusal(TranscriptionsProfile::ZAI, &lang), "language");
    let mut warm = r.clone();
    warm.temperature = Some(0.2);
    assert_eq!(refusal(TranscriptionsProfile::ZAI, &warm), "temperature");
    let t = decoded(
        TranscriptionsProfile::ZAI,
        &r,
        &provider_reply("zai", "responses", "json", "json"),
    );
    assert_eq!(t.text, "Hello there.");
    assert_eq!(t.model, "glm-asr-2512");

    for (profile, id) in [
        (TranscriptionsProfile::ZAI, "zai"),
        (TranscriptionsProfile::MINIMAX, "minimax"),
    ] {
        let mut s = transcribe(None);
        s.stream = StreamMode::on(true);
        let (_, ctx, _) = sent_parts(profile, &s);
        let mut dec = OpenAiTranscriptions::stream_decoder(&ctx);
        let mut events = dec.feed(&provider_reply(id, "streams", "stream-transcribe", "sse"));
        events.extend(dec.finish());
        let Some(StreamEvent::Transcript(end)) = events.last() else {
            panic!("{events:?}")
        };
        let done = end.done.as_ref().unwrap();
        assert!(
            done.text == "Hello there." || done.text == "Hello everyone",
            "{id}: {}",
            done.text
        );
    }

    let mut m = transcribe(Some(TranscriptFormat::VerboseJson));
    m.model = "asr-1.0".into();
    m.language = Some("en".into());
    m.timestamps = vec![TimestampGranularity::Word];
    let (enc, _, parts) = sent_parts(TranscriptionsProfile::MINIMAX, &m);
    assert_eq!(enc.path, "/v1/speech_to_text");
    assert_eq!(enc.headers.get("language"), Some("en"));
    assert!(!parts.iter().any(|(k, _)| k == "language"));
    assert!(
        has(&parts, "timestamp_level", "word") && has(&parts, "response_format", "verbose_json")
    );
    let t = decoded(
        TranscriptionsProfile::MINIMAX,
        &m,
        &provider_reply("minimax", "responses", "verbose", "json"),
    );
    assert_eq!(t.segments[1].speaker.as_deref(), Some("S2"));
    assert_eq!(t.duration, Some(12.744));
    let j = decoded(
        TranscriptionsProfile::MINIMAX,
        &transcribe(None),
        &provider_reply("minimax", "responses", "json", "json"),
    );
    assert_eq!(j.audio_seconds(), Some(26.325));
}

#[test]
fn xai_openrouter_cohere_and_vercel_read_their_replies() {
    let mut r = transcribe(Some(TranscriptFormat::VerboseJson));
    r.model = "grok-voice-transcribe-2.0".into();
    r.diarize = Some(true);
    let (enc, _, parts) = sent_parts(TranscriptionsProfile::XAI, &r);
    assert_eq!(enc.path, "/v1/stt");
    assert_eq!(
        parts.last().map(|(k, _)| k.as_str()),
        Some("file"),
        "SpaceX wants the file last"
    );
    assert!(
        !parts
            .iter()
            .any(|(k, _)| k.starts_with("timestamp") || k == "response_format")
    );
    let t = decoded(
        TranscriptionsProfile::XAI,
        &r,
        &provider_reply("xai", "responses", "json", "json"),
    );
    assert_eq!(t.words[0].text, "The");
    assert_eq!(t.words[1].speaker.as_deref(), Some("0"));
    assert_eq!(t.duration, Some(8.4));

    let o = decoded(
        TranscriptionsProfile::OPENROUTER,
        &transcribe(None),
        &provider_reply("openrouter", "responses", "json", "json"),
    );
    assert_eq!((o.usage.input_tokens, o.usage_seconds), (83, Some(9.2)));
    let mut link = transcribe(None);
    link.audio = AudioInput::new(Source::Url("https://example.com/a.mp3".into()));
    let (_, _, parts) = sent_parts(TranscriptionsProfile::OPENROUTER, &link);
    assert!(has(&parts, "source_url", "https://example.com/a.mp3"));

    let mut c = transcribe(None);
    c.model = "cohere-transcribe-03-2026".into();
    assert_eq!(refusal(TranscriptionsProfile::COHERE, &c), "language");
    c.language = Some("en".into());
    let (enc, _, parts) = sent_parts(TranscriptionsProfile::COHERE, &c);
    assert_eq!(enc.path, "/v2/audio/transcriptions");
    assert!(has(&parts, "language", "en"));
    let t = decoded(
        TranscriptionsProfile::COHERE,
        &c,
        &provider_reply("cohere", "responses", "json", "json"),
    );
    assert_eq!(t.text, "Hello there.");

    let mut v = transcribe(None);
    v.model = "openai/whisper-1".into();
    let (enc, _, _) = sent_parts(TranscriptionsProfile::VERCEL, &v);
    assert_eq!(enc.path, "/v4/ai/transcription-model");
    assert_eq!(enc.headers.get("ai-model-id"), Some("openai/whisper-1"));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body, json!({"audio": "SUQz", "mediaType": "audio/mpeg"}));
    let t = decoded(
        TranscriptionsProfile::VERCEL,
        &v,
        &provider_reply("vercel", "responses", "json", "json"),
    );
    assert_eq!((t.language.as_deref(), t.duration), (Some("en"), Some(4.2)));
    let mut foreign = v.clone();
    foreign.extras = kaeso_kpp::Extras::new(kaeso_kpp::DialectId::OpenAiTranscriptions).with_field(
        "include",
        kaeso_kpp::RawJson::from_serialize(&json!(["logprobs"])).unwrap(),
    );
    assert_eq!(refusal(TranscriptionsProfile::VERCEL, &foreign), "include");
}
