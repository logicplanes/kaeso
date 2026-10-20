#![allow(clippy::unwrap_used)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const GROQ_SECRET: &str = "gsk_test-groq-credential-0003";

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "groq", "name": "groq", "secret": GROQ_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/projects/{}/models", env.project),
            json!({ "provider": "groq", "model_id": "openai/gpt-oss-20b" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    env
}

fn sent(env: &Env) -> (String, Option<String>, Value) {
    let last = env.mock.last().unwrap();
    (
        last.path.clone(),
        last.header("authorization").map(str::to_owned),
        last.json(),
    )
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn an_openai_chat_app_reaches_groq_and_keeps_its_fields() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "groq/openai/gpt-oss-20b",
            "Hello! [mock:fixture=providers/groq/recorded-plain-developer-string]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("groq"));
    let (path, auth, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {GROQ_SECRET}").as_str())
    );
    assert_eq!(body["model"], "openai/gpt-oss-20b");
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["content"], "Hello! 👋 How can I assist you today?");
    assert_eq!(
        message["reasoning"],
        "We need to respond in a helpful manner. Probably greet and ask how can I help."
    );
    assert_eq!(reply["x_groq"]["id"], "req_01m58s7qcqe008dp17brjt05h9");
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        19
    );
}

#[tokio::test]
async fn an_anthropic_app_gets_groq_tool_calls_and_streams_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "openai/gpt-oss-20b",
            "Weather in Paris? [mock:fixture=providers/groq/recorded-tools-system-role]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(body["model"], "openai/gpt-oss-20b");
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    let tool = &reply["content"][0];
    assert_eq!(tool["type"], "tool_use");
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(
        tool["input"],
        json!({ "location": "Paris, France", "unit": "celsius" })
    );
    assert_eq!(reply["usage"]["input_tokens"], 180);
    assert_eq!(reply["usage"]["output_tokens"], 37);

    let mut stream = messages(
        "openai/gpt-oss-20b",
        "Hello! [mock:fixture=providers/groq/recorded-stream-doc]",
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
    assert_eq!(text, "Hello! 👋 How can I help you today?");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["usage"]["output_tokens"], 67);
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
}

#[tokio::test]
async fn groq_refusals_reach_each_app_in_its_own_error_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            "openai/gpt-oss-20b",
            "Yes or no? [mock:error=providers/groq/recorded-logit-bias]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let error = &r.json()["error"];
    assert_eq!(
        error["message"],
        "`logit_bias` is not supported with this model"
    );
    assert_eq!(error["param"], "logit_bias");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            "openai/gpt-oss-20b",
            "Hello! [mock:error=providers/groq/recorded-n-above-one]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["message"], "'n' : number must be at most 1");
}

#[tokio::test]
async fn fields_groq_refuses_are_refused_before_anything_is_sent_and_developer_is_sent_as_system() {
    let env = env().await;
    env.mock.clear();
    let mut body = chat("openai/gpt-oss-20b", "Yes or no?");
    body["logprobs"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "`logprobs` is not supported by Groq; remove it from the request."
    );
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": "openai/gpt-oss-20b",
            "messages": [{ "role": "user", "content": "Hi", "name": "alice" }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "messages[0].name");
    assert!(env.mock.received().is_empty());

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": "openai/gpt-oss-20b",
            "messages": [
                { "role": "developer", "content": "Answer in one word." },
                { "role": "user", "content": "Hi [mock:fixture=providers/groq/recorded-plain-developer-string]" }
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (_, _, sent) = sent(&env);
    assert_eq!(sent["messages"][0]["role"], "system");
}

#[tokio::test]
async fn token_counts_for_groq_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": "openai/gpt-oss-20b",
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "The model `openai/gpt-oss-20b` cannot count tokens."
    );

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "groq/openai/gpt-oss-20b", "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    assert!(env.mock.received().is_empty());
}
