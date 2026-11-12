#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::io::Write as _;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::v1::{Answer, Capture, Env, chat, messages};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum App {
    OpenAi,
    Anthropic,
    Gemini,
}

async fn send(env: &Env, path: &str, app: App, encoding: &str, body: Vec<u8>) -> Answer {
    let mut req = Request::builder()
        .uri(path)
        .method("POST")
        .header("content-type", "application/json");
    if !encoding.is_empty() {
        req = req.header("content-encoding", encoding);
    }
    req = match app {
        App::OpenAi => req.header("authorization", format!("Bearer {}", env.key)),
        App::Anthropic => req
            .header("x-api-key", &env.key)
            .header("anthropic-version", "2023-06-01"),
        App::Gemini => req.header("x-goog-api-key", &env.key),
    };
    let resp = env
        .app
        .router
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
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

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn zstd(data: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest)
}

fn encoded(encoding: &str, data: &[u8]) -> Vec<u8> {
    match encoding {
        "gzip" | "x-gzip" => gzip(data),
        "zstd" => zstd(data),
        _ => data.to_vec(),
    }
}

async fn env(extra: &[(&str, &str)]) -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), Some(extra)).await
}

async fn log_content(env: &Env) {
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/logs/settings",
                env.owner.org_id, env.project
            ),
            Some(json!({ "content_logging": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.body["effective_content_logging"], true, "{:?}", r.body);
}

fn openai_error(r: &Answer, status: StatusCode) -> (String, String) {
    assert_eq!(r.status, status, "{}", r.text());
    let error = &r.json()["error"];
    (
        error["type"].as_str().unwrap().to_owned(),
        error["message"].as_str().unwrap().to_owned(),
    )
}

fn anthropic_error(r: &Answer, status: StatusCode) -> (String, String) {
    assert_eq!(r.status, status, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error", "{body}");
    (
        body["error"]["type"].as_str().unwrap().to_owned(),
        body["error"]["message"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn gzip_and_zstd_bodies_reach_the_provider_decoded_on_every_route() {
    let env = env(&[]).await;
    log_content(&env).await;
    let long = "Kaeso reads compressed bodies. ".repeat(200);
    let routes: [(&str, App, Value); 4] = [
        ("/v1/chat/completions", App::OpenAi, chat("gpt-4o", &long)),
        (
            "/v1/embeddings",
            App::OpenAi,
            json!({ "model": "text-embedding-3-small", "input": long }),
        ),
        (
            "/v1/messages",
            App::Anthropic,
            messages("claude-haiku-4-5", &long),
        ),
        (
            "/v1beta/models/claude-haiku-4-5:generateContent",
            App::Gemini,
            json!({ "contents": [{ "role": "user", "parts": [{ "text": long }] }] }),
        ),
    ];
    for (path, app, body) in routes {
        let plain = serde_json::to_vec(&body).unwrap();
        for encoding in ["gzip", "x-gzip", "zstd", "identity"] {
            env.mock.clear();
            let sent = encoded(encoding, &plain);
            if encoding != "identity" {
                assert!(sent.len() < plain.len() / 4, "{encoding}: {}", sent.len());
            }
            let r = send(&env, path, app, encoding, sent).await;
            assert_eq!(r.status, StatusCode::OK, "{path} {encoding}: {}", r.text());
            let upstream = env.mock.last().unwrap();
            assert_eq!(upstream.header("content-encoding"), None, "{path}");
            let forwarded = String::from_utf8_lossy(&upstream.body);
            assert!(
                forwarded.contains("Kaeso reads compressed bodies."),
                "{path} {encoding}: {forwarded}"
            );
            let id = r.header("kaeso-request-id").unwrap();
            let o = env.capture.recorded(id).await;
            assert_eq!(
                o.request_body.as_deref(),
                Some(plain.as_slice()),
                "{path} {encoding}: the log keeps the decoded body"
            );
            assert!(o.upload.is_some());
        }
    }
}

#[tokio::test]
async fn a_body_that_decodes_past_the_limit_is_refused_with_413() {
    let env = env(&[("KAESO_BODY_LIMITS_MODEL_API", "1MiB")]).await;
    let text = "a".repeat(4 << 20);
    for (path, app, body) in [
        ("/v1/chat/completions", App::OpenAi, chat("gpt-4o", &text)),
        (
            "/v1/messages",
            App::Anthropic,
            messages("claude-haiku-4-5", &text),
        ),
    ] {
        let plain = serde_json::to_vec(&body).unwrap();
        for encoding in ["gzip", "zstd"] {
            env.mock.clear();
            let bomb = encoded(encoding, &plain);
            assert!(bomb.len() < 64 << 10, "{encoding}: {} bytes", bomb.len());
            let r = send(&env, path, app, encoding, bomb).await;
            let (kind, message) = match app {
                App::Anthropic => anthropic_error(&r, StatusCode::PAYLOAD_TOO_LARGE),
                _ => openai_error(&r, StatusCode::PAYLOAD_TOO_LARGE),
            };
            let expected = match app {
                App::Anthropic => "request_too_large",
                _ => "invalid_request_error",
            };
            assert_eq!(kind, expected, "{path} {encoding}");
            assert_eq!(
                message,
                "The request body is larger than 1048576 bytes once decoded."
            );
            assert!(env.mock.received().is_empty());
        }
    }
    assert_eq!(
        env.app.state.translating.free(),
        env.app.state.config.body_limits.in_flight.0
    );
    let small = serde_json::to_vec(&chat("gpt-4o", "Hi")).unwrap();
    let r = send(
        &env,
        "/v1/chat/completions",
        App::OpenAi,
        "zstd",
        zstd(&small),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

#[tokio::test]
async fn other_and_stacked_encodings_are_refused_with_415_in_the_apps_format() {
    let env = env(&[]).await;
    env.mock.clear();
    let plain = serde_json::to_vec(&chat("gpt-4o", "Hi")).unwrap();

    let r = send(
        &env,
        "/v1/chat/completions",
        App::OpenAi,
        "br",
        plain.clone(),
    )
    .await;
    assert_eq!(
        openai_error(&r, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            "invalid_request_error".into(),
            "Kaeso reads request bodies sent as `gzip` or `zstd`, not `br`.".into()
        )
    );

    let r = send(
        &env,
        "/v1/embeddings",
        App::OpenAi,
        "deflate",
        plain.clone(),
    )
    .await;
    assert_eq!(
        openai_error(&r, StatusCode::UNSUPPORTED_MEDIA_TYPE).1,
        "Kaeso reads request bodies sent as `gzip` or `zstd`, not `deflate`."
    );

    let stacked = zstd(&gzip(
        &serde_json::to_vec(&messages("claude-haiku-4-5", "Hi")).unwrap(),
    ));
    let r = send(&env, "/v1/messages", App::Anthropic, "gzip, zstd", stacked).await;
    assert_eq!(
        anthropic_error(&r, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            "invalid_request_error".into(),
            "Kaeso reads request bodies with one content encoding, not `gzip, zstd`.".into()
        )
    );

    let r = send(
        &env,
        "/v1beta/models/claude-haiku-4-5:generateContent",
        App::Gemini,
        "br",
        b"{}".to_vec(),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["code"], 415, "{body}");
    assert_eq!(body["error"]["status"], "INVALID_ARGUMENT", "{body}");
    assert_eq!(
        body["error"]["message"],
        "Kaeso reads request bodies sent as `gzip` or `zstd`, not `br`."
    );

    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_corrupt_body_is_refused_with_400() {
    let env = env(&[]).await;
    env.mock.clear();
    let plain = serde_json::to_vec(&chat("gpt-4o", "Hi")).unwrap();
    let r = send(
        &env,
        "/v1/chat/completions",
        App::OpenAi,
        "zstd",
        plain.clone(),
    )
    .await;
    assert_eq!(
        openai_error(&r, StatusCode::BAD_REQUEST),
        (
            "invalid_request_error".into(),
            "The request body is not valid `zstd`; check its `Content-Encoding`.".into()
        )
    );
    let mut cut = gzip(&serde_json::to_vec(&messages("claude-haiku-4-5", "Hi")).unwrap());
    cut.truncate(cut.len() - 4);
    let r = send(&env, "/v1/messages", App::Anthropic, "gzip", cut).await;
    assert_eq!(
        anthropic_error(&r, StatusCode::BAD_REQUEST),
        (
            "invalid_request_error".into(),
            "The request body is not valid `gzip`; check its `Content-Encoding`.".into()
        )
    );
    assert!(env.mock.received().is_empty());
}
