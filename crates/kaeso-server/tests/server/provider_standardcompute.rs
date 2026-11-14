#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "standardcompute-test-credential-0001";
const ROUTER: &str = "standardcompute";
const GEMINI: &str = "google/gemini-3.5-flash-lite";
const TEXT_ONLY: &str = "deepseek/deepseek-v4-pro";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/standardcompute/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/standardcompute/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            json!({ "provider": "standardcompute", "name": "standardcompute", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [ROUTER, GEMINI, TEXT_ONLY] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "standardcompute", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

fn sent_to_standard_compute(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    sent.json()
}

#[tokio::test]
async fn chat_apps_reach_smart_routing_and_its_requests_have_no_price() {
    let env = env().await;
    let mut body = chat(
        ROUTER,
        &format!(
            "Say hello. {}",
            fixture("responses", "recorded-plain-developer-string")
        ),
    );
    body["messages"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({ "role": "developer", "content": "Be brief." }));
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("standardcompute"));
    let reply = r.json();
    assert_eq!(reply["model"], "StandardCompute");
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        "Hello! How can I assist you today?"
    );
    let sent = sent_to_standard_compute(&env);
    assert_eq!(sent["model"], ROUTER);
    assert_eq!(sent["messages"][0]["role"], "developer");

    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("standardcompute"));
    assert_eq!(o.response_model.as_deref(), Some("StandardCompute"));
    let prices = o.prices.clone().unwrap();
    assert_eq!((prices.input_price, prices.output_price), (None, None));
    assert!(
        !kaeso_server::services::usage::cost_known(&o),
        "Standard Compute bills a plan, so a request's cost is unknown"
    );
}

#[tokio::test]
async fn anthropic_apps_get_routed_reasoning_as_thinking_before_the_answer() {
    let env = env().await;
    let mut body = messages(
        ROUTER,
        &format!(
            "Is 91 a prime number? {}",
            fixture("streams", "recorded-routed-reasoning-stream")
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
    let thinking: String = events
        .iter()
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    let text: String = events
        .iter()
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert!(!thinking.is_empty(), "{events:?}");
    assert_eq!(
        text,
        "No, 91 is not a prime number because it is divisible by 7 and 13."
    );
    let first = events
        .iter()
        .find(|e| e["type"] == "content_block_start")
        .unwrap();
    assert_eq!(first["content_block"]["type"], "thinking");
    let sent = sent_to_standard_compute(&env);
    assert_eq!(sent["model"], ROUTER);
}

#[tokio::test]
async fn responses_apps_get_a_pinned_gemini_models_tool_call() {
    let env = env().await;
    let body = json!({
        "model": GEMINI,
        "stream": true,
        "input": format!(
            "What is the weather in Paris? Use the tool. {}",
            fixture("streams", "recorded-gemini-tools-stream")
        ),
        "tools": [{
            "type": "function",
            "name": "get_weather",
            "description": "Get the current weather for a city.",
            "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
        }]
    });
    let r = call(&env, "/v1/responses", Some(&env.key), false, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let done = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .find(|e| e["type"] == "response.completed")
        .unwrap();
    let call_item = done["response"]["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "function_call")
        .unwrap()
        .clone();
    assert_eq!(
        (&call_item["name"], &call_item["arguments"]),
        (&json!("get_weather"), &json!("{\"city\":\"Paris\"}"))
    );
    assert_eq!(sent_to_standard_compute(&env)["model"], GEMINI);
}

#[tokio::test]
async fn token_counts_and_videos_are_refused_before_anything_is_sent() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": ROUTER, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `standardcompute` cannot count tokens."
    );
    let mut body = chat(GEMINI, "What happens in this video?");
    body["messages"][0]["content"] = json!([
        { "type": "text", "text": "What happens in this video?" },
        { "type": "video_url", "video_url": { "url": "https://example.com/clip.mp4" } }
    ]);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn standard_compute_errors_reach_apps_and_a_used_up_plan_is_a_refused_key() {
    let env = env().await;
    for name in ["recorded-invalid-api-key", "budget-exhausted-doc"] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(ROUTER, &format!("Hello {}", error(name)))),
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
            ROUTER,
            &format!("Hello {}", error("budget-exhausted-doc")),
        )),
    )
    .await;
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("billing or quota"),
        "{}",
        r.text()
    );
    for (name, code) in [
        ("recorded-unknown-model", "model_not_found"),
        ("recorded-image-to-text-model", "unsupported_model_input"),
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(TEXT_ONLY, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{name}: {}", r.text());
        let body = r.json();
        assert_eq!(
            (&body["error"]["code"], &body["error"]["param"]),
            (&json!(code), &json!("model")),
            "{name}"
        );
    }
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("standardcompute"));
    assert!(!format!("{o:?}").contains(SECRET));
}
