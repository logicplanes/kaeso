#![allow(clippy::unwrap_used)]

mod common;

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{call, messages};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn raw(
    env: &common::v1::Env,
    path: &str,
    anthropic: bool,
    body: &'static [u8],
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    req = if anthropic {
        req.header("x-api-key", &env.key)
            .header("anthropic-version", "2023-06-01")
    } else {
        req.header("authorization", format!("Bearer {}", env.key))
    };
    let resp = env
        .app
        .router
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn malformed_json_is_refused_in_every_dialect() {
    let env = common::v1::env(None, None).await;
    let endpoints = [
        ("/v1/chat/completions", false),
        ("/v1/responses", false),
        ("/v1/responses/input_tokens", false),
        ("/v1/embeddings", false),
        ("/v1/messages", true),
        ("/v1/messages/count_tokens", true),
    ];
    let bodies: [&'static [u8]; 5] = [
        b"",
        b"not json",
        b"{\"model\":",
        b"[1,2,3]",
        b"{\"model\": 42, \"messages\": \"x\"}",
    ];
    for (path, anthropic) in endpoints {
        for body in bodies {
            let (status, json) = raw(&env, path, anthropic, body).await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{path} {:?}: {json}",
                String::from_utf8_lossy(body)
            );
            if anthropic {
                assert_eq!(json["type"], "error", "{path}: {json}");
                assert_eq!(
                    json["error"]["type"], "invalid_request_error",
                    "{path}: {json}"
                );
            } else {
                assert!(json["error"]["message"].is_string(), "{path}: {json}");
            }
        }
    }
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn oversize_bodies_are_refused_on_both_apis() {
    let env = common::v1::env(
        None,
        Some(&[
            ("KAESO_BODY_LIMITS_MODEL_API", "2KiB"),
            ("KAESO_BODY_LIMITS_PLATFORM_API", "2KiB"),
        ]),
    )
    .await;
    let big = "x".repeat(4096);
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", &big)),
    )
    .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "request_too_large");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({ "model": "gpt-4o", "messages": [{ "role": "user", "content": big }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", r.text());
    assert_eq!(
        call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "small"))
        )
        .await
        .status,
        StatusCode::OK
    );
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects", env.owner.org_id),
            json!({ "name": big }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{:?}", r.body);
    assert_eq!(r.body["error"]["type"], "request_too_large");
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
    ] {
        assert_eq!(
            r.headers.get(name).and_then(|v| v.to_str().ok()),
            Some(value),
            "{name}"
        );
    }
    assert!(
        r.headers.get("strict-transport-security").is_some(),
        "the test install is https"
    );
    assert!(
        r.headers.get("access-control-allow-origin").is_none(),
        "no CORS"
    );
}

#[tokio::test]
async fn warm_requests_do_not_touch_postgres() {
    let env = common::v1::env(None, None).await;
    for _ in 0..2 {
        assert_eq!(
            call(
                &env,
                "/v1/messages",
                Some(&env.key),
                true,
                Some(messages("claude-haiku-4-5", "warm"))
            )
            .await
            .status,
            StatusCode::OK
        );
    }
    let mut blocker = env.app.state.pg.begin().await.unwrap();
    sqlx::query("LOCK TABLE api_keys, projects, organizations, users, project_memberships, catalogue_models, aliases, model_access, provider_credentials, limits, usage_daily IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let started = Instant::now();
    let r = tokio::time::timeout(
        Duration::from_secs(5),
        call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "while blocked")),
        ),
    )
    .await
    .expect("a warm request waited for PostgreSQL");
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    blocker.rollback().await.unwrap();
}

#[tokio::test]
async fn a_busy_key_records_its_use_once_a_minute() {
    let env = common::v1::env(None, None).await;
    let pg = env.app.state.pg.clone();
    let send = || {
        call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("claude-haiku-4-5", "busy")),
        )
    };
    assert_eq!(send().await.status, StatusCode::OK);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let used: i64 =
            sqlx::query_scalar("SELECT count(*) FROM api_keys WHERE last_used_at IS NOT NULL")
                .fetch_one(&pg)
                .await
                .unwrap();
        if used > 0 {
            break;
        }
        assert!(Instant::now() < deadline, "the key's use was not recorded");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut blocker = pg.begin().await.unwrap();
    sqlx::query("LOCK TABLE api_keys IN EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    for _ in 0..10 {
        assert_eq!(send().await.status, StatusCode::OK);
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
        assert_eq!(
            waiting, 0,
            "requests of a busy key wrote its last use again"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    blocker.rollback().await.unwrap();
}

#[tokio::test]
async fn a_repeated_unknown_key_is_refused_without_postgres() {
    let env = common::v1::env(None, None).await;
    let (prefix, random) = env.key.split_at(env.key.len() - 20);
    let replaced: String = random
        .chars()
        .map(|c| if c == 'Q' { 'R' } else { 'Q' })
        .collect();
    let unknown = format!("{prefix}{replaced}");
    let send = || {
        call(
            &env,
            "/v1/messages",
            Some(&unknown),
            true,
            Some(messages("claude-haiku-4-5", "hi")),
        )
    };
    assert_eq!(send().await.status, StatusCode::UNAUTHORIZED);
    let mut blocker = env.app.state.pg.begin().await.unwrap();
    sqlx::query("LOCK TABLE api_keys IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let r = tokio::time::timeout(Duration::from_secs(2), send())
        .await
        .expect("a repeated unknown key waited for PostgreSQL");
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    blocker.rollback().await.unwrap();
}
