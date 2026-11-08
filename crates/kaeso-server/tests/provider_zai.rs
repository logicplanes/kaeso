#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "0123456789abcdef0123456789abcdef.ZaiTestCredential";
const GLM: &str = "glm-4.6";
const GLM_5: &str = "glm-5.3";

fn error(name: &str) -> String {
    format!("[mock:error=providers/zai/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "zai", "name": "zai", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GLM, GLM_5] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "zai", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_zai(env: &Env, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(sent.header("anthropic-version"), Some("2023-06-01"));
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

#[tokio::test]
async fn an_anthropic_app_reaches_zai_with_its_key_and_its_request_as_written() {
    let env = env().await;
    let request = json!({
        "model": GLM,
        "max_tokens": 2048,
        "system": [{ "type": "text", "text": "You are a coding agent.", "cache_control": { "type": "ephemeral" } }],
        "metadata": { "user_id": "user-42" },
        "thinking": { "type": "enabled", "budget_tokens": 1024 },
        "tools": [{
            "name": "read_file",
            "description": "Read a file.",
            "input_schema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
        }],
        "messages": [
            { "role": "user", "content": "Open main.rs." },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "Read it first.", "signature": "c2lnbmF0dXJl" },
                { "type": "tool_use", "id": "toolu_01", "name": "read_file", "input": { "path": "main.rs" } }
            ] },
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_01", "content": "fn main() {}" },
                { "type": "text", "text": "Say what it does." }
            ] }
        ]
    });
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(request.clone()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("zai"));
    let sent = sent_to_zai(&env, GLM);
    for field in [
        "system",
        "metadata",
        "thinking",
        "tools",
        "messages",
        "max_tokens",
    ] {
        assert_eq!(sent[field], request[field], "{field}");
    }
    assert_eq!(
        r.json()["content"][0]["text"],
        "Hello! It's nice to meet you. How can I help you today?"
    );
}

#[tokio::test]
async fn chat_and_responses_apps_reach_zai_in_the_messages_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": "zai/glm-4.6",
            "max_completion_tokens": 300,
            "messages": [
                { "role": "system", "content": "Answer briefly." },
                { "role": "user", "content": "Hello" }
            ]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_zai(&env, GLM);
    assert_eq!(sent["max_tokens"], 300);
    assert_eq!(sent["system"][0]["text"], "Answer briefly.");
    assert_eq!(
        r.json()["choices"][0]["message"]["content"],
        "Hello! It's nice to meet you. How can I help you today?"
    );

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": GLM_5,
            "max_output_tokens": 4096,
            "reasoning": { "effort": "high" },
            "input": "Hello"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_zai(&env, GLM_5);
    assert_eq!(sent["thinking"], json!({ "type": "adaptive" }));
    assert_eq!(sent["output_config"], json!({ "effort": "high" }));
    assert_eq!(r.json()["output"][0]["type"], "message");
}

#[tokio::test]
async fn zai_errors_reach_each_app_with_the_kind_its_code_names() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(GLM, &format!("Hi {}", error("unknown-model")))),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "not_found_error");
    assert_eq!(
        body["error"]["message"],
        "Unknown Model, please check the model code."
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(GLM, &format!("Hi {}", error("prompt-too-long")))),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "prompt is too long: Prompt too long"
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(GLM, &format!("Hi {}", error("overloaded")))),
    )
    .await;
    assert_eq!(r.status.as_u16(), 529, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "overloaded_error");

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(GLM, &format!("Hi {}", error("rate-limited")))),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Rate limit reached for requests"
    );

    for refusal in ["insufficient-balance", "recorded-invalid-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GLM, &format!("Hi {}", error(refusal)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{refusal}: {}", r.text());
    }
}

#[tokio::test]
async fn token_counts_for_zai_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": GLM,
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "The model `glm-4.6` cannot count tokens."
    );

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "zai/glm-5.3", "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    assert!(env.mock.received().is_empty());
}
