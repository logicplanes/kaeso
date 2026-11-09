#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "nebius-test-credential-0001";
const GLM: &str = "zai-org/GLM-5.3";
const KIMI: &str = "moonshotai/Kimi-K3";
const EMBED: &str = "Qwen/Qwen3-Embedding-8B";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/nebius/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/nebius/errors/{name}]")
}

async fn add_catalogue_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "nebius", "model_id": model }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "nebius", "name": "nebius", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GLM, KIMI, EMBED] {
        add_catalogue_model(&env, model).await;
    }
    env
}

fn sent_to_nebius(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_nebius_with_developer_messages_as_system_and_get_its_reasoning() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GLM,
            "messages": [
                { "role": "developer", "content": "Answer in one short sentence." },
                { "role": "user", "content": format!("What is the capital of France? {}", fixture("responses", "reasoning")) }
            ],
            "reasoning_effort": "low",
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("nebius"));
    assert_eq!(
        r.json()["choices"][0]["message"]["reasoning_content"],
        "The user asks for the capital of France. It is Paris."
    );
    let sent = sent_to_nebius(&env, "/v1/chat/completions");
    assert_eq!(
        (
            &sent["messages"][0]["role"],
            &sent["user"],
            sent.get("safety_identifier")
        ),
        (&json!("system"), &json!("user-0001"), None)
    );
}

#[tokio::test]
async fn anthropic_apps_get_nebius_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
    let mut body = messages(
        GLM,
        &format!(
            "What is 17 times 23? {}",
            fixture("streams", "reasoning-stream")
        ),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let blocks: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "content_block_start")
        .map(|e| &e["content_block"]["type"])
        .collect();
    assert_eq!(blocks, [&json!("thinking"), &json!("text")]);
    let thinking: String = events
        .iter()
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    assert_eq!(thinking, "17 times 20 is 340, plus 51 is 391.");
    let sent = sent_to_nebius(&env, "/v1/chat/completions");
    assert_eq!(sent["stream_options"]["include_usage"], true);

    let mut body = messages(
        KIMI,
        &format!(
            "What is the weather in Paris? {}",
            fixture("responses", "tools")
        ),
    );
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Get the current weather for a city.",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (&reply["content"][1]["type"], &reply["content"][1]["name"]),
        (&json!("tool_use"), &json!("get_weather"))
    );
    assert_eq!(reply["stop_reason"], "tool_use");
}

#[tokio::test]
async fn a_wrong_nebius_key_reaches_apps_as_the_refused_key() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GLM, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
        );
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("nebius"));
    assert!(!format!("{o:?}").contains(SECRET));
}

#[tokio::test]
async fn nebius_embeddings_answer_and_token_counts_are_refused() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["Kaeso routes model requests.", "Nebius serves open models. [mock:fixture=providers/nebius/qwen3-embedding]"],
            "dimensions": 8
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_nebius(&env, "/v1/embeddings");
    assert_eq!(sent["model"], EMBED);
    assert_eq!(r.json()["usage"]["prompt_tokens"], 13);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GLM, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}
