#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{Answer, Capture, Env};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

enum Key<'a> {
    Header(&'a str),
    Query(&'a str),
    Bearer(&'a str),
    None,
}

async fn gemini(env: &Env, method: &str, path: &str, key: Key<'_>, body: Option<Value>) -> Answer {
    let path = match key {
        Key::Query(k) => format!(
            "{path}{}key={k}",
            if path.contains('?') { '&' } else { '?' }
        ),
        _ => path.to_owned(),
    };
    let mut req = Request::builder().uri(&path).method(method);
    match key {
        Key::Header(k) => req = req.header("x-goog-api-key", k),
        Key::Bearer(k) => req = req.header("authorization", format!("Bearer {k}")),
        Key::Query(_) | Key::None => {}
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
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

async fn env() -> Env {
    common::v1::env(Some(Arc::new(Capture::default())), None).await
}

fn ask(text: &str) -> Value {
    json!({ "contents": [{ "role": "user", "parts": [{ "text": text }] }] })
}

fn gemini_error(r: &Answer, code: u16, status: &str) -> String {
    assert_eq!(r.status.as_u16(), code, "{}", r.text());
    let body = r.json();
    assert_eq!(body["error"]["code"], code, "{body}");
    assert_eq!(body["error"]["status"], status, "{body}");
    body["error"]["message"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_gemini_app_reaches_claude_with_its_key_in_the_header_or_the_query() {
    let env = env().await;
    for key in [Key::Header(&env.key), Key::Query(&env.key)] {
        let r = gemini(
            &env,
            "POST",
            "/v1beta/models/claude-haiku-4-5:generateContent",
            key,
            Some(ask("Hello [mock:fixture=text-simple]")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.header("kaeso-provider"), Some("anthropic"));
        let reply = r.json();
        assert_eq!(
            reply["candidates"][0]["content"]["parts"][0]["text"],
            "Hello! It's nice to meet you. How can I help you today?"
        );
        assert_eq!(reply["candidates"][0]["content"]["role"], "model");
        assert_eq!(reply["candidates"][0]["finishReason"], "STOP");
        let sent = env.mock.last().unwrap();
        assert_eq!(sent.path, "/v1/messages");
        assert_eq!(sent.json()["model"], "claude-haiku-4-5");
        assert!(
            sent.headers.iter().all(|(_, v)| !v.contains(&env.key)),
            "the app's key never reaches the provider"
        );
        assert_eq!(sent.query, None);
    }
    let outcomes = env.outcomes().await;
    let logged = outcomes.last().unwrap();
    assert_eq!(logged.ingress, kaeso_kpp::DialectId::Gemini);
    assert_eq!(
        kaeso_server::services::logs::endpoint(logged),
        "models:generateContent"
    );
    assert_eq!(logged.requested_model.as_deref(), Some("claude-haiku-4-5"));
}

#[tokio::test]
async fn a_gemini_app_streams_as_server_sent_events_and_is_told_to_ask_for_them() {
    let env = env().await;
    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:streamGenerateContent?alt=sse",
        Key::Header(&env.key),
        Some(ask("Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.header("content-type")
            .unwrap()
            .starts_with("text/event-stream")
    );
    let chunks: Vec<Value> = r
        .text()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect();
    assert!(chunks.len() > 1, "{}", r.text());
    assert!(chunks.last().unwrap()["candidates"][0]["finishReason"].is_string());
    assert!(chunks.last().unwrap()["usageMetadata"].is_object());
    assert_eq!(env.mock.last().unwrap().json()["stream"], true);

    env.mock.clear();
    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:streamGenerateContent",
        Key::Header(&env.key),
        Some(ask("Hello")),
    )
    .await;
    let message = gemini_error(&r, 400, "INVALID_ARGUMENT");
    assert_eq!(
        message,
        "Kaeso streams Gemini replies as server-sent events only; add `alt=sse` to the URL."
    );
    assert!(env.mock.received().is_empty());
}

#[tokio::test]
async fn refusals_reach_a_gemini_app_in_geminis_error_shape() {
    let env = env().await;
    let message = gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1beta/models/claude-haiku-4-5:generateContent",
            Key::None,
            Some(ask("Hello")),
        )
        .await,
        401,
        "UNAUTHENTICATED",
    );
    assert!(message.contains("`x-goog-api-key`"), "{message}");
    gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1beta/models/claude-haiku-4-5:generateContent",
            Key::Header("sk-kso-not-a-key"),
            Some(ask("Hello")),
        )
        .await,
        401,
        "UNAUTHENTICATED",
    );
    let message = gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1beta/models/no-such-model:generateContent",
            Key::Header(&env.key),
            Some(ask("Hello")),
        )
        .await,
        404,
        "NOT_FOUND",
    );
    assert!(message.contains("`no-such-model`"), "{message}");
    let message = gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1beta/models/claude-haiku-4-5:batchGenerateContent",
            Key::Header(&env.key),
            Some(ask("Hello")),
        )
        .await,
        404,
        "NOT_FOUND",
    );
    assert!(message.contains("`batchGenerateContent`"), "{message}");
    for path in ["/v1beta/files", "/v1beta/cachedContents", "/v1beta"] {
        gemini_error(
            &gemini(&env, "GET", path, Key::Header(&env.key), None).await,
            404,
            "NOT_FOUND",
        );
    }
    let r = gemini(
        &env,
        "DELETE",
        "/v1beta/models/claude-haiku-4-5",
        Key::Header(&env.key),
        None,
    )
    .await;
    gemini_error(&r, 405, "INVALID_ARGUMENT");
    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:generateContent",
        Key::Header(&env.key),
        Some(json!({ "contents": [{ "role": "user", "parts": [{ "text": "Hi" }] }], "generationConfig": { "candidateCount": 2 } })),
    )
    .await;
    gemini_error(&r, 400, "INVALID_ARGUMENT");
    assert!(env.mock.received().iter().all(|r| r.path != "/v1/messages"));
}

#[tokio::test]
async fn a_gemini_app_counts_tokens_and_embeds_on_other_providers() {
    let env = env().await;
    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:countTokens",
        Key::Header(&env.key),
        Some(ask("Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/messages/count_tokens");
    assert_eq!(r.json()["totalTokens"], 14);

    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/text-embedding-3-small:batchEmbedContents",
        Key::Header(&env.key),
        Some(json!({ "requests": [
            { "model": "models/text-embedding-3-small", "content": { "parts": [{ "text": "refund policy [mock:fixture=recorded-string-array]" }] } },
            { "model": "models/text-embedding-3-small", "content": { "parts": [{ "text": "shipping times" }] } }
        ] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/embeddings");
    assert_eq!(sent.json()["input"].as_array().unwrap().len(), 2);
    let reply = r.json();
    assert_eq!(reply["embeddings"].as_array().unwrap().len(), 2);
    assert!(reply["embeddings"][0]["values"][0].is_number());

    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:embedContent",
        Key::Header(&env.key),
        Some(json!({ "content": { "parts": [{ "text": "x" }] } })),
    )
    .await;
    gemini_error(&r, 400, "INVALID_ARGUMENT");
    let outcomes = env.outcomes().await;
    let endpoints: Vec<&str> = outcomes
        .iter()
        .map(kaeso_server::services::logs::endpoint)
        .collect();
    assert!(endpoints.contains(&"models:countTokens"), "{endpoints:?}");
    assert!(
        endpoints.contains(&"models:batchEmbedContents"),
        "{endpoints:?}"
    );
}

#[tokio::test]
async fn a_gemini_app_lists_the_projects_models_a_page_at_a_time() {
    let env = env().await;
    let r = gemini(&env, "GET", "/v1beta/models", Key::Query(&env.key), None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let all = r.json();
    let names: Vec<&str> = all["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"models/claude-haiku-4-5"), "{names:?}");
    assert!(names.contains(&"models/gpt-4o"), "{names:?}");
    assert!(all.get("nextPageToken").is_none(), "{all}");

    let r = gemini(
        &env,
        "GET",
        "/v1beta/models?pageSize=1",
        Key::Header(&env.key),
        None,
    )
    .await;
    let first = r.json();
    assert_eq!(first["models"].as_array().unwrap().len(), 1);
    let token = first["nextPageToken"].as_str().unwrap().to_owned();
    let r = gemini(
        &env,
        "GET",
        &format!("/v1beta/models?pageSize=1&pageToken={token}"),
        Key::Header(&env.key),
        None,
    )
    .await;
    let second = r.json();
    assert_ne!(second["models"][0]["name"], first["models"][0]["name"]);
    assert_eq!(second["models"][0]["name"], all["models"][1]["name"]);

    let r = gemini(
        &env,
        "GET",
        "/v1beta/models/claude-haiku-4-5",
        Key::Header(&env.key),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["name"], "models/claude-haiku-4-5");
    gemini_error(
        &gemini(
            &env,
            "GET",
            "/v1beta/models/no-such-model",
            Key::Header(&env.key),
            None,
        )
        .await,
        404,
        "NOT_FOUND",
    );
    gemini_error(
        &gemini(&env, "GET", "/v1beta/models", Key::None, None).await,
        401,
        "UNAUTHENTICATED",
    );
}

#[tokio::test]
async fn a_gemini_app_on_claude_keeps_claudes_thinking_through_a_tool_loop() {
    let env = env().await;
    let tools = json!([{ "functionDeclarations": [{ "name": "get_time", "description": "Local time", "parametersJsonSchema": { "type": "object", "properties": { "timezone": { "type": "string" } } } }] }]);
    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:generateContent",
        Key::Header(&env.key),
        Some(json!({
            "contents": [{ "role": "user", "parts": [{ "text": "Is it daytime in Paris? [mock:fixture=thinking-interleaved-tools]" }] }],
            "tools": tools,
            "generationConfig": { "thinkingConfig": { "thinkingLevel": "LOW" } }
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!r.text().contains("EqEvMLy0"), "{}", r.text());
    let reply = r.json();
    let call = reply["candidates"][0]["content"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|p| p.get("functionCall"))
        .unwrap()
        .clone();
    assert_eq!(call["id"], "toolu_01afnDK4mGFJyw1zWvMT248V");
    assert_eq!(call["name"], "get_time");

    let r = gemini(
        &env,
        "POST",
        "/v1beta/models/claude-haiku-4-5:generateContent",
        Key::Header(&env.key),
        Some(json!({
            "contents": [
                { "role": "user", "parts": [{ "text": "Is it daytime in Paris?" }] },
                { "role": "model", "parts": [{ "functionCall": call }] },
                { "role": "user", "parts": [{ "functionResponse": { "id": "toolu_01afnDK4mGFJyw1zWvMT248V", "name": "get_time", "response": { "output": "14:05" } } }] }
            ],
            "tools": tools,
            "generationConfig": { "thinkingConfig": { "thinkingLevel": "LOW" } }
        })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let sent = env.mock.last().unwrap().json();
    let turn = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant")
        .unwrap()
        .clone();
    let blocks = turn["content"].as_array().unwrap();
    assert_eq!(blocks[0]["type"], "thinking", "{turn}");
    assert!(
        blocks[0]["signature"]
            .as_str()
            .unwrap()
            .starts_with("EqEvMLy0"),
        "{turn}"
    );
    assert_eq!(
        blocks.last().unwrap()["id"],
        "toolu_01afnDK4mGFJyw1zWvMT248V"
    );
}

const PREFIXES: [&str; 2] = ["/v1beta", "/v1"];

#[tokio::test]
async fn a_gemini_app_on_v1_generates_streams_counts_and_embeds_as_on_v1beta() {
    let env = env().await;
    for prefix in PREFIXES {
        for key in [
            Key::Header(&env.key),
            Key::Query(&env.key),
            Key::Bearer(&env.key),
        ] {
            let r = gemini(
                &env,
                "POST",
                &format!("{prefix}/models/claude-haiku-4-5:generateContent"),
                key,
                Some(ask("Hello [mock:fixture=text-simple]")),
            )
            .await;
            assert_eq!(r.status, StatusCode::OK, "{prefix}: {}", r.text());
            assert_eq!(r.json()["candidates"][0]["content"]["role"], "model");
            assert_eq!(env.mock.last().unwrap().query, None);
        }
        let r = gemini(
            &env,
            "POST",
            &format!("{prefix}/models/claude-haiku-4-5:streamGenerateContent?alt=sse"),
            Key::Header(&env.key),
            Some(ask("Hello")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{prefix}: {}", r.text());
        assert!(r.text().contains("\"finishReason\""), "{}", r.text());
        let r = gemini(
            &env,
            "POST",
            &format!("{prefix}/models/claude-haiku-4-5:countTokens"),
            Key::Header(&env.key),
            Some(ask("Hello")),
        )
        .await;
        assert_eq!(r.json()["totalTokens"], 14, "{prefix}: {}", r.text());
        let r = gemini(
            &env,
            "POST",
            &format!("{prefix}/models/text-embedding-3-small:embedContent"),
            Key::Header(&env.key),
            Some(json!({ "content": { "parts": [{ "text": "refund policy" }] } })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{prefix}: {}", r.text());
        assert!(
            r.json()["embedding"]["values"][0].is_number(),
            "{}",
            r.text()
        );
        let r = gemini(
            &env,
            "POST",
            &format!("{prefix}/models/text-embedding-3-small:batchEmbedContents"),
            Key::Header(&env.key),
            Some(json!({ "requests": [
                { "model": "models/text-embedding-3-small", "content": { "parts": [{ "text": "refund policy [mock:fixture=recorded-string-array]" }] } },
                { "model": "models/text-embedding-3-small", "content": { "parts": [{ "text": "shipping times" }] } }
            ] })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{prefix}: {}", r.text());
        assert_eq!(r.json()["embeddings"].as_array().unwrap().len(), 2);
    }
    let outcomes = env.outcomes().await;
    let mut endpoints: Vec<&str> = outcomes
        .iter()
        .map(kaeso_server::services::logs::endpoint)
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();
    assert_eq!(
        endpoints,
        [
            "models:batchEmbedContents",
            "models:countTokens",
            "models:embedContent",
            "models:generateContent",
            "models:streamGenerateContent"
        ],
        "a call is logged the same under both versions"
    );
}

#[tokio::test]
async fn v1_models_answers_in_geminis_shape_only_with_geminis_key() {
    let env = env().await;
    let names = |r: &Answer| -> Vec<String> {
        r.json()["models"]
            .as_array()
            .unwrap_or_else(|| panic!("not Gemini's listing: {}", r.text()))
            .iter()
            .map(|m| m["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let beta = names(&gemini(&env, "GET", "/v1beta/models", Key::Header(&env.key), None).await);
    for key in [Key::Header(&env.key), Key::Query(&env.key)] {
        let r = gemini(&env, "GET", "/v1/models", key, None).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(names(&r), beta);
    }
    let r = gemini(
        &env,
        "GET",
        "/v1/models?pageSize=1",
        Key::Header(&env.key),
        None,
    )
    .await;
    assert_eq!(names(&r).len(), 1);
    assert!(r.json()["nextPageToken"].is_string(), "{}", r.text());
    let r = gemini(
        &env,
        "GET",
        "/v1/models/claude-haiku-4-5",
        Key::Query(&env.key),
        None,
    )
    .await;
    assert_eq!(r.json()["name"], "models/claude-haiku-4-5", "{}", r.text());
    gemini_error(
        &gemini(
            &env,
            "GET",
            "/v1/models/claude-haiku-4-5?alt=json",
            Key::Header(&env.key),
            None,
        )
        .await,
        400,
        "INVALID_ARGUMENT",
    );
    let message = gemini_error(
        &gemini(
            &env,
            "GET",
            "/v1/models/no-such-model",
            Key::Header(&env.key),
            None,
        )
        .await,
        404,
        "NOT_FOUND",
    );
    assert!(message.contains("`no-such-model`"), "{message}");
    gemini_error(
        &gemini(
            &env,
            "GET",
            "/v1/models",
            Key::Query("sk-kso-not-a-key"),
            None,
        )
        .await,
        401,
        "UNAUTHENTICATED",
    );

    let r = gemini(&env, "GET", "/v1/models", Key::Bearer(&env.key), None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "list", "{}", r.text());
    let r = gemini(
        &env,
        "GET",
        "/v1/models/claude-haiku-4-5",
        Key::Bearer(&env.key),
        None,
    )
    .await;
    assert_eq!(r.json()["id"], "claude-haiku-4-5", "{}", r.text());
    let req = Request::builder()
        .uri("/v1/models")
        .header("authorization", format!("Bearer {}", env.key))
        .header("x-goog-api-key", env.key.as_str())
        .body(Body::empty())
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    let body: Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        body["object"], "list",
        "`authorization` keeps OpenAI's shape"
    );
}

#[tokio::test]
async fn errors_under_v1_are_geminis_for_gemini_calls_and_geminis_key() {
    let env = env().await;
    let message = gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1/models/claude-haiku-4-5:batchGenerateContent",
            Key::Header(&env.key),
            Some(ask("Hello")),
        )
        .await,
        404,
        "NOT_FOUND",
    );
    assert!(message.contains("`batchGenerateContent`"), "{message}");
    gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1/models/claude-haiku-4-5",
            Key::Query(&env.key),
            Some(ask("Hello")),
        )
        .await,
        404,
        "NOT_FOUND",
    );
    gemini_error(
        &gemini(
            &env,
            "POST",
            "/v1/models/claude-haiku-4-5:streamGenerateContent",
            Key::Bearer(&env.key),
            Some(ask("Hello")),
        )
        .await,
        400,
        "INVALID_ARGUMENT",
    );
    for method in ["GET", "DELETE"] {
        let r = gemini(
            &env,
            method,
            "/v1/models/claude-haiku-4-5:generateContent",
            Key::Bearer(&env.key),
            None,
        )
        .await;
        if method == "DELETE" {
            gemini_error(&r, 405, "INVALID_ARGUMENT");
        } else {
            assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
        }
    }
    gemini_error(
        &gemini(&env, "DELETE", "/v1/decisions", Key::Header(&env.key), None).await,
        405,
        "INVALID_ARGUMENT",
    );
    for key in [Key::Header(&env.key), Key::Query(&env.key)] {
        gemini_error(
            &gemini(&env, "GET", "/v1/files", key, None).await,
            404,
            "NOT_FOUND",
        );
    }

    let r = gemini(
        &env,
        "POST",
        "/v1/models/claude-haiku-4-5:batchGenerateContent",
        Key::Bearer(&env.key),
        Some(ask("Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED, "{}", r.text());
    assert_eq!(
        r.json()["error"]["type"],
        "invalid_request_error",
        "{}",
        r.text()
    );
    let r = gemini(&env, "GET", "/v1/files", Key::Bearer(&env.key), None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(r.json()["error"]["type"].is_string(), "{}", r.text());
    assert!(env.mock.received().is_empty());
}
