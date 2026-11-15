#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "ABSKa2Flc28tdGVzdC1jcmVkZW50aWFsLTAwMDE=";
const REGION: &str = "eu-central-1";
const DEEPSEEK: &str = "deepseek.v3.2";
const CLAUDE: &str = "us.anthropic.claude-sonnet-5";
const NOVA: &str = "amazon.nova-pro-v1:0";
const GPT: &str = "openai.gpt-6-sol";

fn error(name: &str) -> String {
    format!("[mock:error=../providers/amazon-bedrock/errors/{name}]")
}

fn credentials(env: &Env) -> String {
    format!("/orgs/{}/credentials", env.owner.org_id)
}

async fn add_key(env: &Env, body: Value) -> common::Resp {
    env.app
        .post(&credentials(env), body, Some(&env.owner.cookie))
        .await
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let r = add_key(
        &env,
        json!({ "provider": "amazon-bedrock", "name": "Bedrock", "secret": SECRET, "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["location"], REGION);
    for model in [DEEPSEEK, CLAUDE, NOVA, GPT] {
        let r = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
                json!({ "provider": "amazon-bedrock", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{model}: {:?}", r.body);
    }
    env
}

#[tokio::test]
async fn chat_models_go_to_the_openai_route_with_a_bearer_key_and_claude_to_messages_with_x_api_key()
 {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            DEEPSEEK,
            "Hello [mock:fixture=recorded-response-format-text]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("amazon-bedrock"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/openai/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    assert_eq!(sent.json()["model"], DEEPSEEK);

    let mut body = messages(
        CLAUDE,
        "Hello [mock:fixture=recorded-thinking-enabled-budget]",
    );
    body["max_tokens"] = json!(2048);
    body["thinking"] = json!({ "type": "enabled", "budget_tokens": 1024 });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][0]["type"], "thinking", "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/anthropic/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(sent.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(sent.json()["model"], CLAUDE);

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(CLAUDE, "Hello [mock:fixture=recorded-text-simple]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/anthropic/v1/messages");
}

#[tokio::test]
async fn a_model_bedrock_serves_only_through_its_own_api_is_refused_before_sending() {
    let env = env().await;
    for model in [NOVA, GPT] {
        let before = env.mock.received().len();
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(
                model,
                "Hello [mock:fixture=recorded-response-format-text]",
            )),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{model}: {}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!(
                "Amazon Bedrock serves `{model}` only through its own API, which Kaeso doesn't call yet."
            )
        );
        assert_eq!(r.json()["error"]["param"], "model");
        assert_eq!(env.mock.received().len(), before, "{model}: nothing sent");
    }
}

#[tokio::test]
async fn bedrock_refusals_of_the_key_are_refused_keys_on_both_routes_and_token_counts_are_refused()
{
    let env = env().await;
    for name in [
        "recorded-invalid-api-key",
        "recorded-long-term-key-format",
        "recorded-short-term-key-format",
        "recorded-refused-api-key",
    ] {
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(chat(DEEPSEEK, &format!("Hello {}", error(name)))),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains("refused"), "{name}: {message}");

        let mut body = messages(CLAUDE, &format!("Hello {}", error(name)));
        body["max_tokens"] = json!(64);
        let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{name}: {}", r.text());
        assert_eq!(
            r.json()["error"]["type"],
            "api_error",
            "{name}: {}",
            r.text()
        );
        let message = r.json()["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains("refused"), "{name}: {message}");
    }
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({ "model": CLAUDE, "messages": [{ "role": "user", "content": "Hello" }] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!("The model `{CLAUDE}` cannot count tokens.")
    );
}

#[tokio::test]
async fn a_key_calls_one_of_bedrocks_regions_which_may_change_and_other_keys_call_none() {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let key = |extra: Value| {
        let mut k = json!({ "provider": "amazon-bedrock", "name": "Bedrock", "secret": SECRET });
        for (name, value) in extra.as_object().unwrap() {
            k[name] = value.clone();
        }
        k
    };
    for (extra, message) in [
        (
            json!({}),
            "Amazon Bedrock keys call one region; give it as `location`.",
        ),
        (
            json!({ "location": "us-west-1" }),
            "Kaeso doesn't call that region for this provider; choose one from the list.",
        ),
        (
            json!({ "location": "global" }),
            "Kaeso doesn't call that region for this provider; choose one from the list.",
        ),
    ] {
        let r = add_key(&env, key(extra.clone())).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{extra}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], "location", "{extra}");
        assert_eq!(r.body["error"]["message"], message, "{extra}");
    }
    let r = add_key(
        &env,
        json!({ "provider": "openai", "name": "OpenAI", "secret": "sk-test-credential-0001", "location": REGION }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "This key calls no location; leave `location` out."
    );

    let r = add_key(&env, key(json!({ "location": "us-gov-west-1" }))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(
        (&r.body["auth_method"], &r.body["location"]),
        (&json!("api_key"), &json!("us-gov-west-1"))
    );
    let id = r.body["id"].as_str().unwrap().to_owned();
    let path = format!("{}/{id}", credentials(&env));
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Bedrock", "location": "ap-northeast-1" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["location"], "ap-northeast-1");
    let r = env
        .app
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "name": "Bedrock", "location": "mars-north-1" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": "bedrock-api-key-dGVzdC1jcmVkZW50aWFsLTAwMDI=" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["location"], "ap-northeast-1",
        "a replaced key keeps its region"
    );
    let r = env
        .app
        .post(
            &format!("{path}/replace"),
            json!({ "secret": SECRET, "location": "eu-west-3" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["location"], "eu-west-3");

    let listed = env
        .app
        .get(
            &format!("/orgs/{}/providers", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let providers = listed.body["data"].as_array().unwrap();
    let bedrock = providers
        .iter()
        .find(|p| p["id"] == "amazon-bedrock")
        .unwrap();
    assert_eq!(bedrock["name"], "Amazon Bedrock");
    assert_eq!(bedrock["key_prefix"], Value::Null);
    assert_eq!(
        bedrock["dialects"],
        json!(["openai_chat", "anthropic_messages"])
    );
    assert_eq!(bedrock["count_tokens"], Value::Null);
    let regions = bedrock["regions"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 22);
    assert!(regions.contains(&json!(REGION)));
    assert_eq!(
        bedrock["regions"]["regions_page"]["url"],
        "https://docs.aws.amazon.com/general/latest/gr/bedrock.html"
    );
    assert!(
        providers
            .iter()
            .filter(|p| p["id"] != "amazon-bedrock")
            .all(|p| p["regions"].is_null())
    );
}

#[tokio::test]
async fn a_model_check_is_inconclusive_and_sends_nothing() {
    let env = env().await;
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/models/check",
                env.owner.org_id, env.project
            ),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let mut failed: Vec<(&str, &str)> = r.body["failed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["model_id"].as_str().unwrap(),
                f["reason"].as_str().unwrap(),
            )
        })
        .collect();
    failed.sort_unstable();
    assert_eq!(
        failed,
        [
            (NOVA, "inconclusive"),
            (DEEPSEEK, "inconclusive"),
            (GPT, "inconclusive"),
            (CLAUDE, "inconclusive"),
        ],
        "{:?}",
        r.body
    );
    let bedrock: Vec<String> = env
        .mock
        .received()
        .into_iter()
        .map(|r| r.path)
        .filter(|p| p.starts_with("/openai/") || p.starts_with("/anthropic/"))
        .collect();
    assert!(
        bedrock.is_empty(),
        "no request reaches Bedrock: {bedrock:?}"
    );
}
