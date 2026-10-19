#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call};
use serde_json::{Value, json};

const SECRET: &str = "tgp_v1_test-credential-0003";
const MODEL: &str = "deepseek-ai/DeepSeek-V4-Flash-0731";
const RECORDED: &str = "../providers/togetherai";

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "togetherai", "name": "togetherai", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({
                "provider": "togetherai", "model_id": MODEL, "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 1048576, "output_limit": 384000,
                    "input_price": "0.14", "output_price": "0.28", "egress_dialect": "openai_chat"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn asking(fixture: &str) -> String {
    format!("What's the weather in Paris? [mock:fixture={RECORDED}/{fixture}]")
}

fn sent_to_together(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn a_chat_app_reaches_together_and_keeps_its_extra_fields() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": MODEL,
            "messages": [{ "role": "user", "content": asking("responses/recorded-tool-call-auto") }],
            "top_k": 40,
            "min_p": 0.05,
            "repetition_penalty": 1.1,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["tool_calls"][0]["function"]["name"], "get_weather");
    assert_eq!(
        message["reasoning_content"],
        "The user wants the weather in Paris in Celsius. I'll call the get_weather tool."
    );
    assert_eq!(reply["usage"]["completion_tokens_details"]["reasoning_tokens"], 19);
    let sent = sent_to_together(&env);
    assert_eq!(sent["model"], MODEL);
    assert_eq!(
        (&sent["top_k"], &sent["min_p"], &sent["repetition_penalty"]),
        (&json!(40), &json!(0.05), &json!(1.1))
    );
}

#[tokio::test]
async fn an_anthropic_app_calls_a_together_model_by_its_prefixed_name() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": format!("togetherai/{MODEL}"),
            "max_tokens": 400,
            "messages": [{ "role": "user", "content": asking("responses/recorded-tool-call-auto") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    assert_eq!(reply["content"][0]["type"], "tool_use");
    assert_eq!(
        reply["content"][0]["input"],
        json!({ "location": "Paris, France", "unit": "celsius" })
    );
    assert_eq!(sent_to_together(&env)["model"], MODEL);
}

#[tokio::test]
async fn an_anthropic_app_streams_a_together_reply() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": MODEL,
            "max_tokens": 300,
            "stream": true,
            "messages": [{ "role": "user", "content": asking("streams/recorded-stream-usage-short") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text: String = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .filter(|e| e["delta"]["type"] == "text_delta")
        .map(|e| e["delta"]["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(text, "Earth.");
    assert!(r.text().contains("event: message_stop"), "{}", r.text());
    assert_eq!(sent_to_together(&env)["stream"], true);
}

#[tokio::test]
async fn an_anthropic_app_gets_together_only_sampling_refused_before_the_call() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": MODEL,
            "max_tokens": 300,
            "top_k": 40,
            "messages": [{ "role": "user", "content": "Name one planet." }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert!(r.text().contains("top_k"), "{}", r.text());
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn a_together_error_reaches_the_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": MODEL,
            "max_tokens": 300,
            "messages": [{ "role": "user", "content": format!("[mock:error={RECORDED}/errors/recorded-non-serverless-model]") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("non-serverless model"),
        "{body}"
    );
}
