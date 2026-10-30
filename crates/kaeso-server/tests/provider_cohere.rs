#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const COHERE_SECRET: &str = "cohere-test-credential-0006";
const COMMAND: &str = "command-a-03-2025";
const REASONING: &str = "command-a-reasoning-08-2025";
const EMBED: &str = "embed-v4.0";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/cohere/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/cohere/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "cohere", "name": "cohere", "secret": COHERE_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [COMMAND, REASONING] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "cohere", "model_id": model }),
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
                "provider": "cohere", "model_id": EMBED, "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 128000, "output_limit": 1536,
                    "input_price": "0.12", "output_price": "0",
                    "egress_dialect": "openai_embeddings"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn sent_to_cohere(env: &Env, path: &str, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {COHERE_SECRET}").as_str())
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
async fn chat_apps_reach_cohere_with_the_bearer_key_and_a_stop_string_as_a_list() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": COMMAND,
            "messages": [
                { "role": "developer", "content": "Answer in one short sentence." },
                { "role": "user", "content": format!("What is the capital of France? {}", fixture("responses", "recorded-developer-role")) }
            ],
            "max_completion_tokens": 64,
            "seed": 7,
            "stop": "6",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("cohere"));
    let body = r.json();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "The capital of France is Paris."
    );
    let sent = sent_to_cohere(&env, "/v1/chat/completions", COMMAND);
    assert_eq!(
        (
            &sent["messages"][0]["role"],
            &sent["max_completion_tokens"],
            &sent["seed"],
            &sent["stop"]
        ),
        (&json!("developer"), &json!(64), &json!(7), &json!(["6"]))
    );
}

#[tokio::test]
async fn anthropic_apps_get_cohere_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            REASONING,
            &format!(
                "What is 17 times 23? {}",
                fixture("responses", "recorded-reasoning")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("First, I need to multiply the units digits of 17 and 23"),
        "{reply}"
    );
    assert_eq!(reply["content"][1]["type"], "text");
    assert_eq!(
        reply["usage"]["output_tokens"],
        json!(
            serde_json::from_slice::<Value>(
                &std::fs::read(
                    kaeso_testkit::mock_provider::kpp_fixtures()
                        .join("openai_chat/providers/cohere/responses/recorded-reasoning.json")
                )
                .unwrap()
            )
            .unwrap()["usage"]["completion_tokens"]
        )
    );

    let mut body = messages(
        REASONING,
        &format!(
            "What is 2 plus 3? {}",
            fixture("streams", "recorded-reasoning-stream")
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
    let thinking: String = events
        .iter()
        .filter(|e| e["delta"]["type"] == "thinking_delta")
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    assert!(!thinking.is_empty());
    let text: String = events
        .iter()
        .filter(|e| e["delta"]["type"] == "text_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "5");

    let mut body = messages(
        COMMAND,
        &format!(
            "What is the weather in Paris right now? {}",
            fixture("streams", "recorded-tools-stream")
        ),
    );
    body["stream"] = json!(true);
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Get the current weather for a city.",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    for part in [
        r#""name":"get_weather","type":"tool_use""#,
        r#"{\"city\":\"Paris\"}"#,
        r#""stop_reason":"tool_use""#,
    ] {
        assert!(text.contains(part), "{part} missing from {text}");
    }
    let sent = sent_to_cohere(&env, "/v1/chat/completions", COMMAND);
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["tools"][0]["function"]["name"], "get_weather");
}

#[tokio::test]
async fn reasoning_effort_reaches_cohere_only_as_none_or_high() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": REASONING,
            "input": format!("What is 17 times 23? {}", fixture("responses", "recorded-reasoning")),
            "reasoning": { "effort": "high" },
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cohere(&env, "/v1/chat/completions", REASONING);
    assert_eq!(sent["reasoning_effort"], "high");

    env.mock.clear();
    let mut body = chat(REASONING, "Hello");
    body["reasoning_effort"] = json!("medium");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(message.contains("`none`, `high`"), "{message}");
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn fields_cohere_refuses_or_ignores_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("logit_bias", json!({ "100": 1 })),
        ("parallel_tool_calls", json!(false)),
        ("store", json!(true)),
        ("metadata", json!({ "purpose": "example" })),
        (
            "functions",
            json!([{ "name": "get_weather", "parameters": { "type": "object" } }]),
        ),
        ("web_search_options", json!({})),
        ("verbosity", json!("low")),
    ] {
        env.mock.clear();
        let mut body = chat(COMMAND, "Hello");
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
        let error = &r.json()["error"];
        assert_eq!(
            (&error["message"], &error["param"]),
            (
                &json!(format!(
                    "`{field}` is not supported by Cohere; remove it from the request."
                )),
                &json!(field)
            ),
            "{field}"
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn cohere_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            COMMAND,
            &format!("Hello {}", error("recorded-unknown-model")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "not_found_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("model 'kaeso-example-no-such-model' not found"),
        "{body}"
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            COMMAND,
            &format!("Hello {}", error("recorded-penalties-together")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider returned an error (HTTP 400): invalid request: frequency_penalty with presence_penalty is not supported for this model."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            COMMAND,
            &format!("Hello {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("cohere"));
    assert!(!format!("{o:?}").contains(COHERE_SECRET));
}

#[tokio::test]
async fn token_counts_for_cohere_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": COMMAND,
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        format!("The model `{COMMAND}` cannot count tokens.")
    );
    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": COMMAND, "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn cohere_embed_models_answer_embeddings_and_refuse_generation() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": "The quick brown fox. [mock:fixture=providers/cohere/recorded-embed-dimensions]",
            "dimensions": 256,
            "encoding_format": "float"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cohere(&env, "/v1/embeddings", EMBED);
    assert_eq!(sent["dimensions"], 256);
    let reply = r.json();
    assert_eq!(reply["data"][0]["embedding"].as_array().unwrap().len(), 256);
    assert_eq!(reply["usage"]["prompt_tokens"], 5);

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
