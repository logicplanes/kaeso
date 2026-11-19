#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "bce-v3/ALTAK-qianfantest0000/0000000000000000";
const ERNIE: &str = "ernie-5.1";
const DEEPSEEK: &str = "deepseek-v4-pro";
const EMBED: &str = "embedding-v1";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/qianfan/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/qianfan/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "qianfan", "name": "qianfan", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [ERNIE, DEEPSEEK, EMBED] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "qianfan", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

// Qianfan's API sits at `/v2`, which the base URL ends with: every path goes without `/v1`.
fn sent_to_qianfan(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_qianfan_at_its_api_root_with_system_messages_and_their_end_user() {
    let env = env().await;
    let mut body = chat(
        ERNIE,
        &format!("你好 {}", fixture("responses", "greeting-doc")),
    );
    body["messages"].as_array_mut().unwrap().insert(
        0,
        json!({ "role": "developer", "content": "You are a helpful assistant." }),
    );
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
    assert_eq!(r.header("kaeso-provider"), Some("qianfan"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "你好！很高兴和你交流。请问有什么我可以帮助你的吗？"
    );
    assert_eq!(reply["usage"]["prompt_tokens"], 11);
    let sent = sent_to_qianfan(&env, "/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_completion_tokens"], &sent["user"]),
        (&json!(64), &json!("user-0001"))
    );
    assert!(sent.get("safety_identifier").is_none(), "{sent}");
}

#[tokio::test]
async fn anthropic_apps_switch_qianfan_thinking_and_get_its_reasoning_and_tool_calls() {
    let env = env().await;
    let mut body = messages(
        DEEPSEEK,
        &format!("你好 {}", fixture("responses", "greeting-doc")),
    );
    body["thinking"] = json!({ "type": "disabled" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_qianfan(&env, "/chat/completions");
    assert_eq!(sent["thinking"], json!({ "type": "disabled" }));

    let body = messages(
        DEEPSEEK,
        &format!(
            "杭州天气 {}",
            fixture("responses", "reasoning-tool-calls-doc")
        ),
    );
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("好的，用户问杭州的天气和现在的时间。"),
        "{reply}"
    );
    assert!(
        reply["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["type"] == "tool_use"),
        "{reply}"
    );
}

#[tokio::test]
async fn fields_qianfan_lacks_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("store", json!(true)),
        ("logprobs", json!(true)),
        ("web_search_options", json!({})),
        ("service_tier", json!("priority")),
    ] {
        env.mock.clear();
        let mut body = chat(ERNIE, "Hello");
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
            format!("`{field}` is not supported by Baidu Qianfan; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn qianfan_embeds_at_v2_without_dimensions_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": ["White T-shirt [mock:fixture=../providers/qianfan/responses/embedding-v1-doc]"],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_qianfan(&env, "/embeddings")["model"], EMBED);
    assert_eq!(r.json()["usage"]["prompt_tokens"], 3);
    env.mock.clear();
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": ["a"], "dimensions": 256 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": ERNIE, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn qianfan_errors_reach_apps_with_their_kind() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            ERNIE,
            &format!("Hello {}", error("recorded-invalid-iam-token")),
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
            ERNIE,
            &format!("Hello {}", error("recorded-over-rate-limit")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("qianfan"));
    assert!(!format!("{o:?}").contains("qianfantest0000"));
}
