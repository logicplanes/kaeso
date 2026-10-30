#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "sk-ws-0123456789abcdef-alibaba-test-credential";
const FLASH: &str = "qwen3.7-flash";
const EFFORT: &str = "qwen3.8-flash";
const EMBED: &str = "text-embedding-v4";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/alibaba/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/alibaba/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "alibaba", "name": "model studio", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [FLASH, EFFORT] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "alibaba", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({
                "provider": "alibaba", "model_id": EMBED, "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 8192, "output_limit": 2048,
                    "input_price": "0.07", "output_price": "0",
                    "egress_dialect": "openai_embeddings"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn sent_to_alibaba(env: &Env, path: &str, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn a_chat_app_reaches_qwen_with_the_bearer_key_and_developer_messages_as_system() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": FLASH,
            "max_completion_tokens": 200,
            "messages": [
                { "role": "developer", "content": "You answer in one short sentence." },
                { "role": "user", "content": fixture("responses", "recorded-basic") },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert!(
        reply["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .starts_with("No, 1001 is not a prime number"),
        "{reply}"
    );
    assert_eq!(reply["usage"]["prompt_tokens"], 34);
    let sent = sent_to_alibaba(&env, "/v1/chat/completions", FLASH);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_completion_tokens"], 200);
    assert!(sent.get("enable_thinking").is_none(), "{sent}");
}

#[tokio::test]
async fn an_anthropic_apps_thinking_reaches_qwen_as_enable_thinking_with_its_budget() {
    let env = env().await;
    let mut body = messages(
        FLASH,
        &format!(
            "What is 17 times 23? {}",
            fixture("responses", "recorded-thinking")
        ),
    );
    body["max_tokens"] = json!(4096);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 2048 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    assert!(
        !reply["content"][0]["thinking"].as_str().unwrap().is_empty(),
        "{reply}"
    );
    assert_eq!(reply["content"][1]["text"], "391");
    let sent = sent_to_alibaba(&env, "/v1/chat/completions", FLASH);
    assert_eq!(sent["enable_thinking"], true, "{sent}");
    assert_eq!(sent["thinking_budget"], 2048, "{sent}");
    assert_eq!(sent["max_completion_tokens"], 4096, "{sent}");
    assert!(sent.get("reasoning_effort").is_none(), "{sent}");

    let mut body = messages(
        FLASH,
        &format!("Is 1001 prime? {}", fixture("responses", "recorded-basic")),
    );
    body["thinking"] = json!({ "type": "disabled" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][0]["type"], "text");
    let sent = sent_to_alibaba(&env, "/v1/chat/completions", FLASH);
    assert_eq!(sent["enable_thinking"], false, "{sent}");
    assert!(sent.get("thinking_budget").is_none(), "{sent}");
}

#[tokio::test]
async fn an_anthropic_app_streams_thinking_then_the_answer_with_usage() {
    let env = env().await;
    let mut body = messages(
        FLASH,
        &format!(
            "What is 17 times 23? {}",
            fixture("streams", "recorded-thinking-stream")
        ),
    );
    body["max_tokens"] = json!(1000);
    body["stream"] = json!(true);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 1024 });
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
        .filter(|e| e["delta"]["type"] == "text_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "391");
    let delta = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(delta["usage"]["output_tokens"], 105, "{delta}");
    let sent = sent_to_alibaba(&env, "/v1/chat/completions", FLASH);
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["stream_options"]["include_usage"], true);
    assert_eq!(sent["thinking_budget"], 1024);
}

#[tokio::test]
async fn a_responses_effort_reaches_only_models_with_effort_levels() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": EFFORT,
            "reasoning": { "effort": "low" },
            "input": format!("What is 17 times 23? {}", fixture("responses", "recorded-reasoning-effort")),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let output = &r.json()["output"];
    assert_eq!(output[0]["type"], "reasoning", "{output}");
    assert_eq!(output[1]["content"][0]["text"], "391", "{output}");
    let sent = sent_to_alibaba(&env, "/v1/chat/completions", EFFORT);
    assert_eq!(sent["reasoning_effort"], "low");
    assert_eq!(sent["enable_thinking"], true);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": FLASH, "reasoning": { "effort": "low" }, "input": "Hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{FLASH}` does not accept an effort level; `low` cannot be sent.")
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_responses_app_gets_parallel_tool_calls_and_a_chat_stream_its_usage() {
    let env = env().await;
    let tool = |name: &str| {
        json!({
            "type": "function", "name": name, "description": "Look something up for a city.",
            "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
        })
    };
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": FLASH,
            "input": fixture("responses", "recorded-tools"),
            "tools": [tool("get_weather"), tool("get_time")],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let output = r.json()["output"].clone();
    let names: Vec<&str> = output
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "function_call")
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["get_weather", "get_time"]);

    let mut body = chat(FLASH, &fixture("streams", "recorded-stream"));
    body["stream"] = json!(true);
    body["stream_options"] = json!({ "include_usage": true });
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let text: String = events
        .iter()
        .filter_map(|e| e["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(text, "1 2 3 4 5");
    assert!(
        events
            .iter()
            .any(|e| e["usage"]["completion_tokens"].is_number()),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn fields_model_studio_ignores_are_refused_before_it_is_called() {
    let env = env().await;
    for (field, value) in [
        ("logit_bias", json!({ "17": 100 })),
        ("store", json!(true)),
        ("web_search_options", json!({})),
        (
            "prediction",
            json!({ "type": "content", "content": "Hello" }),
        ),
    ] {
        let mut body = chat(FLASH, "Hi");
        body[field] = value;
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(body),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!("`{field}` is not supported by QwenCloud; remove it from the request.")
        );
    }
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn model_studio_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(FLASH, &error("recorded-invalid-api-key"))),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(
        body["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(FLASH, &error("recorded-unknown-model"))),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not exist or you do not have access to it"),
        "{}",
        r.text()
    );

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": FLASH, "input": error("recorded-stream-only-model") })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("This model only support stream mode"),
        "{}",
        r.text()
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(FLASH, &error("recorded-free-tier-exhausted"))),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
}

#[tokio::test]
async fn token_counts_for_alibaba_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": FLASH,
            "messages": [{ "role": "user", "content": "Name one planet." }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(
        body["error"]["message"],
        format!("The model `{FLASH}` cannot count tokens.")
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn model_studio_embedding_models_answer_embeddings_and_refuse_generation() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": [
                "The quick brown fox. [mock:fixture=providers/alibaba/recorded-embed-dimensions]",
                "jumps over the lazy dog."
            ],
            "dimensions": 256
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_alibaba(&env, "/v1/embeddings", EMBED);
    assert_eq!(sent["dimensions"], 256);
    let reply = r.json();
    assert_eq!(reply["data"].as_array().unwrap().len(), 2);
    assert_eq!(reply["data"][0]["embedding"].as_array().unwrap().len(), 256);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(EMBED, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{EMBED}` is an embeddings model; it cannot generate text.")
    );
    assert!(env.mock.received().is_empty());
}
