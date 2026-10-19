#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::json;

const OLLAMA_SECRET: &str = "ollama-test-credential-0003";
const MODEL: &str = "gpt-oss:120b";
const UNAUTHORIZED: &str = "[mock:error=../providers/ollama-cloud/errors/unauthorized-live]";

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "ollama-cloud", "name": "cloud", "secret": OLLAMA_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "ollama-cloud", "model_id": MODEL }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn assert_sent_to_ollama(env: &Env, stream: bool) {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {OLLAMA_SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.json()["model"], MODEL);
    assert_eq!(sent.json()["stream"].as_bool().unwrap_or(false), stream);
}

#[tokio::test]
async fn chat_completions_apps_reach_ollama_cloud_by_its_tagged_model_id() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(MODEL, "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "chat.completion");
    assert_eq!(r.header("kaeso-provider"), Some("ollama-cloud"));
    assert_eq!(r.header("kaeso-model"), Some(MODEL));
    assert_sent_to_ollama(&env, false);

    let mut body = chat(&format!("ollama-cloud/{MODEL}"), "Hello");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text().contains("chat.completion.chunk") && r.text().trim_end().ends_with("data: [DONE]"),
        "{}",
        r.text()
    );
    assert_sent_to_ollama(&env, true);
}

#[tokio::test]
async fn anthropic_apps_reach_ollama_cloud_through_chat_completions() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(&format!("ollama-cloud/{MODEL}"), "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["type"], "message");
    assert_eq!(r.json()["role"], "assistant");
    assert_eq!(r.header("kaeso-provider"), Some("ollama-cloud"));
    assert_sent_to_ollama(&env, false);

    let mut body = messages(MODEL, "Hello");
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(
        text.contains("event: message_start") && text.contains("event: message_stop"),
        "{text}"
    );
    assert_sent_to_ollama(&env, true);
}

#[tokio::test]
async fn ollama_cloud_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(MODEL, UNAUTHORIZED)),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], "Unauthorized");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(MODEL, UNAUTHORIZED)),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert_eq!(r.json()["error"]["type"], "authentication_error");
    assert_eq!(r.json()["error"]["message"], "Unauthorized");
}

#[tokio::test]
async fn token_counts_for_ollama_cloud_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": MODEL, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert_eq!(r.json()["error"]["type"], "invalid_request_error");
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{MODEL}` cannot count tokens.")
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn the_model_list_names_ollama_cloud_models_with_their_tags() {
    let env = env().await;
    let r = call(&env, "/v1/models", Some(&env.key), false, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let listed = r.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == MODEL)
        .cloned()
        .unwrap_or_else(|| panic!("{} is not listed: {}", MODEL, r.text()));
    assert_eq!(listed["owned_by"], "ollama-cloud");
    let r = call(
        &env,
        &format!("/v1/models/ollama-cloud/{MODEL}"),
        Some(&env.key),
        false,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["id"], format!("ollama-cloud/{MODEL}"));
    assert_eq!(r.json()["owned_by"], "ollama-cloud");
}
