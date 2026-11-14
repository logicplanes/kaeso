#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Env, call};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";
const GOOGLE_SECRET: &str = "AIzaSyTestGoogleAudioCredential0000000000";
const GROQ_SECRET: &str = "gsk_test-groq-audio-credential-000000000000";
const MISTRAL_SECRET: &str = "mistral-test-audio-credential-00000000";
const MINIMAX_SECRET: &str = "sk-api-test-minimax-audio-0000000000000000";
const ELEVENLABS_SECRET: &str = "sk_test-elevenlabs-audio-00000000000000000";
const VOICE: &str = "hpp4J3VqNfWAUOO0d1Us";

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    for (provider, secret) in [
        ("google", GOOGLE_SECRET),
        ("groq", GROQ_SECRET),
        ("mistral", MISTRAL_SECRET),
        ("minimax", MINIMAX_SECRET),
        ("elevenlabs", ELEVENLABS_SECRET),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/credentials"),
                json!({ "provider": provider, "name": provider, "secret": secret }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{provider}: {:?}", r.body);
    }
    for (provider, model) in [
        ("openai", "gpt-4o-mini-tts"),
        ("openai", "tts-1"),
        ("openai", "whisper-1"),
        ("openai", "gpt-4o-transcribe"),
        ("google", "gemini-3.8-flash-lite-tts"),
        ("google", "gemini-3.5-transcribe"),
        ("groq", "whisper-large-v3"),
        ("mistral", "voxtral-mini-tts-3-0"),
        ("minimax", "speech-2.8-hd"),
        ("anthropic", "claude-haiku-4-5"),
        ("elevenlabs", "eleven_flash_v2_5"),
        ("elevenlabs", "eleven_v3"),
        ("elevenlabs", "eleven_text_to_sound_v2"),
        ("elevenlabs", "scribe_v2"),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert!(
            matches!(r.status, StatusCode::CREATED | StatusCode::CONFLICT),
            "{model}: {:?}",
            r.body
        );
    }
    env
}

async fn speak(env: &Env, body: Value) -> Answer {
    call(env, "/v1/audio/speech", Some(&env.key), false, Some(body)).await
}

async fn upload(env: &Env, path: &str, fields: &[(&str, &str)]) -> Answer {
    upload_file(env, path, fields, ("file", "a.mp3", "audio/mpeg", MP3)).await
}

async fn upload_file(
    env: &Env,
    path: &str,
    fields: &[(&str, &str)],
    (field, filename, media_type, bytes): (&str, &str, &str, &[u8]),
) -> Answer {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!("--kb\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: {media_type}\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri(path)
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("xi-api-key", &env.key)
        .header("content-type", "multipart/form-data; boundary=kb")
        .body(Body::from(body))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

async fn outcome(env: &Env) -> kaeso_server::model_api::hooks::Outcome {
    env.outcomes().await.into_iter().last().unwrap()
}

async fn keep_media(env: &Env) {
    let org = &env.owner.org_id;
    for path in [
        format!("/orgs/{org}/logs/settings"),
        format!("/orgs/{org}/projects/{}/logs/settings", env.project),
    ] {
        let r = env
            .app
            .call(
                axum::http::Method::PATCH,
                &path,
                Some(json!({ "keep_media": true })),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }
}

async fn logged_content(env: &Env) -> Value {
    let o = outcome(env).await;
    let state = &env.app.state;
    let clickhouse = state.clickhouse.as_ref().unwrap();
    clickhouse.migrate(&state.pg).await.unwrap();
    kaeso_server::services::logs::record(state, &o).await;
    kaeso_server::services::logs::flush_all(state, std::time::Duration::from_secs(10)).await;
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/logs/{}",
                env.owner.org_id, env.project, o.request_id
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body.get("content").cloned().unwrap_or_default()
}

#[tokio::test]
async fn an_uploaded_recording_and_spoken_audio_are_logged_whole_only_where_media_is_kept() {
    use base64::Engine as _;
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let env = env().await;
    let fields = [("model", "gpt-4o-transcribe")];
    let r = upload(&env, "/v1/audio/transcriptions", &fields).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    assert_eq!(
        content["request"]["audio"]["source"]["base64"]["data"]["kaeso_media"],
        json!({ "media_type": "audio/mpeg", "bytes": MP3.len(), "reason": "not_kept" })
    );
    assert!(
        content["reply"]["transcription"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Imagine"),
        "{content}"
    );

    let spoken =
        json!({"model": "gpt-4o-mini-tts", "input": "The quick brown fox.", "voice": "alloy"});
    let r = speak(&env, spoken.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    assert_eq!(content["reply"]["format"], 2);
    assert_eq!(
        content["reply"]["speech"]["audio"]["base64"]["data"]["kaeso_media"],
        json!({ "media_type": "audio/mpeg", "bytes": r.body.len(), "reason": "not_kept" }),
        "a streamed reply is logged like one that came whole"
    );

    keep_media(&env).await;
    upload(&env, "/v1/audio/transcriptions", &fields).await;
    let content = logged_content(&env).await;
    assert_eq!(
        content["request"]["audio"]["source"]["base64"]["data"],
        b64(MP3).as_str()
    );
    let r = speak(
        &env,
        json!({"model": "gpt-4o-mini-tts", "input": "The quick brown fox.", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let content = logged_content(&env).await;
    assert_eq!(content["request"]["input"], "The quick brown fox.");
    assert_eq!(
        content["reply"]["speech"]["audio"]["base64"]["data"],
        b64(&r.body).as_str()
    );
}

fn endpoint(o: &kaeso_server::model_api::hooks::Outcome) -> &'static str {
    kaeso_server::services::logs::endpoint(o)
}

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(kaeso_testkit::mock_provider::kpp_fixtures().join(path)).unwrap()
}

#[tokio::test]
async fn openai_speech_comes_back_as_audio_with_its_usage_from_the_events_kaeso_asked_for() {
    let env = env().await;
    let r = speak(
        &env,
        json!({"model": "gpt-4o-mini-tts", "input": "The quick brown fox.", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.headers["content-type"], "audio/mpeg");
    assert_eq!(r.body, fixture("openai_speech/responses/speech.mp3"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/audio/speech");
    assert_eq!(sent.json()["stream_format"], "sse");
    let o = outcome(&env).await;
    assert_eq!(endpoint(&o), "audio/speech");
    assert_eq!(o.usage.as_ref().unwrap().output_audio_tokens, Some(101));
    let media = o.media.clone().unwrap();
    assert_eq!(media.characters, 20);
    assert_eq!(media.logged(), ("character", 20, 0));
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert!(cost > rust_decimal::Decimal::ZERO, "{cost}");
}

#[tokio::test]
async fn an_events_app_gets_speech_events_and_tts_1_is_billed_per_character() {
    let env = env().await;
    let r = speak(&env, json!({"model": "gpt-4o-mini-tts", "input": "Hi", "voice": "alloy", "stream_format": "sse"})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let text = r.text();
    assert_eq!(text.matches("speech.audio.delta").count(), 2, "{text}");
    assert!(text.contains("speech.audio.done"));

    let r = speak(&env, json!({"model": "tts-1", "input": "Hello there.", "voice": "nova", "response_format": "opus"})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.headers["content-type"], "audio/opus");
    assert!(
        env.mock
            .last()
            .unwrap()
            .json()
            .get("stream_format")
            .is_none()
    );
    let o = outcome(&env).await;
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(
        cost,
        "0.00018".parse::<rust_decimal::Decimal>().unwrap(),
        "12 characters at $15 per million"
    );
}

#[tokio::test]
async fn other_providers_speak_in_their_own_forms() {
    let env = env().await;
    let r = speak(
        &env,
        json!({"model": "gemini-3.8-flash-lite-tts", "input": "Hi.", "voice": "Kore"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.headers["content-type"], "audio/mpeg");
    let sent = env.mock.last().unwrap();
    assert!(
        sent.path
            .ends_with("gemini-3.8-flash-lite-tts:generateContent"),
        "{}",
        sent.path
    );
    assert_eq!(
        sent.json()["generationConfig"]["responseFormat"]["audio"]["mimeType"],
        "AUDIO_MP3"
    );

    let r = speak(
        &env,
        json!({"model": "voxtral-mini-tts-3-0", "input": "Hi", "voice": "gb_jane_neutral"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.body.starts_with(b"ID3"));
    assert_eq!(
        env.mock.last().unwrap().json()["voice_id"],
        "gb_jane_neutral"
    );

    let r = speak(
        &env,
        json!({"model": "speech-2.8-hd", "input": "Hi", "voice": "English_CalmWoman"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/t2a_v2");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {MINIMAX_SECRET}").as_str())
    );
    let o = outcome(&env).await;
    assert_eq!(
        o.media.unwrap().characters,
        163,
        "MiniMax's billed characters"
    );
}

// 16 kHz, mono, 16-bit PCM: 32,000 bytes a second.
fn wav(ms: u32) -> Vec<u8> {
    let data = ms * 32;
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    b.extend_from_slice(&16_000u32.to_le_bytes());
    b.extend_from_slice(&32_000u32.to_le_bytes());
    b.extend_from_slice(b"\x02\0\x10\0data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    b
}

#[tokio::test]
async fn audio_length_is_read_from_the_file_when_the_provider_reports_none() {
    let env = env().await;
    let clip = wav(1500);
    let r = upload_file(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-1"), ("response_format", "srt")],
        ("file", "a.wav", "audio/wav", &clip),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env).await;
    let media = o.media.clone().unwrap();
    assert_eq!((media.input_ms, media.measured), (1500, true));
    assert!(kaeso_server::services::usage::cost_known(&o));
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(
        cost,
        "0.00015".parse::<rust_decimal::Decimal>().unwrap(),
        "1.5 s at $0.006 a minute"
    );
    assert_eq!(kaeso_server::services::logs::cost_basis(&o), "measured");

    let r = upload_file(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-1"), ("response_format", "verbose_json")],
        ("file", "a.wav", "audio/wav", &clip),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env).await;
    let media = o.media.clone().unwrap();
    assert_eq!(
        (media.input_ms, media.measured),
        (3320, false),
        "the length the provider reports wins"
    );
    assert_eq!(kaeso_server::services::logs::cost_basis(&o), "");

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-1"), ("response_format", "text")],
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "a file that states no length is still sent"
    );
    let o = outcome(&env).await;
    assert_eq!(o.media.as_ref().unwrap().input_ms, 0);
    assert!(!kaeso_server::services::usage::cost_known(&o));

    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "elevenlabs", "model_id": "eleven_multilingual_sts_v2" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = upload_file(
        &env,
        &format!("/v1/voice-changer/{VOICE}"),
        &[("model_id", "eleven_multilingual_sts_v2")],
        ("audio", "a.wav", "audio/wav", &wav(30_000)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env).await;
    let media = o.media.clone().unwrap();
    assert_eq!((media.input_ms, media.measured), (30_000, true));
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(
        cost,
        "0.06".parse::<rust_decimal::Decimal>().unwrap(),
        "half a minute at $0.12"
    );
}

#[tokio::test]
async fn audio_that_breaks_off_is_an_error_not_a_clean_short_file() {
    let env = env().await;
    let send = |input: &str| {
        Request::builder()
            .uri("/v1/audio/speech")
            .method("POST")
            .header("authorization", format!("Bearer {}", env.key))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"model": "tts-1", "voice": "alloy", "input": input}).to_string(),
            ))
            .unwrap()
    };

    let resp = env
        .app
        .router
        .clone()
        .oneshot(send("Hi. [mock:audio-cut=0]"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    assert!(
        resp.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"]["message"].is_string(), "{error}");
    let outcomes = env.outcomes().await;
    let o = outcomes.last().unwrap();
    assert_eq!(o.status, 502);
    assert_eq!(
        outcomes.iter().filter(|o| o.status == 502).count(),
        1,
        "logged once"
    );

    let resp = env
        .app
        .router
        .clone()
        .oneshot(send("Hi. [mock:audio-cut=4]"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "audio/mpeg");
    assert!(
        resp.into_body().collect().await.is_err(),
        "the body ends in an error after the bytes that arrived"
    );
    assert_eq!(outcome(&env).await.status, 502);
}

#[tokio::test]
async fn transcriptions_take_every_form_and_log_the_audio_length_without_its_bytes() {
    let env = env().await;
    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "gpt-4o-transcribe")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["text"].as_str().unwrap().starts_with("Imagine"));
    let o = outcome(&env).await;
    assert_eq!(endpoint(&o), "audio/transcriptions");
    assert_eq!(o.usage.as_ref().unwrap().input_audio_tokens, Some(14));
    if let Some(logged) = &o.request_body {
        let text = String::from_utf8_lossy(logged);
        assert!(
            !text.contains("ID3") && text.contains(r#""reason":"not_kept""#),
            "{text}"
        );
    }

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-1"), ("response_format", "srt")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    assert!(r.text().starts_with("1\n00:00:00,000"));

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-1"), ("response_format", "verbose_json")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env).await;
    let media = o.media.unwrap();
    assert_eq!(media.logged(), ("millisecond", 3320, 0));
    assert_eq!(media.format.as_deref(), Some("verbose_json"));

    let r = upload(&env, "/v1/audio/translations", &[("model", "whisper-1")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/audio/translations");
    assert_eq!(endpoint(&outcome(&env).await), "audio/translations");

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "gpt-4o-transcribe"), ("stream", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(
        text.contains("transcript.text.delta") && text.contains("transcript.text.done"),
        "{text}"
    );
}

#[tokio::test]
async fn groq_makes_subtitles_kaeso_builds_from_its_segments_and_gemini_transcribes() {
    let env = env().await;
    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "whisper-large-v3"), ("response_format", "vtt")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text()
            .starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:03.320"),
        "{}",
        r.text()
    );
    let sent = env.mock.last().unwrap();
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {GROQ_SECRET}").as_str())
    );
    let body = String::from_utf8_lossy(&sent.body);
    assert!(
        body.contains("verbose_json") && body.contains("segment"),
        "{body}"
    );

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "gemini-3.5-transcribe")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert!(
        sent.path.ends_with("gemini-3.5-transcribe:generateContent"),
        "{}",
        sent.path
    );
    assert_eq!(
        sent.json()["contents"][0]["parts"][0]["inlineData"]["mimeType"],
        "audio/mpeg"
    );
    assert_eq!(r.json()["text"], "Hello, Kizo.");
}

#[tokio::test]
async fn what_a_model_or_provider_cannot_do_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = speak(
        &env,
        json!({"model": "claude-haiku-4-5", "input": "Hi", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("offers no speech API")
    );
    let r = speak(
        &env,
        json!({"model": "gpt-4o", "input": "Hi", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `gpt-4o` does not produce audio."
    );
    let r = speak(
        &env,
        json!({"model": "gemini-3.8-flash-lite-tts", "input": "Hi", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "voice");
    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "gpt-4o-mini-tts")],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = upload(
        &env,
        "/v1/audio/translations",
        &[("model", "gemini-3.5-transcribe")],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(env.mock.received().len(), before);
}

async fn elevenlabs(env: &Env, path: &str, body: Option<Value>) -> Answer {
    elevenlabs_as(env, &env.key, path, body).await
}

async fn elevenlabs_as(env: &Env, key: &str, path: &str, body: Option<Value>) -> Answer {
    let req = Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" })
        .header("xi-api-key", key)
        .header("content-type", "application/json")
        .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

#[tokio::test]
async fn an_elevenlabs_app_speaks_through_kaeso_with_its_own_paths_and_is_billed_per_character() {
    let env = env().await;
    let r = elevenlabs(
        &env,
        &format!("/v1/text-to-speech/{VOICE}?output_format=mp3_44100_128"),
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.headers["content-type"], "audio/mpeg");
    assert_eq!(r.body, fixture("elevenlabs/responses/recorded-speech.mp3"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, format!("/v1/text-to-speech/{VOICE}"));
    assert_eq!(sent.query.as_deref(), Some("output_format=mp3_44100_128"));
    assert_eq!(sent.header("xi-api-key"), Some(ELEVENLABS_SECRET));
    let o = outcome(&env).await;
    assert_eq!(endpoint(&o), "text-to-speech");
    assert_eq!(
        o.media.as_ref().unwrap().characters,
        2,
        "ElevenLabs' `character-cost`"
    );
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(cost, "0.00008".parse::<rust_decimal::Decimal>().unwrap());

    let r = elevenlabs(
        &env,
        &format!("/v1/text-to-speech/{VOICE}/with-timestamps"),
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["alignment"]["characters"], json!(["H", "i", "."]));

    let r = elevenlabs(
        &env,
        &format!("/v1/text-to-speech/{VOICE}/stream/with-timestamps"),
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.text()
            .lines()
            .filter(|l| l.contains("audio_base64"))
            .count(),
        2
    );

    let r = elevenlabs(
        &env,
        "/v1/sound-generation",
        Some(json!({"text": "A short click.", "duration_seconds": 0.5})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let o = outcome(&env).await;
    assert_eq!(endpoint(&o), "sound-generation");
    assert_eq!(o.media.unwrap().output_ms, 500);

    let r = elevenlabs(
        &env,
        "/v1/text-to-dialogue",
        Some(json!({"inputs": [{"text": "Hi.", "voice_id": VOICE}]})),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    let r = elevenlabs(&env, "/v1/models", None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.json()
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["model_id"] == "eleven_flash_v2_5")
    );

    let r = elevenlabs(&env, "/v2/voices?page_size=3", None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["voices"][0]["voice_id"], VOICE);
    assert_eq!(
        env.mock.last().unwrap().header("xi-api-key"),
        Some(ELEVENLABS_SECRET)
    );
}

#[tokio::test]
async fn management_calls_and_unknown_voices_are_refused_in_elevenlabs_shape() {
    let env = env().await;
    let before = env.mock.received().len();
    for path in ["/v1/dubbing", "/v1/user/subscription", "/v1/voices/add"] {
        let r = elevenlabs(&env, path, Some(json!({}))).await;
        assert!(r.status.is_client_error(), "{path}: {}", r.text());
        assert!(
            r.json()["detail"]["message"].is_string(),
            "{path}: {}",
            r.text()
        );
    }
    let r = elevenlabs(&env, "/v2/history", None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(
        r.json()["detail"]["message"]
            .as_str()
            .unwrap()
            .contains("ElevenLabs")
    );
    let r = elevenlabs(
        &env,
        "/v1/text-to-speech/x",
        Some(json!({"text": "Hi.", "model_id": "claude-haiku-4-5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.json()["detail"]["message"].is_string());
    let r = speak(
        &env,
        json!({"model": "eleven_flash_v2_5", "input": "Hi", "voice": "alloy"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "voice");
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn an_openai_app_reaches_elevenlabs_by_voice_id_and_scribe_by_upload() {
    let env = env().await;
    let r = speak(&env, json!({"model": "eleven_flash_v2_5", "input": "Hi.", "voice": VOICE, "response_format": "pcm"})).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.headers["content-type"], "audio/pcm");
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, format!("/v1/text-to-speech/{VOICE}/stream"));
    assert_eq!(sent.query.as_deref(), Some("output_format=pcm_24000"));
    assert_eq!(
        r.body,
        fixture("elevenlabs/responses/recorded-speech-stream.pcm")
    );

    let r = upload(
        &env,
        "/v1/audio/transcriptions",
        &[("model", "scribe_v2"), ("response_format", "verbose_json")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(body["text"], "Hello, Kizo");
    assert_eq!(body["words"].as_array().unwrap().len(), 2);
    assert_eq!(env.mock.last().unwrap().path, "/v1/speech-to-text");
    let o = outcome(&env).await;
    assert!(o.media.unwrap().input_ms > 0);
}

#[tokio::test]
async fn a_voice_id_that_would_leave_its_path_never_reaches_elevenlabs() {
    let env = env().await;
    let before = env.mock.received().len();
    for voice in ["x/../../../v1/user?a=#", "..", "x%2F..%2F..%2Fv1%2Fuser"] {
        let r = speak(
            &env,
            json!({"model": "eleven_flash_v2_5", "input": "Hi.", "voice": {"id": voice}}),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{voice}: {}", r.text());
        assert_eq!(r.json()["error"]["param"], "voice", "{voice}");
    }
    let r = elevenlabs(
        &env,
        "/v1/text-to-speech/..%2F..%2Fv1%2Fuser",
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(r.json()["detail"]["message"].is_string(), "{}", r.text());
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn a_request_refused_before_the_provider_costs_nothing() {
    let env = common::v1::env(
        Some(std::sync::Arc::new(common::v1::Capture::default())),
        None,
    )
    .await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "elevenlabs", "model_id": "eleven_flash_v2_5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let before = env.mock.received().len();
    let r = elevenlabs(
        &env,
        &format!("/v1/text-to-speech/{VOICE}"),
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(env.mock.received().len(), before);
    let o = outcome(&env).await;
    assert_eq!(
        o.media.as_ref().unwrap().characters,
        3,
        "what was asked is still logged"
    );
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(cost, rust_decimal::Decimal::ZERO);
    assert!(kaeso_server::services::usage::cost_known(&o));
    assert_eq!(kaeso_server::services::usage::media_sums(&o).characters, 0);

    let r = elevenlabs(
        &env,
        "/v1/text-to-speech/a_voice",
        Some(json!({"text": "Hi.", "model_id": "eleven_flash_v2_5"})),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["detail"]["param"],
        "voice_id",
        "the request's shape is checked before its credential: {}",
        r.text()
    );
}

#[tokio::test]
async fn a_request_refused_before_its_model_is_known_logs_no_media() {
    let env = env().await;
    let r = speak(
        &env,
        json!({"model": "no-such-tts", "voice": "alloy", "input": "Hello there."}),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    let o = outcome(&env).await;
    assert_eq!(o.model_id, None);
    assert_eq!(o.media, None, "{:?}", o.media);
    assert_eq!(kaeso_server::services::usage::media_sums(&o).characters, 0);
}

#[tokio::test]
async fn voice_listings_count_against_the_keys_request_rate() {
    let env = env().await;
    let org = &env.owner.org_id;
    let base = format!("/orgs/{org}/projects/{}", env.project);
    let key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "rated" }),
            Some(&env.owner.cookie),
        )
        .await
        .body;
    let r = env
        .app
        .post(
            &format!("{base}/limits"),
            json!({ "level": "key", "key": key["id"], "kind": "request_rate", "period": "minute", "amount": 2 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let rated = key["secret"].as_str().unwrap();
    for _ in 0..2 {
        let r = elevenlabs_as(&env, rated, "/v2/voices", None).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    let sent = env.mock.received().len();
    let r = elevenlabs_as(&env, rated, "/v1/voices", None).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(r.headers.contains_key("retry-after"));
    assert_eq!(env.mock.received().len(), sent, "ElevenLabs was not asked");
    let r = elevenlabs(&env, "/v1/voices", None).await;
    assert_eq!(r.status, StatusCode::OK, "another key is not held back");
}
