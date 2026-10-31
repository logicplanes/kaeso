#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::http::StatusCode;
use common::v1::{Capture, Env, call, chat, messages};
use serde_json::{Value, json};

const VERCEL_SECRET: &str = "vck_test-vercel-credential-0005";

const MODEL: &str = "nvidia/nemotron-3.5-lightning-free";

const EMBEDDING_MODEL: &str = "google/gemini-embedding-001";

// An env whose project has Vercel's free model, sent as `sends_as` when it is set.
async fn env_sending_as(sends_as: Option<&str>) -> Env {
    let env = common::v1::env(Some(std::sync::Arc::new(Capture::default())), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "vercel", "name": "vercel", "secret": VERCEL_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let overrides = sends_as.map_or_else(|| json!({}), |d| json!({ "egress_dialect": d }));
    for (model, overrides) in [(MODEL, overrides), (EMBEDDING_MODEL, json!({}))] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": "vercel", "model_id": model, "overrides": overrides }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

async fn env() -> Env {
    env_sending_as(None).await
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

fn deltas(events: &[Value], field: &str) -> String {
    events
        .iter()
        .filter(|e| e["type"] == "content_block_delta")
        .filter_map(|e| e["delta"][field].as_str())
        .collect()
}

#[tokio::test]
async fn an_openai_chat_app_reaches_vercel_with_its_key_and_keeps_vercels_fields() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            &format!("vercel/{MODEL}"),
            "Is 91 prime? [mock:fixture=providers/vercel/recorded-reasoning]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-provider"), Some("vercel"));
    let (path, auth, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {VERCEL_SECRET}").as_str())
    );
    assert_eq!(body["model"], MODEL);
    let reply = r.json();
    let message = &reply["choices"][0]["message"];
    assert_eq!(message["content"], "no, because 91 = 7 × 13.");
    assert!(
        message["reasoning"]
            .as_str()
            .unwrap()
            .starts_with("Here's a thinking process")
    );
    assert_eq!(message["reasoning_details"][0]["type"], "reasoning.text");
    assert_eq!(
        message["provider_metadata"]["gateway"]["routing"]["finalProvider"],
        "nvidia"
    );
    assert_eq!(reply["usage"]["cost"], 0);
    assert_eq!(
        reply["usage"]["completion_tokens_details"]["reasoning_tokens"],
        499
    );
}

#[tokio::test]
async fn an_anthropic_app_gets_vercels_reasoning_as_thinking_beside_its_tool_calls() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Weather in Paris? [mock:fixture=providers/vercel/recorded-tools]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, _, body) = sent(&env);
    assert_eq!(path, "/v1/chat/completions");
    assert_eq!(body["model"], MODEL);
    let reply = r.json();
    assert_eq!(reply["stop_reason"], "tool_use");
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert!(
        reply["content"][0]["thinking"]
            .as_str()
            .unwrap()
            .starts_with("We need to fetch weather for Paris")
    );
    let tool = &reply["content"][1];
    assert_eq!(tool["type"], "tool_use");
    assert_eq!(tool["name"], "get_weather");
    assert_eq!(
        tool["input"],
        json!({ "location": "Paris, France", "unit": "celsius" })
    );
    assert_eq!(reply["usage"]["input_tokens"], 348);
    assert_eq!(reply["usage"]["output_tokens"], 72);

    let mut stream = messages(
        MODEL,
        "Is 91 prime? [mock:fixture=providers/vercel/recorded-reasoning-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let events = sse_data(&r.text());
    assert!(deltas(&events, "thinking").starts_with("Here's a thinking process"));
    assert_eq!(deltas(&events, "text"), "no, because 91 = 7 × 13.");
    let end = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(end["delta"]["stop_reason"], "end_turn");
    assert_eq!(end["usage"]["output_tokens"], 401);
}

#[tokio::test]
async fn a_model_sent_as_messages_reaches_vercels_messages_api_with_the_bearer_key() {
    let env = env_sending_as(Some("anthropic_messages")).await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Is 91 prime? [mock:fixture=providers/vercel/recorded-thinking]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let last = env.mock.last().unwrap();
    assert_eq!(last.path, "/v1/messages");
    assert_eq!(
        last.header("authorization"),
        Some(format!("Bearer {VERCEL_SECRET}").as_str())
    );
    assert_eq!(last.header("x-api-key"), None);
    let reply = r.json();
    assert_eq!(reply["content"][0]["type"], "thinking");
    assert_eq!(reply["content"][0].get("signature"), None);
    assert_eq!(
        reply["content"][1]["text"],
        "No. It is divisible by 7 and 13."
    );

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(
            MODEL,
            "Weather in Paris? [mock:fixture=providers/vercel/recorded-tools]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/messages");
    let call_ = &r.json()["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(call_["function"]["name"], "get_weather");

    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Hi [mock:error=providers/vercel/recorded-rate-limited]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.header("retry-after"), Some("48"));
    let body = r.json();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "rate_limit_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Rate limit exceeded for nvidia/nemotron-3.5-lightning-free")
    );
}

#[tokio::test]
async fn a_model_sent_as_responses_reaches_vercels_responses_api() {
    let env = env_sending_as(Some("openai_responses")).await;
    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(json!({
            "model": MODEL,
            "input": "Weather in Paris? [mock:fixture=providers/vercel/recorded-tools]"
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, auth, body) = sent(&env);
    assert_eq!(path, "/v1/responses");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {VERCEL_SECRET}").as_str())
    );
    assert_eq!(body["model"], MODEL);
    let output = r.json()["output"].clone();
    assert_eq!(output[0]["type"], "reasoning");
    assert_eq!(output[1]["type"], "function_call");
    assert_eq!(output[1]["name"], "get_weather");

    let mut stream = messages(
        MODEL,
        "Is 91 prime? [mock:fixture=providers/vercel/recorded-reasoning-stream]",
    );
    stream["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(stream)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/responses");
    let events = sse_data(&r.text());
    assert!(!deltas(&events, "thinking").is_empty());
    assert_eq!(deltas(&events, "text"), "no, because 91 = 7 × 13.");
}

#[tokio::test]
async fn vercel_refusals_reach_each_app_in_its_own_error_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Hi [mock:error=providers/vercel/recorded-unknown-model]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["type"], "not_found_error");
    assert_eq!(
        body["error"]["message"],
        "Model 'kaeso-example/no-such-model' not found"
    );

    let marker = "Hi [mock:error=providers/vercel/recorded-restricted-model]";
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(MODEL, marker)),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "The provider refused this project's provider key. Ask an organization admin to check it under Providers."
    );
    let o = env.outcomes().await.pop().unwrap();
    assert!(
        o.error_message
            .as_deref()
            .unwrap()
            .contains("Free tier users do not have access to this model."),
        "{:?}",
        o.error_message
    );
}

#[tokio::test]
async fn a_refused_vercel_key_is_reported_as_the_providers_refusal() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages(
            MODEL,
            "Hi [mock:error=providers/vercel/recorded-invalid-api-key]",
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    let o = env.outcomes().await.pop().unwrap();
    assert_eq!(o.provider, Some("vercel"));
}

#[tokio::test]
async fn token_counts_for_vercel_models_are_refused_before_anything_is_sent() {
    for sends_as in [None, Some("anthropic_messages")] {
        let env = env_sending_as(sends_as).await;
        env.mock.clear();
        let r = call(
            &env,
            "/v1/messages/count_tokens",
            Some(&env.key),
            true,
            Some(json!({
                "model": MODEL,
                "messages": [{ "role": "user", "content": "Hello" }]
            })),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        let body = r.json();
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(
            body["error"]["message"],
            format!("The model `{MODEL}` cannot count tokens.")
        );
        let r = call(
            &env,
            "/v1/responses/input_tokens",
            Some(&env.key),
            false,
            Some(json!({ "model": MODEL, "input": "Hello" })),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!("The model `{MODEL}` cannot count tokens.")
        );
        assert!(env.mock.received().is_empty(), "{sends_as:?}");
    }
}

#[tokio::test]
async fn vercels_embedding_models_answer_embeddings_from_their_kpmr_kind() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({
            "model": EMBEDDING_MODEL,
            "input": ["Kaeso routes models. [mock:fixture=providers/vercel/recorded-embedding]"],
            "dimensions": 8
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let (path, auth, body) = sent(&env);
    assert_eq!(path, "/v1/embeddings");
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {VERCEL_SECRET}").as_str())
    );
    assert_eq!(body["dimensions"], 8);
    let reply = r.json();
    assert_eq!(reply["data"].as_array().unwrap().len(), 2);
    assert_eq!(reply["data"][0]["embedding"].as_array().unwrap().len(), 8);
    assert_eq!(reply["usage"]["prompt_tokens"], 12);

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat(EMBEDDING_MODEL, "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
}
