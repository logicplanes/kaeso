#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "00000000-0000-0000-0000-000000000000";
const PRO: &str = "doubao-seed-2-1-pro-260915";
const LITE: &str = "doubao-seed-2-0-lite-260428";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/volcengine/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/volcengine/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "volcengine", "name": "volcengine", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [PRO, LITE] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "volcengine", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

// Ark's API sits at `/api/v3`, which the base URL ends with: every path goes without `/v1`.
fn sent_to_volcengine(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

fn events(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn chat_apps_reach_volcengine_at_its_api_root_with_their_limit_and_without_an_end_user() {
    let env = env().await;
    let mut body = chat(
        PRO,
        &format!("Hello! {}", fixture("responses", "greeting-doc")),
    );
    body["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({ "role": "developer", "content": "Be kind." }));
    body["max_completion_tokens"] = json!(64);
    body["safety_identifier"] = json!("user-0001");
    body["service_tier"] = json!("fast");
    body["web_search_options"] = json!({});
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("volcengine"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello! How can I help you today?"
    );
    assert_eq!(reply["usage"]["prompt_tokens"], 19);
    let sent = sent_to_volcengine(&env);
    assert_eq!(sent["messages"][0]["role"], "developer");
    assert_eq!(
        (
            &sent["max_completion_tokens"],
            &sent["service_tier"],
            &sent["web_search_options"]
        ),
        (&json!(64), &json!("fast"), &json!({}))
    );
    for field in ["max_tokens", "safety_identifier", "user"] {
        assert!(sent.get(field).is_none(), "{field}: {sent}");
    }
}

#[tokio::test]
async fn anthropic_apps_switch_volcengine_thinking_and_get_its_reasoning_as_thinking() {
    let env = env().await;
    let mut body = messages(
        PRO,
        &format!("Hello {}", fixture("responses", "greeting-doc")),
    );
    body["thinking"] = json!({ "type": "disabled" });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_volcengine(&env);
    assert_eq!(sent["thinking"], json!({ "type": "disabled" }));
    assert!(sent.get("reasoning_effort").is_none(), "{sent}");
    assert_eq!(sent["max_completion_tokens"], 256);

    let body = messages(
        PRO,
        &format!("Hello {}", fixture("responses", "reasoning-doc")),
    );
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("嗯，用户问的是推理模型和非推理模型有什么区别。"),
        "{reply}"
    );
    assert_eq!(reply["content"][1]["type"], "text");

    let mut body = messages(
        PRO,
        &format!("Hello {}", fixture("streams", "reasoning-stream-built")),
    );
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = events(&r.text());
    let thinking: String = events
        .iter()
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    assert_eq!(thinking, " need.");
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Hello! How");
}

#[tokio::test]
async fn volcengine_encrypted_reasoning_goes_back_with_an_anthropic_apps_next_turn() {
    let env = env().await;
    let tools = json!([{
        "name": "get_weather",
        "description": "天气查询",
        "input_schema": { "type": "object", "properties": { "location": { "type": "string" } }, "required": ["location"] }
    }]);
    let mut body = messages(
        LITE,
        &format!(
            "今天北京天气怎么样 {}",
            fixture("responses", "tools-encrypted-content-doc")
        ),
    );
    body["tools"] = tools.clone();
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    let content = reply["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "thinking", "{reply}");
    let tool_use = content.iter().find(|b| b["type"] == "tool_use").unwrap();
    assert!(
        !reply.to_string().contains("djF+"),
        "the encrypted original stays inside Kaeso's signature: {reply}"
    );
    let call_id = tool_use["id"].as_str().unwrap().to_owned();

    env.mock.clear();
    let body = json!({
        "model": LITE,
        "max_tokens": 1024,
        "tools": tools,
        "messages": [
            { "role": "user", "content": "今天北京天气怎么样" },
            { "role": "assistant", "content": reply["content"] },
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": call_id, "content": format!("5度 {}", fixture("responses", "greeting-doc")) }
            ] }
        ]
    });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_volcengine(&env);
    let assistant = &sent["messages"][1];
    assert_eq!(assistant["role"], "assistant", "{sent}");
    assert!(
        assistant["encrypted_content"]
            .as_str()
            .is_some_and(|e| e.starts_with("djF+2EICEj3ryfEf")),
        "{assistant}"
    );
    assert_eq!(
        assistant["reasoning_content"],
        "北京天气查询将由我调用相关工具完成。\n"
    );
    assert_eq!(
        assistant["tool_calls"][0]["id"],
        "call_wiezxeyae8jzxl3jx8nhfgb5"
    );
}

#[tokio::test]
async fn fields_volcengine_lacks_or_reads_otherwise_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value, message) in [
        (
            "seed",
            json!(7),
            "`seed` is not supported by Volcengine Ark; remove it from the request.",
        ),
        (
            "store",
            json!(true),
            "`store` is not supported by Volcengine Ark; remove it from the request.",
        ),
        (
            "service_tier",
            json!("priority"),
            "Volcengine Ark does not take `service_tier: \"priority\"`; send one of its own values or leave `service_tier` out.",
        ),
        (
            "web_search_options",
            json!({ "search_context_size": "high" }),
            "`web_search_options.search_context_size` is not supported by Volcengine Ark; remove it from the request.",
        ),
    ] {
        env.mock.clear();
        let mut body = chat(PRO, "Hello");
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
        assert_eq!(r.json()["error"]["message"], message);
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn volcengine_neither_embeds_nor_counts_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": PRO, "input": ["Hello"] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": PRO, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn volcengine_key_refusals_reach_apps_as_the_providers_errors() {
    let env = env().await;
    for name in [
        "recorded-invalid-api-key",
        "recorded-missing-api-key",
        "recorded-unknown-api-key",
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(PRO, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("volcengine"));
    assert!(!format!("{o:?}").contains(SECRET));
}
