#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "abcd1234.test-credential-0001";
const GLM: &str = "zai-org/GLM-5.2";
const DEEPSEEK: &str = "deepseek-ai/DeepSeek-V4.1-Flash-1017";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/baseten/{folder}/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({
                "provider": "baseten", "model_id": model,
                "overrides": {
                    "input_modalities": ["text", "image"], "output_modalities": ["text"],
                    "context_limit": 202752, "output_limit": 131072,
                    "input_price": "0.6", "output_price": "2.2",
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
            json!({ "provider": "baseten", "name": "baseten", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    add_model(&env, GLM).await;
    add_model(&env, DEEPSEEK).await;
    env
}

fn sent_to_baseten(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_baseten_with_max_tokens_system_messages_and_the_end_user_as_user() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GLM,
            "messages": [
                { "role": "developer", "content": "Answer in one sentence." },
                { "role": "user", "content": format!("Is 91 a prime number? {}", fixture("responses", "reasoning-doc")) }
            ],
            "max_completion_tokens": 1024,
            "chat_template_args": { "enable_thinking": true },
            "safety_identifier": "user-0001"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("baseten"));
    assert!(
        r.json()["choices"][0]["message"]["reasoning_content"]
            .as_str()
            .unwrap()
            .starts_with("The user is asking whether 91")
    );
    let sent = sent_to_baseten(&env);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_tokens"], sent.get("max_completion_tokens")),
        (&json!(1024), None)
    );
    assert_eq!(
        sent["chat_template_args"],
        json!({ "enable_thinking": true })
    );
    assert_eq!(
        (&sent["user"], sent.get("safety_identifier")),
        (&json!("user-0001"), None)
    );
}

#[tokio::test]
async fn anthropic_apps_get_basetens_reasoning_as_thinking_and_its_stream_with_usage() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            GLM,
            &format!(
                "Is 91 a prime number? {}",
                fixture("responses", "reasoning-doc")
            ),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(reply["usage"]["output_tokens"], 203);

    let mut body = messages(
        DEEPSEEK,
        &format!(
            "Reply with exactly: Café from Baseten. {}",
            fixture("streams", "text-stream-doc")
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
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Café from Baseten.");
    let last = events
        .iter()
        .rfind(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(last["usage"]["output_tokens"], 6);
}

#[tokio::test]
async fn a_refused_key_reaches_apps_as_the_refused_key_and_what_baseten_lacks_is_refused() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GLM,
            "Hello [mock:error=../providers/baseten/errors/recorded-invalid-api-key]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("baseten"));
    assert!(!format!("{o:?}").contains(SECRET));

    env.mock.clear();
    for (path, anthropic, body) in [
        (
            "/v1/messages/count_tokens",
            true,
            json!({ "model": GLM, "messages": [{ "role": "user", "content": "Hello" }] }),
        ),
        (
            "/v1/embeddings",
            false,
            json!({ "model": GLM, "input": "Hello" }),
        ),
        (
            "/v1/images/generations",
            false,
            json!({ "model": GLM, "prompt": "A lighthouse at dusk" }),
        ),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert!(r.status.is_client_error(), "{path}: {}", r.text());
    }
    assert!(env.mock.received().is_empty());
}
