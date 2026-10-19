mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use kaeso_testkit::mock_provider::MockProvider;
use serde_json::{Value, json};

const DEEPSEEK_SECRET: &str = "sk-test-deepseek-credential-0004";
const FLASH: &str = "deepseek-flash";

async fn env_with_deepseek() -> (Env, MockProvider) {
    let deepseek = MockProvider::start().await;
    let env = common::v1::env(
        Some(Arc::new(Capture::default())),
        Some(&[(
            "KAESO_PROVIDERS_DEEPSEEK_BASE_URL",
            deepseek.base_url.as_str(),
        )]),
    )
    .await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "deepseek", "name": "DeepSeek", "secret": DEEPSEEK_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "deepseek", "model_id": FLASH }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (env, deepseek)
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn an_openai_chat_app_gets_deepseeks_tool_call_and_reasoning_content() {
    let (env, deepseek) = env_with_deepseek().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            "Weather in Paris? [mock:fixture=providers/deepseek/recorded-thinking-tools-auto]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let message = &r.json()["choices"][0]["message"];
    assert_eq!(
        message["reasoning_content"],
        "The user wants the weather in Paris in Celsius."
    );
    assert_eq!(message["tool_calls"][0]["function"]["name"], "get_weather");
    assert_eq!(r.json()["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(r.header("kaeso-provider"), Some("deepseek"));
    let sent = deepseek.last().expect("the request reached DeepSeek");
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {DEEPSEEK_SECRET}").as_str())
    );
    assert_eq!(sent.json()["model"], FLASH);
    assert!(
        env.mock
            .received()
            .iter()
            .all(|r| r.path != "/v1/chat/completions"),
        "nothing went to the OpenAI base URL"
    );
}

#[tokio::test]
async fn an_anthropic_messages_app_pays_deepseeks_cache_hits_at_the_cache_read_price() {
    let (env, _deepseek) = env_with_deepseek().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            FLASH,
            "Refund? [mock:fixture=providers/deepseek/recorded-prompt-cache-hit]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "message");
    assert_eq!(body["stop_reason"], "end_turn");
    assert_eq!(body["usage"]["cache_read_input_tokens"], 128);
    assert_eq!(body["usage"]["input_tokens"], 154);
    assert_eq!(body["usage"]["output_tokens"], 87);
    let outcomes = env.outcomes().await;
    let o = outcomes.last().unwrap();
    assert_eq!(o.provider, Some("deepseek"));
    assert_eq!(o.usage.as_ref().unwrap().cache_read_tokens, 128);
    let prices = o.prices.as_ref().unwrap();
    assert!(prices.cache_read_price.is_some());
    assert!(prices.cache_read_price < prices.input_price);
}

#[tokio::test]
async fn an_anthropic_messages_app_streams_deepseek_without_its_reasoning() {
    let (env, _deepseek) = env_with_deepseek().await;
    let mut body = messages(
        FLASH,
        "17 * 23? [mock:fixture=providers/deepseek/recorded-thinking-stream]",
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let text: String = events
        .iter()
        .filter(|e| e["type"] == "content_block_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "391");
    assert!(
        !events
            .iter()
            .any(|e| e["content_block"]["type"] == "thinking")
    );
    assert_eq!(events.last().unwrap()["type"], "message_stop");
}

#[tokio::test]
async fn deepseeks_refusal_of_a_thinking_tool_choice_reaches_both_apps_as_400() {
    let (env, _deepseek) = env_with_deepseek().await;
    let marker = "[mock:error=providers/deepseek/recorded-thinking-tool-choice-required]";
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(FLASH, marker)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Thinking mode does not support this tool_choice")
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(FLASH, marker)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert_eq!(r.json()["error"]["type"], "invalid_request_error");
}
