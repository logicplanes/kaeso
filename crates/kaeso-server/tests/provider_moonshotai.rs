#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call};
use serde_json::{Value, json};

const SECRET: &str = "sk-moonshot-test-credential-0000000000000000";
const K26: &str = "kimi-k2.6";
const K3: &str = "kimi-k3";
const WEATHER: &str = "Get the current weather for a city";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/moonshotai/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/moonshotai/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "moonshotai", "name": "moonshot", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [K26, K3] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "moonshotai", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_moonshot(env: &Env, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(sent.header("x-api-key"), None);
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .collect()
}

fn anthropic_weather_tool() -> Value {
    json!([{
        "name": "get_weather", "description": WEATHER,
        "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
    }])
}

#[tokio::test]
async fn a_chat_app_gets_the_reasoning_content_and_its_developer_message_goes_as_system() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": K26,
            "max_completion_tokens": 800,
            "messages": [
                { "role": "developer", "content": "Answer in one sentence." },
                { "role": "user", "content": fixture("responses", "recorded-reasoning") },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let message = &r.json()["choices"][0]["message"];
    assert!(
        message["content"]
            .as_str()
            .unwrap()
            .starts_with("1001 is not a prime number"),
        "{message}"
    );
    assert!(
        message["reasoning_content"]
            .as_str()
            .unwrap()
            .starts_with("The user is asking whether 1001 is a prime number"),
        "{message}"
    );
    let sent = sent_to_moonshot(&env, K26);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_completion_tokens"], 800);
}

#[tokio::test]
async fn an_anthropic_app_gets_thinking_then_the_answer_from_a_k3_model() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": format!("moonshotai/{K3}"),
            "max_tokens": 800,
            "thinking": { "type": "enabled", "budget_tokens": 2048 },
            "messages": [{ "role": "user", "content": fixture("responses", "recorded-reasoning-effort-low") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(
        reply["content"][0]["thinking"],
        "1001 = 7 × 143 = 7 × 11 × 13, so not prime."
    );
    assert_eq!(
        reply["content"][1]["text"],
        "No, 1001 is not prime — it equals 7 × 11 × 13."
    );
    let sent = sent_to_moonshot(&env, K3);
    assert_eq!(sent["reasoning_effort"], "low");
    assert_eq!(sent["max_completion_tokens"], 800);
}

#[tokio::test]
async fn an_anthropic_app_streams_thinking_then_the_answer() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": K26,
            "max_tokens": 400,
            "stream": true,
            "messages": [{ "role": "user", "content": fixture("streams", "recorded-reasoning-stream") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    let collect = |kind: &str, field: &str| -> String {
        events
            .iter()
            .filter(|e| e["delta"]["type"] == kind)
            .map(|e| e["delta"][field].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(
        collect("thinking_delta", "thinking").starts_with("The user"),
        "{}",
        r.text()
    );
    assert_eq!(collect("text_delta", "text"), "56");
    let thinking_blocks = events
        .iter()
        .filter(|e| e["type"] == "content_block_start" && e["content_block"]["type"] == "thinking")
        .count();
    assert_eq!(thinking_blocks, 1, "{}", r.text());
    assert!(r.text().contains("event: message_stop"), "{}", r.text());
    assert_eq!(sent_to_moonshot(&env, K26)["stream"], true);
}

#[tokio::test]
async fn an_anthropic_prefill_reaches_moonshot_as_partial_and_the_reply_continues_it() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": K26,
            "max_tokens": 20,
            "messages": [
                { "role": "user", "content": fixture("responses", "recorded-partial") },
                { "role": "assistant", "content": "1. Red\n2." },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["content"][0]["text"], " Blue\n3. Green");
    let sent = sent_to_moonshot(&env, K26);
    assert_eq!(
        sent["messages"][1],
        json!({ "role": "assistant", "content": "1. Red\n2.", "partial": true })
    );
}

#[tokio::test]
async fn thinking_an_anthropic_app_sends_back_reaches_moonshot_as_reasoning_content() {
    let env = env().await;
    let ask = |messages: Value| {
        json!({
            "model": K26,
            "max_tokens": 400,
            "tools": anthropic_weather_tool(),
            "messages": messages,
        })
    };
    let first =
        json!([{ "role": "user", "content": fixture("responses", "recorded-reasoning-tools") }]);
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(ask(first.clone())),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking", "{reply}");
    let call_id = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap()["id"]
        .clone();
    assert_eq!(call_id, "get_weather_0_2fdaa899");
    let mut next = first.as_array().unwrap().clone();
    next.push(json!({ "role": "assistant", "content": reply["content"] }));
    next.push(json!({ "role": "user", "content": [
        { "type": "tool_result", "tool_use_id": call_id, "content": "18 °C and sunny" },
        { "type": "text", "text": fixture("responses", "recorded-tool-result") },
    ]}));
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(ask(json!(next))),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_moonshot(&env, K26);
    assert!(
        sent["messages"][1]["reasoning_content"]
            .as_str()
            .unwrap()
            .starts_with("The user is asking for the weather in Paris"),
        "{sent}"
    );
    assert_eq!(sent["messages"][1]["tool_calls"][0]["id"], call_id);
    assert!(sent["messages"][1].get("partial").is_none(), "{sent}");
}

#[tokio::test]
async fn a_responses_app_gets_reasoning_before_the_tool_call() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": K26,
            "max_output_tokens": 400,
            "input": fixture("responses", "recorded-reasoning-tools"),
            "tools": [{
                "type": "function", "name": "get_weather", "description": WEATHER,
                "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
            }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let output = &r.json()["output"];
    assert_eq!(output[0]["type"], "reasoning", "{output}");
    let call = output
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap_or_else(|| panic!("{output}"));
    assert_eq!(call["name"], "get_weather");
    assert_eq!(call["arguments"], r#"{"city": "Paris"}"#);
    assert_eq!(sent_to_moonshot(&env, K26)["max_completion_tokens"], 400);
}

#[tokio::test]
async fn fields_moonshot_ignores_and_out_of_range_values_are_refused_before_it_is_called() {
    let env = env().await;
    let before = env.mock.received().len();
    for (field, value) in [
        ("parallel_tool_calls", json!(false)),
        ("store", json!(true)),
        ("service_tier", json!("flex")),
        ("temperature", json!(0.5)),
    ] {
        let mut body = json!({
            "model": K26,
            "messages": [{ "role": "user", "content": "Hi" }],
        });
        body[field] = value;
        let r = call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(body),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert!(r.text().contains(field), "{}", r.text());
    }
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": K3,
            "reasoning_effort": "medium",
            "messages": [{ "role": "user", "content": "Hi" }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("`low`, `high`, `max`"), "{}", r.text());
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn moonshot_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": K26, "input": error("recorded-invalid-api-key") })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": K26,
            "max_tokens": 10,
            "messages": [{ "role": "user", "content": error("exceeded-current-quota") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": K26,
            "messages": [{ "role": "user", "content": error("recorded-rate-limited") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert!(!r.text().contains("ak-0000"), "{}", r.text());

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": K26,
            "messages": [{ "role": "user", "content": error("recorded-tool-choice-required-thinking") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.text()
            .contains("tool_choice 'required' is incompatible with thinking enabled"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn token_counting_is_refused_in_the_apps_format_without_calling_moonshot() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": K26,
            "messages": [{ "role": "user", "content": "Name one planet." }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(
        body["error"]["message"],
        format!("The model `{K26}` cannot count tokens.")
    );
    assert_eq!(env.mock.received().len(), before);
}
