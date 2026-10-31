#![allow(clippy::unwrap_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use common::v1::{Answer, Capture, Env, chat, messages};
use futures::future::BoxFuture;
use http_body_util::BodyExt;
use kaeso_server::model_api::hooks::{Admission, Limits, Refusal, Reservation};
use kaeso_server::state::AppState;
use serde_json::{Value, json};
use tower::ServiceExt;

const APP: &str = "https://app.example.com";
const LOCAL: &str = "http://localhost:5173";
const OTHER: &str = "https://other.example.com";
const ORIGINS: &str = "https://APP.example.com/, http://localhost:5173";
const SDK_HEADERS: &str = "authorization, content-type, x-api-key, anthropic-version, anthropic-beta, anthropic-dangerous-direct-browser-access, openai-organization, openai-project, idempotency-key, x-stainless-os, x-stainless-retry-count, cookie, x-forwarded-for";
const REFLECTED: &str = "anthropic-beta, anthropic-dangerous-direct-browser-access, anthropic-version, authorization, content-type, idempotency-key, openai-organization, openai-project, x-api-key, x-stainless-os, x-stainless-retry-count";
const EXPOSED: &str = "kaeso-request-id, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, kaeso-cache, age, retry-after, retry-after-ms";

const ROUTES: &[(&str, &str)] = &[
    ("/v1/chat/completions", "POST"),
    ("/v1/responses", "POST"),
    ("/v1/responses/input_tokens", "POST"),
    ("/v1/embeddings", "POST"),
    ("/v1/messages", "POST"),
    ("/v1/messages/count_tokens", "POST"),
    ("/v1/models", "GET"),
    ("/v1/models/gpt-4o", "GET"),
];

#[derive(Debug, Default)]
struct Counting(AtomicUsize);

impl Limits for Counting {
    fn admit<'a>(
        &'a self,
        _state: &'a AppState,
        _a: &'a Admission,
    ) -> BoxFuture<'a, Result<Reservation, Refusal>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(Reservation::default()) })
    }
}

async fn env(origins: Option<&str>) -> Env {
    let settings: Vec<(&str, &str)> = origins
        .map(|o| vec![("KAESO_CORS_ALLOWED_ORIGINS", o)])
        .unwrap_or_default();
    common::v1::env(None, Some(&settings)).await
}

async fn send(env: &Env, req: Request<Body>) -> Answer {
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

fn preflight(path: &str, origin: &str, method: &str) -> Request<Body> {
    Request::builder()
        .method(Method::OPTIONS)
        .uri(path)
        .header("origin", origin)
        .header("access-control-request-method", method)
        .header("access-control-request-headers", SDK_HEADERS)
        .body(Body::empty())
        .unwrap()
}

fn request(
    method: &str,
    path: &str,
    origin: Option<&str>,
    key: Option<&str>,
    body: Option<Value>,
) -> Request<Body> {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    if let Some(k) = key {
        req = req.header("authorization", format!("Bearer {k}"));
    }
    match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    }
}

fn cors_headers(headers: &HeaderMap) -> Vec<String> {
    headers
        .keys()
        .map(|n| n.as_str().to_owned())
        .filter(|n| n.starts_with("access-control-"))
        .collect()
}

fn vary(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all("vary")
        .iter()
        .flat_map(|v| v.to_str().unwrap().split(','))
        .map(|v| v.trim().to_ascii_lowercase())
        .collect()
}

#[tokio::test]
async fn without_the_setting_the_model_api_answers_as_before() {
    let env = env(None).await;
    for (path, method) in ROUTES {
        let r = send(&env, preflight(path, APP, method)).await;
        assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED, "{path}");
        assert_eq!(
            r.json()["error"]["message"],
            format!("`{path}` does not accept `OPTIONS` requests."),
            "{path}"
        );
        assert!(cors_headers(&r.headers).is_empty(), "{path}");
        assert!(vary(&r.headers).is_empty(), "{path}");
    }
    let r = send(&env, preflight("/v1/unknown", APP, "POST")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(cors_headers(&r.headers).is_empty());

    let r = send(
        &env,
        request(
            "POST",
            "/v1/chat/completions",
            Some(APP),
            Some(&env.key),
            Some(chat("gpt-4o", "Hello")),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(cors_headers(&r.headers).is_empty());
    assert!(vary(&r.headers).is_empty());
}

#[tokio::test]
async fn listed_origins_get_a_preflight_answer_on_every_model_route() {
    let env = env(Some(ORIGINS)).await;
    for origin in [APP, LOCAL] {
        for (path, method) in ROUTES {
            let r = send(&env, preflight(path, origin, method)).await;
            assert_eq!(r.status, StatusCode::NO_CONTENT, "{path}");
            assert!(r.body.is_empty(), "{path}");
            assert_eq!(r.header("access-control-allow-origin"), Some(origin));
            assert_eq!(r.header("access-control-allow-methods"), Some("GET, POST"));
            assert_eq!(r.header("access-control-allow-headers"), Some(REFLECTED));
            assert_eq!(r.header("access-control-max-age"), Some("7200"));
            assert_eq!(r.header("access-control-allow-credentials"), None);
            assert_eq!(r.header("access-control-expose-headers"), None);
            assert_eq!(
                vary(&r.headers),
                [
                    "origin",
                    "access-control-request-method",
                    "access-control-request-headers"
                ]
            );
            assert_eq!(r.header("kaeso-request-id"), None, "{path}");
        }
    }
    let r = send(
        &env,
        Request::builder()
            .method(Method::OPTIONS)
            .uri("/v1/models")
            .header("origin", APP)
            .header("access-control-request-method", "GET")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(r.header("access-control-allow-origin"), Some(APP));
    assert_eq!(r.header("access-control-allow-headers"), None);
}

#[tokio::test]
async fn answers_to_listed_origins_carry_the_origin_vary_and_the_readable_headers() {
    let env = env(Some(ORIGINS)).await;
    let calls = [
        request(
            "POST",
            "/v1/chat/completions",
            Some(APP),
            Some(&env.key),
            Some(chat("gpt-4o", "Hello")),
        ),
        request(
            "POST",
            "/v1/responses",
            Some(APP),
            Some(&env.key),
            Some(json!({ "model": "gpt-4o", "input": "Hello" })),
        ),
        request(
            "POST",
            "/v1/embeddings",
            Some(APP),
            Some(&env.key),
            Some(json!({ "model": "text-embedding-3-small", "input": "Hello" })),
        ),
        request(
            "POST",
            "/v1/messages",
            Some(LOCAL),
            Some(&env.key),
            Some(messages("claude-haiku-4-5", "Hello")),
        ),
        request("GET", "/v1/models", Some(APP), Some(&env.key), None),
    ];
    for req in calls {
        let path = req.uri().path().to_owned();
        let origin = req.headers()["origin"].to_str().unwrap().to_owned();
        let r = send(&env, req).await;
        assert_eq!(r.status, StatusCode::OK, "{path}: {}", r.text());
        assert_eq!(
            r.header("access-control-allow-origin"),
            Some(origin.as_str()),
            "{path}"
        );
        assert_eq!(r.header("access-control-expose-headers"), Some(EXPOSED));
        assert_eq!(r.header("access-control-allow-credentials"), None);
        assert_eq!(r.header("access-control-allow-methods"), None);
        assert_eq!(vary(&r.headers), ["origin"], "{path}");
        assert!(r.header("kaeso-request-id").unwrap().starts_with("req_"));
    }

    let r = send(
        &env,
        request(
            "POST",
            "/v1/chat/completions",
            Some(APP),
            None,
            Some(chat("gpt-4o", "Hello")),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.header("access-control-allow-origin"), Some(APP));
    assert_eq!(r.header("access-control-expose-headers"), Some(EXPOSED));
    assert!(r.json()["error"]["message"].is_string());
}

#[tokio::test]
async fn other_origins_get_no_cors_headers_but_vary() {
    let env = env(Some(ORIGINS)).await;
    for origin in [
        OTHER,
        "http://app.example.com",
        "https://app.example.com:8443",
        "null",
    ] {
        for (path, method) in ROUTES {
            let r = send(&env, preflight(path, origin, method)).await;
            assert_eq!(r.status, StatusCode::NO_CONTENT, "{origin} {path}");
            assert!(r.body.is_empty(), "{origin} {path}");
            assert!(cors_headers(&r.headers).is_empty(), "{origin} {path}");
            assert_eq!(vary(&r.headers).first().map(String::as_str), Some("origin"));
        }
        let r = send(
            &env,
            request(
                "POST",
                "/v1/chat/completions",
                Some(origin),
                Some(&env.key),
                Some(chat("gpt-4o", "Hello")),
            ),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert!(cors_headers(&r.headers).is_empty(), "{origin}");
        assert_eq!(vary(&r.headers), ["origin"], "{origin}");
    }
    let r = send(
        &env,
        request("GET", "/v1/models", None, Some(&env.key), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(cors_headers(&r.headers).is_empty());
    assert_eq!(vary(&r.headers), ["origin"]);

    let r = send(
        &env,
        request("OPTIONS", "/v1/chat/completions", Some(APP), None, None),
    )
    .await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(r.header("access-control-allow-origin"), Some(APP));
}

#[tokio::test]
async fn the_platform_api_health_and_unknown_paths_never_get_cors_headers() {
    let env = env(Some(ORIGINS)).await;
    for path in [
        "/platform/v1/me",
        "/platform/v1/orgs",
        "/healthz",
        "/readyz",
        "/v10/models",
        "/",
    ] {
        let r = send(&env, preflight(path, APP, "GET")).await;
        assert!(cors_headers(&r.headers).is_empty(), "{path}");
        assert!(!vary(&r.headers).contains(&"origin".to_owned()), "{path}");
        let r = send(&env, request("GET", path, Some(APP), None, None)).await;
        assert!(cors_headers(&r.headers).is_empty(), "{path}");
        assert!(!vary(&r.headers).contains(&"origin".to_owned()), "{path}");
    }
    let r = env
        .app
        .call_with(
            Method::GET,
            "/me",
            None,
            Some(&env.owner.cookie),
            Some(common::ORIGIN),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(cors_headers(&r.headers).is_empty());
}

#[tokio::test]
async fn preflights_need_no_key_and_are_neither_logged_nor_limited() {
    let capture = Arc::new(Capture::default());
    let limits = Arc::new(Counting::default());
    let env = common::v1::env_hooked(
        Some(capture.clone()),
        Some(limits.clone()),
        Some(&[
            ("KAESO_CORS_ALLOWED_ORIGINS", ORIGINS),
            ("KAESO_KEY_FAILURES_PER_ADDRESS", "1"),
        ]),
    )
    .await;
    let sent = env.mock.received().len();
    for _ in 0..3 {
        for (path, method) in ROUTES {
            let r = send(&env, preflight(path, APP, method)).await;
            assert_eq!(r.status, StatusCode::NO_CONTENT, "{path}");
        }
    }
    assert!(capture.all().is_empty());
    assert_eq!(limits.0.load(Ordering::SeqCst), 0);
    assert_eq!(env.mock.received().len(), sent);

    let r = send(
        &env,
        request(
            "POST",
            "/v1/chat/completions",
            Some(APP),
            Some(&env.key),
            Some(chat("gpt-4o", "Hello")),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("access-control-allow-origin"), Some(APP));
    common::eventually("the request is recorded", || {
        let n = capture.all().len();
        async move { n == 1 }
    })
    .await;
    assert_eq!(limits.0.load(Ordering::SeqCst), 1);
}
