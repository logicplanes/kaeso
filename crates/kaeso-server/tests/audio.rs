#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

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

async fn env() -> Env {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    for (provider, secret) in [
        ("google", GOOGLE_SECRET),
        ("groq", GROQ_SECRET),
        ("mistral", MISTRAL_SECRET),
        ("minimax", MINIMAX_SECRET),
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
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--kb\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        b"--kb\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n",
    );
    body.extend_from_slice(MP3);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri(path)
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
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
            !text.contains("ID3") && text.contains("not logged"),
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
