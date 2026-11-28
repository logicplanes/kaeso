#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Answer, Capture, Env, call, call_app};
use serde_json::{Value, json};

async fn env(extra: &[(&str, &str)]) -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), Some(extra)).await
}

fn tools() -> Value {
    json!([{ "type": "function", "function": { "name": "get_weather", "description": "Weather in a city", "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] } } }])
}

fn first_turn(fixture: &str, stream: bool) -> Value {
    json!({
        "model": "claude-haiku-4-5",
        "reasoning_effort": "low",
        "stream": stream,
        "tools": tools(),
        "messages": [{ "role": "user", "content": format!("Weather in Paris? [mock:fixture={fixture}]") }],
    })
}

fn second_turn(tool_call_id: &str) -> Value {
    json!({
        "model": "claude-haiku-4-5",
        "reasoning_effort": "low",
        "tools": tools(),
        "messages": [
            { "role": "user", "content": "Weather in Paris?" },
            { "role": "assistant", "content": null, "tool_calls": [{ "id": tool_call_id, "type": "function", "function": { "name": "get_weather", "arguments": "{\"city\":\"Paris\"}" } }] },
            { "role": "tool", "tool_call_id": tool_call_id, "content": "18 °C, sunny" },
        ],
    })
}

fn tool_call_id(r: &Answer) -> String {
    let text = r.text();
    if let Ok(v) = serde_json::from_str::<Value>(&text) {
        return v["choices"][0]["message"]["tool_calls"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
    }
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .find_map(|c| {
            c["choices"][0]["delta"]["tool_calls"][0]["id"]
                .as_str()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| panic!("no tool call in {text}"))
}

fn sent_assistant_turn(env: &Env) -> Value {
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    sent.json()["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant")
        .unwrap()
        .clone()
}

fn chat(env: &Env, key: &str, body: Value) -> impl Future<Output = Answer> {
    let key = key.to_owned();
    async move { call(env, "/v1/chat/completions", Some(&key), false, Some(body)).await }
}

#[tokio::test]
async fn a_chat_app_on_claude_keeps_its_thinking_through_a_tool_loop() {
    let env = env(&[]).await;
    for (fixture, stream, signature) in [
        ("thinking-interleaved-tools", false, "EqEvMLy0mgL4Jn097PRt"),
        ("thinking-long-tool-loop", true, "EqQBCkYIBhgCKkBkZWVw"),
    ] {
        let first = chat(&env, &env.key, first_turn(fixture, stream)).await;
        assert_eq!(first.status, StatusCode::OK, "{}", first.text());
        assert!(!first.text().contains(signature), "{fixture}");
        let id = tool_call_id(&first);

        let second = chat(&env, &env.key, second_turn(&id)).await;
        assert_eq!(second.status, StatusCode::OK, "{}", second.text());
        let turn = sent_assistant_turn(&env);
        let blocks = turn["content"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "thinking", "{fixture}: {turn}");
        assert!(
            blocks[0]["signature"]
                .as_str()
                .unwrap()
                .starts_with(signature),
            "{fixture}: {turn}"
        );
        assert!(
            !blocks[0]["thinking"].as_str().unwrap().is_empty(),
            "{fixture}"
        );
        assert_eq!(blocks.last().unwrap()["type"], "tool_use");
        assert_eq!(blocks.last().unwrap()["id"], id.as_str());
        assert!(
            env.mock.last().unwrap().json().get("thinking").is_some(),
            "{fixture}: thinking stays on"
        );

        let other = env
            .app
            .post(
                &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
                json!({ "name": format!("other-{fixture}") }),
                Some(&env.owner.cookie),
            )
            .await
            .body["secret"]
            .as_str()
            .unwrap()
            .to_owned();
        let r = chat(&env, &other, second_turn(&id)).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        let turn = sent_assistant_turn(&env);
        assert!(
            turn["content"]
                .as_array()
                .unwrap()
                .iter()
                .all(|b| b["type"] != "thinking"),
            "{fixture}: {turn}"
        );
    }

    let mut conn = env.app.state.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("reasoning:*"))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(!keys.is_empty());
    assert!(
        keys.iter().all(|k| k.contains("reasoning:v2:")),
        "entries carry the version of their shape: {keys:?}"
    );
    for k in keys {
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&k)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert!(ttl > 0 && ttl <= 3_600_000, "{k}: {ttl}");
        let fields: std::collections::HashMap<String, Vec<u8>> = redis::cmd("HGETALL")
            .arg(&k)
            .query_async(&mut conn)
            .await
            .unwrap();
        let all: Vec<u8> = fields.values().flatten().copied().collect();
        let text = String::from_utf8_lossy(&all);
        assert!(!text.contains("EqEvMLy0") && !text.contains("Paris"), "{k}");
    }
}

#[tokio::test]
async fn thinking_is_kept_when_the_app_closes_right_after_the_last_event() {
    let env = env(&[]).await;
    let fixture = "thinking-long-tool-loop";
    let gate = common::v1::hold_back_the_end(&env, "anthropic_messages", fixture);
    let mut first = first_turn(fixture, true);
    first["messages"][0]["content"] = json!(format!(
        "Weather in Paris? [mock:fixture={fixture}] {}",
        gate.steps_marker()
    ));
    let (_, mut body) = common::v1::open_stream(&env, "/v1/chat/completions", false, &first).await;
    let seen = common::v1::read_until(&mut body, "data: [DONE]").await;
    drop(body);
    let id = tool_call_id(&Answer {
        status: StatusCode::OK,
        headers: axum::http::HeaderMap::new(),
        body: seen.into_bytes(),
    });
    common::eventually("the first turn is recorded", || async {
        !env.capture.all().is_empty()
    })
    .await;
    let second = chat(&env, &env.key, second_turn(&id)).await;
    gate.finish();
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    let turn = sent_assistant_turn(&env);
    assert_eq!(turn["content"][0]["type"], "thinking", "{turn}");
}

#[tokio::test]
async fn without_valkey_or_when_off_the_tool_loop_goes_on_without_thinking() {
    let env = env(&[("KAESO_CONTINUITY_REASONING_TTL", "0s")]).await;
    let first = chat(
        &env,
        &env.key,
        first_turn("thinking-interleaved-tools", false),
    )
    .await;
    let id = tool_call_id(&first);
    let r = chat(&env, &env.key, second_turn(&id)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let turn = sent_assistant_turn(&env);
    assert!(
        turn["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["type"] != "thinking"),
        "{turn}"
    );
    let mut conn = env.app.state.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("reasoning:*"))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(keys.is_empty(), "nothing kept when off: {keys:?}");

    let down = common::App::start_with(
        &env.services,
        &[
            ("KAESO_VALKEY_URL", "redis://127.0.0.1:1"),
            (
                "KAESO_PROVIDERS_OPENAI_BASE_URL",
                env.mock.base_url.as_str(),
            ),
            (
                "KAESO_PROVIDERS_ANTHROPIC_BASE_URL",
                env.mock.base_url.as_str(),
            ),
        ],
    )
    .await;
    let first = call_app(
        &down,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(first_turn("thinking-interleaved-tools", false)),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let r = call_app(
        &down,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(second_turn(&tool_call_id(&first))),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

fn responses_tools() -> Value {
    json!([{ "type": "function", "name": "get_weather", "description": "Weather in a city", "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] } }])
}

fn responses_first_turn(store: Option<bool>) -> Value {
    let mut body = json!({
        "model": "claude-haiku-4-5",
        "reasoning": { "effort": "low" },
        "tools": responses_tools(),
        "input": "Weather in Paris? [mock:fixture=thinking-interleaved-tools]",
    });
    if let Some(store) = store {
        body["store"] = json!(store);
    }
    body
}

// The step an SDK that leaves `store` on sends after a tool call: the reply's reasoning as an
// `item_reference`, the call and its output.
fn responses_second_turn(reasoning: &str, call_id: &str) -> Value {
    json!({
        "model": "claude-haiku-4-5",
        "reasoning": { "effort": "low" },
        "tools": responses_tools(),
        "input": [
            { "role": "user", "content": "Weather in Paris?" },
            { "type": "item_reference", "id": reasoning },
            { "type": "function_call", "call_id": call_id, "name": "get_weather", "arguments": "{\"city\":\"Paris\"}" },
            { "type": "function_call_output", "call_id": call_id, "output": "18 °C, sunny" },
        ],
    })
}

fn responses(env: &Env, key: &str, body: Value) -> impl Future<Output = Answer> {
    let key = key.to_owned();
    async move { call(env, "/v1/responses", Some(&key), false, Some(body)).await }
}

fn reasoning_and_call(r: &Answer) -> (String, String) {
    let output = r.json()["output"].as_array().unwrap().clone();
    let id = |ty: &str, field: &str| {
        output
            .iter()
            .find(|i| i["type"] == ty)
            .unwrap_or_else(|| panic!("no {ty} in {output:?}"))[field]
            .as_str()
            .unwrap()
            .to_owned()
    };
    (id("reasoning", "id"), id("function_call", "call_id"))
}

#[tokio::test]
async fn a_responses_app_that_references_its_reasoning_keeps_claudes_thinking() {
    let env = env(&[]).await;
    let first = responses(&env, &env.key, responses_first_turn(None)).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let (reasoning, call_id) = reasoning_and_call(&first);
    assert!(reasoning.starts_with("rs_"), "{reasoning}");

    let second = responses(&env, &env.key, responses_second_turn(&reasoning, &call_id)).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert_eq!(second.header("kaeso-left-out"), None);
    let turn = sent_assistant_turn(&env);
    let blocks = turn["content"].as_array().unwrap();
    assert_eq!(blocks[0]["type"], "thinking", "{turn}");
    assert!(
        blocks[0]["signature"]
            .as_str()
            .unwrap()
            .starts_with("EqEvMLy0mgL4Jn097PRt"),
        "{turn}"
    );
    assert_eq!(blocks.last().unwrap()["type"], "tool_use");
    assert_eq!(blocks.last().unwrap()["id"], call_id.as_str());

    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = env.mock.received().len();
    let r = responses(&env, &other, responses_second_turn(&reasoning, &call_id)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!(
            "Kaeso no longer keeps the reasoning `{reasoning}` points to; send the items themselves in `input`, as SDKs do with `store: false`."
        )
    );
    assert_eq!(r.json()["error"]["param"], "input");
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn a_streamed_responses_turn_keeps_its_reasoning_for_the_next_step() {
    let env = env(&[]).await;
    // The provider's stream stays open, its end unread, until the next step is answered, so the
    // reasoning must be kept before the upstream body closes.
    let gate =
        common::v1::hold_back_the_end(&env, "anthropic_messages", "thinking-interleaved-tools");
    let mut body = responses_first_turn(None);
    body["stream"] = json!(true);
    body["input"] = json!(format!(
        "{} {}",
        body["input"].as_str().unwrap(),
        gate.steps_marker()
    ));
    let (_, mut stream) = common::v1::open_stream(&env, "/v1/responses", false, &body).await;
    let completed = completed_event(&mut stream).await;
    let output = completed["output"].as_array().unwrap();
    let reasoning = output.iter().find(|i| i["type"] == "reasoning").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let call_id = output
        .iter()
        .find(|i| i["type"] == "function_call")
        .unwrap()["call_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second = responses(&env, &env.key, responses_second_turn(&reasoning, &call_id)).await;
    gate.finish();
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    let turn = sent_assistant_turn(&env);
    assert_eq!(turn["content"][0]["type"], "thinking", "{turn}");
    drop(stream);
}

async fn completed_event(stream: &mut axum::body::Body) -> Value {
    use http_body_util::BodyExt;
    let mut seen = String::new();
    loop {
        let frame = stream.frame().await.unwrap().unwrap();
        if let Ok(data) = frame.into_data() {
            seen.push_str(&String::from_utf8_lossy(&data));
        }
        let found = seen.split("\n\n").find_map(|frame| {
            let data = frame.lines().find_map(|l| l.strip_prefix("data: "))?;
            let event: Value = serde_json::from_str(data).ok()?;
            (event["type"] == "response.completed").then(|| event["response"].clone())
        });
        if let Some(response) = found {
            return response;
        }
    }
}

#[tokio::test]
async fn reasoning_is_kept_when_kaeso_itself_tells_the_provider_not_to_store() {
    // The project keeps no responses at OpenAI, so Kaeso sends `store: false`; the app left
    // `store` on, and its next step will point to the reasoning by id.
    let env = env(&[]).await;
    let r = responses(
        &env,
        &env.key,
        json!({
            "model": "gpt-4o",
            "tools": responses_tools(),
            "input": "Weather in Paris and Tokyo? [mock:fixture=tools-parallel]",
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().json()["store"], false);
    let mut conn = env.app.state.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("reasoning:*"))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(!keys.is_empty(), "the reasoning before the calls is kept");

    // The app's next step points to that reasoning, which OpenAI, told not to store, couldn't
    // look up: Kaeso resolves the reference itself.
    let output = r.json()["output"].as_array().unwrap().clone();
    let reasoning = output.iter().find(|i| i["type"] == "reasoning").unwrap()["id"].clone();
    let mut input = vec![
        json!({ "role": "user", "content": "Weather in Paris and Tokyo?" }),
        json!({ "type": "item_reference", "id": reasoning }),
    ];
    let calls: Vec<&Value> = output
        .iter()
        .filter(|i| i["type"] == "function_call")
        .collect();
    assert_eq!(calls.len(), 2);
    for c in &calls {
        input.push(json!({ "type": "function_call", "call_id": c["call_id"], "name": c["name"], "arguments": c["arguments"] }));
    }
    for c in &calls {
        input.push(
            json!({ "type": "function_call_output", "call_id": c["call_id"], "output": "18 °C" }),
        );
    }
    let step = json!({ "model": "gpt-4o", "tools": responses_tools(), "input": input });
    let second = responses(&env, &env.key, step.clone()).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    let sent = env.mock.last().unwrap().json();
    let types: Vec<&str> = sent["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["type"].as_str())
        .collect();
    // This reasoning has no `encrypted_content`, which OpenAI needs back under `store: false`,
    // so the resolved reference sends nothing in its place.
    assert!(!types.contains(&"item_reference"), "{sent}");
    assert!(!types.contains(&"reasoning"), "{sent}");
    assert_eq!(
        types.iter().filter(|t| **t == "function_call").count(),
        2,
        "{sent}"
    );
    // The reference was resolved from what Kaeso kept: a key that holds nothing is refused.
    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = responses(&env, &other, step).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
}

#[tokio::test]
async fn a_responses_reference_in_a_turn_without_tool_calls_is_left_out() {
    let env = env(&[]).await;
    let r = responses(
        &env,
        &env.key,
        json!({
            "model": "claude-haiku-4-5",
            "input": [
                { "role": "user", "content": "Hi" },
                { "type": "item_reference", "id": "rs_kaeso_earlier_0" },
                { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Hello." }] },
                { "role": "user", "content": "Tell me more." },
            ],
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-left-out"), Some("item_reference"));
    let sent = env.mock.last().unwrap().json();
    assert_eq!(sent["messages"].as_array().unwrap().len(), 3, "{sent}");
}

#[tokio::test]
async fn a_responses_app_that_sends_store_false_gets_no_reasoning_kept() {
    let env = env(&[]).await;
    let first = responses(&env, &env.key, responses_first_turn(Some(false))).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let (reasoning, call_id) = reasoning_and_call(&first);
    let mut conn = env.app.state.valkey.conn();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("reasoning:*"))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(keys.is_empty(), "{keys:?}");
    let r = responses(&env, &env.key, responses_second_turn(&reasoning, &call_id)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
}

// A model that writes text before its tool call, as Claude without thinking does.
fn responses_text_turn(stream: bool, store: Option<bool>) -> Value {
    let mut body = json!({
        "model": "claude-haiku-4-5",
        "stream": stream,
        "tools": responses_tools(),
        "input": "Weather in Paris? [mock:fixture=tools-single]",
    });
    if let Some(store) = store {
        body["store"] = json!(store);
    }
    body
}

// The SDK's next step: the reply's message as an `item_reference`, the call and its output.
fn responses_text_reference(message: &str, call_id: &str) -> Value {
    json!({
        "model": "claude-haiku-4-5",
        "tools": responses_tools(),
        "input": [
            { "role": "user", "content": "Weather in Paris?" },
            { "type": "item_reference", "id": message },
            { "type": "function_call", "call_id": call_id, "name": "get_weather", "arguments": "{\"city\":\"Paris\"}" },
            { "type": "function_call_output", "call_id": call_id, "output": "18 °C, sunny" },
        ],
    })
}

fn message_and_call(output: &[Value]) -> (String, String) {
    let id = |ty: &str, field: &str| {
        output
            .iter()
            .find(|i| i["type"] == ty)
            .unwrap_or_else(|| panic!("no {ty} in {output:?}"))[field]
            .as_str()
            .unwrap()
            .to_owned()
    };
    (id("message", "id"), id("function_call", "call_id"))
}

async fn kept_keys(env: &Env) -> Vec<String> {
    let mut conn = env.app.state.valkey.conn();
    redis::cmd("KEYS")
        .arg(env.app.state.valkey.key("reasoning:*"))
        .query_async(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_responses_app_that_references_the_text_before_its_tool_call_gets_it_back() {
    let env = env(&[]).await;
    common::v1::stored_responses(&env, true).await;
    let first = responses(&env, &env.key, responses_text_turn(false, None)).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    let (message, call_id) = message_and_call(first.json()["output"].as_array().unwrap());
    assert!(message.starts_with("msg_"), "{message}");

    let second = responses(&env, &env.key, responses_text_reference(&message, &call_id)).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    assert_eq!(second.header("kaeso-left-out"), None);
    let turn = sent_assistant_turn(&env);
    assert_eq!(
        turn["content"],
        json!([
            { "type": "text", "text": "I'll check the current weather in San Francisco for you." },
            { "type": "tool_use", "id": call_id, "name": "get_weather", "input": { "city": "Paris" } },
        ]),
        "{turn}"
    );

    let other = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/keys", env.owner.org_id, env.project),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = env.mock.received().len();
    let r = responses(&env, &other, responses_text_reference(&message, &call_id)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        format!(
            "Kaeso no longer keeps the text `{message}` points to; send the items themselves in `input`, as SDKs do with `store: false`."
        )
    );
    assert_eq!(r.json()["error"]["param"], "input");
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn a_streamed_responses_turn_keeps_its_text_for_the_next_step() {
    let env = env(&[]).await;
    common::v1::stored_responses(&env, true).await;
    // The provider's stream stays open, its end unread, until the next step is answered, so the
    // text must be kept before the upstream body closes.
    let gate = common::v1::hold_back_the_end(&env, "anthropic_messages", "tools-single");
    let mut first = responses_text_turn(true, None);
    first["input"] = json!(format!(
        "{} {}",
        first["input"].as_str().unwrap(),
        gate.steps_marker()
    ));
    let (_, mut stream) = common::v1::open_stream(&env, "/v1/responses", false, &first).await;
    let completed = completed_event(&mut stream).await;
    let (message, call_id) = message_and_call(completed["output"].as_array().unwrap());
    let second = responses(&env, &env.key, responses_text_reference(&message, &call_id)).await;
    gate.finish();
    assert_eq!(second.status, StatusCode::OK, "{}", second.text());
    let turn = sent_assistant_turn(&env);
    assert_eq!(
        turn["content"][0],
        json!({ "type": "text", "text": "I'll check the current weather in San Francisco for you." }),
        "{turn}"
    );
}

#[tokio::test]
async fn a_reference_to_a_message_outside_a_tool_turn_is_refused() {
    let env = env(&[]).await;
    let before = env.mock.received().len();
    let r = responses(
        &env,
        &env.key,
        json!({
            "model": "claude-haiku-4-5",
            "input": [
                { "role": "user", "content": "Hi" },
                { "type": "item_reference", "id": "msg_kaeso_earlier_0" },
                { "role": "user", "content": "Tell me more." },
            ],
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(
        r.json()["error"]["message"],
        "Kaeso keeps only the text before a tool call, not the message `msg_kaeso_earlier_0` points to; send the items themselves in `input`, as SDKs do with `store: false`."
    );
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
}

#[tokio::test]
async fn text_is_kept_only_for_responses_apps_that_leave_store_on() {
    let env = env(&[]).await;
    common::v1::stored_responses(&env, true).await;
    let r = responses(&env, &env.key, responses_text_turn(false, Some(false))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = chat(
        &env,
        &env.key,
        json!({
            "model": "claude-haiku-4-5",
            "tools": tools(),
            "messages": [{ "role": "user", "content": "Weather in Paris? [mock:fixture=tools-single]" }],
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.json()["choices"][0]["message"]["content"]
            .as_str()
            .is_some_and(|t| !t.is_empty()),
        "{}",
        r.text()
    );
    let keys = kept_keys(&env).await;
    assert!(keys.is_empty(), "{keys:?}");

    let r = responses(&env, &env.key, responses_text_turn(false, None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(kept_keys(&env).await.len(), 1);
}

#[tokio::test]
async fn a_project_without_stored_responses_keeps_no_text() {
    let env = env(&[]).await;
    let refused = |r: &Answer, message: &str| {
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
        assert_eq!(
            r.json()["error"]["message"],
            format!(
                "Kaeso keeps the text `{message}` points to only where the project allows responses stored at the provider; send the items themselves in `input`, as SDKs do with `store: false`."
            )
        );
        assert_eq!(r.json()["error"]["param"], "input");
    };
    let first = responses(&env, &env.key, responses_text_turn(false, None)).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text());
    assert!(
        kept_keys(&env).await.is_empty(),
        "the turn's text isn't kept"
    );
    let (message, call_id) = message_and_call(first.json()["output"].as_array().unwrap());
    let before = env.mock.received().len();
    let r = responses(&env, &env.key, responses_text_reference(&message, &call_id)).await;
    refused(&r, &message);
    assert_eq!(env.mock.received().len(), before, "nothing was sent");

    common::v1::stored_responses(&env, true).await;
    let kept = responses(&env, &env.key, responses_text_turn(false, None)).await;
    let (message, call_id) = message_and_call(kept.json()["output"].as_array().unwrap());
    assert_eq!(kept_keys(&env).await.len(), 1);
    common::v1::stored_responses(&env, false).await;
    let r = responses(&env, &env.key, responses_text_reference(&message, &call_id)).await;
    refused(&r, &message);
}
