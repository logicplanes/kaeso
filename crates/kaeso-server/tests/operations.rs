#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use kaeso_core::secret;
use kaeso_store::pg::tokens::{self, TokenKind};
use kaeso_testkit::services::TestServices;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

async fn get_json(
    app: Router,
    path: &str,
) -> (StatusCode, serde_json::Value, axum::http::HeaderMap) {
    let res = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap(), headers)
}

#[tokio::test]
async fn health_and_readiness() {
    let s = TestServices::start().await;
    let state = common::state(&s).await;
    state.clickhouse.migrate(&state.pg).await.unwrap();
    let (status, body, headers) =
        get_json(kaeso_server::app::router(state.clone()), "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    let id = headers["kaeso-request-id"].to_str().unwrap();
    assert!(id.starts_with("req_") && id.len() == 30, "{id}");

    let (status, body, _) = get_json(kaeso_server::app::router(state.clone()), "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["clickhouse"], "down");
    s.cleanup().await;
}

#[tokio::test]
async fn clickhouse_down_is_degraded_and_valkey_down_is_not_ready() {
    let s = TestServices::start().await;
    let ch_down = common::state_with(&s, &[("KAESO_CLICKHOUSE_URL", "http://127.0.0.1:1")]).await;
    let (status, body, _) = get_json(kaeso_server::app::router(ch_down), "/readyz").await;
    assert_eq!(
        (status, body["status"].as_str()),
        (StatusCode::OK, Some("degraded"))
    );

    let vk_down = common::state_with(&s, &[("KAESO_VALKEY_URL", "redis://127.0.0.1:1")]).await;
    let (status, body, _) = get_json(kaeso_server::app::router(vk_down), "/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "not_ready");
    assert_eq!(body["valkey"], "down");
    assert_eq!(body["postgres"], "ok");
    s.cleanup().await;
}

#[tokio::test]
async fn setup_links_are_issued_until_a_user_exists() {
    let s = TestServices::start().await;
    let state = common::state(&s).await;
    let first = kaeso_server::setup::issue_link(&state)
        .await
        .unwrap()
        .unwrap();
    assert!(
        first.starts_with("https://kaeso.test/setup#token="),
        "{first}"
    );
    let second = kaeso_server::setup::issue_link(&state)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first, second);
    let token = |url: &str| url.split("#token=").nth(1).unwrap().to_owned();
    let now = state.clock.now();
    assert!(
        tokens::peek(
            &state.pg,
            TokenKind::Setup,
            &secret::hash(&token(&first)),
            now
        )
        .await
        .unwrap()
        .is_none()
    );
    let live = tokens::peek(
        &state.pg,
        TokenKind::Setup,
        &secret::hash(&token(&second)),
        now,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(live.expires_at > now + time::Duration::hours(23));
    let (stored,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM sign_in_tokens WHERE token_hash = $1")
            .bind(token(&second).as_bytes())
            .fetch_one(&state.pg)
            .await
            .unwrap();
    assert_eq!(stored, 0);
    sqlx::query("INSERT INTO users (id, email, name) VALUES (gen_random_uuid(), 'owner@example.com', 'Owner')")
        .execute(&state.pg)
        .await
        .unwrap();
    assert!(
        kaeso_server::setup::issue_link(&state)
            .await
            .unwrap()
            .is_none()
    );
    s.cleanup().await;
}

fn handler(
    started: &Arc<Notify>,
    wait: Duration,
    reply: &'static str,
) -> axum::routing::MethodRouter {
    let started = started.clone();
    get(move || {
        let started = started.clone();
        async move {
            started.notify_one();
            tokio::time::sleep(wait).await;
            reply
        }
    })
}

async fn reached(started: &Notify) {
    tokio::time::timeout(Duration::from_secs(10), started.notified())
        .await
        .expect("the request reached the handler");
}

#[tokio::test]
async fn graceful_shutdown_finishes_open_requests_and_refuses_new_ones() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let started = Arc::new(Notify::new());
    let router = Router::new()
        .route(
            "/slow",
            handler(&started, Duration::from_millis(500), "done"),
        )
        .route(
            "/forever",
            handler(&started, Duration::from_secs(3600), "never"),
        );
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(kaeso_server::serve::serve_until(
        listener,
        router,
        shutdown.clone(),
        Duration::from_secs(2),
    ));
    let client = reqwest::Client::new();
    let in_flight = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .get(format!("http://{addr}/slow"))
                .send()
                .await?
                .text()
                .await
        }
    });
    reached(&started).await;
    shutdown.cancel();
    assert_eq!(in_flight.await.unwrap().unwrap(), "done");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        client
            .get(format!("http://{addr}/slow"))
            .send()
            .await
            .is_err()
    );
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn graceful_shutdown_gives_up_after_the_grace_period() {
    let started = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(kaeso_server::serve::serve_until(
        listener,
        Router::new().route(
            "/forever",
            handler(&started, Duration::from_secs(3600), "never"),
        ),
        shutdown.clone(),
        Duration::from_millis(300),
    ));
    let stuck = tokio::spawn(async move { reqwest::get(format!("http://{addr}/forever")).await });
    reached(&started).await;
    let started = tokio::time::Instant::now();
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(300), "{waited:?}");
    assert!(waited < Duration::from_secs(2), "{waited:?}");
    stuck.abort();
}

#[tokio::test]
async fn metrics_are_exposed() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    metrics::with_local_recorder(&recorder, kaeso_server::metrics::describe);
    let res = kaeso_server::metrics::router(handle)
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let text = String::from_utf8(
        axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let build_info = format!(
        "kaeso_build_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    );
    assert!(text.contains(&build_info), "{text}");
}
