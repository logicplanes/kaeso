#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use common::v1::{Answer, Capture, Env, call, chat, messages};
use kaeso_server::model_api::hooks::Outcome;
use serde_json::json;

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

fn server_timing(r: &Answer) -> HashMap<String, f64> {
    let header = r.header("server-timing").expect("a Server-Timing header");
    header
        .split(',')
        .map(|metric| {
            let (name, dur) = metric.trim().split_once(";dur=").unwrap();
            (name.to_owned(), dur.parse::<f64>().unwrap())
        })
        .collect()
}

async fn outcome_of(env: &Env, r: &Answer) -> Outcome {
    let id = r.header("kaeso-request-id").unwrap();
    env.capture.recorded(id).await
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn assert_same(header: f64, measured: Duration) {
    assert!(
        (header - ms(measured)).abs() < 0.001,
        "the header says {header} ms, the log {measured:?}"
    );
}

#[tokio::test]
async fn kaesos_own_time_is_kept_apart_from_the_providers() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let timing = server_timing(&r);
    let o = outcome_of(&env, &r).await;
    let overhead = o.overhead.unwrap();
    assert!(!overhead.is_zero());
    assert!(overhead < o.duration);
    assert_same(timing["kaeso"], overhead);
    assert_same(timing["upload"], o.upload.unwrap());
    assert_same(timing["upstream"], o.ttfb.unwrap());
    assert_eq!(o.stream_processing, None);

    let mut body = chat("claude-haiku-4-5", "Hello");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text().contains("[DONE]"));
    let timing = server_timing(&r);
    let o = outcome_of(&env, &r).await;
    assert_same(timing["kaeso"], o.overhead.unwrap());
    assert_same(timing["upstream"], o.ttfb.unwrap());
    let processing = o.stream_processing.unwrap();
    assert!(!processing.is_zero());
    assert!(processing < o.duration);
}

#[tokio::test]
async fn failures_after_the_provider_call_keep_kaesos_time() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "[mock:error=overloaded]")),
    )
    .await;
    assert!(r.status.is_server_error(), "{}", r.status);
    let timing = server_timing(&r);
    let o = outcome_of(&env, &r).await;
    assert_eq!(o.error_kind.as_deref(), Some("overloaded"));
    assert_same(timing["kaeso"], o.overhead.unwrap());
    assert_same(timing["upstream"], o.ttfb.unwrap());

    let mut body = chat("claude-haiku-4-5", "Hi [mock:cut]");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let o = outcome_of(&env, &r).await;
    assert_eq!(o.error_kind.as_deref(), Some("stream_interrupted"));
    assert_same(server_timing(&r)["kaeso"], o.overhead.unwrap());
    assert!(o.stream_processing.is_some());
}

#[tokio::test]
async fn a_request_refused_before_the_provider_call_times_kaeso_alone() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("no-such-model", "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let timing = server_timing(&r);
    assert_eq!(timing.len(), 2, "{timing:?}");
    assert!(timing["kaeso"] > 0.0);
    let o = outcome_of(&env, &r).await;
    assert_same(timing["upload"], o.upload.unwrap());
    assert_eq!(o.overhead, None);
    assert_eq!(o.ttfb, None);
    assert_eq!(
        env.mock.received().len(),
        before,
        "nothing reached the provider"
    );
}

#[tokio::test]
async fn a_slow_upload_is_timed_apart_from_kaesos_work() {
    use axum::body::{Body, Bytes};
    use axum::http::Request;
    use futures::StreamExt;
    use tokio::sync::oneshot;
    use tower::ServiceExt;
    const HELD: Duration = Duration::from_millis(300);
    let env = env().await;
    let mut head = serde_json::to_vec(&messages("claude-haiku-4-5", "Hello")).unwrap();
    let tail = head.split_off(head.len() / 2);
    let (asking, asked) = oneshot::channel::<()>();
    let (rest, held) = oneshot::channel::<Bytes>();
    let first = futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(head)) });
    let second = futures::stream::once(async move {
        let _ = asking.send(());
        Ok(held.await.unwrap_or_default())
    });
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from_stream(first.chain(second)))
        .unwrap();
    let answer = tokio::spawn(env.app.router.clone().oneshot(req));
    asked
        .await
        .expect("the gateway waits for the rest of the body");
    tokio::time::sleep(HELD).await;
    rest.send(Bytes::from(tail)).unwrap();
    let resp = answer.await.unwrap().unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let r = Answer {
        status: resp.status(),
        headers: resp.headers().clone(),
        body: Vec::new(),
    };
    let timing = server_timing(&r);
    let o = outcome_of(&env, &r).await;
    let upload = o.upload.unwrap();
    let overhead = o.overhead.unwrap();
    assert!(upload >= HELD, "{upload:?}");
    assert!(overhead < HELD, "{overhead:?}");
    assert_same(timing["upload"], upload);
    assert_same(timing["kaeso"], overhead);
}
