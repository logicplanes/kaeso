#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use kaeso_server::model_api::hooks::Outcome;
use kaeso_testkit::mock_provider::MockProvider;
use serde_json::json;

const FLASH: &str = "deepseek-flash";

async fn env_with_chat_provider() -> (Env, MockProvider) {
    let provider = MockProvider::start().await;
    let env = common::v1::env(
        Some(Arc::new(Capture::default())),
        Some(&[(
            "KAESO_PROVIDERS_DEEPSEEK_BASE_URL",
            provider.base_url.as_str(),
        )]),
    )
    .await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "deepseek", "name": "DeepSeek", "secret": "sk-test-deepseek-credential-0005" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "deepseek", "model_id": FLASH }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    (env, provider)
}

fn outcome(env: &Env, id: &str) -> Outcome {
    env.capture
        .all()
        .into_iter()
        .find(|o| o.request_id.to_string() == id)
        .unwrap_or_else(|| panic!("no outcome for {id} in {:#?}", env.capture.all()))
}

#[tokio::test]
async fn replies_and_streams_without_usage_are_charged_an_estimate() {
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .unwrap();
    kaeso_server::metrics::describe();
    let (env, _provider) = env_with_chat_provider().await;

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(FLASH, "Hello there [mock:fixture=without-usage]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["choices"][0]["message"]["content"],
        "Hello! How can I assist you today?"
    );
    assert!(r.json().get("usage").is_none(), "{}", r.text());
    let o = outcome(&env, r.header("kaeso-request-id").unwrap());
    assert_eq!(o.status, 200);
    assert!(o.usage_estimated);
    let u = o.usage.unwrap();
    assert!(u.input_tokens > 0, "{u:?}");
    assert_eq!(u.output_tokens, 34_u64.div_ceil(4), "{u:?}");

    let mut body = messages(FLASH, "Hello there [mock:fixture=stream-doc]");
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.text().contains("message_stop"), "{}", r.text());
    let o = outcome(&env, r.header("kaeso-request-id").unwrap());
    assert_eq!(o.status, 200);
    assert!(o.error_kind.is_none(), "{:?}", o.error_kind);
    assert!(o.usage_estimated);
    let u = o.usage.unwrap();
    assert!(u.input_tokens > 0, "{u:?}");
    assert!(u.output_tokens > 0, "{u:?}");

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            FLASH,
            "Hello there [mock:fixture=plain-developer-string]",
        )),
    )
    .await;
    let o = outcome(&env, r.header("kaeso-request-id").unwrap());
    assert!(!o.usage_estimated);
    let u = o.usage.unwrap();
    assert_eq!((u.input_tokens, u.output_tokens), (19, 10));

    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "text-embedding-3-small", "input": "hello [mock:fixture=without-usage]" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json().get("usage").is_none(), "{}", r.text());
    let o = outcome(&env, r.header("kaeso-request-id").unwrap());
    assert!(o.usage_estimated);
    let u = o.usage.unwrap();
    assert!(u.input_tokens > 0, "{u:?}");
    assert_eq!(u.output_tokens, 0, "{u:?}");

    let needle = "kaeso_usage_estimates_total{reason=\"unreported\",provider=\"deepseek\"} 2";
    let text = handle.render();
    assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    let needle = "kaeso_usage_estimates_total{reason=\"unreported\",provider=\"openai\"} 1";
    let text = handle.render();
    assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
}
