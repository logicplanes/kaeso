mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::json;

const SECRET: &str = "fw-test-credential-0003";
const MODEL: &str = "accounts/fireworks/models/gpt-oss-120b";
const EMBEDDER: &str = "nomic-ai/nomic-embed-text-v1.5";

async fn env() -> Env {
    let mock = kaeso_testkit::mock_provider::MockProvider::start().await;
    let base = format!("{}/inference", mock.base_url);
    let env = common::v1::env(
        None,
        Some(&[("KAESO_PROVIDERS_FIREWORKS_AI_BASE_URL", base.as_str())]),
    )
    .await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "fireworks-ai", "name": "fireworks", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let models = format!("/orgs/{org}/projects/{}/models", env.project);
    for body in [
        json!({ "provider": "fireworks-ai", "model_id": MODEL }),
        json!({
            "provider": "fireworks-ai", "model_id": EMBEDDER, "custom": true,
            "overrides": {
                "input_modalities": ["text"], "output_modalities": ["text"],
                "context_limit": 8192, "output_limit": 768,
                "input_price": "0.008", "output_price": "0", "egress_dialect": "openai_embeddings"
            }
        }),
    ] {
        let r = env
            .app
            .post(&models, body, Some(&env.owner.cookie))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    Env { mock, ..env }
}

fn sent_to_fireworks(env: &Env, path: &str) -> serde_json::Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_fireworks_models_by_their_slashed_ids() {
    let env = env().await;
    for name in [MODEL.to_owned(), format!("fireworks-ai/{MODEL}")] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(&name, "Hello")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.json()["object"], "chat.completion");
        assert_eq!(r.header("kaeso-provider"), Some("fireworks-ai"));
        assert_eq!(r.header("kaeso-model"), Some(MODEL));
        let body = sent_to_fireworks(&env, "/inference/v1/chat/completions");
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["messages"][0]["content"], "Hello");
    }

    let mut body = chat(MODEL, "Hello");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text().contains("chat.completion.chunk") && r.text().trim_end().ends_with("data: [DONE]"),
        "{}",
        r.text()
    );
    let body = sent_to_fireworks(&env, "/inference/v1/chat/completions");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[tokio::test]
async fn anthropic_apps_reach_fireworks_models_through_kpp() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(MODEL, "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["type"], "message");
    assert_eq!(r.json()["role"], "assistant");
    assert_eq!(r.header("kaeso-provider"), Some("fireworks-ai"));
    let body = sent_to_fireworks(&env, "/inference/v1/chat/completions");
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["max_completion_tokens"], 256);

    let mut body = messages(&format!("fireworks-ai/{MODEL}"), "Hello");
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(
        text.contains("event: message_start") && text.contains("event: message_stop"),
        "{text}"
    );
    sent_to_fireworks(&env, "/inference/v1/chat/completions");

    let count = json!({ "model": MODEL, "messages": [{ "role": "user", "content": "Hi" }] });
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(count),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot count tokens"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn embeddings_go_to_the_fireworks_embeddings_endpoint() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": EMBEDDER, "input": "hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "list");
    let body = sent_to_fireworks(&env, "/inference/v1/embeddings");
    assert_eq!(body, json!({ "model": EMBEDDER, "input": "hello" }));

    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": MODEL, "input": "hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
}
