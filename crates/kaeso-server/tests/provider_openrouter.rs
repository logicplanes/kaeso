#![allow(clippy::unwrap_used)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const OPENROUTER_SECRET: &str = "sk-or-v1-test-openrouter-credential-0004";

const MODEL: &str = "nvidia/nemotron-3.5-lightning:free";

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "openrouter", "name": "openrouter", "secret": OPENROUTER_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "openrouter", "model_id": MODEL }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn sent(env: &Env) -> (String, Option<String>, Value) {
    let last = env.mock.last().unwrap();
    (
        last.path.clone(),
        last.header("authorization").map(str::to_owned),
        last.json(),
    )
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn an_openai_chat_app_reaches_openrouter_and_keeps_its_fields() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            &format!("openrouter/{MODEL}"),
            "Is 91 prime? [mock:fixture=providers/openrouter/recorded-free-reasoning]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("openrouter"));
    let (path, auth, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {OPENROUTER_SECRET}").as_str())
    );
    assert_eq!(body["model"], MODEL);
    let reply = r.json();
    assert_eq!(reply["provider"], "Nvidia");
    let choice = &reply["choices"][0];
    assert_eq!(choice["message"]["content"], "No, because 91 = 7 × 13.");
    assert_eq!(choice["native_finish_reason"], "stop");
    assert!(
        choice["message"]["reasoning"]
            .as_str()
            .unwrap()
            .starts_with("Here's a thinking process")
    );
    assert_eq!(
        choice["message"]["reasoning_details"][0]["type"],
        "reasoning.text"
    );
    assert_eq!(reply["usage"]["cost"], 0);
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        418
    );
}

#[tokio::test]
async fn an_anthropic_app_gets_openrouter_tool_calls_and_streams_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Weather in Paris? [mock:fixture=providers/openrouter/recorded-free-tools]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(body["model"], MODEL);
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    let tool = &reply["content"][0];
    assert_eq!(tool["type"], "tool_use");
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(
        tool["input"],
        json!({ "location": "Paris, France", "unit": "celsius" })
    );
    assert_eq!(reply["usage"]["input_tokens"], 348);
    assert_eq!(reply["usage"]["output_tokens"], 55);

    let mut stream = messages(
        MODEL,
        "Say hi. [mock:fixture=providers/openrouter/recorded-keepalive-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(!text.contains("OPENROUTER PROCESSING"), "{text}");
    let events = sse_data(&text);
    let said: String = events
        .iter()
        .filter(|e| e["type"] == "content_block_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(said, "Hi there! 👋 How can I help you today?");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
    assert_eq!(end["usage"]["output_tokens"], 37);
}

#[tokio::test]
async fn an_error_in_the_middle_of_an_openrouter_stream_reaches_each_app_as_an_error() {
    let env = env().await;
    let mut stream = messages(
        MODEL,
        "Hello! [mock:fixture=providers/openrouter/doc-error-mid-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let error = events.iter().find(|e| e["type"] == "error").unwrap();
    assert_eq!(error["error"]["type"], "rate_limit_error");
    assert_eq!(error["error"]["message"], "Rate limit exceeded");
    assert!(!events.iter().any(|e| e["type"] == "message_stop"));

    let mut stream = chat(
        MODEL,
        "Hello! [mock:fixture=providers/openrouter/doc-error-mid-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(stream),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let last = events.last().unwrap();
    assert_eq!(last["error"]["code"], 429);
    assert_eq!(last["choices"][0]["finish_reason"], "error");
}

#[tokio::test]
async fn openrouter_refusals_reach_each_app_in_its_own_error_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            MODEL,
            "Hello! [mock:error=providers/openrouter/recorded-routing-fields]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(error["message"], "Provider returned error");
    assert_eq!(error["metadata"]["provider_name"], "Google AI Studio");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Hello! [mock:error=providers/openrouter/recorded-invalid-messages]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "Input required: specify \"prompt\" or \"messages\""
    );
}

#[tokio::test]
async fn token_counts_for_openrouter_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": MODEL,
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        format!("The model `{MODEL}` cannot count tokens.")
    );
    assert!(env.mock.received().is_empty());
}
