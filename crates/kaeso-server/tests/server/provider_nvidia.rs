#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "nvapi-test-credential-0001";
const SUPER: &str = "nvidia/nemotron-3-super-120b-a12b";
const RETIRED: &str = "meta/llama-3.1-8b-instruct";
const EMBED: &str = "nvidia/nemotron-3.5-embed-1b";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/nvidia/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/nvidia/errors/{name}]")
}

async fn add_model(env: &Env, model: &str, egress: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({
                "provider": "nvidia", "model_id": model, "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 131072, "output_limit": 16384,
                    "input_price": "0.1", "output_price": "0.4",
                    "reasoning": egress == "openai_chat", "tool_call": egress == "openai_chat",
                    "egress_dialect": egress
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
            json!({ "provider": "nvidia", "name": "nvidia", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_model(&env, SUPER, "openai_chat").await;
    add_model(&env, RETIRED, "openai_chat").await;
    add_model(&env, EMBED, "openai_embeddings").await;
    env
}

fn sent_to_nvidia(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_nvidia_with_max_tokens_system_messages_and_their_thinking_switch() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": SUPER,
            "messages": [
                { "role": "developer", "content": "Rhyme." },
                { "role": "user", "content": format!("Write a limerick about the wonders of GPU computing. {}", fixture("responses", "reasoning")) }
            ],
            "max_completion_tokens": 8192,
            "chat_template_kwargs": { "enable_thinking": true },
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("nvidia"));
    assert_eq!(
        r.json()["choices"][0]["message"]["reasoning_content"],
        "A limerick has five lines in AABBA. Theme: parallel cores, fast training."
    );
    let sent = sent_to_nvidia(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_tokens"], sent.get("max_completion_tokens")),
        (&json!(8192), None)
    );
    assert_eq!(
        sent["chat_template_kwargs"],
        json!({ "enable_thinking": true })
    );
    assert!(
        sent.get("safety_identifier").is_none() && sent.get("user").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_nvidias_streamed_reasoning_as_thinking() {
    let env = env().await;
    let mut body = messages(
        SUPER,
        &format!(
            "Which number is larger, 9.11 or 9.8? {}",
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
    let blocks: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "content_block_start")
        .map(|e| &e["content_block"]["type"])
        .collect();
    assert_eq!(blocks, [&json!("thinking"), &json!("text")]);
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "9.8 is larger than 9.11.");
}

#[tokio::test]
async fn a_key_nvidia_doesnt_know_is_not_allowed_and_a_retired_model_is_gone() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            SUPER,
            &format!("Hello {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers."
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("nvidia"));
    assert!(!format!("{o:?}").contains(SECRET));

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            RETIRED,
            &format!("Hello {}", error("recorded-retired-model")),
        )),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("has reached its end of life"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn embeddings_apps_send_input_type_and_what_nvidia_lacks_is_refused() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["What is the capital of France? [mock:fixture=providers/nvidia/query]"],
            "input_type": "query",
            "truncate": "END"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_nvidia(&env, "/v1/embeddings");
    assert_eq!(
        (&sent["input_type"], &sent["truncate"]),
        (&json!("query"), &json!("END"))
    );
    assert_eq!(r.json()["usage"]["prompt_tokens"], 9);

    env.mock.clear();
    for (path, anthropic, body) in [
        (
            "/v1/messages/count_tokens",
            true,
            json!({ "model": SUPER, "messages": [{ "role": "user", "content": "Hello" }] }),
        ),
        (
            "/v1/images/generations",
            false,
            json!({ "model": SUPER, "prompt": "A lighthouse at dusk" }),
        ),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert!(r.status.is_client_error(), "{path}: {}", r.text());
    }
    assert!(env.mock.received().is_empty());
}
