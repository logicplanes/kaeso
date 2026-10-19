#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const MISTRAL_SECRET: &str = "mistral-test-credential-0004";
const SMALL: &str = "mistral-small-latest";
const EMBED: &str = "mistral-embed";
const HAIKU: &str = "**Haiku:**\n*Waves crash on the shore—*\n*white foam";
const ANSWER: &str = "No, 91 is not a prime number because it can be divided evenly by 7 and 13.";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/mistral/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/mistral/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "mistral", "name": "mistral", "secret": MISTRAL_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [SMALL, EMBED] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "mistral", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_mistral(env: &Env, path: &str, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {MISTRAL_SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.header("anthropic-version"), None);
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

fn assert_no_refused_keys(sent: &Value) {
    for key in ["max_completion_tokens", "seed", "user", "safety_identifier"] {
        assert!(sent.get(key).is_none(), "{key} was sent: {sent}");
    }
    for message in sent["messages"].as_array().unwrap() {
        assert_ne!(message["role"], "developer", "{sent}");
    }
}

#[tokio::test]
async fn chat_apps_reach_mistral_in_its_own_spellings() {
    let env = env().await;
    let text = format!(
        "Write a haiku. {}",
        fixture("responses", "recorded-max-tokens-length")
    );
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": SMALL,
            "messages": [
                { "role": "developer", "content": "Be brief." },
                { "role": "user", "content": text }
            ],
            "max_completion_tokens": 16,
            "seed": 7,
            "user": "user-42",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("mistral"));
    let body = r.json();
    assert_eq!(body["choices"][0]["message"]["content"], HAIKU);
    assert_eq!(body["usage"]["completion_tokens"], 16);
    let sent = sent_to_mistral(&env, "/v1/chat/completions", SMALL);
    assert_no_refused_keys(&sent);
    assert_eq!(
        (
            &sent["messages"][0]["role"],
            &sent["max_tokens"],
            &sent["random_seed"]
        ),
        (&json!("system"), &json!(16), &json!(7))
    );
}

#[tokio::test]
async fn anthropic_apps_reach_mistral_and_get_its_thinking_and_its_answer() {
    let env = env().await;
    let mut body = messages(
        SMALL,
        &format!(
            "Is 91 prime? {}",
            fixture("responses", "recorded-reasoning-effort-high")
        ),
    );
    body["system"] = json!("Answer in one sentence.");
    body["metadata"] = json!({ "user_id": "user-42" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["type"], "message");
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("I need to determine if 91 is a prime number."),
        "{reply}"
    );
    assert!(
        reply["content"][0]["signature"]
            .as_str()
            .unwrap()
            .starts_with("kpp:reasoning:v1:")
    );
    assert_eq!(
        reply["content"][1],
        json!({ "type": "text", "text": ANSWER })
    );
    assert_eq!(reply["stop_reason"], "end_turn");
    let sent = sent_to_mistral(&env, "/v1/chat/completions", SMALL);
    assert_no_refused_keys(&sent);
    assert_eq!(
        (&sent["messages"][0]["role"], &sent["max_tokens"]),
        (&json!("system"), &json!(256))
    );

    let mut body = messages(
        SMALL,
        &format!(
            "Weather in Lisbon? {}",
            fixture("streams", "recorded-stream-tool-call")
        ),
    );
    body["stream"] = json!(true);
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Current weather for a city.",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } } }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    for part in [
        "event: message_start",
        r#""name":"get_weather","type":"tool_use""#,
        r#""stop_reason":"tool_use""#,
        "event: message_stop",
    ] {
        assert!(text.contains(part), "{part} missing from {text}");
    }
    let sent = sent_to_mistral(&env, "/v1/chat/completions", SMALL);
    assert_eq!(
        (&sent["stream"], &sent["max_tokens"]),
        (&json!(true), &json!(256))
    );
}

#[tokio::test]
async fn streamed_thinking_reaches_anthropic_apps_before_the_whole_answer() {
    let env = env().await;
    let mut body = messages(
        SMALL,
        &format!(
            "Is 91 prime? {}",
            fixture("streams", "recorded-reasoning-effort-high-stream")
        ),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .collect();
    let deltas = |kind: &str, key: &str| -> String {
        events
            .iter()
            .filter(|event| event["delta"]["type"] == kind)
            .map(|event| event["delta"][key].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(deltas("text_delta", "text"), ANSWER);
    assert!(deltas("thinking_delta", "thinking").starts_with("I need to determine if 91 is"));
    let blocks: Vec<&Value> = events
        .iter()
        .filter(|event| event["type"] == "content_block_start")
        .map(|event| &event["content_block"]["type"])
        .collect();
    assert_eq!(blocks, [&json!("thinking"), &json!("text")]);
}

#[tokio::test]
async fn responses_apps_reach_mistral_through_chat_completions() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": SMALL,
            "instructions": "Be brief.",
            "input": format!("Write a haiku. {}", fixture("responses", "recorded-max-tokens-length")),
            "max_output_tokens": 16,
            "safety_identifier": "user-42",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(body["object"], "response");
    assert!(r.text().contains("Waves crash on the shore"), "{body}");
    let sent = sent_to_mistral(&env, "/v1/chat/completions", SMALL);
    assert_no_refused_keys(&sent);
    assert_eq!(
        (&sent["messages"][0]["role"], &sent["max_tokens"]),
        (&json!("system"), &json!(16))
    );
}

#[tokio::test]
async fn fields_mistral_refuses_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let mut body = chat(SMALL, "Hi");
    body["logit_bias"] = json!({ "50256": -100 });
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(
        (&error["message"], &error["param"]),
        (
            &json!("`logit_bias` is not supported by Mistral; remove it from the request."),
            &json!("logit_bias")
        )
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": SMALL,
            "messages": [{ "role": "user", "content": "Hi", "name": "alice" }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "messages[0].name");

    let mut body = chat(SMALL, "Say hello.");
    body["modalities"] = json!(["text", "audio"]);
    body["audio"] = json!({ "voice": "alloy", "format": "mp3" });
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
        "`audio` is not supported by Mistral; remove it from the request."
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": SMALL,
            "max_tokens": 64,
            "messages": [
                { "role": "user", "content": "Name a colour." },
                { "role": "assistant", "content": "The colour is" }
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        (&body["type"], &body["error"]["message"]),
        (
            &json!("error"),
            &json!(
                "Mistral cannot continue a prefilled assistant message; end the conversation with a user message."
            )
        )
    );

    let mut body = messages(SMALL, "Is 91 prime?");
    body["max_tokens"] = json!(4096);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 2048 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `mistral-small-latest` supports the effort levels `none`, `high`; a thinking budget of 2048 tokens becomes `low`, so send a `budget_tokens` of at least 16384."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn mistral_errors_reach_apps_in_their_own_format() {
    let env = env().await;
    let text = format!("Hi {}", error("recorded-model-not-found"));
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(SMALL, &text)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
    assert!(
        message.starts_with(
            "The provider returned an error (HTTP 400) that is not in the OpenAI error format:"
        ) && message.contains("Invalid model: mistral-tiny-nonexistent"),
        "{message}"
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(SMALL, &text)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        (&body["type"], &body["error"]["type"]),
        (&json!("error"), &json!("invalid_request_error"))
    );
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid model: mistral-tiny-nonexistent"),
        "{body}"
    );
}

#[tokio::test]
async fn embeddings_come_back_as_base64_when_the_app_asks_although_mistral_sends_floats() {
    let env = env().await;
    let input = format!(
        "Base64 was asked for. {}",
        fixture("responses", "recorded-embed-base64-ignored")
    );
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": input, "encoding_format": "base64" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("mistral"));
    let body = r.json();
    assert_eq!(body["model"], EMBED);
    let vector = body["data"][0]["embedding"].as_str().unwrap();
    assert_eq!(vector.len(), 5464);
    let sent = sent_to_mistral(&env, "/v1/embeddings", EMBED);
    assert_eq!(
        (&sent["input"], &sent["encoding_format"]),
        (&json!(input), &json!("base64"))
    );
}

#[tokio::test]
async fn token_counts_for_mistral_models_are_refused_in_the_apps_format() {
    let env = env().await;
    env.mock.clear();
    let expected = format!("The model `{SMALL}` cannot count tokens.");
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": SMALL, "messages": [{ "role": "user", "content": "Hi" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["message"], expected);
    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": SMALL, "input": "Hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        (&r.json()["error"]["message"], &r.json()["error"]["param"]),
        (&json!(expected), &json!("model"))
    );
    assert!(env.mock.received().is_empty());
}
