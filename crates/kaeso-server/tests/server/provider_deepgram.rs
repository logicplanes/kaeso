#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env, call};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const SECRET: &str = "deepgram-test-credential-0001";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "deepgram", "name": "deepgram", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in ["nova-3", "whisper-large", "aura-2", "aura"] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "deepgram", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

async fn upload(env: &Env, fields: &[(&str, &str)], file: &[u8]) -> Answer {
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
    body.extend_from_slice(file);
    body.extend_from_slice(b"\r\n--kb--\r\n");
    let req = Request::builder()
        .uri("/v1/audio/transcriptions")
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

async fn speak(env: &Env, body: Value) -> Answer {
    call(env, "/v1/audio/speech", Some(&env.key), false, Some(body)).await
}

fn sent_to_deepgram(env: &Env, path: &str) -> kaeso_testkit::mock_provider::Received {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Token {SECRET}").as_str())
    );
    sent
}

fn speech_fixture() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../kaeso-codecs/fixtures/deepgram/responses/speak.mp3"),
    )
    .unwrap()
}

#[tokio::test]
async fn openai_apps_transcribe_through_deepgram_with_the_audio_as_the_body() {
    let env = env().await;
    let r = upload(
        &env,
        &[("model", "nova-3"), ("response_format", "verbose_json")],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("deepgram"));
    let body = r.json();
    assert_eq!(body["text"], "Four score and seven years ago. Yes.");
    assert_eq!(body["language"], "en");
    assert_eq!(body["duration"], 4.1);
    assert_eq!(body["words"][0]["word"], "Four");
    assert_eq!(body["segments"][1]["text"], "Yes.");
    assert!(body.get("metadata").is_none(), "{body}");
    let sent = sent_to_deepgram(&env, "/v1/listen");
    assert_eq!(
        sent.query.as_deref(),
        Some("model=nova-3&detect_language=true&smart_format=true&utterances=true")
    );
    assert_eq!(sent.header("content-type"), Some("audio/mpeg"));
    assert_eq!(&sent.body[..], MP3, "the upload itself, without its form");
    let o = env.outcomes().await.pop().unwrap();
    let media = o.media.clone().unwrap();
    assert_eq!(
        (media.input_ms, media.measured),
        (4100, false),
        "Deepgram's `metadata.duration`"
    );
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    let low = "0.00029".parse::<rust_decimal::Decimal>().unwrap();
    let high = "0.0003".parse::<rust_decimal::Decimal>().unwrap();
    assert!(
        cost > low && cost < high,
        "4.1 s at $0.0043 a minute: {cost}"
    );

    let r = upload(
        &env,
        &[("model", "nova-3"), ("response_format", "srt")],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.header("content-type").unwrap().starts_with("text/plain"));
    assert!(
        r.text()
            .starts_with("1\n00:00:00,419 --> 00:00:02,914\nFour score and seven years ago.\n"),
        "{}",
        r.text()
    );

    let r = upload(
        &env,
        &[
            ("model", "nova-3"),
            ("response_format", "diarized_json"),
            ("language", "en"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["segments"][1]["speaker"], "1");
    let sent = sent_to_deepgram(&env, "/v1/listen");
    assert_eq!(
        sent.query.as_deref(),
        Some("model=nova-3&language=en&smart_format=true&utterances=true&diarize_model=latest")
    );

    let r = upload(&env, &[("model", "whisper-large")], MP3).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json(),
        json!({"text": "Four score and seven years ago. Yes.", "usage": {"type": "duration", "seconds": 4.1}})
    );
}

#[tokio::test]
async fn openai_apps_speak_through_deepgram_with_the_voice_in_the_model() {
    let env = env().await;
    let r = speak(
        &env,
        json!({"model": "aura-2", "voice": "thalia", "input": "Hello there."}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("content-type"), Some("audio/mpeg"));
    assert_eq!(r.body, speech_fixture());
    let sent = sent_to_deepgram(&env, "/v1/speak");
    assert_eq!(
        sent.query.as_deref(),
        Some("model=aura-2-thalia-en&encoding=mp3")
    );
    assert_eq!(sent.json(), json!({"text": "Hello there."}));
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(
        o.media.as_ref().unwrap().characters,
        32,
        "Deepgram's `dg-char-count`"
    );
    let (_, cost) = kaeso_server::services::usage::costed(&o);
    assert_eq!(
        cost,
        "0.00096".parse::<rust_decimal::Decimal>().unwrap(),
        "32 characters at $30 a million"
    );

    let r = speak(
        &env,
        json!({"model": "aura", "voice": "asteria", "input": "Hi.", "response_format": "pcm"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("content-type"), Some("audio/pcm"));
    let sent = sent_to_deepgram(&env, "/v1/speak");
    assert_eq!(
        sent.query.as_deref(),
        Some("model=aura-asteria-en&encoding=linear16&container=none&sample_rate=24000")
    );

    let r = speak(
        &env,
        json!({"model": "aura-2", "voice": "aura-2-agustina-es", "input": "Hola.", "stream_format": "sse"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(text.contains("speech.audio.delta") && text.contains("speech.audio.done"));
    let sent = sent_to_deepgram(&env, "/v1/speak");
    assert_eq!(
        sent.query.as_deref(),
        Some("model=aura-2-agustina-es&encoding=mp3")
    );
}

#[tokio::test]
async fn what_deepgram_cannot_do_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    for (body, param) in [
        (
            json!({"model": "aura", "voice": "aura-2-thalia-en", "input": "Hi."}),
            "voice",
        ),
        (
            json!({"model": "aura-2", "voice": "thalia", "input": "a".repeat(2001)}),
            "input",
        ),
        (
            json!({"model": "aura-2", "voice": "thalia", "input": "Hi.", "speed": 2.0}),
            "speed",
        ),
        (
            json!({"model": "aura-2", "voice": "thalia", "input": "Hi.", "instructions": "Whisper."}),
            "instructions",
        ),
    ] {
        let r = speak(&env, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{param}: {}", r.text());
        assert_eq!(r.json()["error"]["param"], param, "{}", r.text());
    }
    for (fields, param) in [
        (vec![("model", "nova-3"), ("prompt", "Kaeso")], "prompt"),
        (
            vec![("model", "nova-3"), ("temperature", "0.2")],
            "temperature",
        ),
    ] {
        let r = upload(&env, &fields, MP3).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{param}: {}", r.text());
        assert_eq!(r.json()["error"]["param"], param, "{}", r.text());
    }
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn deepgram_refusals_reach_apps_as_clear_errors() {
    let env = env().await;
    let r = upload(
        &env,
        &[("model", "nova-3")],
        b"[mock:error=corrupt-audio-doc-400]",
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Bad Request: failed to process audio: corrupt or unsupported data"
    );
    let r = speak(
        &env,
        json!({"model": "aura-2", "voice": "thalia", "input": "Hi [mock:error=too-many-requests-doc-429]"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let r = speak(
        &env,
        json!({"model": "aura-2", "voice": "thalia", "input": "Hi [mock:error=recorded-invalid-auth-401]"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("deepgram"));
    assert!(!format!("{o:?}").contains(SECRET));
}
