#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "deepinfra-test-credential-0001";
const FLASH: &str = "deepseek-ai/DeepSeek-V4-Flash-0731";
const GPT_OSS: &str = "openai/gpt-oss-120b";
const EMBED: &str = "BAAI/bge-m3";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/deepinfra/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/deepinfra/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "deepinfra", "model_id": model }),
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
            json!({ "provider": "deepinfra", "name": "deepinfra", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [FLASH, GPT_OSS, EMBED] {
        add_model(&env, model).await;
    }
    env
}

fn sent_to_deepinfra(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_deepinfra_with_its_field_names_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        GPT_OSS,
        &format!("Hello! {}", fixture("responses", "greeting-published")),
    );
    body["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({ "role": "developer", "content": "Be kind." }));
    body["max_completion_tokens"] = json!(64);
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
    assert_eq!(r.header("kaeso-provider"), Some("deepinfra"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello! How can I help you today?"
    );
    assert_eq!(reply["usage"]["completion_tokens"], 28);
    let sent = sent_to_deepinfra(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_tokens"], &sent["user"]),
        (&json!(64), &json!("user-0001"))
    );
    assert!(
        sent.get("max_completion_tokens").is_none() && sent.get("safety_identifier").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_deepinfra_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
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
    let events: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect();
    let thinking: String = events
        .iter()
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    assert_eq!(thinking, "17 times 20 is 340, plus 51 is 391.");
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 31);

    let mut body = messages(
        FLASH,
        &format!(
            "What is the weather in San Francisco? {}",
            fixture("responses", "tools-doc")
        ),
    );
    body["tools"] = json!([{
        "name": "get_current_weather",
        "description": "Get the current weather in a given location",
        "input_schema": { "type": "object", "properties": { "location": { "type": "string" } }, "required": ["location"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (&reply["content"][0]["type"], &reply["content"][1]["input"]),
        (&json!("thinking"), &json!({ "location": "San Francisco" }))
    );
    assert_eq!(reply["stop_reason"], "tool_use");
}

#[tokio::test]
async fn fields_deepinfra_would_drop_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("logit_bias", json!({ "50256": -100 })),
        ("top_logprobs", json!(2)),
        ("store", json!(true)),
        ("metadata", json!({ "team": "search" })),
    ] {
        env.mock.clear();
        let mut body = chat(FLASH, "Hello");
        body[field] = value;
        if field == "top_logprobs" {
            body["logprobs"] = json!(true);
        }
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(body),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{field}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!("`{field}` is not supported by DeepInfra; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn deepinfra_embeds_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["The food was delicious and the waiter was friendly. [mock:fixture=providers/deepinfra/bge-m3]"],
            "encoding_format": "float"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["usage"]["prompt_tokens"], 24);
    let sent = sent_to_deepinfra(&env, "/v1/embeddings");
    assert_eq!(sent["model"], EMBED);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": FLASH, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn deepinfra_errors_reach_apps_and_a_key_without_its_scope_is_not_allowed() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(FLASH, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            &format!("Hello {}", error("missing-scope-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert!(
        r.json()["error"]["message"].as_str().unwrap().starts_with(
            "The provider doesn't allow this request with this project's provider key"
        ),
        "{}",
        r.text()
    );
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            &format!("Hello {}", error("engine-overloaded-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("deepinfra"));
    assert!(!format!("{o:?}").contains(SECRET));
}
