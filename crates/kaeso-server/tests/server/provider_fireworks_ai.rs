#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "fw_test-credential-0001";
const FLASH: &str = "accounts/fireworks/models/glm-5p3-flash";
const KIMI: &str = "accounts/fireworks/models/kimi-k3";
const KIMI_AS_MESSAGES: &str = "fireworks-ai/accounts/fireworks/models/kimi-k3-messages";
const EMBED: &str = "accounts/fireworks/models/qwen3-embedding-8b";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/fireworks-ai/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/fireworks-ai/errors/{name}]")
}

async fn add_catalogue_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "fireworks-ai", "model_id": model }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn add_model(env: &Env, model: &str, overrides: Value) {
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "fireworks-ai", "model_id": model, "custom": true, "overrides": overrides }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

fn text_model(egress: &str) -> Value {
    json!({
        "input_modalities": ["text", "image", "video"], "output_modalities": ["text"],
        "context_limit": 262144, "output_limit": 32768,
        "input_price": "0.15", "output_price": "0.5",
        "reasoning": true, "tool_call": true,
        "egress_dialect": egress
    })
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "fireworks-ai", "name": "fireworks", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_catalogue_model(&env, FLASH).await;
    add_catalogue_model(&env, KIMI).await;
    add_catalogue_model(&env, EMBED).await;
    add_model(
        &env,
        "accounts/fireworks/models/kimi-k3-messages",
        text_model("anthropic_messages"),
    )
    .await;
    env
}

fn sent_to_fireworks(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    sent.json()
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn chat_apps_reach_fireworks_with_the_bearer_key_and_get_its_reasoning() {
    let env = env().await;
    let mut body = chat(
        FLASH,
        &format!(
            "What is the capital of France? {}",
            fixture("responses", "reasoning")
        ),
    );
    body["reasoning_effort"] = json!("low");
    body["safety_identifier"] = json!("user-0001");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("fireworks-ai"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["reasoning_content"],
        "The user asks for the capital of France. It is Paris."
    );
    let sent = sent_to_fireworks(&env, "/v1/chat/completions");
    assert_eq!(sent["model"], FLASH);
    assert_eq!(
        (&sent["user"], sent.get("safety_identifier")),
        (&json!("user-0001"), None),
        "Fireworks takes the end user only as `user`"
    );
}

#[tokio::test]
async fn anthropic_apps_get_fireworks_reasoning_as_thinking_in_replies_and_streams() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            FLASH,
            &format!(
                "What is the capital of France? {}",
                fixture("responses", "reasoning")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (&reply["content"][0]["type"], &reply["content"][1]["text"]),
        (
            &json!("thinking"),
            &json!("The capital of France is Paris.")
        )
    );
    assert_eq!(reply["usage"]["output_tokens"], 27);

    let mut body = messages(
        FLASH,
        &format!(
            "What is 17 times 23? {}",
            fixture("streams", "reasoning-stream")
        ),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let blocks: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "content_block_start")
        .map(|e| &e["content_block"]["type"])
        .collect();
    assert_eq!(blocks, [&json!("thinking"), &json!("text")]);
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "17 × 23 = 391.");
    let usage = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(usage["usage"]["output_tokens"], 45);
}

#[tokio::test]
async fn a_tool_call_and_its_reasoning_go_back_to_fireworks_as_reasoning_content() {
    let env = env().await;
    let tools = json!([{
        "name": "get_weather",
        "description": "Get the current weather for a city.",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    let question = format!(
        "What is the weather in Paris? {}",
        fixture("responses", "tools")
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": KIMI, "max_tokens": 512, "tools": tools,
            "messages": [{ "role": "user", "content": question }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let turn = r.json()["content"].clone();
    assert_eq!(
        (&turn[0]["type"], &turn[1]["type"]),
        (&json!("thinking"), &json!("tool_use"))
    );
    let tool_use_id = turn[1]["id"].as_str().unwrap().to_owned();

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": KIMI, "max_tokens": 512, "tools": tools,
            "messages": [
                { "role": "user", "content": "What is the weather in Paris?" },
                { "role": "assistant", "content": turn },
                { "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": tool_use_id, "content": format!("18 °C, cloudy {}", fixture("responses", "reasoning-content-sent-back")) }
                ]}
            ]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["content"][1]["text"],
        "It is 18 °C and cloudy in Paris."
    );
    let sent = sent_to_fireworks(&env, "/v1/chat/completions");
    assert_eq!(
        sent["messages"][1]["reasoning_content"],
        "The user asks for the weather in Paris. I should call get_weather with city Paris."
    );
    assert_eq!(
        sent["messages"][1]["tool_calls"][0]["id"],
        "call_0aB1cD2eF3gH4iJ5kL6mN7oP"
    );
}

#[tokio::test]
async fn a_model_sent_as_messages_reaches_fireworks_anthropic_endpoint_with_the_bearer_key() {
    let env = env().await;
    let mut body = messages(
        KIMI_AS_MESSAGES,
        &format!(
            "What is the capital of France? {}",
            "[mock:fixture=../providers/fireworks-ai/responses/thinking-effort]"
        ),
    );
    body["output_config"] = json!({ "effort": "low" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["content"][1]["text"],
        "The capital of France is Paris."
    );
    let sent = sent_to_fireworks(&env, "/v1/messages");
    assert_eq!(sent["model"], "accounts/fireworks/models/kimi-k3-messages");

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            KIMI_AS_MESSAGES,
            "Hello [mock:error=../providers/fireworks-ai/errors/recorded-invalid-api-key]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
}

#[tokio::test]
async fn fireworks_errors_reach_apps_and_a_wrong_key_is_the_refused_key() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages(FLASH, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        let body = r.json();
        assert_eq!(body["type"], "error", "{name}");
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("fireworks-ai"));
    assert!(!format!("{o:?}").contains(SECRET));
}

#[tokio::test]
async fn fireworks_embeddings_and_refused_token_counts() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["Kaeso routes model requests.", "Fireworks serves open models. [mock:fixture=providers/fireworks-ai/qwen3-embedding]"],
            "dimensions": 8
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_fireworks(&env, "/v1/embeddings");
    assert_eq!(
        (&sent["model"], &sent["dimensions"]),
        (&json!(EMBED), &json!(8))
    );
    let reply = r.json();
    assert_eq!(reply["data"].as_array().unwrap().len(), 2);
    assert_eq!(reply["usage"]["prompt_tokens"], 14);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": KIMI_AS_MESSAGES, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}
