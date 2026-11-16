#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use crate::common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::v1::{Answer, Env, call, chat, messages};
use http_body_util::BodyExt;
use kaeso_testkit::guardrail::MockGuardrail;
use serde_json::{Value, json};
use tower::ServiceExt;

const LOOPBACK: (&str, &str) = ("KAESO_GUARDRAILS_ALLOWED_NETWORKS", "127.0.0.0/8");

async fn env_with(extra: &[(&str, &str)]) -> Env {
    let e = common::v1::env(None, Some(extra)).await;
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

async fn env() -> Env {
    env_with(&[LOOPBACK]).await
}

fn guardrails_path(env: &Env) -> String {
    format!("/orgs/{}/guardrails", env.owner.org_id)
}

fn project_path(env: &Env) -> String {
    format!(
        "/orgs/{}/projects/{}/guardrails",
        env.owner.org_id, env.project
    )
}

async fn add(env: &Env, body: Value) -> Value {
    let r = env
        .app
        .post(&guardrails_path(env), body, Some(&env.owner.cookie))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    r.body
}

fn assignments_path(env: &Env) -> String {
    format!("/orgs/{}/guardrails/projects", env.owner.org_id)
}

async fn attach(env: &Env, ids: &[&str]) -> Value {
    let r = env
        .app
        .call(
            Method::PUT,
            &assignments_path(env),
            Some(json!({ "project": env.project, "guardrails": ids })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body
}

// Adds a guardrail at `url` and puts it in front of the project's requests; returns its id
// and secret.
async fn guard(env: &Env, name: &str, url: &str, extra: Value) -> (String, String) {
    let mut body = json!({ "name": name, "url": url });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let created = add(env, body).await;
    let id = created["guardrail"]["id"].as_str().unwrap().to_owned();
    attach(env, &[&id]).await;
    (id, created["secret"].as_str().unwrap().to_owned())
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Chat,
    Messages,
    Gemini,
    Responses,
}

const FORMATS: [Format; 4] = [
    Format::Chat,
    Format::Messages,
    Format::Gemini,
    Format::Responses,
];

async fn gemini(env: &Env, text: &str) -> Answer {
    let body = json!({ "contents": [{ "role": "user", "parts": [{ "text": text }] }] });
    let req = Request::builder()
        .uri("/v1beta/models/claude-haiku-4-5:generateContent")
        .method("POST")
        .header("x-goog-api-key", &env.key)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

async fn ask(env: &Env, format: Format, text: &str) -> Answer {
    let a = match format {
        Format::Chat => {
            call(
                env,
                "/v1/chat/completions",
                Some(&env.key),
                false,
                Some(chat("gpt-4o", text)),
            )
            .await
        }
        Format::Messages => {
            call(
                env,
                "/v1/messages",
                Some(&env.key),
                true,
                Some(messages("claude-haiku-4-5", text)),
            )
            .await
        }
        Format::Gemini => gemini(env, text).await,
        Format::Responses => {
            call(
                env,
                "/v1/responses",
                Some(&env.key),
                false,
                Some(json!({ "model": "gpt-4o", "input": text })),
            )
            .await
        }
    };
    env.app.state.in_flight.settled().await;
    a
}

fn error_message(format: Format, a: &Answer) -> String {
    let j = a.json();
    match format {
        Format::Messages => {
            assert_eq!(j["type"], "error", "{j}");
            j["error"]["message"].as_str().unwrap().to_owned()
        }
        Format::Gemini => {
            assert_eq!(j["error"]["code"], 400, "{j}");
            j["error"]["message"].as_str().unwrap().to_owned()
        }
        Format::Chat | Format::Responses => j["error"]["message"].as_str().unwrap().to_owned(),
    }
}

async fn logged(env: &Env, query: &str) -> Vec<Value> {
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    let r = env
        .app
        .get(
            &format!(
                "/orgs/{}/projects/{}/logs{query}",
                env.owner.org_id, env.project
            ),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body["data"].as_array().unwrap().clone()
}

#[tokio::test]
async fn a_guardrail_that_blocks_a_word_refuses_requests_in_each_chat_format() {
    let env = env().await;
    let words = MockGuardrail::start().await;
    guard(&env, "Words", &words.url("/check"), json!({})).await;
    for format in FORMATS {
        let a = ask(&env, format, "say something forbidden").await;
        assert_eq!(
            a.status,
            StatusCode::BAD_REQUEST,
            "{format:?}: {}",
            a.text()
        );
        assert_eq!(
            error_message(format, &a),
            "The guardrail `Words` blocked this request: Forbidden words are not allowed.",
            "{format:?}"
        );
        let header = a.header("kaeso-guardrails").unwrap();
        assert!(
            header.starts_with(r#""Words";phase=request;action=block;category="words";dur="#),
            "{header}"
        );
    }
    assert!(env.mock.received().is_empty(), "no provider was asked");
    let blocked = logged(&env, "?guardrail_action=block").await;
    assert_eq!(blocked.len(), 4);
    for entry in &blocked {
        assert_eq!(entry["status"], 400);
        assert_eq!(entry["guardrails"][0]["name"], "Words");
        assert_eq!(entry["guardrails"][0]["phase"], "request");
        assert_eq!(entry["guardrails"][0]["action"], "block");
        assert_eq!(entry["guardrails"][0]["category"], "words");
        assert_eq!(entry["cost"], "0");
    }
    assert!(logged(&env, "?guardrail_action=allow").await.is_empty());
}

#[tokio::test]
async fn an_allowed_request_reaches_the_provider_and_the_guardrail_gets_a_signed_question() {
    let env = env().await;
    let words = MockGuardrail::start().await;
    let (id, secret) = guard(&env, "Words", &words.url("/check"), json!({})).await;
    for format in FORMATS {
        let a = ask(&env, format, "Hello there").await;
        assert_eq!(a.status, StatusCode::OK, "{format:?}: {}", a.text());
        let header = a.header("kaeso-guardrails").unwrap();
        assert!(
            header.starts_with(r#""Words";phase=request;action=allow;dur="#),
            "{header}"
        );
    }
    assert_eq!(env.mock.received().len(), 4);
    let seen = words.seen();
    assert_eq!(seen.len(), 4);
    let key = STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap())
        .unwrap();
    let formats: Vec<&str> = seen
        .iter()
        .map(|s| s.body["format"].as_str().unwrap())
        .collect();
    assert_eq!(
        formats,
        [
            "openai_chat",
            "anthropic_messages",
            "gemini",
            "openai_responses"
        ]
    );
    for s in &seen {
        let ts: i64 = s.timestamp.as_deref().unwrap().parse().unwrap();
        let want = kaeso_server::services::guardrails::signature(
            &key,
            s.id.as_deref().unwrap(),
            ts,
            &s.raw,
        );
        assert_eq!(s.signature.as_deref(), Some(want.as_str()));
        assert_eq!(s.body["phase"], "request");
        assert_eq!(s.body["organization"], env.owner.org_id);
        assert_eq!(s.body["project"], env.project);
        assert!(s.body["key"].as_str().unwrap().starts_with("key_"));
        assert!(s.body["member"].as_str().unwrap().starts_with("usr_"));
        assert_eq!(s.body["service_account"], Value::Null);
        assert_eq!(s.body["source"], "api");
        assert_eq!(
            s.id.as_deref().unwrap(),
            format!("{}.request", s.body["request_id"].as_str().unwrap())
        );
        let last = s.body["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        assert_eq!(last["role"], "user");
        assert_eq!(last["content"][0]["text"], "Hello there");
    }
    assert_eq!(seen[0].body["model"], "gpt-4o");
    let allowed = logged(&env, "?guardrail_action=allow").await;
    assert_eq!(allowed.len(), 4);
    assert_eq!(allowed[0]["guardrails"][0]["guardrail"], id);
}

#[tokio::test]
async fn a_replacement_reaches_the_provider_in_each_chat_format() {
    let env = env().await;
    let words = MockGuardrail::start().await;
    guard(&env, "Secrets", &words.url("/check"), json!({})).await;
    for format in FORMATS {
        let a = ask(&env, format, "my secret is 42").await;
        assert_eq!(a.status, StatusCode::OK, "{format:?}: {}", a.text());
        let sent = String::from_utf8_lossy(&env.mock.last().unwrap().body).into_owned();
        assert!(sent.contains("my [REDACTED] is 42"), "{format:?}: {sent}");
        assert!(!sent.contains("secret"), "{format:?}: {sent}");
        assert!(
            a.header("kaeso-guardrails")
                .unwrap()
                .contains("action=replace;category=\"secrets\""),
            "{format:?}"
        );
    }
    assert_eq!(logged(&env, "?guardrail_action=replace").await.len(), 4);
}

#[tokio::test]
async fn guardrails_run_in_the_order_the_project_sets_and_the_first_block_ends_the_check() {
    let env = env().await;
    let words = MockGuardrail::start().await;
    let first = add(
        &env,
        json!({ "name": "Secrets", "url": words.url("/check") }),
    )
    .await;
    let second = add(
        &env,
        json!({ "name": "Blocker", "url": words.url("/check?block=REDACTED") }),
    )
    .await;
    let ids = [
        first["guardrail"]["id"].as_str().unwrap(),
        second["guardrail"]["id"].as_str().unwrap(),
    ];
    let set = attach(&env, &ids).await;
    let names: Vec<&str> = set["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Secrets", "Blocker"]);
    let a = ask(&env, Format::Chat, "my secret").await;
    assert_eq!(a.status, StatusCode::BAD_REQUEST, "{}", a.text());
    let header = a.header("kaeso-guardrails").unwrap();
    assert!(
        header.contains(r#""Secrets";phase=request;action=replace"#),
        "{header}"
    );
    assert!(
        header.contains(r#""Blocker";phase=request;action=block"#),
        "{header}"
    );
    assert_eq!(
        words.seen()[1].body["messages"][0]["content"][0]["text"],
        "my [REDACTED]",
        "the second guardrail sees the first one's replacement"
    );

    attach(&env, &[ids[1], ids[0]]).await;
    let a = ask(&env, Format::Chat, "my secret").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let listed = env.app.get(&project_path(&env), &env.owner.cookie).await;
    assert_eq!(listed.body["data"][0]["name"], "Blocker");
    assert_eq!(listed.body["data"][0]["phases"], json!(["request"]));
    assert_eq!(listed.body["data"][0].get("url"), None);
}

#[tokio::test]
async fn a_block_never_falls_back_and_never_reaches_the_response_cache() {
    let env = env().await;
    let words = MockGuardrail::start().await;
    guard(&env, "Words", &words.url("/check"), json!({})).await;
    let r = env
        .app
        .call(
            Method::PATCH,
            &format!(
                "/orgs/{}/projects/{}/response-cache",
                env.owner.org_id, env.project
            ),
            Some(json!({ "enabled": true })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let models = env
        .app
        .get(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            &env.owner.cookie,
        )
        .await;
    let id_of = |name: &str| {
        models.body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == name)
            .unwrap()["id"]
            .clone()
    };
    let route = json!({
        "type": "model",
        "model": id_of("claude-haiku-4-5"),
        "fall_back_on": ["unavailable", "rejected", "limited"],
        "fallback": { "type": "model", "model": id_of("gpt-4o") }
    });
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/aliases",
                env.owner.org_id, env.project
            ),
            json!({ "name": "sturdy", "route": route }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for _ in 0..2 {
        let a = call(
            &env,
            "/v1/messages",
            Some(&env.key),
            true,
            Some(messages("sturdy", "forbidden")),
        )
        .await;
        assert_eq!(a.status, StatusCode::BAD_REQUEST, "{}", a.text());
        assert_eq!(a.header("kaeso-cache"), None);
        assert_eq!(a.header("kaeso-route"), None);
    }
    env.app.state.in_flight.settled().await;
    assert!(env.mock.received().is_empty());
    let allowed = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("sturdy", "fine")),
    )
    .await;
    assert_eq!(allowed.status, StatusCode::OK, "{}", allowed.text());
    assert_eq!(allowed.header("kaeso-cache"), Some("miss"));
}

#[tokio::test]
async fn a_guardrail_that_times_out_follows_its_on_error_setting() {
    let env = env().await;
    let slow = MockGuardrail::start().await;
    let (id, _) = guard(
        &env,
        "Slow",
        &slow.url("/slow"),
        json!({ "timeout_ms": 200 }),
    )
    .await;
    let a = ask(&env, Format::Messages, "Hi").await;
    assert_eq!(a.status, StatusCode::SERVICE_UNAVAILABLE, "{}", a.text());
    assert_eq!(
        error_message(Format::Messages, &a),
        "The guardrail `Slow` could not check this request, so Kaeso refused it; try again shortly."
    );
    let chat = ask(&env, Format::Chat, "Hi").await;
    assert_eq!(
        chat.status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "the OpenAI formats answer Kaeso's own 503 with 500"
    );
    assert!(
        a.header("kaeso-guardrails")
            .unwrap()
            .contains("action=block;failure=timeout"),
    );
    assert!(env.mock.received().is_empty());

    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/{id}", guardrails_path(&env)),
            Some(json!({ "on_error": "allow" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["on_error"], "allow");
    let a = ask(&env, Format::Chat, "Hi").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert!(
        a.header("kaeso-guardrails")
            .unwrap()
            .contains("action=allow;failure=timeout"),
    );
    let entries = logged(&env, "?guardrail_action=allow").await;
    assert_eq!(entries[0]["guardrails"][0]["failure"], "timeout");
}

#[tokio::test]
async fn failing_unreadable_and_redirecting_guardrails_count_as_failures() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    for (path, failure, status) in [
        ("/fail", "status", Some(500)),
        ("/nonsense", "invalid", Some(200)),
        ("/moved", "status", Some(307)),
    ] {
        guard(&env, path, &g.url(path), json!({})).await;
        let a = ask(&env, Format::Messages, "Hi").await;
        assert_eq!(
            a.status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{path}: {}",
            a.text()
        );
        let entries = logged(&env, "?guardrail_action=block").await;
        let run = entries
            .iter()
            .map(|e| &e["guardrails"][0])
            .find(|r| r["name"] == path)
            .unwrap();
        assert_eq!(run["failure"], failure, "{path}");
        assert_eq!(run["status"], json!(status), "{path}");
    }
    assert!(
        g.seen().iter().all(|s| s.path != "/check"),
        "a redirect is never followed"
    );
}

#[tokio::test]
async fn reply_guardrails_check_whole_replies_and_streams_only_when_they_hold_them() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    let (id, _) = guard(
        &env,
        "Replies",
        &g.url("/check?block=nice"),
        json!({ "phases": ["reply"] }),
    )
    .await;
    let a = ask(&env, Format::Messages, "Hi").await;
    assert_eq!(a.status, StatusCode::BAD_REQUEST, "{}", a.text());
    assert_eq!(
        error_message(Format::Messages, &a),
        "The guardrail `Replies` blocked the model's reply: Forbidden words are not allowed."
    );
    assert_eq!(
        env.mock.received().len(),
        1,
        "the reply came from the provider"
    );
    let asked = g.seen().pop().unwrap();
    assert_eq!(asked.body["phase"], "reply");
    assert_eq!(
        asked.id.unwrap(),
        format!("{}.reply", asked.body["request_id"].as_str().unwrap())
    );
    assert_eq!(asked.body["reply"]["role"], "assistant");
    assert!(
        asked.body["reply"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("nice")
    );
    let blocked = logged(&env, "?guardrail_action=block").await;
    assert_eq!(blocked[0]["guardrails"][0]["phase"], "reply");
    assert_ne!(
        blocked[0]["input_tokens"], 0,
        "the provider's usage is kept"
    );

    let mut body = messages("claude-haiku-4-5", "Hi");
    body["stream"] = json!(true);
    let streamed = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(body.clone()),
    )
    .await;
    assert_eq!(streamed.status, StatusCode::OK);
    assert!(
        streamed.text().contains("nice to meet"),
        "skip lets a stream through"
    );
    let asked_before = g.seen().len();

    let r = env
        .app
        .call(
            Method::PATCH,
            &format!("{}/{id}", guardrails_path(&env)),
            Some(json!({ "streams": "hold" })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let held = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    env.app.state.in_flight.settled().await;
    assert_eq!(held.status, StatusCode::BAD_REQUEST, "{}", held.text());
    assert_eq!(held.json()["type"], "error");
    assert_eq!(g.seen().len(), asked_before + 1);
    assert!(
        g.seen().last().unwrap().body["reply"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("nice to meet you"),
        "the guardrail sees the whole streamed reply"
    );
}

#[tokio::test]
async fn a_held_stream_that_passes_reaches_the_app_whole() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(
        &env,
        "Replies",
        &g.url("/check?block=never"),
        json!({ "phases": ["request", "reply"], "streams": "hold" }),
    )
    .await;
    let mut body = messages("claude-haiku-4-5", "Hi");
    body["stream"] = json!(true);
    let a = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    env.app.state.in_flight.settled().await;
    assert_eq!(a.status, StatusCode::OK);
    let text = a.text();
    assert!(text.contains("event: message_stop"), "{text}");
    for piece in ["Hello! It's ", "nice to meet ", "you. How can "] {
        assert!(text.contains(piece), "{text}");
    }
    let header = a.header("kaeso-guardrails").unwrap();
    assert!(header.contains("phase=request;action=allow"), "{header}");
    let entries = logged(&env, "").await;
    let phases: Vec<&str> = entries[0]["guardrails"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases, ["request", "reply"]);
}

#[tokio::test]
async fn a_reply_guardrail_can_rewrite_a_whole_reply() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(
        &env,
        "Rewrite",
        &g.url("/check?replace=Hello"),
        json!({ "phases": ["reply"] }),
    )
    .await;
    let a = ask(&env, Format::Messages, "Hi").await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert_eq!(
        a.json()["content"][0]["text"],
        "[REDACTED]! It's nice to meet you. How can I help you today?"
    );
}

#[tokio::test]
async fn a_key_over_its_request_rate_is_refused_before_the_guardrail_is_asked() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(&env, "Words", &g.url("/check"), json!({})).await;
    let r = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/limits", env.owner.org_id, env.project),
            json!({ "level": "project", "kind": "request_rate", "period": "minute", "amount": 2 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(
        ask(&env, Format::Chat, "forbidden").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(ask(&env, Format::Chat, "fine").await.status, StatusCode::OK);
    let a = ask(&env, Format::Chat, "fine").await;
    assert_eq!(a.status, StatusCode::TOO_MANY_REQUESTS, "{}", a.text());
    assert_eq!(g.seen().len(), 2, "the guardrail wasn't asked a third time");
}

#[tokio::test]
async fn embeddings_pass_unchecked() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(&env, "Words", &g.url("/check"), json!({})).await;
    let a = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "text-embedding-3-small", "input": "forbidden" })),
    )
    .await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    assert!(a.header("kaeso-guardrails").is_none());
    assert!(g.seen().is_empty());
}

#[tokio::test]
async fn the_playground_is_checked_like_the_api() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(&env, "Words", &g.url("/check"), json!({})).await;
    let r = env
        .app
        .post(
            &format!(
                "/orgs/{}/projects/{}/playground/responses",
                env.owner.org_id, env.project
            ),
            json!({ "model": "gpt-4o", "input": "forbidden", "stream": true }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        "The guardrail `Words` blocked this request: Forbidden words are not allowed."
    );
    assert_eq!(g.seen()[0].body["source"], "playground");
}

#[tokio::test]
async fn guardrails_are_managed_through_the_platform_api() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    let created = add(
        &env,
        json!({ "name": "  PII check ", "url": g.url("/check"), "phases": ["request", "reply"], "streams": "hold", "timeout_ms": 500 }),
    )
    .await;
    let view = &created["guardrail"];
    let id = view["id"].as_str().unwrap();
    assert!(id.starts_with("grd_"));
    assert_eq!(view["name"], "PII check");
    assert_eq!(view["timeout_ms"], 500);
    assert_eq!(view["on_error"], "block");
    assert_eq!(view["phases"], json!(["request", "reply"]));
    assert_eq!(view["streams"], "hold");
    assert_eq!(view["projects"], json!([]));
    let secret = created["secret"].as_str().unwrap();
    assert!(secret.starts_with("whsec_"));

    let one = env
        .app
        .get(
            &format!("{}/{id}", guardrails_path(&env)),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.body.get("secret"), None);
    let listed = env.app.get(&guardrails_path(&env), &env.owner.cookie).await;
    assert_eq!(listed.body["data"][0]["id"], id);

    attach(&env, &[id]).await;
    let listed = env.app.get(&guardrails_path(&env), &env.owner.cookie).await;
    assert_eq!(listed.body["data"][0]["projects"], json!([env.project]));

    let rotated = env
        .app
        .post(
            &format!("{}/{id}/secret", guardrails_path(&env)),
            json!({}),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(rotated.status, StatusCode::OK, "{:?}", rotated.body);
    let new_secret = rotated.body["secret"].as_str().unwrap();
    assert_ne!(new_secret, secret);
    let a = ask(&env, Format::Chat, "Hi").await;
    assert_eq!(a.status, StatusCode::OK);
    let s = g.seen().pop().unwrap();
    let key = STANDARD
        .decode(new_secret.strip_prefix("whsec_").unwrap())
        .unwrap();
    let want = kaeso_server::services::guardrails::signature(
        &key,
        s.id.as_deref().unwrap(),
        s.timestamp.as_deref().unwrap().parse().unwrap(),
        &s.raw,
    );
    assert_eq!(
        s.signature.as_deref(),
        Some(want.as_str()),
        "the new secret signs at once"
    );

    for (body, param) in [
        (
            json!({ "name": "x", "url": g.url("/check"), "timeout_ms": 50 }),
            "timeout_ms",
        ),
        (
            json!({ "name": "x", "url": g.url("/check"), "timeout_ms": 10_001 }),
            "timeout_ms",
        ),
        (
            json!({ "name": "x", "url": g.url("/check"), "phases": [] }),
            "phases",
        ),
        (json!({ "name": "", "url": g.url("/check") }), "name"),
        (json!({ "name": "x", "url": "ftp://example.com" }), "url"),
    ] {
        let r = env
            .app
            .post(&guardrails_path(&env), body, Some(&env.owner.cookie))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        assert_eq!(r.body["error"]["param"], param);
    }

    let r = env
        .app
        .call(
            Method::PUT,
            &assignments_path(&env),
            Some(
                json!({ "project": env.project, "guardrails": ["grd_00000000000000000000000000"] }),
            ),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = env
        .app
        .call(
            Method::PUT,
            &assignments_path(&env),
            Some(json!({ "project": env.project, "guardrails": [id, id] })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    let r = env
        .app
        .call(
            Method::PUT,
            &assignments_path(&env),
            Some(json!({ "project": "proj_00000000000000000000000000", "guardrails": [id] })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "project");

    let r = env
        .app
        .call(
            Method::DELETE,
            &format!("{}/{id}", guardrails_path(&env)),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let a = ask(&env, Format::Chat, "forbidden").await;
    assert_eq!(
        a.status,
        StatusCode::OK,
        "a deleted guardrail checks nothing"
    );
    let listed = env.app.get(&project_path(&env), &env.owner.cookie).await;
    assert_eq!(listed.body["data"], json!([]));

    let audit = env
        .app
        .get(
            &format!("/orgs/{}/audit-log", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let actions: Vec<&str> = audit.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .filter(|a| a.contains("guardrail"))
        .collect();
    for action in [
        "guardrail.create",
        "project.guardrails_update",
        "guardrail.secret_rotate",
        "guardrail.delete",
    ] {
        assert!(actions.contains(&action), "{action} in {actions:?}");
    }
}

#[tokio::test]
async fn a_test_asks_the_guardrail_without_a_request() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    let created = add(&env, json!({ "name": "Words", "url": g.url("/check") })).await;
    let id = created["guardrail"]["id"].as_str().unwrap();
    let test = |body: Value| {
        let path = format!("{}/{id}/test", guardrails_path(&env));
        let env = &env;
        async move { env.app.post(&path, body, Some(&env.owner.cookie)).await }
    };
    let blocked = test(json!({ "messages": [{ "role": "user", "content": [{ "type": "text", "text": "forbidden" }] }] })).await;
    assert_eq!(blocked.status, StatusCode::OK, "{:?}", blocked.body);
    assert_eq!(blocked.body["action"], "block");
    assert_eq!(blocked.body["category"], "words");
    assert_eq!(blocked.body["message"], "Forbidden words are not allowed.");
    let replaced = test(json!({ "messages": [{ "role": "user", "content": [{ "type": "text", "text": "a secret" }] }] })).await;
    assert_eq!(replaced.body["action"], "replace");
    assert_eq!(
        replaced.body["messages"][0]["content"][0]["text"],
        "a [REDACTED]"
    );
    let reply = test(json!({
        "phase": "reply",
        "messages": [{ "role": "user", "content": [{ "type": "text", "text": "Hi" }] }],
        "reply": { "role": "assistant", "content": [{ "type": "text", "text": "forbidden" }] }
    }))
    .await;
    assert_eq!(reply.body["action"], "block");
    let s = g.seen().pop().unwrap();
    assert_eq!(s.body["source"], "test");
    assert_eq!(s.body["project"], Value::Null);
    let no_reply = test(json!({ "phase": "reply", "messages": [] })).await;
    assert_eq!(no_reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(no_reply.body["error"]["param"], "reply");
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn a_private_address_is_refused_unless_the_install_allows_its_network() {
    let env = env_with(&[]).await;
    let g = MockGuardrail::start().await;
    let r = env
        .app
        .post(
            &guardrails_path(&env),
            json!({ "name": "Local", "url": g.url("/check") }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
    assert_eq!(r.body["error"]["param"], "url");
    assert_eq!(
        r.body["error"]["message"],
        "`url` points to an address this install does not allow."
    );
}

#[tokio::test]
async fn a_project_admin_sees_the_project_guardrails_but_cannot_change_them() {
    let env = env().await;
    let g = MockGuardrail::start().await;
    guard(&env, "Words", &g.url("/check"), json!({})).await;
    let (cookie, _) = common::join_first_project(
        &env.app,
        &env.owner,
        json!({
            "email": "dev@example.com",
            "role": "developer",
            "project_roles": [{ "project_id": env.project, "role": "admin" }]
        }),
        "Dev",
    )
    .await;
    let listed = env.app.get(&project_path(&env), &cookie).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.body);
    assert_eq!(listed.body["data"][0]["name"], "Words");
    let r = env
        .app
        .call(
            Method::PUT,
            &assignments_path(&env),
            Some(json!({ "project": env.project, "guardrails": [] })),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
    let r = env.app.get(&guardrails_path(&env), &cookie).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "developers see the organization's guardrails"
    );
    let r = env
        .app
        .post(
            &guardrails_path(&env),
            json!({ "name": "Mine", "url": g.url("/check") }),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
}
