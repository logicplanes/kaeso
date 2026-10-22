#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
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
async fn a_request_returns_its_translation_memory_before_the_provider_answers() {
    let env = common::v1::env(None, Some(&[("KAESO_BODY_LIMITS_IN_FLIGHT", "128KiB")])).await;
    let state = &env.app.state;
    assert_eq!(state.translating.free(), 128 << 10);
    let mut gate = env.mock.gate();
    let mut held = messages("claude-haiku-4-5", &format!("Long {}", gate.marker()));
    held["stream"] = json!(true);
    let first = call(&env, "/v1/messages", Some(&env.key), true, Some(held));
    let second = async {
        gate.wait_for(1).await;
        assert_eq!(
            state.translating.free(),
            121 << 10,
            "the open call keeps only its decoded request: its content, one message and one block"
        );
        let next = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "Short")),
        )
        .await;
        assert_eq!(state.translating.waiting(), 0);
        gate.open();
        next
    };
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    state.in_flight.settled().await;
    assert_eq!(
        (state.translating.free(), state.translating.waiting()),
        (128 << 10, 0)
    );
}

fn post(path: &str, key: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method("POST")
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn padded_stalled_stream() -> Vec<u8> {
    let body = serde_json::to_vec(&json!({
        "model": "gpt-4o",
        "stream": true,
        "messages": [{ "role": "user", "content": "[mock:stall]" }],
    }))
    .unwrap();
    let mut out = b"{".to_vec();
    out.extend(std::iter::repeat_n(b' ', (8 << 20) - body.len() - 64));
    out.extend_from_slice(&body[1..]);
    out
}

async fn new_key(env: &common::v1::Env, name: &str) -> (String, uuid::Uuid) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
            json!({ "name": name }),
            Some(&env.owner.cookie),
        )
        .await;
    (
        r.body["secret"].as_str().unwrap().to_owned(),
        common::uuid(r.body["id"].as_str().unwrap()),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stalled_streams_of_padded_bodies_leave_the_translation_memory_to_everyone() {
    let env = common::v1::env(None, None).await;
    let budget = &env.app.state.translating;
    let total = env.app.state.config.body_limits.in_flight.0;
    let body = padded_stalled_stream();
    assert_eq!(body.len(), (8 << 20) - 64);
    let settings = format!(
        "/orgs/{}/projects/{}/logs/settings",
        env.owner.org_id, env.project
    );
    let content_logging = |on: bool| {
        env.app.call(
            Method::PATCH,
            &settings,
            Some(json!({ "content_logging": on })),
            Some(&env.owner.cookie),
        )
    };
    assert_eq!(content_logging(false).await.status, StatusCode::OK);
    let open = || async {
        let r = env
            .app
            .router
            .clone()
            .oneshot(post("/v1/chat/completions", &env.key, body.clone()))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        r
    };
    let mut streams = Vec::new();
    for _ in 0..13 {
        streams.push(open().await);
    }
    assert_eq!(
        total - budget.free(),
        13 * (7 << 10),
        "each open stream keeps 7 KiB for its decoded request, one message and one block, not its padding"
    );
    assert_eq!(content_logging(true).await.status, StatusCode::OK);
    streams.push(open().await);
    assert!(
        total - budget.free() >= 13 * (7 << 10) + body.len() as u64,
        "a body kept for its log is counted until it is logged"
    );
    let (victim, _) = new_key(&env, "victim").await;
    let answered = call(
        &env,
        "/v1/chat/completions",
        Some(&victim),
        false,
        Some(chat("gpt-4o", "Hi")),
    )
    .await;
    assert_eq!(answered.status, StatusCode::OK, "{}", answered.text());
    assert_eq!(budget.waiting(), 0);
    drop(streams);
}

async fn open_stalled_chat(env: &common::v1::Env, body: Vec<u8>) -> axum::response::Response {
    let settings = format!(
        "/orgs/{}/projects/{}/logs/settings",
        env.owner.org_id, env.project
    );
    let off = json!({ "content_logging": false });
    let r = env
        .app
        .call(Method::PATCH, &settings, Some(off), Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = env
        .app
        .router
        .clone()
        .oneshot(post("/v1/chat/completions", &env.key, body))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    r
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tool_schema_padded_with_whitespace_keeps_memory_for_its_padding() {
    let env = common::v1::env(None, None).await;
    let budget = &env.app.state.translating;
    let total = env.app.state.config.body_limits.in_flight.0;
    let padding: usize = 1 << 20;
    let body = format!(
        r#"{{"model":"gpt-4o","stream":true,"messages":[{{"role":"user","content":"[mock:stall]"}}],"tools":[{{"type":"function","function":{{"name":"lookup","parameters":{{"type":"object"{}}}}}}}]}}"#,
        " ".repeat(padding)
    );
    let stream = open_stalled_chat(&env, body.into_bytes()).await;
    let held = total - budget.free();
    assert!(
        (padding as u64..padding as u64 + (16 << 10)).contains(&held),
        "the schema keeps its padding in the decoded request, so the open stream keeps memory for it: {held} bytes"
    );
    drop(stream);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn many_tiny_messages_keep_their_structure_while_the_provider_answers() {
    let env = common::v1::env(None, None).await;
    let budget = &env.app.state.translating;
    let total = env.app.state.config.body_limits.in_flight.0;
    let mut said: Vec<Value> = (0..2_000)
        .map(|_| json!({ "role": "user", "content": "a" }))
        .collect();
    said.push(json!({ "role": "user", "content": "[mock:stall]" }));
    let body = serde_json::to_vec(&json!({ "model": "gpt-4o", "stream": true, "messages": said }))
        .unwrap();
    let structure = 2_001 * (6 << 10);
    let content = body.len() as u64;
    let stream = open_stalled_chat(&env, body).await;
    let held = total - budget.free();
    assert!(
        (structure..structure + content + (1 << 10)).contains(&held),
        "2,001 messages of one block each keep 6 KiB apiece besides their content: {held} bytes"
    );
    drop(stream);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_key_over_its_share_is_refused_after_the_wait_while_other_keys_are_served() {
    let env = common::v1::env(None, Some(&[("KAESO_BODY_LIMITS_IN_FLIGHT", "64MiB")])).await;
    let budget = &env.app.state.translating;
    assert_eq!(budget.share(), 16 << 20, "a key may hold a quarter");
    let (greedy, greedy_id) = new_key(&env, "greedy").await;
    let (other, _) = new_key(&env, "other").await;
    let share = budget.hold(greedy_id, u64::MAX).await.unwrap();
    let refused = call(
        &env,
        "/v1/chat/completions",
        Some(&greedy),
        false,
        Some(chat("gpt-4o", "Hi")),
    );
    let served = async {
        common::eventually("the greedy key's request waits", || async {
            budget.waiting() == 1
        })
        .await;
        call(
            &env,
            "/v1/messages",
            Some(&other),
            true,
            Some(messages("claude-haiku-4-5", "Hi")),
        )
        .await
    };
    let (refused, served) = tokio::join!(refused, served);
    assert_eq!(served.status, StatusCode::OK, "{}", served.text());
    assert_eq!(
        refused.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        refused.text()
    );
    assert_eq!(refused.header("retry-after"), Some("10"));
    let error = &refused.json()["error"];
    assert_eq!(error["code"], "server_is_overloaded");
    assert_eq!(
        error["message"],
        "Kaeso is busy translating other requests; try again shortly."
    );
    drop(share);
    assert_eq!(budget.free(), 64 << 20);
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
