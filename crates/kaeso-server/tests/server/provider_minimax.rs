#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use axum::http::StatusCode;
use common::v1::{Env, call, chat, messages};
use serde_json::{Value, json};

const SECRET: &str = "sk-api-test-minimax-credential-0000000000000000";
const M3: &str = "MiniMax-M3";
const FLASH: &str = "MiniMax-M3.1-Flash";

fn fixture(name: &str) -> String {
    format!("[mock:fixture=providers/minimax/{name}]")
}

fn error(name: &str) -> String {
    format!("[mock:error=providers/minimax/{name}]")
}

async fn env() -> Env {
    let env = common::v1::env(None, None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "minimax", "name": "minimax", "secret": SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for model in [M3, FLASH] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "minimax", "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn sent_to_minimax(env: &Env, model: &str) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(SECRET));
    assert_eq!(sent.header("authorization"), None);
    let body = sent.json();
    assert_eq!(body["model"], model);
    body
}

fn weather_tool() -> Value {
    json!([{
        "name": "get_weather",
        "description": "Get the current weather for a given location.",
        "input_schema": {
            "type": "object",
            "properties": { "location": { "type": "string" } },
            "required": ["location"]
        }
    }])
}

#[tokio::test]
async fn an_anthropic_app_reaches_minimax_with_its_key_and_gets_signed_thinking_and_a_tool_call() {
    let env = env().await;
    let request = json!({
        "model": FLASH,
        "max_tokens": 4096,
        "thinking": { "type": "adaptive" },
        "output_config": { "effort": "max" },
        "tool_choice": { "type": "auto" },
        "tools": weather_tool(),
        "messages": [{ "role": "user", "content": format!("Weather in San Francisco? {}", fixture("tool-use-auto")) }]
    });
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(request.clone()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("minimax"));
    let sent = sent_to_minimax(&env, FLASH);
    for field in [
        "thinking",
        "output_config",
        "tool_choice",
        "tools",
        "max_tokens",
    ] {
        assert_eq!(sent[field], request[field], "{field}");
    }
    let reply = r.json();
    assert_eq!(reply["id"], "066b133406cd8cfd88eb3b2b893a5daa");
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(
        reply["content"][0]["signature"],
        "fa86efec7c27d4aa23eac60ba7a1f4e5f0f37a18d6ec4944fe495aebb6bd0930"
    );
    assert_eq!(reply["content"][2]["name"], "get_weather");
    assert_eq!(reply["usage"]["cache_read_input_tokens"], 404);
}

#[tokio::test]
async fn chat_and_responses_apps_get_minimaxs_answers_in_their_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(json!({
            "model": "minimax/MiniMax-M3.1-Flash",
            "max_completion_tokens": 1024,
            "tool_choice": "auto",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "parameters": { "type": "object", "properties": { "location": { "type": "string" } } }
                }
            }],
            "messages": [{ "role": "user", "content": format!("Weather? {}", fixture("tool-use-auto")) }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = sent_to_minimax(&env, FLASH);
    assert_eq!(sent["tool_choice"], json!({ "type": "auto" }));
    let call_ = &r.json()["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(call_["id"], "call_function_9pjh8jjdebck_1");
    assert_eq!(call_["function"]["name"], "get_weather");

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": M3,
            "max_output_tokens": 4096,
            "reasoning": { "effort": "high" },
            "input": format!("Which is bigger, 9.11 or 9.9? {}", fixture("deep-thinking"))
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("reasoning.effort"));
    let sent = sent_to_minimax(&env, M3);
    assert!(sent.get("output_config").is_none(), "{sent}");
    let output = r.json()["output"].clone();
    assert_eq!(output[0]["type"], "reasoning");
    assert_eq!(output[1]["type"], "message");

    let mut stream = messages(
        FLASH,
        &format!("Describe it. {}", fixture("image-url-stream")),
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let text = r.text();
    assert!(text.contains("\"signature_delta\""), "{text}");
    assert!(text.contains("event: message_stop"), "{text}");
}

#[tokio::test]
async fn what_minimax_ignores_is_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let mut top_k = messages(M3, "Hi");
    top_k["top_k"] = json!(40);
    let mut stop = chat(M3, "Hi");
    stop["stop"] = json!(["END"]);
    let mut temperature = chat(M3, "Hi");
    temperature["temperature"] = json!(0.2);
    let mut forced = messages(M3, "Hi");
    forced["tools"] = weather_tool();
    forced["tool_choice"] = json!({ "type": "any" });
    for (path, anthropic, body, param) in [
        ("/v1/messages", true, top_k, "top_k"),
        ("/v1/chat/completions", false, stop, "stop"),
        ("/v1/chat/completions", false, temperature, "temperature"),
        ("/v1/messages", true, forced, "tool_choice"),
    ] {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{param}: {}", r.text());
        let body = r.json();
        let message = body["error"]["message"].as_str().unwrap();
        assert!(message.contains("MiniMax"), "{param}: {message}");
        if !anthropic {
            assert_eq!(body["error"]["param"], param);
        }
    }
    assert_eq!(
        call(
            &env,
            "/v1/chat/completions",
            Some(&env.key),
            false,
            Some(json!({ "model": M3, "temperature": 0.2, "messages": [{ "role": "user", "content": "Hi" }] })),
        )
        .await
        .json()["error"]["message"],
        "MiniMax always samples at `temperature` 1; remove `temperature` or set it to 1."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn claude_codes_context_management_that_only_clears_thinking_is_left_out_and_named() {
    let env = env().await;
    let mut body = messages(M3, "Hi");
    body["context_management"] =
        json!({ "edits": [{ "type": "clear_thinking_20251015", "keep": "all" }] });
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("context_management"));
    let sent = sent_to_minimax(&env, M3);
    assert!(sent.get("context_management").is_none(), "{sent}");
}

#[tokio::test]
async fn minimax_errors_reach_each_app_in_its_own_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(M3, &format!("Hi {}", error("rate-limited")))),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            M3,
            &format!("Hi {}", error("unsupported-content-type")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "messages.0.content.1: unsupported content type 'foo'"
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            M3,
            &format!("Hi {}", error("recorded-invalid-api-key")),
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
}

#[tokio::test]
async fn token_counts_for_minimax_models_are_refused_before_anything_is_sent() {
    let env = env().await;
    env.mock.clear();
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(json!({
            "model": M3,
            "messages": [{ "role": "user", "content": "Hello" }]
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The model `MiniMax-M3` cannot count tokens."
    );
    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "minimax/MiniMax-M3.1-Flash", "input": "Hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["error"]["param"], "model");
    assert!(env.mock.received().is_empty());
}
