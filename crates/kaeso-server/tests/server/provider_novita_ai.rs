#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "sk_novita-test-credential-0001";
const FLASH: &str = "deepseek/deepseek-v4.1-flash";
const R1: &str = "deepseek/deepseek-r1-0528";
const MINIMAX: &str = "minimax/minimax-m3";
const KIMI: &str = "moonshotai/kimi-k2.5";
const EMBED: &str = "baai/bge-m3";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/novita-ai/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/novita-ai/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "novita-ai", "model_id": model }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "novita-ai", "name": "novita-ai", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [FLASH, R1, MINIMAX, KIMI, EMBED] {
        add_model(&env, model).await;
    }
    env
}

fn sent_to_novita(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_novita_ai_with_its_field_names_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        R1,
        &format!("Hello, world! {}", fixture("responses", "greeting-doc")),
    );
    body["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({ "role": "developer", "content": "Be kind." }));
    body["max_completion_tokens"] = json!(400);
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
    assert_eq!(r.header("kaeso-provider"), Some("novita-ai"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello! 👋 How can I assist you today? 😊"
    );
    assert_eq!(reply["usage"]["completion_tokens"], 13);
    let sent = sent_to_novita(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_tokens"], &sent["separate_reasoning"]),
        (&json!(400), &json!(true))
    );
    for field in ["max_completion_tokens", "safety_identifier", "user"] {
        assert!(sent.get(field).is_none(), "{field}: {sent}");
    }
}

#[tokio::test]
async fn anthropic_apps_get_novita_ai_reasoning_as_thinking_and_can_turn_thinking_off() {
    let env = env().await;
    let mut body = messages(
        FLASH,
        &format!(
            "Explain Newton's Second Law. {}",
            fixture("streams", "reasoning-stream-doc")
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
    assert_eq!(
        thinking,
        "Force, mass and acceleration: F equals m times a."
    );
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 48);
    let sent = sent_to_novita(&env, "/v1/chat/completions");
    assert_eq!(sent["separate_reasoning"], true);

    env.mock.clear();
    let mut body = messages(
        KIMI,
        &format!(
            "What is the capital of France? {}",
            fixture("responses", "greeting-doc")
        ),
    );
    body["thinking"] = json!({ "type": "disabled" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_novita(&env, "/v1/chat/completions");
    assert_eq!(sent["enable_thinking"], false);
    assert!(sent.get("reasoning_effort").is_none(), "{sent}");
}

#[tokio::test]
async fn anthropic_apps_get_an_interleaved_tool_call_with_its_thinking() {
    let env = env().await;
    let mut body = messages(
        MINIMAX,
        &format!(
            "How's the weather in ShangHai? {}",
            fixture("responses", "interleaved-tool-call-doc")
        ),
    );
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Get weather for a specific location",
        "input_schema": { "type": "object", "properties": { "location": { "type": "string" } }, "required": ["location"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (
            &reply["content"][0]["type"],
            &reply["content"][0]["thinking"]
        ),
        (
            &json!("thinking"),
            &json!("The user asked for Shanghai weather...")
        )
    );
    let call_block = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(call_block["input"], json!({ "location": "ShangHai" }));
    assert_eq!(reply["stop_reason"], "tool_use");
}

#[tokio::test]
async fn fields_novita_ai_would_drop_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("store", json!(true)),
        ("metadata", json!({ "team": "search" })),
        ("prompt_cache_key", json!("contract-42")),
        ("parallel_tool_calls", json!(false)),
        ("service_tier", json!("priority")),
    ] {
        env.mock.clear();
        let mut body = chat(FLASH, "Hello");
        body[field] = value;
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
            format!("`{field}` is not supported by Novita AI; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn novita_ai_embeds_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["The food was delicious.", "The waiter was friendly. [mock:fixture=providers/novita-ai/bge-m3]"],
            "encoding_format": "float"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["usage"]["prompt_tokens"], 14);
    let sent = sent_to_novita(&env, "/v1/embeddings");
    assert_eq!(sent["model"], EMBED);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": "Hello", "dimensions": 256 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "`dimensions` is not supported by Novita AI; remove it from the request."
    );
    assert!(env.mock.received().is_empty());

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
async fn novita_ai_errors_reach_apps_in_their_format_with_novitas_message() {
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
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            FLASH,
            &format!("Hello {}", error("rate-limit-exceeded-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("rate limit exceeded, please retry later"),
        "{}",
        r.text()
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("novita-ai"));
    assert!(!format!("{o:?}").contains(SECRET));
}
