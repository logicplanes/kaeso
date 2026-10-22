#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{call, chat, messages};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn nested_text(levels: usize) -> Vec<u8> {
    let open = r#"{"text":"#.repeat(levels);
    format!(
        r#"{{"model":"gpt-4o","input":"hi","text":{open}{{}}{}}}"#,
        "}".repeat(levels)
    )
    .into_bytes()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_body_nested_thousands_deep_is_refused_and_the_gateway_keeps_serving() {
    let env = common::v1::env(None, None).await;
    env.mock.clear();
    let body = nested_text(5_000);
    assert!(body.len() < 64 << 10, "{} bytes", body.len());
    let req = Request::builder()
        .uri("/v1/responses")
        .method("POST")
        .header("authorization", format!("Bearer {}", env.key))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let router = env.app.router.clone();
    let resp = tokio::spawn(async move { router.oneshot(req).await.unwrap() })
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let error: Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(error["error"]["type"], "invalid_request_error");
    assert_eq!(
        error["error"]["message"],
        "The body nests more than 127 levels deep; Kaeso takes at most 127."
    );
    assert!(env.mock.received().is_empty());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

fn tiny_items(n: usize) -> Value {
    json!({
        "model": "gpt-4o",
        "input": (0..n).map(|_| json!({ "role": "user", "content": "a" })).collect::<Vec<_>>(),
    })
}

#[tokio::test]
async fn a_small_body_of_many_tiny_items_is_refused_before_it_is_decoded_or_sent() {
    let env = common::v1::env(None, None).await;
    let body = tiny_items(kaeso_codecs::limits::MAX_MESSAGES + 1);
    let size = serde_json::to_vec(&body).unwrap().len();
    assert!(size < 4 << 20, "the crafted body is small: {size} bytes");
    env.mock.clear();
    let r = call(&env, "/v1/responses", Some(&env.key), false, Some(body)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(error["type"], "invalid_request_error");
    assert_eq!(error["param"], "input");
    assert_eq!(
        error["message"],
        "`input` holds more than 100000 items; Kaeso takes at most 100000."
    );
    assert!(env.mock.received().is_empty());
    assert_eq!(
        env.app.state.translating.free(),
        env.app.state.config.body_limits.in_flight.0
    );

    let mut tools = chat("gpt-4o", "Hi");
    tools["tools"] = json!(
        (0..=kaeso_codecs::limits::MAX_TOOLS)
            .map(|i| json!({ "type": "function", "function": { "name": format!("f{i}") } }))
            .collect::<Vec<_>>()
    );
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(tools),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "tools");
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn the_model_api_takes_bodies_up_to_8_mib_by_default() {
    let env = common::v1::env(None, None).await;
    assert_eq!(env.app.state.config.body_limits.model_api.0, 8 << 20);
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", &"a".repeat(8 << 20))),
    )
    .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "request_too_large");
    assert_eq!(
        r.json()["error"]["message"],
        "The request body is larger than 8388608 bytes."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn requests_wait_while_others_hold_the_memory_for_translating() {
    let env = common::v1::env(None, Some(&[("KAESO_BODY_LIMITS_IN_FLIGHT", "12KiB")])).await;
    let state = &env.app.state;
    assert_eq!(state.translating.free(), 12 << 10);
    let mut gate = env.mock.gate();
    let mut held = messages("claude-haiku-4-5", &format!("Long {}", gate.marker()));
    held["stream"] = json!(true);
    let first = call(&env, "/v1/messages", Some(&env.key), true, Some(held));
    let second = async {
        gate.wait_for(1).await;
        let left = state.translating.free();
        assert!(
            left < 6 << 10,
            "the first request holds its estimate: {left} left"
        );
        let next = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "Short")),
        );
        let watch = async {
            common::eventually("the second request waits for memory", || async {
                state.translating.waiting() == 1
            })
            .await;
            assert_eq!(
                env.mock.received().len(),
                1,
                "only the first reached the provider"
            );
            gate.open();
        };
        let (r, ()) = tokio::join!(next, watch);
        r
    };
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert_eq!(env.mock.received().len(), 2);
    state.in_flight.settled().await;
    assert_eq!(
        (state.translating.free(), state.translating.waiting()),
        (12 << 10, 0)
    );
}

#[tokio::test]
async fn the_playground_takes_bodies_up_to_the_platform_api_limit() {
    let env = common::v1::env(None, None).await;
    let path = format!(
        "/orgs/{}/projects/{}/playground/responses",
        env.owner.org_id, env.project
    );
    let body = json!({ "model": "claude-haiku-4-5", "input": "a".repeat(1 << 20), "stream": true });
    let r = env.app.post(&path, body, Some(&env.owner.cookie)).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{:?}", r.body);
    assert_eq!(r.error_type(), "request_too_large");
}
