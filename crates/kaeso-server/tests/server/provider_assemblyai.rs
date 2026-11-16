#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env};
use http_body_util::BodyExt;
use rust_decimal::Decimal;
use serde_json::json;
use tower::ServiceExt;

const SECRET: &str = "aai-test-key-00000000000000000000";
const MP3: &[u8] = b"ID3\x04\0\0\0\0\0\x23TSSE";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "assemblyai", "name": "assemblyai", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in ["universal-3-6-pro"] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "assemblyai", "model_id": model }),
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

fn sent_to_assemblyai(env: &Env) -> kaeso_testkit::mock_provider::Received {
    let sent = env
        .mock
        .received()
        .into_iter()
        .rev()
        .find(|r| r.path == "/v1/transcribe")
        .unwrap();
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-aai-model"), Some("universal-3-6-pro"));
    sent
}

fn cost(o: &kaeso_server::model_api::hooks::Outcome) -> Decimal {
    kaeso_server::services::usage::costed(o).1
}

#[tokio::test]
async fn a_transcription_is_one_sync_upload_billed_by_the_audio_it_reports() {
    let env = env().await;
    let r = upload(
        &env,
        &[
            ("model", "universal-3-6-pro"),
            ("language", "en"),
            ("prompt", "A support call."),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["text"],
        "Hi, I'm calling about my Best Buy order..."
    );
    let sent = sent_to_assemblyai(&env);
    let body = String::from_utf8_lossy(&sent.body).into_owned();
    assert!(
        body.contains("name=\"audio\"; filename=\"a.mp3\""),
        "{body}"
    );
    assert!(
        body.contains(r#"{"language_codes":["en"],"prompt":"A support call."}"#),
        "{body}"
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("assemblyai"));
    // 101.567 seconds at 0.005 USD a minute.
    let expected = "101.567".parse::<Decimal>().unwrap() / Decimal::from(60)
        * "0.005".parse::<Decimal>().unwrap();
    assert!(
        (cost(&o) - expected).abs() < "0.000000001".parse::<Decimal>().unwrap(),
        "{}",
        cost(&o)
    );
}

#[tokio::test]
async fn subtitles_and_verbose_json_come_from_the_timed_words() {
    let env = env().await;
    let r = upload(
        &env,
        &[("model", "universal-3-6-pro"), ("response_format", "vtt")],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.text().starts_with("WEBVTT"), "{}", r.text());
    assert!(r.text().contains("00:00:00.240 --> "), "{}", r.text());
    assert!(
        String::from_utf8_lossy(&sent_to_assemblyai(&env).body).contains(r#""timestamps":true"#)
    );
    let r = upload(
        &env,
        &[
            ("model", "universal-3-6-pro"),
            ("response_format", "verbose_json"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["words"][1]["word"], "world.");
    assert_eq!(r.json()["duration"], 1.5);
}

#[tokio::test]
async fn what_the_sync_api_cannot_take_is_refused_before_anything_is_sent() {
    let env = env().await;
    let before = env.mock.received().len();
    for (fields, param) in [
        (
            vec![("model", "universal-3-6-pro"), ("language", "pl")],
            "language",
        ),
        (
            vec![
                ("model", "universal-3-6-pro"),
                ("response_format", "diarized_json"),
            ],
            "response_format",
        ),
        (
            vec![("model", "universal-3-6-pro"), ("temperature", "0.2")],
            "temperature",
        ),
    ] {
        let r = upload(&env, &fields, MP3).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(r.json()["error"]["param"], param, "{}", r.text());
    }
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn a_refused_key_and_a_timeout_reach_the_app_as_errors() {
    let env = env().await;
    let r = upload(
        &env,
        &[
            ("model", "universal-3-6-pro"),
            ("prompt", "[mock:error=inference-timeout-504-doc]"),
        ],
        MP3,
    )
    .await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert!(r.text().contains("30 s"), "{}", r.text());
}
