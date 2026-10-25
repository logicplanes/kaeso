#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

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
