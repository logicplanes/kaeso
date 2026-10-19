mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const XAI_SECRET: &str = "xai-test-xai-credential-0001";

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "xai", "name": "xai", "secret": XAI_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [
        json!({ "provider": "xai", "model_id": "grok-4.3" }),
        json!({
            "provider": "xai",
            "model_id": "grok-4.20-0309-non-reasoning",
            "overrides": { "egress_dialect": "openai_chat" }
        }),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                model,
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent(env: &Env) -> Option<(String, Option<String>, Value)> {
    let last = env.mock.last()?;
    Some((
        last.path.clone(),
        last.header("authorization").map(str::to_owned),
        last.json(),
    ))
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn an_openai_responses_app_reaches_xai_and_keeps_its_citations_and_usage() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": "xai/grok-4.3",
            "input": "Opening hours? [mock:fixture=providers/xai/recorded-web-search]",
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("xai"));
    let (path, auth, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {XAI_SECRET}").as_str())
    );
    assert_eq!(body["model"], "grok-4.3");
    let reply = r.json();
    let message = reply["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "message")
        .unwrap();
    let citation = &message["content"][0]["annotations"][0];
    assert_eq!(citation["type"], "url_citation");
    assert_eq!(citation["url"], "https://oceanario.pt/en/");
    assert_eq!(reply["usage"]["num_server_side_tools_used"], 2);
    assert_eq!(reply["usage"]["cost_in_usd_ticks"], 232_046_000);
    assert_eq!(
        reply["usage"]["server_side_tool_usage_details"]["web_search_calls"],
        2
    );
}

#[tokio::test]
async fn an_openai_chat_app_keeps_xai_reasoning_content_on_the_chat_dialect() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "grok-4.20-0309-non-reasoning",
            "17 * 24? [mock:fixture=providers/xai/recorded-reasoning-content]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(body["model"], "grok-4.20-0309-non-reasoning");
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["content"], "408");
    assert_eq!(
        message["reasoning_content"],
        "Calculating 17 multiplied by 24."
    );
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        178
    );
    assert_eq!(
        reply["usage"]["prompt_tokens_details"]["cached_tokens"],
        128
    );
    assert_eq!(reply["usage"]["cost_in_usd_ticks"], 5_706_000);
}

#[tokio::test]
async fn an_anthropic_app_gets_xai_tool_calls_and_streams_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "grok-4.3",
            "Weather in Boston? [mock:fixture=providers/xai/recorded-tools-function]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env).unwrap();
    assert_eq!(path, "/v1/responses");
    assert_eq!(body["model"], "grok-4.3");
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    let tool = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(tool["name"], "get_current_weather");
    assert_eq!(
        tool["input"],
        json!({ "location": "Boston, MA", "unit": "fahrenheit" })
    );
    assert_eq!(reply["usage"]["output_tokens"], 200);

    let mut stream = messages(
        "grok-4.3",
        "Hello! [mock:fixture=providers/xai/recorded-text-basic-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let text: String = events
        .iter()
        .filter(|e| e["type"] == "content_block_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "Hello! How can I help you today?");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["usage"]["output_tokens"], 116);
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
}

#[tokio::test]
async fn xai_errors_reach_each_app_in_its_own_error_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "grok-4.20-0309-non-reasoning",
            "Say hi. [mock:error=providers/xai/recorded-presence-penalty-reasoning-model]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(
        error["message"],
        "Model grok-4.3 does not support parameter presencePenalty."
    );
    assert_eq!(error["type"], "invalid_request_error");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "grok-4.3",
            "Weather? [mock:error=providers/xai/recorded-reasoning-encrypted]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "Could not decrypt the provided encrypted_content. Ensure the value is the unmodified encrypted_content from a previous response."
    );
}

#[tokio::test]
async fn token_counts_for_xai_models_are_refused_in_each_apps_format() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": "grok-4.3",
            "messages": [{ "role": "user", "content": "Hello" }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "The model `grok-4.3` cannot count tokens."
    );

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "grok-4.3", "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(error["type"], "invalid_request_error");
    assert_eq!(error["message"], "The model `grok-4.3` cannot count tokens.");
    assert!(env.mock.received().is_empty());
}
