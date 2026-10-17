#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::*;
use secrecy::SecretString;
use serde_json::{Value, json};
use tower::ServiceExt;

static TOKEN_TEXT: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| "t".repeat(32));

fn token() -> &'static str {
    TOKEN_TEXT.as_str()
}

async fn call(
    router: &axum::Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn the_api_needs_the_token_except_for_the_health_check() {
    let world = World::new("api-auth", Daemon::install(), Verifier::default()).await;
    let router = kaeso_updater::server::router(world.engine.clone(), SecretString::from(token()));
    let (status, _) = call(&router, Method::GET, "/healthz", None, None).await;
    assert_eq!(status, StatusCode::OK);
    for (method, path) in [
        (Method::GET, "/v1/status"),
        (Method::POST, "/v1/updates"),
        (Method::GET, &format!("/v1/updates/{JOB}")),
    ] {
        let (status, body) = call(&router, method.clone(), path, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        assert!(!body.to_string().contains(token()));
        let (status, _) = call(&router, method, path, Some("u".repeat(32).as_str()), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn an_update_starts_once_and_reports_its_progress() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Starting);
    let world = World::new("api-flow", daemon, Verifier::default()).await;
    let router = kaeso_updater::server::router(world.engine.clone(), SecretString::from(token()));
    let (status, body) = call(&router, Method::GET, "/v1/status", Some(token()), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["protocol"], 1);
    assert_eq!(body["docker"], true);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    let (status, _) = call(
        &router,
        Method::GET,
        &format!("/v1/updates/{JOB}"),
        Some(token()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for bad in [
        json!({ "id": "upd_x", "version": TO }),
        json!({ "id": JOB, "version": "latest" }),
        json!({ "version": TO }),
    ] {
        let (status, _) = call(
            &router,
            Method::POST,
            "/v1/updates",
            Some(token()),
            Some(bad.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (status, body) = call(
        &router,
        Method::POST,
        "/v1/updates",
        Some(token()),
        Some(json!({ "id": JOB, "version": TO })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["state"], "running");
    assert_eq!(body["step"], "verifying");
    let (status, _) = call(
        &router,
        Method::POST,
        "/v1/updates",
        Some(token()),
        Some(json!({ "id": "upd_01k7a3f8m2xq4v9c5r6t8w0yz2", "version": TO })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    world.finished().await;
    let (status, body) = call(
        &router,
        Method::GET,
        &format!("/v1/updates/{JOB}"),
        Some(token()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "rolled_back", "{body}");
    assert_eq!(body["reason"], "unhealthy");
    assert_eq!(body["from"], FROM);
}

#[tokio::test]
async fn an_update_whose_state_cannot_be_saved_is_refused_as_unavailable() {
    let world = World::new("api-state", Daemon::install(), Verifier::default()).await;
    std::fs::remove_dir_all(world.dir.join("state")).unwrap();
    let router = kaeso_updater::server::router(world.engine.clone(), SecretString::from(token()));
    let (status, body) = call(
        &router,
        Method::POST,
        "/v1/updates",
        Some(token()),
        Some(json!({ "id": JOB, "version": TO })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("cannot save the update's state")
    );
}

#[tokio::test]
async fn a_body_larger_than_four_kib_is_refused() {
    let world = World::new("api-size", Daemon::install(), Verifier::default()).await;
    let router = kaeso_updater::server::router(world.engine.clone(), SecretString::from(token()));
    let (status, _) = call(
        &router,
        Method::POST,
        "/v1/updates",
        Some(token()),
        Some(json!({ "id": JOB, "version": TO, "padding": "x".repeat(5000) })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}
