#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "venice-test-credential-0001";
const GLM: &str = "zai-org-glm-5-1";
const QWEN: &str = "qwen3-235b-a22b-thinking-2507";
const EMBED: &str = "text-embedding-qwen3-0-6b";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/venice/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/venice/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "venice", "model_id": model }),
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
            json!({ "provider": "venice", "name": "venice", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GLM, QWEN, EMBED] {
        add_model(&env, model).await;
    }
    env
}

fn sent_to_venice(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_venice_without_its_own_system_prompt_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        GLM,
        &format!(
            "Why is the sky blue? {}",
            fixture("responses", "greeting-doc")
        ),
    );
    body["messages"].as_array_mut().unwrap().insert(
        0,
        json!({ "role": "developer", "content": "Answer briefly." }),
    );
    body["max_completion_tokens"] = json!(256);
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
    assert_eq!(r.header("kaeso-provider"), Some("venice"));
    let reply = r.json();
    assert!(
        reply["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .starts_with("The sky appears blue")
    );
    assert_eq!(reply["usage"]["completion_tokens"], 146);
    let sent = sent_to_venice(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "developer");
    assert_eq!(sent["max_completion_tokens"], 256);
    assert_eq!(
        sent["venice_parameters"],
        json!({ "include_venice_system_prompt": false })
    );
    assert!(sent.get("safety_identifier").is_none() && sent.get("user").is_none());
}

#[tokio::test]
async fn venice_parameters_an_app_sets_are_sent_as_written() {
    let env = env().await;
    let mut body = chat(
        GLM,
        &format!("Hello {}", fixture("responses", "greeting-doc")),
    );
    body["venice_parameters"] = json!({
        "include_venice_system_prompt": true,
        "enable_web_search": "off"
    });
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_venice(&env, "/v1/chat/completions")["venice_parameters"],
        json!({ "include_venice_system_prompt": true, "enable_web_search": "off" })
    );
}

#[tokio::test]
async fn anthropic_apps_get_venice_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
    let mut body = messages(
        QWEN,
        &format!(
            "Add 17 and 25 with the tool. {}",
            fixture("responses", "tool-call-reasoning-published")
        ),
    );
    body["tools"] = json!([{
        "name": "add",
        "description": "Add two numbers",
        "input_schema": {
            "type": "object",
            "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
            "required": ["x", "y"]
        }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    let kinds: Vec<&str> = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["type"].as_str().unwrap())
        .filter(|kind| *kind != "text")
        .collect();
    assert_eq!(kinds, ["thinking", "tool_use"]);
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("The user wants me to add 17 and 25")
    );
    let call_block = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(call_block["input"], json!({ "x": 17, "y": 25 }));
    assert_eq!(reply["stop_reason"], "tool_use");
    let sent = sent_to_venice(&env, "/v1/chat/completions");
    assert_eq!(
        sent["venice_parameters"],
        json!({ "include_venice_system_prompt": false })
    );
}

#[tokio::test]
async fn fields_venice_would_take_without_acting_on_them_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("store", json!(true)),
        ("metadata", json!({ "team": "search" })),
    ] {
        env.mock.clear();
        let mut body = chat(GLM, "Hello");
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
            format!("`{field}` is not supported by Venice; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn venice_embeds_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["Venice [mock:fixture=providers/venice/qwen3-0-6b-published]", "two", "three"],
            "dimensions": 256
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["usage"]["prompt_tokens"], 34);
    assert_eq!(reply["data"].as_array().unwrap().len(), 3);
    let sent = sent_to_venice(&env, "/v1/embeddings");
    assert_eq!(
        (&sent["model"], &sent["dimensions"]),
        (&json!(EMBED), &json!(256))
    );

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": GLM, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.estimated_count() > 0);
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn venice_text_errors_reach_apps_with_their_text_and_the_kind_of_their_status() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GLM,
            &format!("Hello {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            GLM,
            &format!("Hello {}", error("unknown-model-published")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Specified model not found")
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("venice"));
    assert!(!format!("{o:?}").contains(SECRET));
}
