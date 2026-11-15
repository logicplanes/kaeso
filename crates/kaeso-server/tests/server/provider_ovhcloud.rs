#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "ovhcloud-test-credential-0001";
const GPT_OSS: &str = "gpt-oss-120b";
const GPT_OSS_SMALL: &str = "gpt-oss-20b";
const QWEN: &str = "Qwen3.5-9B";
const MISTRAL: &str = "Mistral-Small-3.2-24B-Instruct-2506";
const EMBED: &str = "Qwen3-Embedding-8B";
const KEY_REFUSED: &str = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/ovhcloud/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/ovhcloud/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "ovhcloud", "model_id": model }),
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
            json!({ "provider": "ovhcloud", "name": "ovhcloud", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GPT_OSS, GPT_OSS_SMALL, QWEN, MISTRAL, EMBED] {
        add_model(&env, model).await;
    }
    env
}

fn sent_to_ovhcloud(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_ovhcloud_with_system_and_no_end_user_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        GPT_OSS_SMALL,
        &format!("Hi {}", fixture("responses", "recorded-greeting")),
    );
    body["messages"].as_array_mut().unwrap().insert(
        0,
        json!({ "role": "developer", "content": "You answer in one short sentence." }),
    );
    body["max_completion_tokens"] = json!(256);
    body["safety_identifier"] = json!("user-0001");
    body["user"] = json!("user-0001");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("ovhcloud"));
    let reply = r.json();
    assert_eq!(reply["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(reply["usage"]["completion_tokens"], 51);
    let sent = sent_to_ovhcloud(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_completion_tokens"], 256);
    for field in ["user", "safety_identifier", "max_tokens"] {
        assert!(sent.get(field).is_none(), "{field}: {sent}");
    }
}

#[tokio::test]
async fn anthropic_apps_get_ovhcloud_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
    let mut body = messages(
        GPT_OSS_SMALL,
        &format!(
            "Say hello in three words. {}",
            fixture("streams", "recorded-reasoning-stream")
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
    assert_eq!(thinking, "Just greet in 3 words.");
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Hello, how are?");
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 22);

    let mut body = messages(
        GPT_OSS,
        &format!(
            "What is the weather in Paris? {}",
            fixture("responses", "recorded-tools")
        ),
    );
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Get the current weather in a city",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    body["tool_choice"] = json!({ "type": "any" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (
            &reply["content"][0]["type"],
            &reply["content"][0]["thinking"]
        ),
        (&json!("thinking"), &json!("We need to call function."))
    );
    let call = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(
        (&call["name"], &call["input"]),
        (&json!("get_weather"), &json!({ "city": "Paris" }))
    );
    assert_eq!(reply["stop_reason"], "tool_use");
    let sent = sent_to_ovhcloud(&env, "/v1/chat/completions");
    assert_eq!(sent["tool_choice"], "required");
}

#[tokio::test]
async fn a_reply_cut_while_thinking_reaches_anthropic_apps_as_thinking_cut_by_the_limit() {
    let env = env().await;
    let body = messages(
        QWEN,
        &format!(
            "What is 17 times 23? {}",
            fixture("responses", "recorded-thinking-length")
        ),
    );
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    assert_eq!(reply["stop_reason"], "max_tokens");
    let sent = sent_to_ovhcloud(&env, "/v1/chat/completions");
    assert_eq!(sent["model"], QWEN);
}

#[tokio::test]
async fn fields_ovhcloud_would_drop_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("metadata", json!({ "team": "search" })),
        ("store", json!(true)),
        (
            "prediction",
            json!({ "type": "content", "content": "Hello" }),
        ),
        ("web_search_options", json!({})),
        ("prompt_cache_key", json!("greeting")),
    ] {
        env.mock.clear();
        let mut body = chat(GPT_OSS, "Hello");
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
            format!(
                "`{field}` is not supported by OVHcloud AI Endpoints; remove it from the request."
            )
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn a_responses_apps_cache_key_is_left_out_for_ovhcloud() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": GPT_OSS_SMALL,
            "input": format!("Hi {}", fixture("responses", "recorded-greeting")),
            "prompt_cache_key": "codex-session-1",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("prompt_cache_key"));
    let sent = sent_to_ovhcloud(&env, "/v1/chat/completions");
    assert!(sent.get("prompt_cache_key").is_none(), "{sent}");
}

#[tokio::test]
async fn ovhcloud_embeds_with_dimensions_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["Hello", format!("Bonjour {}", fixture("responses", "recorded-qwen3-dims"))],
            "dimensions": 32,
            "encoding_format": "float"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["usage"]["prompt_tokens"], 4);
    assert_eq!(reply["data"][1]["embedding"].as_array().unwrap().len(), 32);
    let sent = sent_to_ovhcloud(&env, "/v1/embeddings");
    assert_eq!(
        (&sent["model"], &sent["dimensions"]),
        (&json!(EMBED), &json!(32))
    );

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GPT_OSS, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_refused_ovhcloud_key_is_reported_as_refused_and_its_other_errors_reach_apps() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT_OSS_SMALL,
            &format!("Hi {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], KEY_REFUSED);

    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": format!("Hello {}", error("recorded-invalid-api-key")),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], KEY_REFUSED);

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            MISTRAL,
            &format!(
                "Capital of France? {}",
                error("recorded-developer-role-mistral")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider returned an error (HTTP 400): Unknown message role: developer."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT_OSS_SMALL,
            &format!("Hi {}", error("recorded-service-tier")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("is not currently supported"),
        "{}",
        r.text()
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("ovhcloud"));
    assert!(!format!("{o:?}").contains(SECRET));
}
