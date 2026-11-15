#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "tokenhub-test-credential-0001";
const HY4: &str = "hy4";
const HY3: &str = "hy3";
const MT: &str = "hy-mt2-plus";
const EMBED: &str = "kinfra-text-embedding-0.6b";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/tencent-tokenhub/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/tencent-tokenhub/errors/{name}]")
}

async fn add_model(env: &Env, model: &str) {
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            json!({ "provider": "tencent-tokenhub", "model_id": model }),
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
            json!({ "provider": "tencent-tokenhub", "name": "tokenhub", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [HY4, HY3, MT, EMBED] {
        add_model(&env, model).await;
    }
    env
}

fn sent_to_tokenhub(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_tokenhub_with_its_field_names_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        MT,
        &format!(
            "Translate into English. {}",
            fixture("responses", "translation-doc")
        ),
    );
    body["messages"].as_array_mut().unwrap().insert(
        0,
        json!({ "role": "developer", "content": "Translate only." }),
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
    assert_eq!(r.header("kaeso-provider"), Some("tencent-tokenhub"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Please ensure that all attendees have received the agenda before the meeting starts."
    );
    assert_eq!(reply["usage"]["completion_tokens"], 15);
    let sent = sent_to_tokenhub(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(
        (&sent["max_completion_tokens"], &sent["safety_identifier"]),
        (&json!(64), &json!("user-0001"))
    );
}

#[tokio::test]
async fn anthropic_apps_get_hy_reasoning_as_thinking_and_its_tool_calls() {
    let env = env().await;
    let body = messages(
        HY4,
        &format!(
            "How many apples are left? {}",
            fixture("responses", "thinking-doc")
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
            .starts_with("用户问：小明有5个苹果")
    );
    assert_eq!(reply["content"][1]["type"], "text");
    assert_eq!(reply["usage"]["output_tokens"], 178);

    let mut body = messages(
        HY4,
        &format!(
            "What is the weather in Shenzhen? {}",
            fixture("responses", "tools-doc")
        ),
    );
    body["tools"] = json!([{
        "name": "get_weather",
        "description": "Get the current weather of a city",
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    let kinds: Vec<&str> = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["thinking", "text", "tool_use"]);
    assert_eq!(reply["content"][2]["input"], json!({ "city": "深圳" }));
    assert_eq!(reply["stop_reason"], "tool_use");
}

#[tokio::test]
async fn thinking_turned_off_gets_hy3s_lowest_level_and_a_level_it_takes_is_sent() {
    let env = env().await;
    let mut body = chat(
        HY3,
        &format!("Hello {}", fixture("responses", "reasoning-effort-doc")),
    );
    body["reasoning_effort"] = json!("none");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), Some("reasoning_effort"));
    assert_eq!(
        sent_to_tokenhub(&env, "/v1/chat/completions")["reasoning_effort"],
        "low"
    );
    env.mock.clear();

    let mut body = chat(
        HY3,
        &format!("Hello {}", fixture("responses", "reasoning-effort-doc")),
    );
    body["reasoning_effort"] = json!("low");
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
        sent_to_tokenhub(&env, "/v1/chat/completions")["reasoning_effort"],
        "low"
    );
}

#[tokio::test]
async fn fields_tokenhub_would_ignore_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("logit_bias", json!({ "50256": -100 })),
        ("store", json!(true)),
        ("metadata", json!({ "team": "search" })),
        ("service_tier", json!("priority")),
        ("modalities", json!(["text"])),
    ] {
        env.mock.clear();
        let mut body = chat(HY4, "Hello");
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
            format!("`{field}` is not supported by Tencent TokenHub; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn tokenhub_embeds_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": "TokenHub [mock:fixture=providers/tencent-tokenhub/kinfra-text-doc]",
            "encoding_format": "float"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["usage"]["prompt_tokens"], 10);
    let sent = sent_to_tokenhub(&env, "/v1/embeddings");
    assert_eq!(sent["model"], EMBED);

    env.mock.clear();
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBED, "input": "hi", "dimensions": 256 })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "`dimensions` is not supported by Tencent TokenHub; remove it from the request."
    );
    assert!(env.mock.received().is_empty());

    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": HY4, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn tokenhub_errors_reach_apps_with_the_kind_of_their_status() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(HY4, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(HY4, &format!("Hello {}", error("rate-limit-doc")))),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            HY4,
            &format!("Hello {}", error("model-not-found-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("gpt-9-turbo does not exist")
    );
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            HY4,
            &format!("Hello {}", error("model-not-found-doc")),
        )),
    )
    .await;
    assert_eq!(r.json()["error"]["type"], "not_found_error", "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("tencent-tokenhub"));
    assert!(!format!("{o:?}").contains(SECRET));
}
