#![allow(clippy::unwrap_used)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use kaeso_core::clock::Clock;
use serde_json::{Value, json};

const OFF: &str = "Request logs are off on this install.";

async fn env() -> Env {
    common::v1::env(None, Some(&[("KAESO_CLICKHOUSE_URL", "")])).await
}

fn project(env: &Env) -> String {
    format!("/orgs/{}/projects/{}", env.owner.org_id, env.project)
}

fn off(r: &common::Resp) {
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{:?}", r.body);
    assert_eq!(
        r.body.pointer("/error/message").and_then(Value::as_str),
        Some(OFF),
        "{:?}",
        r.body
    );
}

#[tokio::test]
async fn requests_are_answered_and_counted_without_request_logs() {
    let env = env().await;
    assert!(!env.app.state.request_logs());
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    env.app.state.in_flight.settled().await;
    kaeso_server::services::usage::flush(&env.app.state)
        .await
        .unwrap();
    assert_eq!(env.app.state.logs.len(), 0, "nothing waits for ClickHouse");

    let get = async |path: String| env.app.get(&path, &env.owner.cookie).await;
    let keys = |body: &Value| -> Vec<(String, u64)> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["key"].as_str().unwrap().to_owned(),
                    r["requests"].as_u64().unwrap(),
                )
            })
            .collect()
    };
    let by_model = get(format!("{}/usage?group_by=model", project(&env))).await;
    assert_eq!(by_model.status, StatusCode::OK, "{:?}", by_model.body);
    assert_eq!(
        keys(&by_model.body),
        [("anthropic/claude-haiku-4-5".to_owned(), 2)]
    );
    let by_route = get(format!("/orgs/{}/usage?group_by=route", env.owner.org_id)).await;
    assert_eq!(
        keys(&by_route.body),
        [
            (
                "anthropic_messages:anthropic_messages:anthropic".to_owned(),
                1
            ),
            ("openai_chat:anthropic_messages:anthropic".to_owned(), 1),
        ]
    );
    let span = get(format!("{}/usage/span", project(&env))).await;
    let today = env.app.clock.now().date().to_string();
    assert_eq!(span.body, json!({ "first_day": today, "last_day": today }));
}

#[tokio::test]
async fn every_request_log_endpoint_says_logs_are_off() {
    let env = env().await;
    let org = format!("/orgs/{}", env.owner.org_id);
    let session = env.app.get("/auth/session", &env.owner.cookie).await;
    assert_eq!(session.body["request_logs"], false, "{:?}", session.body);
    for path in [
        format!("{}/logs", project(&env)),
        format!(
            "{}/logs/{}",
            project(&env),
            kaeso_core::id::RequestId::from_uuid(uuid::Uuid::now_v7())
        ),
        format!("{}/logs/export", project(&env)),
        format!("{org}/logs/storage"),
    ] {
        off(&env.app.get(&path, &env.owner.cookie).await);
    }
    off(&env
        .app
        .post(
            &format!("{org}/logs/deletions"),
            json!({ "from": "2026-01-01T00:00:00Z", "to": "2026-02-01T00:00:00Z" }),
            Some(&env.owner.cookie),
        )
        .await);
    let settings = env
        .app
        .get(&format!("{org}/logs/settings"), &env.owner.cookie)
        .await;
    assert_eq!(settings.status, StatusCode::OK, "{:?}", settings.body);
    assert!(
        !kaeso_server::services::logs::keeps_content(&env.app.state, common::uuid(&env.project))
            .await,
        "no bodies are kept for a log nobody writes"
    );
}

#[tokio::test]
async fn instance_pages_show_clickhouse_as_off() {
    let env = env().await;
    let system = env.app.get("/instance/system", &env.owner.cookie).await;
    assert_eq!(system.status, StatusCode::OK, "{:?}", system.body);
    assert_eq!(system.body["clickhouse"], "off");
    assert_eq!(system.body["status"], "ready");
    assert_eq!(
        system.body["storage"]["clickhouse_bytes_on_disk"],
        Value::Null
    );
    assert_eq!(system.body["storage"]["request_log_rows"], Value::Null);
    let usage = env
        .app
        .get(
            &format!("/instance/orgs/{}/usage", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(usage.status, StatusCode::OK, "{:?}", usage.body);
    assert_eq!(usage.body["log_entries"], Value::Null);
    assert_eq!(usage.body["log_logical_bytes"], Value::Null);
    assert_eq!(usage.body["requests_30d"], 0);
}
