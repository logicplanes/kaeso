#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env, call};
use http_body_util::BodyExt;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use tower::ServiceExt;

const SECRET: &str = "sk_car_kaesotest000000000000000000";
const VOICE: &str = "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "cartesia", "name": "cartesia", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in ["sonic-3.6", "ink-whisper"] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "cartesia", "model_id": model }),
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

fn sent_to_cartesia(env: &Env, path: &str) -> kaeso_testkit::mock_provider::Received {
    let sent = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path == path)
        .unwrap();
    assert_eq!(sent.header("x-api-key"), Some(SECRET));
    assert_eq!(
        sent.header("cartesia-version"),
        Some(kaeso_codecs::cartesia::VERSION)
    );
    assert_eq!(sent.header("authorization"), None);
    sent
}

fn cost(o: &kaeso_server::model_api::hooks::Outcome) -> Decimal {
    kaeso_server::services::usage::costed(o).1
}

#[tokio::test]
async fn openai_speech_reaches_sonic_with_its_voice_id_and_is_billed_per_character() {
    let env = env().await;
    let r = speak(
        &env,
        json!({ "model": "sonic-3.6", "input": "Hello there.", "voice": VOICE, "response_format": "wav", "speed": 1.2 }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!r.body.is_empty());
    let sent = sent_to_cartesia(&env, "/tts/bytes");
    assert_eq!(
        sent.json(),
        json!({
            "model_id": "sonic-3.6",
            "transcript": "Hello there.",
            "voice": VOICE,
            "output_format": {"container": "wav", "encoding": "pcm_s16le", "sample_rate": 24000},
            "generation_config": {"speed": 1.2},
        })
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("cartesia"));
    assert_eq!(o.media.as_ref().unwrap().characters, 12);
    assert_eq!(cost(&o), "0.00078".parse::<Decimal>().unwrap());
}

#[tokio::test]
async fn an_unknown_voice_reaches_the_app_as_cartesias_not_found_error() {
    let env = env().await;
    let r = speak(
        &env,
        json!({ "model": "sonic-3.6", "input": "Hi [mock:error=voice-not-found-404-built]", "voice": "alloy" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], "Voice not found.");
    assert_eq!(
        sent_to_cartesia(&env, "/tts/bytes").json()["voice"],
        "alloy"
    );
}

#[tokio::test]
async fn what_cartesia_cannot_take_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    for (body, param) in [
        (
            json!({ "model": "sonic-3.6", "input": "Hi", "voice": VOICE, "response_format": "opus" }),
            "response_format",
        ),
        (
            json!({ "model": "sonic-3.6", "input": "Hi", "voice": VOICE, "speed": 2.0 }),
            "speed",
        ),
        (
            json!({ "model": "sonic-3.6", "input": "Hi", "voice": VOICE, "instructions": "Whisper." }),
            "instructions",
        ),
    ] {
        let r = speak(&env, body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["param"], param, "{}", r.text());
    }
    let r = upload(&env, &[("model", "ink-whisper")], MP3).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "language");
    let r = upload(
        &env,
        &[
            ("model", "ink-whisper"),
            ("language", "en"),
            ("response_format", "diarized_json"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn a_transcription_is_an_stt_upload_with_words_and_is_billed_by_its_length() {
    let env = env().await;
    let r = upload(
        &env,
        &[
            ("model", "ink-whisper"),
            ("language", "en"),
            ("response_format", "verbose_json"),
            ("timestamp_granularities[]", "word"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["text"], " Hello world");
    assert_eq!(reply["words"][0]["word"], " Hello");
    let sent = sent_to_cartesia(&env, "/stt");
    let body = String::from_utf8_lossy(&sent.body).into_owned();
    for part in [
        "name=\"model\"\r\n\r\nink-whisper",
        "name=\"language\"\r\n\r\nen",
        "name=\"timestamp_granularities[]\"\r\n\r\nword",
        "name=\"file\"",
    ] {
        assert!(body.contains(part), "{part} in {body}");
    }
    assert!(!body.contains("response_format"), "{body}");
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("cartesia"));
    assert!(cost(&o) > Decimal::ZERO, "{o:?}");
}

#[tokio::test]
async fn subtitles_are_written_from_cartesias_timed_words() {
    let env = env().await;
    let r = upload(
        &env,
        &[
            ("model", "ink-whisper"),
            ("language", "en"),
            ("response_format", "srt"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text().contains("00:00:00,000 --> 00:00:00,900"),
        "{}",
        r.text()
    );
    assert!(r.text().contains("Hello world"), "{}", r.text());
}
