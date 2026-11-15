#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "ionos-test-credential-0001";
const OSS: &str = "openai/gpt-oss-120b";
const QWEN: &str = "Qwen/Qwen3.8-27B";
const EMBED: &str = "BAAI/bge-m3";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/ionos/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/ionos/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "ionos", "name": "ionos", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [OSS, QWEN, EMBED] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "ionos", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

fn sent_to_ionos(env: &Env, path: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_ionos_with_system_messages_and_the_end_user_as_user() {
    let env = env().await;
    let mut body = chat(
        OSS,
        &format!(
            "Our Q1 sales were 10k, 12k and 15k. Predict Q2. {}",
            fixture("responses", "gpt-oss-reasoning-doc")
        ),
    );
    body["messages"].as_array_mut().unwrap().insert(
        0,
        json!({ "role": "developer", "content": "Answer briefly." }),
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
    assert_eq!(r.header("kaeso-provider"), Some("ionos"));
    assert_eq!(r.json()["usage"]["completion_tokens"], 48);
    let sent = sent_to_ionos(&env, "/v1/chat/completions");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["user"], "user-0001");
    assert!(sent.get("safety_identifier").is_none(), "{sent}");
}

#[tokio::test]
async fn anthropic_apps_get_ionos_reasoning_as_thinking() {
    let env = env().await;
    let body = messages(
        QWEN,
        &format!(
            "Describe a water bottle in one sentence. {}",
            fixture("responses", "qwen-reasoning-doc")
        ),
    );
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("The user wants a single-sentence product description")
    );
    assert_eq!(reply["content"][1]["type"], "text", "{reply}");
}

#[tokio::test]
async fn fields_outside_ionos_schema_are_refused_before_anything_is_sent() {
    let env = env().await;
    for (field, value) in [
        ("store", json!(true)),
        ("metadata", json!({ "team": "search" })),
        ("service_tier", json!("flex")),
    ] {
        env.mock.clear();
        let mut body = chat(OSS, "Hello");
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
            format!(
                "`{field}` is not supported by IONOS AI Model Hub; remove it from the request."
            )
        );
        assert!(env.mock.received().is_empty(), "{field}");
    }
}

#[tokio::test]
async fn ionos_embeds_and_its_own_error_bodies_reach_apps_with_their_messages() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBED,
            "input": "The food was delicious and the waiter was friendly. [mock:fixture=providers/ionos/bge-m3-doc]"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["usage"]["prompt_tokens"], 10);
    sent_to_ionos(&env, "/v1/embeddings");

    for name in ["recorded-invalid-api-key", "recorded-missing-api-key"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(OSS, &format!("Hello {}", error(name)))),
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
            OSS,
            &format!("Hello {}", error("validation-error-doc")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("field 'messages' must not be empty"),
        "{}",
        r.text()
    );
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("ionos"));
    assert!(!format!("{o:?}").contains(SECRET));
}
