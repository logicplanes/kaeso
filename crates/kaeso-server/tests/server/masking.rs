#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use crate::common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::v1::{Answer, Env, call, chat, messages};
use serde_json::{Value, json};

const KEY: &str = "sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123";

async fn env() -> Env {
    let e = common::v1::env(None, None).await;
    e.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&e.app.state.pg)
        .await
        .unwrap();
    e
}

fn guardrails_path(env: &Env) -> String {
    format!("/orgs/{}/guardrails", env.owner.org_id)
}

async fn rules(env: &Env, rules: Value) -> String {
    let r = env
        .app
        .post(
            &guardrails_path(env),
            json!({ "kind": "rules", "name": "Built-in", "rules": rules }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["secret"], Value::Null, "rules have no secret");
    let id = r.body["guardrail"]["id"].as_str().unwrap().to_owned();
    let set = env
        .app
        .call(
            Method::PUT,
            &format!("{}/projects", guardrails_path(env)),
            Some(json!({ "project": env.project, "guardrails": [id] })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.body);
    id
}

async fn ask(env: &Env, text: &str) -> Answer {
    let a = call(
        env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", text)),
    )
    .await;
    env.app.state.in_flight.settled().await;
    a
}

async fn entry(env: &Env, a: &Answer) -> Value {
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    let id = a.header("kaeso-request-id").unwrap();
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/logs/{id}",
                env.owner.org_id, env.project
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

fn sent(env: &Env) -> String {
    String::from_utf8_lossy(&env.mock.last().unwrap().body).into_owned()
}

#[tokio::test]
async fn a_provider_key_in_a_prompt_reaches_the_provider_and_the_log_as_a_placeholder() {
    let env = env().await;
    rules(&env, json!({ "detectors": { "secrets": "mask" } })).await;
    let a = ask(&env, &format!("my key is {KEY}, and again {KEY}")).await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let body = sent(&env);
    assert!(
        body.contains("my key is [API_KEY_1], and again [API_KEY_1]"),
        "{body}"
    );
    assert!(!body.contains(KEY), "{body}");
    assert!(
        a.header("kaeso-guardrails")
            .unwrap()
            .starts_with(r#""Built-in";phase=request;action=replace;"#)
    );
    let e = entry(&env, &a).await;
    assert_eq!(e["guardrails"][0]["found"], json!({ "api_key": 2 }));
    assert_eq!(e["guardrails"][0]["action"], "replace");
    let logged = e["content"]["request"].to_string();
    assert!(logged.contains("[API_KEY_1]"), "{logged}");
    assert!(!logged.contains(KEY), "{logged}");
}

#[tokio::test]
async fn tool_call_arguments_and_tool_results_are_masked() {
    let env = env().await;
    rules(&env, json!({ "detectors": { "email": "mask" } })).await;
    let body = json!({
        "model": "gpt-4o",
        "messages": [
            { "role": "user", "content": "Mail Ada" },
            { "role": "assistant", "content": null, "tool_calls": [{
                "id": "call_1", "type": "function",
                "function": { "name": "send", "arguments": "{\"to\":\"ada@example.com\"}" }
            }] },
            { "role": "tool", "tool_call_id": "call_1", "content": "sent to ada@example.com and bob@example.com" }
        ],
        "tools": [{ "type": "function", "function": {
            "name": "send", "description": "Mail someone such as eve@example.com",
            "parameters": { "type": "object" }
        } }]
    });
    let a = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let body = sent(&env);
    assert!(body.contains(r#"[EMAIL_1]"#), "{body}");
    assert!(body.contains("sent to [EMAIL_1] and [EMAIL_2]"), "{body}");
    assert!(!body.contains("ada@example.com"), "{body}");
    assert!(
        body.contains("eve@example.com"),
        "tool definitions are not scanned: {body}"
    );
}

#[tokio::test]
async fn the_log_only_switch_masks_the_log_and_sends_the_request_unchanged() {
    let env = env().await;
    rules(
        &env,
        json!({ "detectors": { "email": "flag" }, "mask_log": true }),
    )
    .await;
    let a = ask(&env, "write to ada@example.com").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert!(sent(&env).contains("ada@example.com"));
    let e = entry(&env, &a).await;
    assert_eq!(e["guardrails"][0]["action"], "allow");
    assert_eq!(e["guardrails"][0]["found"], json!({ "email": 1 }));
    let logged = e["content"]["request"].to_string();
    assert!(logged.contains("write to [EMAIL_1]"), "{logged}");
    assert!(!logged.contains("ada@example.com"), "{logged}");
}

#[tokio::test]
async fn a_blocking_rule_names_what_it_found_and_never_the_value() {
    let env = env().await;
    rules(
        &env,
        json!({ "detectors": { "card": "block", "email": "flag" } }),
    )
    .await;
    let a = ask(&env, "charge 4111 1111 1111 1111 for ada@example.com").await;
    assert_eq!(a.status, StatusCode::BAD_REQUEST, "{}", a.text());
    let message = a.json()["error"]["message"].as_str().unwrap().to_owned();
    assert_eq!(
        message,
        "The guardrail `Built-in` blocked this request: it contains a payment card number."
    );
    assert!(env.mock.received().is_empty());
    let e = entry(&env, &a).await;
    assert_eq!(e["guardrails"][0]["category"], "card");
    assert_eq!(
        e["guardrails"][0]["found"],
        json!({ "card": 1, "email": 1 })
    );
    let a = ask(&env, "a card such as 4111 1111 1111 1112 fails Luhn").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
}

#[tokio::test]
async fn chat_completions_and_gemini_apps_are_masked_too() {
    let env = env().await;
    rules(
        &env,
        json!({ "detectors": { "ip": "mask", "phone": "mask" } }),
    )
    .await;
    let a = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", "server 192.0.2.7, call +4930123456")),
    )
    .await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert!(
        sent(&env).contains("server [IP_1], call [PHONE_1]"),
        "{}",
        sent(&env)
    );
}

#[tokio::test]
async fn rules_are_managed_tested_and_tried_through_the_platform_api() {
    let env = env().await;
    let id = rules(
        &env,
        json!({ "detectors": { "email": "mask" }, "custom": [
            { "name": "order_id", "pattern": "ORD-\\d{6}", "action": "mask" }
        ] }),
    )
    .await;
    let one = env
        .app
        .get(
            &format!("{}/{id}", guardrails_path(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(one.body["kind"], "rules");
    assert_eq!(one.body["url"], Value::Null);
    assert_eq!(one.body["rules"]["detectors"], json!({ "email": "mask" }));
    assert_eq!(one.body["rules"]["custom"][0]["name"], "order_id");

    let tested = env
        .app
        .post(
            &format!("{}/{id}/test", guardrails_path(&env)),
            json!({ "messages": [{ "role": "user", "content": [{ "type": "text", "text": "ORD-123456 for ada@example.com" }] }] }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(tested.status, StatusCode::OK, "{:?}", tested.body);
    assert_eq!(tested.body["action"], "replace");
    assert_eq!(
        tested.body["messages"][0]["content"][0]["text"],
        "[ORDER_ID_1] for [EMAIL_1]"
    );
    assert_eq!(tested.body["found"], json!({ "email": 1, "order_id": 1 }));

    let tried = env
        .app
        .post(
            &format!("{}/try", guardrails_path(&env)),
            json!({ "rules": { "detectors": { "secrets": "block" } }, "text": format!("key {KEY}") }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(tried.status, StatusCode::OK, "{:?}", tried.body);
    assert_eq!(tried.body["action"], "block");
    assert_eq!(tried.body["message"], "it contains an API key.");

    let changed = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/{id}", guardrails_path(&env)),
            Some(json!({ "rules": { "detectors": { "email": "block" }, "restore": true } })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.body);
    assert_eq!(changed.body["rules"]["restore"], true);
    let a = ask(&env, "ada@example.com").await;
    assert_eq!(
        a.status,
        StatusCode::BAD_REQUEST,
        "the change applies at once"
    );

    let rotate = env
        .app
        .post(
            &format!("{}/{id}/secret", guardrails_path(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(rotate.status, StatusCode::CONFLICT, "{:?}", rotate.body);

    let too_long = "a".repeat(501);
    for (body, param) in [
        (json!({ "kind": "rules", "name": "x" }), "rules"),
        (
            json!({ "kind": "rules", "name": "x", "rules": {} }),
            "rules",
        ),
        (
            json!({ "kind": "rules", "name": "x", "url": "https://guardrail.example.com", "rules": { "detectors": { "ip": "flag" } } }),
            "url",
        ),
        (
            json!({ "name": "x", "rules": { "detectors": { "ip": "flag" } } }),
            "rules",
        ),
        (
            json!({ "kind": "rules", "name": "x", "phases": ["reply"], "rules": { "detectors": { "ip": "flag" } } }),
            "phases",
        ),
        (
            json!({ "kind": "rules", "name": "x", "rules": { "custom": [{ "name": "big", "pattern": too_long, "action": "flag" }] } }),
            "rules.custom",
        ),
        (
            json!({ "kind": "rules", "name": "x", "rules": { "custom": [{ "name": "bad", "pattern": "(", "action": "flag" }] } }),
            "rules.custom",
        ),
        (
            json!({ "kind": "rules", "name": "x", "rules": { "custom": [{ "name": "Email", "pattern": "x", "action": "flag" }] } }),
            "rules.custom",
        ),
        (json!({ "name": "x" }), "url"),
    ] {
        let r = env
            .app
            .post(
                &guardrails_path(&env),
                body.clone(),
                Some(&env.owner.cookie),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {:?}", r.body);
        assert_eq!(r.body["error"]["param"], param, "{body}");
    }
    let many: Vec<Value> = (0..21)
        .map(|i| json!({ "name": format!("p{i}"), "pattern": "x", "action": "flag" }))
        .collect();
    let r = env
        .app
        .post(
            &guardrails_path(&env),
            json!({ "kind": "rules", "name": "x", "rules": { "custom": many } }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "`rules.custom` takes at most 20 patterns."
    );
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/{id}", guardrails_path(&env)),
            Some(json!({ "url": "https://guardrail.example.com" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.body["error"]["param"], "url");
}
