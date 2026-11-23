#![allow(clippy::unwrap_used)]

use crate::common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::v1::{Answer, Capture, Env, call};
use serde_json::{Value, json};

async fn env() -> Env {
    let env = common::v1::env(Some(Arc::new(Capture::default())), None).await;
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    for (provider, model) in [
        ("anthropic", "claude-opus-4-8"),
        ("anthropic", "claude-sonnet-5-5"),
        ("openai", "gpt-5.5"),
    ] {
        let r = env
            .app
            .post(
                &format!("{base}/models"),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    }
    env
}

fn tools() -> Value {
    json!([{ "type": "function", "name": "get_weather", "description": "Weather in a city",
        "parameters": { "type": "object", "properties": { "city": { "type": "string" } }, "required": ["city"] } }])
}

async fn responses(env: &Env, body: Value) -> Answer {
    let r = call(env, "/v1/responses", Some(&env.key), false, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r
}

fn events(r: &Answer) -> Vec<Value> {
    r.text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<Value>(d).ok())
        .collect()
}

fn sent(env: &Env) -> Value {
    env.mock.last().unwrap().json()
}

fn fixture(name: &str) -> Value {
    let path = kaeso_testkit::mock_provider::kpp_fixtures()
        .join("anthropic_messages/responses")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[tokio::test]
async fn a_responses_app_on_claude_streams_thinking_and_carries_it_through_a_tool_loop() {
    let env = env().await;
    let first = responses(
        &env,
        json!({
            "model": "claude-opus-4-8",
            "stream": true,
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "reasoning": { "effort": "medium", "summary": "auto" },
            "tools": tools(),
            "input": "Can I see the aurora tonight? [mock:fixture=thinking-redacted]",
        }),
    )
    .await;
    let upstream = sent(&env);
    assert_eq!(upstream["model"], "claude-opus-4-8");
    assert_eq!(
        upstream["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(upstream["output_config"]["effort"], "medium");

    let evs = events(&first);
    let types: Vec<&str> = evs.iter().map(|e| e["type"].as_str().unwrap()).collect();
    let at = |t: &str| types.iter().position(|x| *x == t).unwrap();
    assert!(at("response.output_item.added") < at("response.reasoning_summary_part.added"));
    assert!(
        at("response.reasoning_summary_part.added") < at("response.reasoning_summary_text.delta")
    );
    assert!(
        at("response.reasoning_summary_text.delta") < at("response.reasoning_summary_text.done")
    );
    assert!(
        at("response.reasoning_summary_text.done") < at("response.reasoning_summary_part.done")
    );
    let summary: String = evs
        .iter()
        .filter(|e| e["type"] == "response.reasoning_summary_text.delta")
        .map(|e| e["delta"].as_str().unwrap())
        .collect();
    let original = fixture("thinking-redacted.json");
    assert_eq!(
        summary,
        original["content"][0]["thinking"].as_str().unwrap()
    );
    for (i, e) in evs.iter().enumerate() {
        assert_eq!(e["sequence_number"], i, "{e}");
    }
    let output = evs.last().unwrap()["response"]["output"].clone();
    let kinds: Vec<&str> = output
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["reasoning", "reasoning", "function_call"]);

    let mut input = vec![json!({ "role": "user", "content": "Can I see the aurora tonight?" })];
    for item in output.as_array().unwrap() {
        input.push(item.clone());
        if item["type"] == "function_call" {
            input.push(json!({ "type": "function_call_output", "call_id": item["call_id"], "output": "{\"cloud_cover\":0.1}" }));
        }
    }
    responses(
        &env,
        json!({
            "model": "claude-opus-4-8",
            "stream": true,
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "reasoning": { "effort": "medium", "summary": "auto" },
            "tools": tools(),
            "input": input,
        }),
    )
    .await;
    let upstream = sent(&env);
    let turn = &upstream["messages"][1];
    let blocks = turn["content"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["thinking", "redacted_thinking", "tool_use"],
        "{turn}"
    );
    assert_eq!(blocks[0]["signature"], original["content"][0]["signature"]);
    assert_eq!(blocks[0]["thinking"], original["content"][0]["thinking"]);
    assert_eq!(blocks[1]["data"], original["content"][1]["data"]);
    assert_eq!(blocks[2]["id"], original["content"][2]["id"]);
    assert_eq!(upstream["messages"][2]["content"][0]["type"], "tool_result");
}

#[tokio::test]
async fn the_catalogue_decides_how_claude_thinks_for_a_responses_app() {
    let env = env().await;
    for (model, thinking) in [
        (
            "claude-opus-4-8",
            json!({ "type": "adaptive", "display": "summarized" }),
        ),
        (
            "claude-haiku-4-5",
            json!({ "type": "enabled", "budget_tokens": 8192, "display": "summarized" }),
        ),
    ] {
        responses(
            &env,
            json!({ "model": model, "reasoning": { "summary": "auto" }, "temperature": 0.3, "top_p": 0.9,
                "input": "Is 1071 divisible by 7? [mock:fixture=thinking-enabled-budget]" }),
        )
        .await;
        let upstream = sent(&env);
        assert_eq!(upstream["thinking"], thinking, "{model}: {upstream}");
        assert!(upstream.get("temperature").is_none(), "{model}: {upstream}");
        assert!(upstream.get("top_p").is_none(), "{model}: {upstream}");
        let r = responses(
            &env,
            json!({ "model": model, "reasoning": { "summary": "auto" },
                "input": "Is 1071 divisible by 7? [mock:fixture=thinking-enabled-budget]" }),
        )
        .await;
        let reasoning = r.json()["output"][0].clone();
        assert_eq!(reasoning["type"], "reasoning");
        assert!(!reasoning["summary"][0]["text"].as_str().unwrap().is_empty());
        assert_eq!(
            r.json()["usage"]["output_tokens_details"]["reasoning_tokens"],
            fixture("thinking-enabled-budget.json")["usage"]["output_tokens_details"]["thinking_tokens"]
        );
    }
    responses(
        &env,
        json!({ "model": "claude-haiku-4-5", "reasoning": { "effort": "xhigh", "summary": "auto" },
            "tools": tools(), "tool_choice": "required",
            "input": "Weather in Oslo? [mock:fixture=tools-single]" }),
    )
    .await;
    let upstream = sent(&env);
    assert!(upstream.get("thinking").is_none(), "{upstream}");
    assert_eq!(upstream["tool_choice"]["type"], "any");
    responses(
        &env,
        json!({ "model": "claude-opus-4-8", "input": "Hi [mock:fixture=text-simple]" }),
    )
    .await;
    assert!(sent(&env).get("thinking").is_none());
}

#[tokio::test]
async fn an_anthropic_app_on_an_openai_reasoning_model_gets_readable_thinking() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(json!({
            "model": "gpt-5.5",
            "max_tokens": 4000,
            "thinking": { "type": "adaptive" },
            "messages": [{ "role": "user", "content": "What is 17 * 24? [mock:fixture=reasoning-summary-stream]" }],
            "stream": true,
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let upstream = sent(&env);
    assert_eq!(upstream["reasoning"], json!({ "summary": "auto" }));
    assert_eq!(upstream["include"], json!(["reasoning.encrypted_content"]));
    let thinking: String = events(&r)
        .iter()
        .filter(|e| e["delta"]["type"] == "thinking_delta")
        .map(|e| e["delta"]["thinking"].as_str().unwrap().to_owned())
        .collect();
    assert!(thinking.contains("Multiplying 17 by 24"), "{thinking}");
    assert!(events(&r).iter().any(|e| {
        e["delta"]["type"] == "signature_delta"
            && e["delta"]["signature"]
                .as_str()
                .unwrap()
                .starts_with("kpp:reasoning:v1:")
    }));
}

fn chat_tools() -> Value {
    json!([{
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Weather in a city",
            "parameters": {
                "type": "object",
                "properties": { "location": { "type": "string" } },
                "required": ["location"]
            }
        }
    }])
}

async fn chat(env: &Env, body: Value) -> Answer {
    let r = call(
        env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r
}

#[tokio::test]
async fn an_openrouter_chat_app_on_claude_streams_reasoning_carries_it_through_tools_and_hides_it_when_excluded()
 {
    let env = env().await;
    let original = fixture("thinking-redacted.json");
    let expected_thinking = original["content"][0]["thinking"].as_str().unwrap();
    let expected_call_id = original["content"][2]["id"].as_str().unwrap();

    let first = chat(
        &env,
        json!({
            "model": "claude-sonnet-5-5",
            "stream": true,
            "stream_options": { "include_usage": true },
            "reasoning": { "effort": "max" },
            "tools": chat_tools(),
            "messages": [
                { "role": "system", "content": "Use tools when needed." },
                { "role": "user", "content": "Can I see the aurora tonight? [mock:fixture=thinking-redacted]" }
            ]
        }),
    )
    .await;
    let upstream = sent(&env);
    assert_eq!(upstream["model"], "claude-sonnet-5-5");
    assert_eq!(
        upstream["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(upstream["output_config"]["effort"], "max");
    assert_eq!(upstream["stream"], true);

    let chunks = events(&first);
    let reasoning: String = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["reasoning"].as_str())
        .collect();
    assert_eq!(reasoning, expected_thinking, "{chunks:?}");
    let tool_name = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["tool_calls"][0]["function"]["name"].as_str())
        .next();
    assert_eq!(tool_name, Some("get_weather"), "{chunks:?}");
    let usage = chunks
        .iter()
        .find_map(|c| c.get("usage").filter(|u| !u.is_null()))
        .unwrap();
    assert_eq!(
        usage["completion_tokens_details"]["reasoning_tokens"],
        original["usage"]["output_tokens_details"]["thinking_tokens"]
    );

    chat(
        &env,
        json!({
            "model": "claude-sonnet-5-5",
            "stream": true,
            "reasoning": { "effort": "max" },
            "tools": chat_tools(),
            "messages": [
                { "role": "system", "content": "Use tools when needed." },
                { "role": "user", "content": "Can I see the aurora tonight?" },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": expected_call_id,
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"Reykjavik, Iceland\",\"unit\":\"celsius\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": expected_call_id,
                    "content": "{\"cloud_cover\":0.1}"
                }
            ]
        }),
    )
    .await;
    let upstream = sent(&env);
    let blocks = upstream["messages"][1]["content"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["thinking", "redacted_thinking", "tool_use"]);
    assert_eq!(blocks[0]["signature"], original["content"][0]["signature"]);
    assert_eq!(blocks[1]["data"], original["content"][1]["data"]);

    let excluded_stream = chat(
        &env,
        json!({
            "model": "claude-sonnet-5-5",
            "stream": true,
            "stream_options": { "include_usage": true },
            "reasoning": { "effort": "max", "exclude": true },
            "tools": chat_tools(),
            "messages": [
                { "role": "user", "content": "Can I see the aurora tonight? [mock:fixture=thinking-redacted]" }
            ]
        }),
    )
    .await;
    let upstream = sent(&env);
    assert_eq!(upstream["thinking"], json!({ "type": "adaptive" }));
    assert_eq!(upstream["output_config"]["effort"], "max");
    let excl_chunks = events(&excluded_stream);
    assert!(
        excl_chunks
            .iter()
            .all(|c| c["choices"][0]["delta"].get("reasoning").is_none()),
        "{excl_chunks:?}"
    );
    assert!(excl_chunks.iter().any(|c| {
        c["choices"][0]["delta"]["tool_calls"][0]["function"]["name"] == "get_weather"
    }));
    let excl_usage = excl_chunks
        .iter()
        .find_map(|c| c.get("usage").filter(|u| !u.is_null()))
        .unwrap();
    assert_eq!(
        excl_usage["completion_tokens_details"]["reasoning_tokens"],
        original["usage"]["output_tokens_details"]["thinking_tokens"]
    );

    let sync_inc = chat(
        &env,
        json!({
            "model": "claude-sonnet-5-5",
            "reasoning": { "effort": "max" },
            "tools": chat_tools(),
            "messages": [
                { "role": "user", "content": "Can I see the aurora tonight? [mock:fixture=thinking-redacted]" }
            ]
        }),
    )
    .await
    .json();
    assert_eq!(
        sync_inc["choices"][0]["message"]["reasoning"],
        expected_thinking
    );
    assert_eq!(
        sync_inc["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "get_weather"
    );

    let sync_excl = chat(
        &env,
        json!({
            "model": "claude-sonnet-5-5",
            "reasoning": { "effort": "max", "exclude": true },
            "tools": chat_tools(),
            "messages": [
                { "role": "user", "content": "Can I see the aurora tonight? [mock:fixture=thinking-redacted]" }
            ]
        }),
    )
    .await
    .json();
    assert!(
        sync_excl["choices"][0]["message"]
            .get("reasoning")
            .is_none()
    );
    assert_eq!(
        sync_excl["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "get_weather"
    );
    assert_eq!(
        sync_excl["usage"]["completion_tokens_details"]["reasoning_tokens"],
        original["usage"]["output_tokens_details"]["thinking_tokens"]
    );
}

#[tokio::test]
async fn an_openrouter_chat_app_on_openai_responses_streams_reasoning_and_hides_it_when_excluded() {
    let env = env().await;
    let inc = chat(
        &env,
        json!({
            "model": "gpt-5.5",
            "stream": true,
            "reasoning": { "effort": "high" },
            "messages": [
                { "role": "user", "content": "What is 17 * 24? [mock:fixture=reasoning-summary-stream]" }
            ]
        }),
    )
    .await;
    let upstream = sent(&env);
    assert_eq!(
        upstream["reasoning"],
        json!({ "effort": "high", "summary": "auto" })
    );
    assert_eq!(upstream["include"], json!(["reasoning.encrypted_content"]));
    let reasoning: String = events(&inc)
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["reasoning"].as_str())
        .collect();
    assert!(reasoning.contains("Multiplying 17 by 24"), "{reasoning}");

    let excl = chat(
        &env,
        json!({
            "model": "gpt-5.5",
            "stream": true,
            "reasoning": { "effort": "high", "exclude": true },
            "messages": [
                { "role": "user", "content": "What is 17 * 24? [mock:fixture=reasoning-summary-stream]" }
            ]
        }),
    )
    .await;
    let upstream = sent(&env);
    assert_eq!(upstream["reasoning"], json!({ "effort": "high" }));
    assert!(
        events(&excl)
            .iter()
            .all(|c| c["choices"][0]["delta"].get("reasoning").is_none())
    );
}
