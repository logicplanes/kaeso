#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "stepfun-test-credential-0001";
const STEP: &str = "step-5-preview";
const FLASH: &str = "step-3.7-flash";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/stepfun-ai/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/stepfun-ai/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "stepfun-ai", "name": "stepfun", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [STEP, FLASH] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "stepfun-ai", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_stepfun(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_stepfun_with_its_field_names_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        STEP,
        &format!(
            "Hello, please introduce StepFun's artificial intelligence! {}",
            fixture("responses", "text-chat-doc")
        ),
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
    assert_eq!(r.header("kaeso-provider"), Some("stepfun-ai"));
    assert_eq!(r.json()["usage"]["completion_tokens"], 176);
    let sent = sent_to_stepfun(&env);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_tokens"], 64);
    for field in ["max_completion_tokens", "safety_identifier", "user"] {
        assert!(sent.get(field).is_none(), "{field}: {sent}");
    }
}

#[tokio::test]
async fn anthropic_apps_get_stepfun_reasoning_as_thinking_cache_hits_and_tool_calls() {
    let env = env().await;
    let mut body = messages(
        FLASH,
        &format!(
            "How many 'r's are in the word strawberry? {}",
            fixture("streams", "reasoning-stream-schema")
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
        "s-t-r-a-w-b-e-r-r-y: one r after t, two before y."
    );
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 23);
    let sent = sent_to_stepfun(&env);
    assert!(sent.get("stream_options").is_none(), "{sent}");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            STEP,
            &format!("Hello {}", fixture("responses", "cached-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let usage = &r.json()["usage"];
    assert_eq!(
        (&usage["input_tokens"], &usage["cache_read_input_tokens"]),
        (&json!(79), &json!(512))
    );

    let mut body = messages(
        STEP,
        &format!(
            "What’s the weather in Shanghai right now? {}",
            fixture("responses", "tools-doc")
        ),
    );
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Check the weather",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let call = r.json()["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .cloned()
        .unwrap();
    assert_eq!(
        (&call["name"], &call["input"]),
        (&json!("get_weather"), &json!({ "city": " Shanghai " }))
    );
}

#[tokio::test]
async fn fields_stepfun_would_drop_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("presence_penalty", json!(0.5)),
        ("seed", json!(7)),
        ("logprobs", json!(true)),
        ("store", json!(true)),
        ("prompt_cache_key", json!("chat-0001")),
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
            format!("`{field}` is not supported by StepFun; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn stepfun_counts_no_tokens() {
    let env = env().await;
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
async fn a_refused_stepfun_key_reaches_apps_as_the_providers_error() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(FLASH, &format!("Hi {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("stepfun-ai"));
    assert!(!format!("{o:?}").contains(SECRET));
}
