#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::json;

const NVIDIA_SECRET: &str = "nvapi-test-credential-0003";
const LLAMA: &str = "meta/llama-3.3-70b-instruct";
const EMBEDDER: &str = "nvidia/nv-embed-v1";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/nvidia/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/nvidia/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "nvidia", "name": "nvidia", "secret": NVIDIA_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [LLAMA, EMBEDDER] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "nvidia", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn assert_sent_to_nvidia(env: &Env, path: &str, model: &str) -> serde_json::Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {NVIDIA_SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

#[tokio::test]
async fn chat_apps_reach_nim_models_whose_ids_contain_a_slash() {
    let env = env().await;
    let text = format!(
        "Write a limerick. {}",
        fixture("responses", "doc-llama-3-3-70b")
    );
    for name in [LLAMA.to_owned(), format!("nvidia/{LLAMA}")] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(&name, &text)),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.header("kaeso-provider"), Some("nvidia"));
        assert_eq!(r.header("kaeso-model"), Some(LLAMA));
        let body = r.json();
        assert_eq!(body["object"], "chat.completion");
        assert_eq!(
            body["choices"][0]["message"]["content"],
            "The python functions..."
        );
        assert_eq!(body["usage"]["total_tokens"], 21);
        assert_sent_to_nvidia(&env, "/v1/chat/completions", LLAMA);
    }
}

#[tokio::test]
async fn anthropic_apps_reach_nim_models_through_chat_completions() {
    let env = env().await;
    let text = format!(
        "Write a limerick. {}",
        fixture("responses", "doc-llama-3-3-70b")
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(LLAMA, &text)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "The python functions...");
    assert_eq!(body["stop_reason"], "end_turn");
    assert_eq!(
        (
            body["usage"]["input_tokens"].as_u64(),
            body["usage"]["output_tokens"].as_u64()
        ),
        (Some(9), Some(12))
    );
    let sent = assert_sent_to_nvidia(&env, "/v1/chat/completions", LLAMA);
    assert_eq!(sent["messages"][0]["role"], "user");
    assert_eq!(env.mock.last().unwrap().header("anthropic-version"), None);

    let mut body = messages(
        LLAMA,
        &format!(
            "Add and multiply. {}",
            fixture("streams", "sdk-parallel-tool-calls-one-chunk")
        ),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    for part in [
        "event: message_start",
        r#""name":"xxyyzz","type":"tool_use""#,
        r#""name":"zzyyxx","type":"tool_use""#,
        r#""stop_reason":"tool_use""#,
        "event: message_stop",
    ] {
        assert!(text.contains(part), "{part} missing from {text}");
    }
    let sent = assert_sent_to_nvidia(&env, "/v1/chat/completions", LLAMA);
    assert_eq!(sent["stream"], true);
}

#[tokio::test]
async fn nim_errors_reach_apps_in_their_own_format() {
    let env = env().await;
    let text = format!("Hi {}", error("doc-nvcf-unprocessable-entity"));
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(LLAMA, &text)),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(
        message.starts_with(
            "The provider returned an error (HTTP 422) that is not in the OpenAI error format:"
        ) && message.contains("Unprocessable Entity"),
        "{message}"
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(LLAMA, &text)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Unprocessable Entity"),
        "{body}"
    );
}

#[tokio::test]
async fn embeddings_keep_the_nim_fields_openai_does_not_have() {
    let env = env().await;
    let input = format!(
        "What is the capital of France? {}",
        fixture("responses", "doc-nv-embedqa-e5-v5-query")
    );
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBEDDER,
            "input": input,
            "input_type": "query",
            "encoding_format": "float",
            "truncate": "NONE",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("nvidia"));
    let body = r.json();
    assert_eq!(body["object"], "list");
    assert_eq!(body["model"], "nvidia/nv-embedqa-e5-v5");
    assert_eq!(body["data"][0]["embedding"].as_array().unwrap().len(), 1024);
    let sent = assert_sent_to_nvidia(&env, "/v1/embeddings", EMBEDDER);
    assert_eq!(
        (
            &sent["input_type"],
            &sent["truncate"],
            &sent["encoding_format"]
        ),
        (&json!("query"), &json!("NONE"), &json!("float"))
    );
    assert_eq!(sent["input"], input);
}

#[tokio::test]
async fn token_counts_for_nim_models_are_refused_in_the_apps_format() {
    let env = env().await;
    env.mock.clear();
    let expected = format!("The model `{LLAMA}` cannot count tokens.");
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": LLAMA, "messages": [{ "role": "user", "content": "Hi" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        (
            &body["type"],
            &body["error"]["type"],
            &body["error"]["message"]
        ),
        (
            &json!("error"),
            &json!("invalid_request_error"),
            &json!(expected)
        )
    );
    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": LLAMA, "input": "Hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        (&body["error"]["message"], &body["error"]["param"]),
        (&json!(expected), &json!("model"))
    );
    assert!(env.mock.received().is_empty());
}
