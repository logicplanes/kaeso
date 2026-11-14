#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "sambanova-test-credential-0001";
const GPT_OSS: &str = "gpt-oss-120b";
const GEMMA: &str = "gemma-4-31B-it";
const GLM_AS_MESSAGES: &str = "GLM-5.3";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/sambanova/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/sambanova/errors/{name}]")
}

async fn add_model(env: &Env, model: &str, overrides: Value) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "sambanova", "model_id": model, "overrides": overrides }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "sambanova", "name": "sambanova", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_model(&env, GPT_OSS, json!({})).await;
    add_model(&env, GEMMA, json!({ "tool_call": true })).await;
    add_model(
        &env,
        GLM_AS_MESSAGES,
        json!({ "egress_dialect": "anthropic_messages" }),
    )
    .await;
    env
}

fn sent_to_sambanova(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_sambanova_and_get_its_reply_with_its_timing_usage() {
    let env = env().await;
    let mut body = chat(
        GPT_OSS,
        &format!(
            "create a poem using palindromes {}",
            fixture("responses", "palindrome-poem-doc")
        ),
    );
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
    assert_eq!(r.header("kaeso-provider"), Some("sambanova"));
    let reply = r.json();
    assert_eq!(reply["usage"]["completion_tokens"], 350);
    assert_eq!(reply["usage"]["time_to_first_token"], 0.06312894821166992);
    let sent = sent_to_sambanova(&env, "/v1/chat/completions");
    assert!(
        sent.get("safety_identifier").is_none() && sent.get("user").is_none(),
        "SambaNova declares no end-user field: {sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_sambanova_reasoning_as_thinking_and_the_usage_of_its_last_chunk() {
    let env = env().await;
    let mut body = messages(
        GPT_OSS,
        &format!(
            "What is 17 times 23? {}",
            fixture("streams", "reasoning-stream")
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
    assert_eq!(thinking, "17 times 20 is 340, plus 51 is 391.");
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 31);

    let mut body = messages(
        GEMMA,
        &format!(
            "Call the tool to get the integer 7. {}",
            fixture("responses", "thinking-tool-call-doc")
        ),
    );
    body["tools"] = json!([{
        "name": "return_integer",
        "description": "Returns the integer it is given.",
        "input_schema": { "type": "object", "properties": { "value": { "type": "integer" } }, "required": ["value"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (&reply["content"][0]["type"], &reply["content"][1]["input"]),
        (&json!("thinking"), &json!({ "value": 7 }))
    );
}

#[tokio::test]
async fn penalties_sambanova_ignores_are_refused_before_anything_is_sent() {
    let env = env().await;
    for field in ["frequency_penalty", "presence_penalty"] {
        env.mock.clear();
        let mut body = chat(GPT_OSS, "Hello");
        body[field] = json!(0.5);
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
            format!("`{field}` is not supported by SambaNova; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn every_sambanova_model_counts_tokens_at_its_anthropic_endpoint() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            GLM_AS_MESSAGES,
            &format!(
                "What is 27 * 43? {}",
                "[mock:fixture=../providers/sambanova/responses/thinking]"
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][1]["text"], "27 × 43 = 1161.");
    let sent = sent_to_sambanova(&env, "/v1/messages");
    assert_eq!(sent["model"], GLM_AS_MESSAGES);

    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GLM_AS_MESSAGES, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["input_tokens"].is_u64());
    sent_to_sambanova(&env, "/v1/messages/count_tokens");

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": GPT_OSS, "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["input_tokens"].is_u64());
    let sent = sent_to_sambanova(&env, "/v1/messages/count_tokens");
    assert_eq!(sent["model"], GPT_OSS);
}

#[tokio::test]
async fn sambanova_errors_reach_apps_and_a_wrong_key_is_the_refused_key() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(GPT_OSS, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT_OSS,
            &format!("Hello {}", error("top-p-above-max-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(
        (&body["error"]["param"], &body["error"]["code"]),
        (&json!("top_p"), &json!("decimal_above_max_value"))
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("sambanova"));
    assert!(!format!("{o:?}").contains(SECRET));
}
