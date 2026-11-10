#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

// The only test in its binary: the allocator counts every thread of the process, the mock
// provider's included.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use base64::Engine as _;
use bytes::Bytes;
use http_body_util::BodyExt;
use peak_alloc::PeakAlloc;
use serde_json::json;
use tower::ServiceExt;

#[global_allocator]
static ALLOC: PeakAlloc = PeakAlloc;

const IMAGE_BYTES: usize = 4 << 20;

fn image() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut x: u32 = 0x9e37_79b9;
    while bytes.len() < IMAGE_BYTES {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push(x.to_le_bytes()[0]);
    }
    bytes
}

fn chat(model: &str, b64: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "model": model,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "What is in this image?"},
            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
        ]}],
    }))
    .unwrap()
}

fn messages(model: &str, b64: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "model": model,
        "max_tokens": 64,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "What is in this image?"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}},
        ]}],
    }))
    .unwrap()
}

async fn settings(env: &common::v1::Env, path: &str, content: bool, media: bool) {
    let r = env
        .app
        .call(
            Method::PATCH,
            path,
            Some(json!({ "content_logging": content, "keep_media": media })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_large_inline_image_is_held_decoded_once_beside_the_body_read_and_the_body_sent() {
    let env = common::v1::env(None, None).await;
    let org = env.owner.org_id.clone();
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "openrouter", "name": "openrouter", "secret": "sk-or-v1-memory-test-0001" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "openrouter", "model_id": "openai/gpt-4o" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let b64 = base64::engine::general_purpose::STANDARD.encode(image());
    let paths = [
        (
            "Chat to Chat",
            "/v1/chat/completions",
            chat("openrouter/openai/gpt-4o", &b64),
        ),
        (
            "Chat to Responses",
            "/v1/chat/completions",
            chat("gpt-4o", &b64),
        ),
        (
            "Messages to Messages",
            "/v1/messages",
            messages("claude-haiku-4-5", &b64),
        ),
        (
            "Messages to Responses",
            "/v1/messages",
            messages("gpt-4o", &b64),
        ),
    ];
    drop(b64);
    let org_logs = format!("/orgs/{org}/logs/settings");
    let project_logs = format!("/orgs/{org}/projects/{}/logs/settings", env.project);
    for (logging, content, media) in [
        ("without content logging", false, false),
        ("with content logging", true, false),
        ("keeping media", true, true),
    ] {
        settings(&env, &org_logs, true, media).await;
        settings(&env, &project_logs, content, media).await;
        for (path, uri, body) in &paths {
            let body = Bytes::from(body.clone());
            let size = body.len();
            let mut req = Request::builder()
                .uri(*uri)
                .method("POST")
                .header("content-type", "application/json");
            req = if *uri == "/v1/messages" {
                req.header("x-api-key", &env.key)
                    .header("anthropic-version", "2023-06-01")
            } else {
                req.header("authorization", format!("Bearer {}", env.key))
            };
            let req = req.body(Body::from(body)).unwrap();
            env.mock.clear();
            let before = ALLOC.current_usage();
            ALLOC.reset_peak_usage();
            let resp = env.app.router.clone().oneshot(req).await.unwrap();
            let status = resp.status();
            let reply = resp.into_body().collect().await.unwrap().to_bytes();
            env.app.state.in_flight.settled().await;
            let peak = ALLOC.peak_usage().saturating_sub(before);
            assert_eq!(status, StatusCode::OK, "{path} {logging}: {reply:?}");
            let sent = env.mock.last().unwrap().body.len();
            assert!(
                sent > IMAGE_BYTES * 4 / 3,
                "{path}: the image reached the provider"
            );
            // The gateway holds the body and the decoded image while it decodes, then the image
            // and the body it sends: 1.75 times the body. The mock provider in this process
            // reads and keeps the body it is sent, about twice its size, and a log that keeps
            // the image holds the body's text once more. Before, the gateway alone held 4.7 to
            // 7.8 times the body.
            let bound = if media { size * 11 / 2 } else { size * 9 / 2 };
            assert!(
                peak <= bound,
                "{path} {logging}: {peak} bytes at the peak for a body of {size}"
            );
        }
    }
}
