#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "upstage-test-credential-0001";
const PRO: &str = "solar-pro4";
const EMBED: &str = "solar-embedding-2-query";

fn fixture(name: &str) -> String {
    format!("[mock:fixture=providers/upstage/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=providers/upstage/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "upstage", "name": "upstage", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [PRO, EMBED] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "upstage", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_upstage(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_upstage_with_its_field_names_and_get_its_reply() {
    let env = env().await;
    let mut body = chat(
        PRO,
        &format!("Hi, how are you? {}", fixture("greeting-doc")),
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
    assert_eq!(r.header("kaeso-provider"), Some("upstage"));
    let reply = r.json();
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hi! I'm doing well, thank you for asking. How are you doing today? Is there anything I can help you with?"
    );
    assert_eq!(reply["usage"]["completion_tokens"], 29);
    let sent = sent_to_upstage(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_tokens"], 64);
    assert!(
        sent.get("max_completion_tokens").is_none()
            && sent.get("safety_identifier").is_none()
            && sent.get("user").is_none(),
        "{sent}"
    );
}

#[tokio::test]
async fn anthropic_apps_get_upstage_reasoning_as_thinking() {
    let env = env().await;
    let mut body = messages(
        PRO,
        &format!(
            "An $80 item gets a 20% discount, then 10% tax is added. {}",
            fixture("reasoning-medium-doc")
        ),
    );
    body["max_tokens"] = json!(4096);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 2048 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("Thinking Process:"),
        "{reply}"
    );
    assert_eq!(
        (&reply["content"][1]["type"], &reply["content"][1]["text"]),
        (&json!("text"), &json!("$70.40"))
    );
    assert_eq!(reply["usage"]["output_tokens"], 371);
    let sent = sent_to_upstage(&env, "/v1/chat/completions");
    assert!(sent["reasoning_effort"].is_string(), "{sent}");
    assert_eq!(sent["max_tokens"], 4096);

    let mut body = messages(
        PRO,
        &format!("Hi, how are you? {}", fixture("reasoning-stream-doc")),
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
    assert_eq!(thinking, "Thinking Process");
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Hello! I");
}

#[tokio::test]
async fn fields_upstage_does_not_document_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("stop", json!(["END"])),
        ("seed", json!(7)),
        ("logit_bias", json!({ "50256": -100 })),
        ("store", json!(true)),
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
        assert_eq!(
            r.json()["error"]["message"],
            format!("`{field}` is not supported by Upstage; remove it from the request.")
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }

    env.mock.clear();
    let mut body = messages(PRO, "Hello");
    body["stop_sequences"] = json!(["END"]);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn upstage_embeds_and_counts_no_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": format!("Solar embeddings are awesome {}", fixture("solar-embedding-2-query-doc")),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["usage"]["prompt_tokens"], 21);
    assert_eq!(reply["data"][0]["embedding"][0], 0.01850688);
    let sent = sent_to_upstage(&env, "/v1/embeddings");
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
        "`dimensions` is not supported by Upstage; remove it from the request."
    );
    assert!(env.mock.received().is_empty());

    env.mock.clear();
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
async fn upstage_errors_reach_apps_and_name_no_key() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            PRO,
            &format!("Hi {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("upstage"));
    assert!(!format!("{o:?}").contains(SECRET));
}
