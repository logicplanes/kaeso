mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::v1::{ANTHROPIC_SECRET, Capture, Env, OPENAI_SECRET, call, chat, messages};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

async fn env() -> Env {
    let capture = Arc::new(Capture::default());
    common::v1::env(Some(capture), None).await
}

#[tokio::test]
async fn same_provider_calls_send_the_credential_and_only_kpp_output() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["type"], "message");
    assert_eq!(r.header("kaeso-model"), Some("claude-haiku-4-5"));
    assert_eq!(r.header("kaeso-provider"), Some("anthropic"));
    assert!(r.header("kaeso-request-id").unwrap().starts_with("req_"));
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.header("x-api-key"), Some(ANTHROPIC_SECRET));
    assert_eq!(sent.header("authorization"), None);
    assert!(
        !sent.headers.iter().any(|(_, v)| v.contains(&env.key)),
        "the Kaeso key never reaches a provider"
    );
    assert_eq!(sent.header("anthropic-version"), Some("2023-06-01"));

    let outcomes = env.capture.all();
    let o = outcomes.last().unwrap();
    assert_eq!(o.status, 200);
    assert_eq!(o.provider, Some("anthropic"));
    assert_eq!(o.model_id.as_deref(), Some("claude-haiku-4-5"));
    assert!(o.usage.as_ref().unwrap().output_tokens > 0);
    assert!(o.prices.as_ref().unwrap().input_price.is_some());
    assert!(!o.cancelled);

    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "chat.completion");
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/responses");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {OPENAI_SECRET}").as_str())
    );
    assert_eq!(sent.json()["store"], false);
}

#[tokio::test]
async fn apps_reach_the_other_provider_through_kpp() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("claude-haiku-4-5", "Hi")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "chat.completion");
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.json()["max_tokens"], 64000);
    assert_eq!(sent.json()["model"], "claude-haiku-4-5");

    let mut body = messages("openai/gpt-4o", "Hi");
    body["stream"] = json!(true);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.header("content-type")
            .unwrap()
            .starts_with("text/event-stream")
    );
    let text = r.text();
    assert!(
        text.contains("event: message_start") && text.contains("event: message_stop"),
        "{text}"
    );
    let sent = env.mock.last().unwrap();
    assert_eq!(sent.path, "/v1/responses");
    assert_eq!(sent.json()["model"], "gpt-4o");
    assert_eq!(sent.json()["stream"], true);

    let mut body = chat("claude-haiku-4-5", "Hi");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text().contains("chat.completion.chunk") && r.text().trim_end().ends_with("data: [DONE]"),
        "{}",
        r.text()
    );
    let mut found = None;
    for _ in 0..100 {
        if let Some(o) = env
            .capture
            .all()
            .into_iter()
            .rev()
            .find(|o| o.stream && o.provider == Some("anthropic"))
        {
            found = Some(o);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let o = found.expect("stream outcome");
    assert!(o.usage.unwrap().output_tokens > 0);
    assert!(o.reply.is_some());
}

#[tokio::test]
async fn token_counting_embeddings_and_listings() {
    let env = env().await;
    let count =
        json!({ "model": "claude-haiku-4-5", "messages": [{ "role": "user", "content": "Hi" }] });
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(count),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["input_tokens"].as_u64().unwrap() > 0);
    let count = json!({ "model": "gpt-4o", "messages": [{ "role": "user", "content": "Hi" }] });
    let r = call(
        &env,
        "/v1/messages/count_tokens",
        Some(&env.key),
        true,
        Some(count),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(env.mock.last().unwrap().path, "/v1/responses/input_tokens");
    let r = call(
        &env,
        "/v1/responses/input_tokens",
        Some(&env.key),
        false,
        Some(json!({ "model": "gpt-4o", "input": "Hi" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "text-embedding-3-small", "input": "hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["object"], "list");
    let r = call(
        &env,
        "/v1/embeddings",
        Some(&env.key),
        false,
        Some(json!({ "model": "claude-haiku-4-5", "input": "hello" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot create embeddings")
    );

    let r = call(&env, "/v1/models", Some(&env.key), false, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let ids: Vec<String> = r.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(r.json()["object"], "list");
    assert!(
        ids.iter().any(|i| i == "claude-haiku-4-5") && ids.iter().any(|i| i == "gpt-4o"),
        "{ids:?}"
    );
    let r = call(&env, "/v1/models", Some(&env.key), true, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let first = &r.json()["data"][0];
    assert_eq!(first["type"], "model");
    assert!(first["display_name"].as_str().is_some());
    let r = call(
        &env,
        "/v1/models/claude-haiku-4-5",
        Some(&env.key),
        true,
        None,
    )
    .await;
    assert_eq!(r.json()["display_name"], "Claude Haiku 4.5 (latest)");
    assert_eq!(r.json()["created_at"], "2025-10-15T00:00:00Z");
    let r = call(
        &env,
        "/v1/models/openai%2Fgpt-4o",
        Some(&env.key),
        false,
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["owned_by"], "openai");
    let r = call(&env, "/v1/models/gpt-5", Some(&env.key), false, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn errors_arrive_in_the_apps_format() {
    let env = env().await;
    let r = call(
        &env,
        "/v1/messages",
        None,
        true,
        Some(messages("claude-haiku-4-5", "x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.json()["type"], "error");
    assert_eq!(r.json()["error"]["type"], "authentication_error");
    let r = call(
        &env,
        "/v1/chat/completions",
        Some("sk-kso-nope"),
        false,
        Some(chat("gpt-4o", "x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.json()["error"]["message"].as_str().is_some());
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-opus-4-6", "x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("catalogue")
    );
    let base = format!("/orgs/{}/projects/{}", env.owner.org_id, env.project);
    let models = env
        .app
        .get(&format!("{base}/models"), &env.owner.cookie)
        .await;
    let gpt = models.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == "gpt-4o")
        .unwrap()["id"]
        .clone();
    let narrow = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "narrow", "models": [gpt] }),
            Some(&env.owner.cookie),
        )
        .await;
    let narrow_key = narrow.body["secret"].as_str().unwrap();
    let r = call(
        &env,
        "/v1/messages",
        Some(narrow_key),
        true,
        Some(messages("claude-haiku-4-5", "x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("key's model list")
    );
    let before = env.mock.received().len();
    let mut body = messages("gpt-4o", "x");
    body["top_k"] = json!(5);
    let r = call(&env, "/v1/messages", Some(&env.key), true, Some(body)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("top_k"));
    assert_eq!(env.mock.received().len(), before, "nothing was sent");
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "[mock:error=rate-limit]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.json()["error"]["type"], "rate_limit_error");
    assert_eq!(r.header("retry-after"), Some("20"));
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("claude-haiku-4-5", "[mock:error=overloaded]")),
    )
    .await;
    assert!(r.status.is_server_error(), "{}", r.status);
    assert!(r.json()["error"]["message"].as_str().is_some());
    let started = std::time::Instant::now();
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o-mini", "[mock:stall]")),
    )
    .await;
    assert_eq!(r.status, StatusCode::GATEWAY_TIMEOUT, "{}", r.text());
    assert!(started.elapsed() < Duration::from_secs(5));
    let creds = env
        .app
        .get(
            &format!("/orgs/{}/credentials", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    let openai = creds.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["provider"] == "openai")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.app
        .call(
            axum::http::Method::DELETE,
            &format!("/orgs/{}/credentials/{openai}", env.owner.org_id),
            None,
            Some(&env.owner.cookie),
        )
        .await;
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(chat("gpt-4o", "x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(
        r.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("`openai`")
    );
    let r = call(&env, "/v1/nothing", Some(&env.key), true, Some(json!({}))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["type"], "error");
}

#[tokio::test]
async fn a_disconnect_cancels_the_provider_request_and_is_recorded() {
    let env = env().await;
    let mut body = messages("claude-haiku-4-5", "[mock:slow]");
    body["stream"] = json!(true);
    let req = Request::builder()
        .uri("/v1/messages")
        .method("POST")
        .header("x-api-key", &env.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = env.app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let mut body = resp.into_body();
    let first = body.frame().await.unwrap().unwrap();
    assert!(first.is_data());
    drop(body);
    let mut dropped = false;
    for _ in 0..200 {
        if env.mock.stream_counts().2 == 1 {
            dropped = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        dropped,
        "the provider stream was not cancelled: {:?}",
        env.mock.stream_counts()
    );
    let mut cancelled = None;
    for _ in 0..200 {
        if let Some(o) = env.capture.all().into_iter().find(|o| o.cancelled) {
            cancelled = Some(o);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let o = cancelled.expect("a cancelled outcome");
    assert!(o.stream);
    assert_eq!(o.status, 499);
    assert_eq!(o.error_kind.as_deref(), Some("client_disconnected"));
    let p = o.stream_progress.expect("how far the stream got");
    assert!(p.events > 0 && p.bytes > 0, "{p:?}");
    assert!(
        o.usage.is_some(),
        "usage received before the disconnect is kept"
    );

    let mut body = chat("claude-haiku-4-5", "[mock:cut]");
    body["stream"] = json!(true);
    let r = call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(body),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text().contains("\"error\""), "{}", r.text());
}

#[tokio::test]
async fn every_upstream_body_is_what_kpp_encoded() {
    use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
    use kaeso_kpp::{DialectId, Request as KppRequest, RequestKind, StreamMode};
    let env = env().await;
    let cases = [
        (
            "/v1/chat/completions",
            false,
            DialectId::OpenAiChat,
            chat("gpt-4o", "one"),
            DialectId::OpenAiResponses,
            "openai",
        ),
        (
            "/v1/messages",
            true,
            DialectId::AnthropicMessages,
            messages("claude-haiku-4-5", "two"),
            DialectId::AnthropicMessages,
            "anthropic",
        ),
        (
            "/v1/chat/completions",
            false,
            DialectId::OpenAiChat,
            json!({ "model": "claude-haiku-4-5", "stream": true, "messages": [{ "role": "user", "content": "three" }] }),
            DialectId::AnthropicMessages,
            "anthropic",
        ),
    ];
    for (path, anthropic, ingress, body, egress, provider) in cases {
        let r = call(&env, path, Some(&env.key), anthropic, Some(body.clone())).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        let sent = env.mock.last().unwrap();
        let mut req = dispatch::decode_request(
            ingress,
            RequestKind::Generate,
            &serde_json::to_vec(&body).unwrap(),
            &HeaderList::new(),
        )
        .unwrap();
        let app_req = req.clone();
        if let KppRequest::Generate(g) = &mut req {
            if egress == DialectId::AnthropicMessages && g.sampling.max_output_tokens.is_none() {
                g.sampling.max_output_tokens = Some(64000);
            }
            if let StreamMode::On { include_usage, .. } = &mut g.stream {
                *include_usage = true;
            }
        }
        let mut ctx = EgressCtx::new(&app_req).with_provider(provider);
        if provider == "anthropic" {
            ctx = ctx.with_adaptive_thinking(false);
        }
        let expected = dispatch::encode_request(egress, &req, &mut ctx).unwrap();
        assert_eq!(
            sent.body.as_ref(),
            expected.body.as_slice(),
            "{path}: the provider received something kpp did not encode"
        );
    }
}

#[tokio::test]
async fn rejections_are_errors_in_the_apps_format() {
    let env = env().await;
    let raw =
        |method: &str, path: &str, anthropic: bool, content_type: Option<&str>, body: &str| {
            let mut req = Request::builder().method(method).uri(path);
            if anthropic {
                req = req
                    .header("x-api-key", env.key.as_str())
                    .header("anthropic-version", "2023-06-01");
            } else {
                req = req.header("authorization", format!("Bearer {}", env.key));
            }
            if let Some(c) = content_type {
                req = req.header("content-type", c);
            }
            req.body(Body::from(body.to_owned())).unwrap()
        };
    let cases = [
        (
            raw("GET", "/v1/chat/completions", false, None, ""),
            405,
            false,
        ),
        (raw("DELETE", "/v1/messages", true, None, ""), 405, true),
        (raw("POST", "/v1/models", false, None, ""), 405, false),
        (raw("GET", "/v1", false, None, ""), 404, false),
        (raw("GET", "/v1/nope", true, None, ""), 404, true),
        (
            raw(
                "POST",
                "/v1/chat/completions",
                false,
                Some("application/json"),
                "{",
            ),
            400,
            false,
        ),
        (
            raw("POST", "/v1/messages", true, Some("application/json"), "{"),
            400,
            true,
        ),
        (
            raw(
                "POST",
                "/v1/chat/completions",
                false,
                Some("text/plain"),
                "hello",
            ),
            400,
            false,
        ),
    ];
    for (req, status, anthropic) in cases {
        let what = format!("{} {}", req.method(), req.uri());
        let resp = env.app.router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status().as_u16(), status, "{what}");
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        assert!(
            content_type.starts_with("application/json"),
            "{what}: {content_type}"
        );
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| panic!("{what}: not JSON: {}", String::from_utf8_lossy(&bytes)));
        if anthropic {
            assert_eq!(body["type"], "error", "{what}: {body}");
            assert!(body["error"]["type"].as_str().is_some(), "{what}: {body}");
        } else {
            assert!(body["error"]["type"].as_str().is_some(), "{what}: {body}");
            assert!(body.get("type").is_none(), "{what}: {body}");
        }
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|m| !m.is_empty()),
            "{what}: {body}"
        );
    }
}

#[tokio::test]
async fn a_provider_response_beyond_the_limit_fails_in_the_apps_format() {
    let capture = Arc::new(Capture::default());
    let env = common::v1::env(
        Some(capture),
        Some(&[("KAESO_BODY_LIMITS_PROVIDER_RESPONSE", "64B")]),
    )
    .await;
    let r = call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hello")),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_GATEWAY, "{}", r.text());
    assert_eq!(r.json()["type"], "error");
    assert_eq!(
        r.json()["error"]["message"],
        "The provider's response is larger than this install accepts."
    );
    let mut streaming = messages("claude-haiku-4-5", "Hello");
    streaming["stream"] = json!(true);
    let streamed = call(&env, "/v1/messages", Some(&env.key), true, Some(streaming)).await;
    assert_eq!(streamed.status, StatusCode::OK, "{}", streamed.text());
    assert!(
        streamed.text().contains("message_stop"),
        "streams are not capped by it"
    );
}
