#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "hf_test-credential-0001";
const GPT_OSS: &str = "openai/gpt-oss-120b:cheapest";
const DEEPSEEK: &str = "deepseek-ai/DeepSeek-V4.1-Flash-1017:novita";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/huggingface/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/huggingface/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({
                "provider": "huggingface", "model_id": model, "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 131072, "output_limit": 32768,
                    "input_price": "0.1", "output_price": "0.5",
                    "reasoning": true, "tool_call": true,
                    "egress_dialect": "openai_chat"
                }
            }),
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
            json!({ "provider": "huggingface", "name": "hugging face", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_model(&env, GPT_OSS).await;
    add_model(&env, DEEPSEEK).await;
    env
}

fn sent_to_hugging_face(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn chat_apps_reach_the_router_with_the_provider_suffix_max_tokens_and_no_end_user() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": format!("huggingface/{GPT_OSS}"),
            "messages": [
                { "role": "developer", "content": "Answer briefly." },
                { "role": "user", "content": format!("How many 'G's in 'huggingface'? {}", fixture("responses", "provider-suffix")) }
            ],
            "max_completion_tokens": 512,
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("huggingface"));
    assert_eq!(
        r.json()["choices"][0]["message"]["content"],
        "There are three 'g's in \"huggingface\"."
    );
    let sent = sent_to_hugging_face(&env);
    assert_eq!(sent["model"], GPT_OSS);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_tokens"], sent.get("max_completion_tokens")),
        (&json!(512), None)
    );
    assert!(
        sent.get("safety_identifier").is_none() && sent.get("user").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_either_reasoning_field_as_thinking() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            GPT_OSS,
            &format!(
                "How many 'G's in 'huggingface'? {}",
                fixture("responses", "provider-suffix")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(
        (
            &reply["content"][0]["type"],
            &reply["content"][0]["thinking"]
        ),
        (
            &json!("thinking"),
            &json!("Count the letter g in huggingface: h-u-g-g-i-n-g-f-a-c-e. That is three.")
        )
    );

    let mut body = messages(
        DEEPSEEK,
        &format!("Hello! {}", fixture("streams", "reasoning-stream")),
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
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 23);
    let sent = sent_to_hugging_face(&env);
    assert_eq!(sent["model"], DEEPSEEK);
}

#[tokio::test]
async fn responses_apps_reach_the_routers_chat_completions() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": GPT_OSS,
            "input": format!("How many 'G's in 'huggingface'? {}", fixture("responses", "provider-suffix")),
            "max_output_tokens": 256
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["output"][0]["type"], "reasoning");
    assert_eq!(sent_to_hugging_face(&env)["max_tokens"], 256);
}

#[tokio::test]
async fn a_refused_token_reaches_apps_as_the_refused_key_and_what_the_router_lacks_is_refused() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GPT_OSS,
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
    assert_eq!(o.provider, Some("huggingface"));
    assert!(!format!("{o:?}").contains(SECRET));

    env.mock.clear();
    for (path, anthropic, body) in [
        (
            "/v1/messages/count_tokens",
            true,
            json!({ "model": GPT_OSS, "messages": [{ "role": "user", "content": "Hello" }] }),
        ),
        (
            "/v1/embeddings",
            false,
            json!({ "model": GPT_OSS, "input": "Hello" }),
        ),
        (
            "/v1/images/generations",
            false,
            json!({ "model": GPT_OSS, "prompt": "A lighthouse at dusk" }),
        ),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert!(r.status.is_client_error(), "{path}: {}", r.text());
    }
    assert!(env.mock.received().is_empty());
}
