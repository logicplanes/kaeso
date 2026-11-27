#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! A request reaches a provider that serves its dialect natively as it came: an Anthropic app's
//! Messages request goes to DeepSeek's, Moonshot's or QwenCloud's own Messages API instead of
//! being translated into Chat Completions, while a model sent as another format keeps it.

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use kaeso_kpp::DialectId;
use serde_json::{Value, json};

const SECRET: &str = "sk-test-native-dialect-credential-0000";

async fn env_with(provider: &str, models: &[(&str, Value)]) -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": provider, "name": provider, "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for (model, overrides) in models {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": provider, "model_id": model, "overrides": overrides }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

async fn egress(env: &Env) -> Option<DialectId> {
    env.outcomes().await.last().unwrap().egress
}

#[tokio::test]
async fn an_anthropic_app_reaches_deepseeks_messages_api_as_it_came() {
    let env = env_with("deepseek", &[("deepseek-flash", json!({}))]).await;
    let mut body = messages("deepseek-flash", "Hello there.");
    body["system"] = json!([
        { "type": "text", "text": "Be brief.", "cache_control": { "type": "ephemeral" } }
    ]);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 1024 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("deepseek"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/anthropic/v1/messages");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    let sent = sent.json();
    assert_eq!(sent["model"], "deepseek-flash");
    assert_eq!(sent["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(sent["thinking"]["type"], "enabled");
    assert!(sent.get("messages").is_some());
    assert_eq!(r.json()["type"], "message");
    assert_eq!(egress(&env).await, Some(DialectId::AnthropicMessages));
}

#[tokio::test]
async fn an_anthropic_apps_stream_reaches_deepseeks_messages_api_and_streams_back() {
    let env = env_with("deepseek", &[("deepseek-flash", json!({}))]).await;
    let mut body = messages("deepseek-flash", "Hello there.");
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/anthropic/v1/messages");
    let text = r.text();
    assert!(text.contains("event: message_start"), "{text}");
    assert!(text.contains("event: message_stop"), "{text}");
}

#[tokio::test]
async fn a_chat_app_still_reaches_deepseeks_chat_completions() {
    let env = env_with("deepseek", &[("deepseek-flash", json!({}))]).await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("deepseek-flash", "Hello there.")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/chat/completions");
    assert_eq!(egress(&env).await, Some(DialectId::OpenAiChat));
}

#[tokio::test]
async fn a_model_sent_as_chat_completions_keeps_the_translation() {
    let env = env_with(
        "deepseek",
        &[("deepseek-flash", json!({ "egress_dialect": "openai_chat" }))],
    )
    .await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("deepseek-flash", "Hello there.")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/chat/completions");
    assert_eq!(r.json()["type"], "message");
    assert_eq!(egress(&env).await, Some(DialectId::OpenAiChat));
}

#[tokio::test]
async fn what_deepseek_ignores_is_refused_before_anything_is_sent() {
    let env = env_with("deepseek", &[("deepseek-flash", json!({}))]).await;
    let mut body = messages("deepseek-flash", "Hello there.");
    body["top_k"] = json!(5);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("top_k"), "{}", r.text());
    assert!(env.mock.last().is_none());
}

#[tokio::test]
async fn an_anthropic_apps_custom_tools_reach_deepseek_without_their_type() {
    let env = env_with("deepseek", &[("deepseek-flash", json!({}))]).await;
    let mut body = messages("deepseek-flash", "Weather in Paris?");
    body["tools"] = json!([{
        "type": "custom", "name": "get_weather",
        "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), None);
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/anthropic/v1/messages");
    assert_eq!(
        sent.json()["tools"],
        json!([{
            "name": "get_weather",
            "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
        }])
    );
}

#[tokio::test]
async fn only_moonshots_k3_models_go_to_its_messages_api() {
    let env = env_with(
        "moonshotai",
        &[("kimi-k3", json!({})), ("kimi-k2.6", json!({}))],
    )
    .await;
    for (model, path, dialect) in [
        (
            "kimi-k3",
            "/anthropic/v1/messages",
            DialectId::AnthropicMessages,
        ),
        ("kimi-k2.6", "/v1/chat/completions", DialectId::OpenAiChat),
    ] {
        let r = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages(model, "Hello there.")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{model}: {}", r.text());
        assert_eq!(env.mock.last().unwrap().path, path, "{model}");
        assert_eq!(egress(&env).await, Some(dialect), "{model}");
    }
}

#[tokio::test]
async fn qwens_messages_api_is_at_the_root_of_the_host() {
    let env = env_with("alibaba", &[("qwen3.7-flash", json!({}))]).await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("qwen3.7-flash", "Hello there.")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/apps/anthropic/v1/messages");
    assert_eq!(sent.json()["model"], "qwen3.7-flash");
}

#[tokio::test]
async fn deepinfra_counts_an_anthropic_apps_tokens_at_its_messages_api() {
    let env = env_with(
        "deepinfra",
        &[("deepseek-ai/DeepSeek-V4-Flash-0731", json!({}))],
    )
    .await;
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": "deepseek-ai/DeepSeek-V4-Flash-0731",
            "messages": [{ "role": "user", "content": "Hello there." }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        env.mock.last().unwrap().path,
        "/anthropic/v1/messages/count_tokens"
    );
    assert!(r.json()["input_tokens"].as_u64().is_some(), "{}", r.text());
}

#[tokio::test]
async fn ollama_takes_no_forced_tool_choice_on_its_messages_api() {
    let env = env_with("ollama-cloud", &[("gpt-oss:20b", json!({}))]).await;
    let mut body = messages("gpt-oss:20b", "Weather in Paris?");
    body["tools"] = json!([{
        "name": "get_weather",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } } }
    }]);
    body["tool_choice"] = json!({ "type": "any" });
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(body.clone()),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.last().is_none());
    body["tool_choice"] = json!({ "type": "auto" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/messages");
}
