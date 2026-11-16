#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

// A step tries its model again after a short-lived failure: the project's `retries` for a
// request that names a model, and a route step's own.

use crate::common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode};
use common::v1::{Answer, Capture, Env, call, messages};
use serde_json::{Value, json};

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

fn base(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

async fn model_id(env: &Env, model: &str) -> String {
    let models = env
        .app
        .get(&format!("{}/models", base(env)), &env.owner.cookie)
        .await;
    models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == model)
        .unwrap_or_else(|| panic!("{model} is in the catalogue"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn alias(env: &Env, name: &str, route: Value) {
    let r = env
        .app
        .post(
            &format!("{}/aliases", base(env)),
            json!({ "name": name, "route": route }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn set_retries(env: &Env, retries: u64) -> common::Resp {
    env.app
        .call(
            Method::PATCH,
            &base(env),
            Some(json!({ "retries": retries })),
            Some(&env.owner.cookie),
        )
        .await
}

async fn ask(env: &Env, model: &str, text: &str) -> Answer {
    call(
        env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(model, text)),
    )
    .await
}

async fn ask_streamed(env: &Env, model: &str, text: &str) -> Answer {
    let mut body = messages(model, text);
    body["stream"] = json!(true);
    call(env, "/v1/messages", Some(&env.key), true, Some(body)).await
}

// The model of every request the provider got, in order.
fn asked(env: &Env) -> Vec<String> {
    env.mock
        .received()
        .iter()
        .filter_map(|r| r.json()["model"].as_str().map(str::to_owned))
        .collect()
}

fn walked(r: &Answer) -> Vec<String> {
    r.header("kaeso-route")
        .map(|h| {
            h.split(", ")
                .map(|step| {
                    step.split(';')
                        .filter(|p| !p.starts_with("dur="))
                        .collect::<Vec<_>>()
                        .join(";")
                })
                .collect()
        })
        .unwrap_or_default()
}

const HAIKU: &str = "claude-haiku-4-5";

#[tokio::test]
async fn a_model_named_directly_is_tried_again_as_often_as_the_project_says() {
    let env = env().await;
    let r = ask(&env, HAIKU, "Hi [mock:flaky=none 1 overloaded]").await;
    assert_eq!(
        r.status.as_u16(),
        529,
        "no retries unless asked: {}",
        r.text()
    );
    assert_eq!(asked(&env).len(), 1);

    let set = set_retries(&env, 2).await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.body);
    assert_eq!(set.body["retries"], 2);
    let refused = set_retries(&env, 4).await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.body
    );
    assert_eq!(refused.body["error"]["param"], "retries");
    let project = env.app.get(&base(&env), &env.owner.cookie).await;
    assert_eq!(project.body["retries"], 2);

    let r = ask(&env, HAIKU, "Hi [mock:flaky=twice 2 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(asked(&env).len(), 4, "a first try and two more");
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.status, 200);
    assert_eq!(o.fallbacks.len(), 2, "{:?}", o.fallbacks);
    for f in &o.fallbacks {
        assert!(f.retried && !f.skipped, "{f:?}");
        assert_eq!(
            (f.model.as_str(), f.status, f.error_kind.as_str()),
            ("anthropic/claude-haiku-4-5", 529, "overloaded")
        );
    }

    let r = ask(&env, HAIKU, "Hi [mock:flaky=thrice 3 overloaded]").await;
    assert_eq!(
        r.status.as_u16(),
        529,
        "the last try's failure: {}",
        r.text()
    );
    assert_eq!(asked(&env).len(), 7);
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!((o.status, o.fallbacks.len()), (529, 2));
}

#[tokio::test]
async fn an_alias_step_tries_its_model_again_before_it_falls_back() {
    let env = env().await;
    let claude = model_id(&env, HAIKU).await;
    let gpt = model_id(&env, "gpt-4o").await;
    alias(
        &env,
        "sturdy",
        json!({ "type": "model", "model": claude, "retries": 1, "fallback": { "type": "model", "model": gpt } }),
    )
    .await;

    let r = ask(&env, "sturdy", "Hi [mock:flaky=once 1 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some(HAIKU));
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200;tries=2"]
    );
    assert_eq!(asked(&env), [HAIKU, HAIKU]);

    // A step that answered on its second try isn't held: the next request goes to it again.
    let r = ask(&env, "sturdy", "Hi").await;
    assert_eq!(
        walked(&r),
        ["\"route\";model=\"anthropic/claude-haiku-4-5\";status=200"]
    );

    let r = ask(&env, "sturdy", "Hi [mock:flaky=twice 2 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"));
    assert_eq!(
        walked(&r),
        [
            "\"route\";model=\"anthropic/claude-haiku-4-5\";status=529;tries=2",
            "\"route.fallback\";model=\"openai/gpt-4o\";status=200"
        ]
    );
    assert_eq!(asked(&env), [HAIKU, HAIKU, HAIKU, HAIKU, HAIKU, "gpt-4o"]);
    let o = env.outcomes().await.pop().unwrap();
    let retried: Vec<(&str, bool)> = o
        .fallbacks
        .iter()
        .map(|f| (f.path.as_str(), f.retried))
        .collect();
    assert_eq!(retried, [("route", true), ("route", false)]);

    // The step failed after its last try, so it is held like any step that fell back.
    let r = ask(&env, "sturdy", "Hi").await;
    assert_eq!(r.header("kaeso-model"), Some("gpt-4o"), "{:?}", walked(&r));
}

#[tokio::test]
async fn a_steps_own_retries_win_over_the_projects() {
    let env = env().await;
    let claude = model_id(&env, HAIKU).await;
    alias(
        &env,
        "once",
        json!({ "type": "model", "model": claude, "retries": 0 }),
    )
    .await;
    assert_eq!(set_retries(&env, 3).await.status, StatusCode::OK);
    let r = ask(&env, "once", "Hi [mock:flaky=step 1 overloaded]").await;
    assert_eq!(r.status.as_u16(), 529, "{}", r.text());
    assert_eq!(asked(&env).len(), 1);
    let r = ask(&env, HAIKU, "Hi [mock:flaky=project 1 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(asked(&env).len(), 3);
}

#[tokio::test]
async fn a_dropped_connection_and_a_stream_that_never_began_are_tried_again() {
    let env = env().await;
    assert_eq!(set_retries(&env, 1).await.status, StatusCode::OK);
    let r = ask(&env, HAIKU, "Hi [mock:flaky=dropped 1 drop]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(asked(&env).len(), 2);
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(
        (o.fallbacks[0].status, o.fallbacks[0].error_kind.as_str()),
        (502, "provider_error")
    );

    let r = ask_streamed(&env, HAIKU, "Hi [mock:flaky=stream 1 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.text().contains("message_stop"), "{}", r.text());
    assert_eq!(asked(&env).len(), 4);
}

#[tokio::test]
async fn failures_a_second_try_wont_change_are_not_tried_again() {
    let env = env().await;
    assert_eq!(set_retries(&env, 3).await.status, StatusCode::OK);
    for (fixture, status) in [
        ("rate-limit", 429),
        ("invalid-request-prefill", 400),
        // A refused provider key is the org's to fix, not the provider's moment.
        ("authentication", 502),
    ] {
        let before = asked(&env).len();
        let r = ask(&env, HAIKU, &format!("Hi [mock:fail={HAIKU} {fixture}]")).await;
        assert_eq!(r.status.as_u16(), status, "{fixture}: {}", r.text());
        assert_eq!(asked(&env).len(), before + 1, "{fixture} is sent once");
    }

    // A provider that takes too long may still be working on the request, and bill it.
    let before = asked(&env).len();
    let r = call_with_timeout(&env, &format!("Hi [mock:fail={HAIKU} stall]"), "1").await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert_eq!(asked(&env).len(), before + 1);
}

async fn call_with_timeout(env: &Env, text: &str, seconds: &str) -> Answer {
    let req = axum::http::Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("content-type", "application/json")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("kaeso-timeout", seconds)
        .body(axum::body::Body::from(
            serde_json::to_vec(&messages(HAIKU, text)).unwrap(),
        ))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(env.app.router.clone(), req)
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = http_body_util::BodyExt::collect(resp.into_body())
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
async fn the_tries_count_once_against_a_request_rate() {
    let env = env().await;
    assert_eq!(set_retries(&env, 1).await.status, StatusCode::OK);
    let limit = env
        .app
        .post(
            &format!("{}/limits", base(&env)),
            json!({ "level": "project", "kind": "request_rate", "period": "minute", "amount": 1 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(limit.status, StatusCode::CREATED, "{:?}", limit.body);
    let r = ask(&env, HAIKU, "Hi [mock:flaky=rate 1 overloaded]").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(asked(&env).len(), 2);
    let r = ask(&env, HAIKU, "Hi").await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(
        asked(&env).len(),
        2,
        "refused before it reached the provider"
    );
}

#[tokio::test]
async fn the_tries_share_the_steps_time() {
    let env = env().await;
    let claude = model_id(&env, HAIKU).await;
    alias(
        &env,
        "quick",
        json!({ "type": "model", "model": claude, "retries": 3, "timeout_seconds": 2 }),
    )
    .await;
    let began = Instant::now();
    let r = ask(
        &env,
        "quick",
        &format!("Hi [mock:fail={HAIKU} overloaded 900]"),
    )
    .await;
    let took = began.elapsed();
    assert!(
        [529, 504].contains(&r.status.as_u16()),
        "{}: {}",
        r.status,
        r.text()
    );
    // 0.9 s each: a third try would end after the step's 2 seconds.
    assert!(asked(&env).len() <= 2, "{:?}", asked(&env));
    assert!(took < Duration::from_millis(2_600), "{took:?}");
}
