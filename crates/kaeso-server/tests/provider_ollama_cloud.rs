#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call};
use serde_json::{Value, json};

const SECRET: &str = "0123456789abcdef0123456789abcdef.ollama-test-credential";
const GPT_OSS: &str = "gpt-oss:20b";
const GEMMA: &str = "gemma4:31b";
const ANSWER: &str = "No, 1001 is not a prime number because it can be factored as 7\u{202f}×\u{202f}11\u{202f}×\u{202f}13.";

fn fixture(folder: &str, name: &str) -> String {
    format!("[mock:fixture=../providers/ollama-cloud/{folder}/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=../providers/ollama-cloud/errors/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "ollama-cloud", "name": "ollama", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GPT_OSS, GEMMA] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "ollama-cloud", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_ollama(env: &Env, model: &str) -> Value {
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

#[tokio::test]
async fn a_chat_app_reaches_a_tagged_model_and_gets_its_reasoning_back() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GPT_OSS,
            "max_tokens": 400,
            "reasoning_effort": "low",
            "messages": [{ "role": "user", "content": fixture("responses", "recorded-reasoning-effort") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["choices"][0]["message"]["content"], ANSWER);
    assert_eq!(
        reply["choices"][0]["message"]["reasoning"],
        "Need one sentence."
    );
    assert_eq!(reply["usage"]["prompt_tokens_details"]["cached_tokens"], 64);
    let sent = sent_to_ollama(&env, GPT_OSS);
    assert_eq!(sent["max_tokens"], 400);
    assert_eq!(sent["reasoning_effort"], "low");
}

#[tokio::test]
async fn an_anthropic_app_gets_thinking_and_its_output_limit_as_max_tokens() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": format!("ollama-cloud/{GPT_OSS}"),
            "max_tokens": 300,
            "messages": [{ "role": "user", "content": fixture("responses", "recorded-reasoning-effort") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(reply["content"][0]["thinking"], "Need one sentence.");
    assert_eq!(reply["content"][1]["text"], ANSWER);
    assert_eq!(reply["usage"]["cache_read_input_tokens"], 64);
    let sent = sent_to_ollama(&env, GPT_OSS);
    assert_eq!(sent["max_tokens"], 300, "{sent}");
    assert!(sent.get("max_completion_tokens").is_none(), "{sent}");
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
            "model": GPT_OSS,
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
        !collect("thinking_delta", "thinking").is_empty(),
        "{}",
        r.text()
    );
    assert_eq!(collect("text_delta", "text"), "101");
    assert!(r.text().contains("event: message_stop"), "{}", r.text());
    let sent = sent_to_ollama(&env, GPT_OSS);
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["max_tokens"], 400);
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
            "model": GPT_OSS,
            "max_output_tokens": 400,
            "input": fixture("responses", "recorded-reasoning-tools"),
            "tools": [{
                "type": "function", "name": "get_weather",
                "description": "Get the current weather for a city",
                "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
            }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let output = &r.json()["output"];
    assert_eq!(output[0]["type"], "reasoning");
    assert_eq!(output[0]["summary"][0]["text"], "Need to call get_weather.");
    let call = output
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap_or_else(|| panic!("{output}"));
    assert_eq!(call["name"], "get_weather");
    assert_eq!(call["arguments"], r#"{"city":"Paris"}"#);
    let sent = sent_to_ollama(&env, GPT_OSS);
    assert_eq!(sent["max_tokens"], 400);
}

#[tokio::test]
async fn fields_ollama_ignores_are_refused_before_it_is_called() {
    let env = env().await;
    let before = env.mock.received().len();
    for (field, value) in [
        ("logprobs", json!(true)),
        ("logit_bias", json!({ "1": -100 })),
    ] {
        let mut body = json!({
            "model": GEMMA,
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
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn ollama_errors_reach_each_app_in_its_own_format_and_key_refusals_name_no_detail() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": GEMMA,
            "max_tokens": 10,
            "messages": [{ "role": "user", "content": error("recorded-model-outside-plan") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(
        body["error"]["message"],
        "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": GEMMA,
            "input": error("recorded-invalid-api-key"),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GEMMA,
            "messages": [{ "role": "user", "content": error("recorded-unknown-model") }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not found"),
        "{}",
        r.text()
    );
}

#[tokio::test]
async fn token_counting_is_refused_in_the_apps_format_without_calling_ollama() {
    let env = env().await;
    let before = env.mock.received().len();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": GPT_OSS,
            "messages": [{ "role": "user", "content": "Name one planet." }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(
        body["error"]["message"],
        format!("The model `{GPT_OSS}` cannot count tokens.")
    );
    assert_eq!(env.mock.received().len(), before);
}

#[tokio::test]
async fn thinking_an_anthropic_app_sends_back_reaches_ollama_as_reasoning() {
    let env = env().await;
    let ask = |content: Value| {
        json!({
            "model": GPT_OSS,
            "max_tokens": 400,
            "tools": [{
                "name": "get_weather", "description": "Get the current weather for a city",
                "input_schema": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] }
            }],
            "messages": content,
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
    let call_id = reply["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap()["id"]
        .clone();
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
    let sent = sent_to_ollama(&env, GPT_OSS);
    assert_eq!(
        sent["messages"][1]["reasoning"], "Need to call get_weather.",
        "{sent}"
    );
    assert_eq!(sent["messages"][1]["tool_calls"][0]["id"], call_id);
}
