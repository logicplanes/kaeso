#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "csk-test-cerebras-credential-0000000000000000000000";
const GPT_OSS: &str = "gpt-oss-120b";
const QWEN: &str = "qwen-3.8-27b";

fn fixture(name: &str) -> String {
    format!("[mock:fixture=providers/cerebras/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=providers/cerebras/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "cerebras", "name": "cerebras", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [GPT_OSS, QWEN] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "cerebras", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_cerebras(env: &Env, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

fn weather_tool() -> Value {
    json!([{
        "name": "get_weather",
        "description": "Get the current weather for a city.",
        "input_schema": {
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }
    }])
}

#[tokio::test]
async fn a_chat_app_reaches_cerebras_with_the_bearer_key_and_gets_its_reasoning_and_timings() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": "cerebras/gpt-oss-120b",
            "max_completion_tokens": 300,
            "messages": [
                { "role": "developer", "content": "Answer in one short sentence." },
                { "role": "user", "content": format!("Capital of France? {}", fixture("recorded-basic")) },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("cerebras"));
    let sent = sent_to_cerebras(&env, GPT_OSS);
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["max_completion_tokens"], 300);
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["content"], "Paris is the capital of France.");
    assert_eq!(message["reasoning"], "Need short sentence answer.");
    assert_eq!(reply["time_info"]["prompt_time"], 0.001489698);
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        5
    );
}

#[tokio::test]
async fn an_anthropic_app_gets_thinking_and_a_tool_call_then_a_stream_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": GPT_OSS,
            "max_tokens": 400,
            "metadata": { "user_id": "user-42" },
            "tools": weather_tool(),
            "messages": [{ "role": "user", "content": format!("Weather in Paris? {}", fixture("recorded-tools")) }],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cerebras(&env, GPT_OSS);
    assert_eq!(sent["user"], "user-42");
    assert!(sent.get("safety_identifier").is_none(), "{sent}");
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(
        reply["content"][0]["thinking"],
        "We need to call get_weather."
    );
    let tool = &reply["content"][1];
    assert_eq!(tool["type"], "tool_use");
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(tool["input"], json!({ "city": "Paris", "unit": "celsius" }));
    assert_eq!(reply["usage"]["input_tokens"], 150);
    assert_eq!(reply["usage"]["output_tokens"], 36);

    let mut stream = messages(
        QWEN,
        &format!(
            "What is 17 times 23? {}",
            fixture("recorded-stream-reasoning")
        ),
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cerebras(&env, QWEN);
    assert_eq!(sent["stream_options"]["include_usage"], true);
    let events = sse_data(&r.text());
    let thinking: String = events
        .iter()
        .filter(|e| e["delta"]["type"] == "thinking_delta")
        .filter_map(|e| e["delta"]["thinking"].as_str())
        .collect();
    assert!(thinking.starts_with("We need answer user"), "{thinking}");
    let text: String = events
        .iter()
        .filter(|e| e["delta"]["type"] == "text_delta")
        .filter_map(|e| e["delta"]["text"].as_str())
        .collect();
    assert_eq!(text, "\n\n391");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["usage"]["output_tokens"], 51);
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
}

#[tokio::test]
async fn an_anthropic_apps_thinking_reaches_each_model_at_an_effort_it_takes() {
    let env = env().await;
    let ask = |model: &str, thinking: Value| {
        json!({
            "model": model,
            "max_tokens": 2000,
            "thinking": thinking,
            "messages": [{ "role": "user", "content": format!("17 times 23? {}", fixture("recorded-basic")) }],
        })
    };
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(ask(
            GPT_OSS,
            json!({ "type": "enabled", "budget_tokens": 1024 }),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_cerebras(&env, GPT_OSS)["reasoning_effort"], "low");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(ask(QWEN, json!({ "type": "disabled" }))),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(sent_to_cerebras(&env, QWEN)["reasoning_effort"], "none");
}

#[tokio::test]
async fn a_responses_app_gets_the_reasoning_then_the_answer() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": QWEN,
            "reasoning": { "effort": "low" },
            "prompt_cache_key": "codex-session-1",
            "parallel_tool_calls": true,
            "input": format!("What is 17 times 23? {}", fixture("recorded-reasoning")),
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), None);
    let sent = sent_to_cerebras(&env, QWEN);
    assert_eq!(sent["reasoning_effort"], "low");
    assert_eq!(sent["prompt_cache_key"], "codex-session-1");
    assert_eq!(sent["parallel_tool_calls"], true);
    let reply = r.json();
    assert_eq!(reply["output"][0]["type"], "reasoning");
    assert!(
        reply["output"][0]["summary"][0]["text"]
            .as_str()
            .unwrap()
            .contains("17*23 = 391")
    );
    assert_eq!(reply["output"][1]["content"][0]["text"], "\n\n391");
    assert_eq!(
        reply["usage"]["output_tokens_details"]["reasoning_tokens"],
        46
    );
}

#[tokio::test]
async fn grok_clis_replayed_reasoning_reaches_cerebras_as_reasoning() {
    let env = env().await;
    let path = kaeso_testkit::mock_provider::kpp_fixtures()
        .join("openai_chat/requests/grok-cli-reasoning-content.json");
    let mut body: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    body["model"] = json!(GPT_OSS);
    let last = body["messages"].as_array().unwrap().len() - 1;
    body["messages"][last]["content"] = json!(format!("say ok {}", fixture("recorded-stream")));
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cerebras(&env, GPT_OSS);
    let assistants: Vec<&Value> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "assistant")
        .collect();
    assert_eq!(assistants[0]["reasoning"], "The user wants echo ok.");
    assert_eq!(assistants[1]["reasoning"], "Answer briefly.");
    assert!(
        assistants
            .iter()
            .all(|m| m.get("reasoning_content").is_none()),
        "{sent}"
    );
    let text: String = sse_data(&r.text())
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(text, "Hello! How can I help you today?");
}

#[tokio::test]
async fn claude_codes_system_messages_after_the_first_turn_reach_cerebras_in_the_next_user_turn() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": QWEN,
            "max_tokens": 200,
            "system": "Answer in one short sentence.",
            "messages": [
                { "role": "user", "content": "Pick a colour." },
                { "role": "assistant", "content": "Blue." },
                { "role": "system", "content": "From now on, answer in French." },
                { "role": "user", "content": format!("Pick another one. {}", fixture("recorded-system-mid-conversation")) },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_cerebras(&env, QWEN);
    let roles: Vec<&str> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
    assert_eq!(
        sent["messages"][3]["content"][0]["text"],
        "From now on, answer in French."
    );
    assert_eq!(r.json()["content"][1]["text"], "Rouge.");
}

#[tokio::test]
async fn requests_cerebras_would_misread_or_refuse_are_refused_or_adapted_before_sending() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": GPT_OSS,
            "max_tokens": 300,
            "messages": [
                { "role": "user", "content": "Name three colours as a numbered list." },
                { "role": "assistant", "content": "1. Red\n2." },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Cerebras cannot continue a prefilled assistant message; end the conversation with a user message."
    );
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": GPT_OSS,
            "messages": [
                { "role": "user", "content": "Hi" },
                { "role": "assistant", "content": "Hello!", "refusal": null },
                { "role": "user", "content": "Say bye." },
            ],
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "messages[1].refusal");
    assert!(env.mock.received().is_empty());

    let mut body = chat(
        GPT_OSS,
        &format!("What time is it? {}", fixture("recorded-tools")),
    );
    body["tools"] = json!([{ "type": "function", "function": { "name": "get_time" } }]);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        sent_to_cerebras(&env, GPT_OSS)["tools"][0]["function"]["parameters"],
        json!({ "type": "object", "properties": {} })
    );
}

#[tokio::test]
async fn cerebras_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            QWEN,
            &format!("Describe it. {}", error("recorded-image-url")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let e = &r.json()["error"];
    assert_eq!(
        e["message"],
        "Remote image URLs are not supported; send images as data URIs (data:image/png;base64,...)"
    );
    assert_eq!(e["param"], "messages");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            GPT_OSS,
            &format!("Hi {}", error("recorded-unknown-model")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "not_found_error");
    assert_eq!(
        body["error"]["message"],
        "Model does not exist or you do not have access to it."
    );

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({ "model": GPT_OSS, "input": error("recorded-rate-limited") })),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.header("retry-after"), Some("60"));

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(GPT_OSS, &error("recorded-invalid-api-key"))),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
}

#[tokio::test]
async fn token_counts_for_cerebras_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": GPT_OSS,
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(
        body["error"]["message"],
        "The model `gpt-oss-120b` cannot count tokens."
    );

    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "cerebras/qwen-3.8-27b", "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    assert!(env.mock.received().is_empty());
}
