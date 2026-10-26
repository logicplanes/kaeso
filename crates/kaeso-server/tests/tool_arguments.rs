mod common;

use axum::http::StatusCode;
use common::v1::{Env, call};
use serde_json::{Value, json};

const CLAUDE: &str = "claude-haiku-4-5";

fn codex_thread() -> Option<Value> {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kaeso-codecs/fixtures/openai_responses/requests/codex-desktop-blank-arguments.json"
    ))
    .ok()?;
    let mut body: Value = serde_json::from_str(&fixture).ok()?;
    let fields = body.as_object_mut()?;
    fields.insert("model".to_owned(), json!(CLAUDE));
    fields.insert("stream".to_owned(), json!(false));
    Some(body)
}

fn sent_tool_uses(env: &Env) -> Vec<Value> {
    let sent = env.mock.last().map(|r| r.json()).unwrap_or_default();
    sent.get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| m.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(|b| json!({"name": b.get("name"), "input": b.get("input")}))
        .collect()
}

fn sse_data(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str(d).ok())
        .collect()
}

#[tokio::test]
async fn codex_gets_claudes_calls_without_arguments_as_an_empty_object_and_can_send_them_back() {
    let env = common::v1::env(None, None).await;

    let mut first = codex_thread().unwrap();
    first["stream"] = json!(true);
    first["input"].as_array_mut().unwrap().truncate(4);
    first["input"][3]["content"][0]["text"] =
        json!("What is my goal? [mock:fixture=tools-no-input]");
    let r = call(&env, "/v1/responses", Some(&env.key), false, Some(first)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let calls: Vec<Value> = sse_data(&r.text())
        .into_iter()
        .filter(|e| {
            e["type"] == "response.output_item.done" && e["item"]["type"] == "function_call"
        })
        .map(|e| json!({"name": e["item"]["name"], "arguments": e["item"]["arguments"]}))
        .collect();
    assert_eq!(calls, [json!({"name": "get_goal", "arguments": "{}"})]);

    let r = call(
        &env,
        "/v1/responses",
        Some(&env.key),
        false,
        Some(codex_thread().unwrap()),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("kaeso-adapted"), None);
    assert_eq!(
        sent_tool_uses(&env)[1],
        json!({"name": "get_goal", "input": {}})
    );
}

#[tokio::test]
async fn a_cut_off_call_in_the_history_reaches_claude_as_an_object_and_is_named() {
    let env = common::v1::env(None, None).await;
    let mut body = codex_thread().unwrap();
    body["input"][5]["arguments"] = json!("{\"cmd\": \"cat /home/user/");
    let r = call(&env, "/v1/responses", Some(&env.key), false, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.header("kaeso-adapted"),
        Some("tool call toolu_01GtrXhD5GoEnUhCCW1fRitv")
    );
    assert_eq!(
        sent_tool_uses(&env)[0],
        json!({"name": "exec_command", "input": {"input": "{\"cmd\": \"cat /home/user/"}})
    );
}
