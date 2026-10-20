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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
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
        Duration::from_secs(30),
        CancellationToken::new(),
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
    server.await.unwrap().unwrap();
    let refused = reqwest::Client::new()
        .get(format!("http://{addr}/slow"))
        .send()
        .await
        .unwrap_err();
    assert!(refused.is_connect(), "{refused:?}");
}

#[tokio::test]
async fn graceful_shutdown_gives_up_after_the_grace_period() {
    let started = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let overdue = CancellationToken::new();
    let server = tokio::spawn(kaeso_server::serve::serve_until(
        listener,
        Router::new().route(
            "/forever",
            handler(&started, Duration::from_secs(3600), "never"),
        ),
        shutdown.clone(),
        Duration::from_millis(300),
        Duration::from_secs(30),
        overdue.clone(),
    ));
    let stuck = tokio::spawn(async move { reqwest::get(format!("http://{addr}/forever")).await });
    reached(&started).await;
    let started = tokio::time::Instant::now();
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(300), "{waited:?}");
    assert!(waited < Duration::from_secs(5), "{waited:?}");
    assert!(overdue.is_cancelled(), "open requests are told to end");
    let closed = tokio::time::timeout(Duration::from_secs(5), stuck)
        .await
        .expect("the client sees the connection close")
        .unwrap()
        .unwrap_err();
    assert!(!closed.is_timeout(), "{closed:?}");
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

#[tokio::test]
async fn a_database_outage_answers_503_with_a_clear_message() {
    let s = TestServices::start().await;
    let app = common::App::start(&s).await;
    let owner = app.setup().await;
    let projects = format!("/orgs/{}/projects", owner.org_id);
    assert_eq!(
        app.get(&projects, &owner.cookie).await.status,
        StatusCode::OK
    );
    app.state.pg.close().await;
    let r = app.get(&projects, &owner.cookie).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{:?}", r.body);
    assert_eq!(r.error_type(), "unavailable");
    assert_eq!(
        r.body["error"]["message"],
        "Kaeso cannot reach its database right now; try again shortly."
    );
    s.cleanup().await;
}

#[tokio::test]
async fn shutdown_sends_the_emails_still_queued() {
    let s = TestServices::start().await;
    let mail = kaeso_mail::MemorySink::new();
    mail.fail_next("owner@example.com", 1);
    let state = kaeso_server::serve::prepare_with(
        common::config(
            &s,
            &[
                ("KAESO_LISTEN", "127.0.0.1:0"),
                ("KAESO_METRICS_LISTEN", "127.0.0.1:0"),
            ],
        ),
        kaeso_server::serve::Overrides {
            mail: Some(kaeso_mail::Transport::Memory(mail.clone())),
            ..kaeso_server::serve::Overrides::default()
        },
    )
    .await
    .unwrap();
    state.mailer.send(kaeso_mail::templates::test(
        kaeso_mail::Locale::En,
        "owner@example.com",
    ));
    let metrics = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();
    kaeso_server::serve::run(state, metrics, std::future::ready(()))
        .await
        .unwrap();
    assert_eq!(mail.to("owner@example.com").len(), 1);
    s.cleanup().await;
}

#[tokio::test]
async fn the_operator_resets_two_factor_by_email_address() {
    use kaeso_server::services::users::{OperatorReset, operator_reset_2fa};
    let s = TestServices::start().await;
    let app = common::App::start(&s).await;
    let owner = app.setup().await;
    let user: kaeso_core::id::UserId = owner.user_id.parse().unwrap();
    assert_eq!(
        operator_reset_2fa(&app.state, "nobody@example.com")
            .await
            .unwrap(),
        OperatorReset::UnknownEmail
    );
    kaeso_store::pg::totp::set_pending(&app.state.pg, user.uuid(), &[1], &[2], &[3], "k1")
        .await
        .unwrap();
    kaeso_store::pg::totp::use_step(
        &app.state.pg,
        user.uuid(),
        1,
        Some(time::OffsetDateTime::now_utc()),
    )
    .await
    .unwrap();
    let before = app.mail.to("owner@example.com").len();
    assert_eq!(
        operator_reset_2fa(&app.state, "owner@example.com")
            .await
            .unwrap(),
        OperatorReset::Removed
    );
    assert_eq!(
        app.get("/auth/session", &owner.cookie).await.status,
        StatusCode::UNAUTHORIZED
    );
    app.wait_for_mail("owner@example.com", before + 1).await;
    assert!(
        app.mail
            .to("owner@example.com")
            .iter()
            .any(|m| m.kind.as_str() == "two_factor_reset")
    );
    assert_eq!(
        operator_reset_2fa(&app.state, "owner@example.com")
            .await
            .unwrap(),
        OperatorReset::NothingToRemove
    );
    let cookie = app
        .post(
            "/auth/password",
            serde_json::json!({ "email": "owner@example.com", "password": "correct horse battery" }),
            None,
        )
        .await
        .cookie()
        .unwrap();
    let audit = app
        .get("/instance/audit-log?action=user.2fa_reset", &cookie)
        .await;
    let events = audit.body["data"].as_array().unwrap();
    assert_eq!(events.len(), 2, "{:?}", audit.body);
    assert_eq!(events[0]["actor"]["kind"], "system");
    s.cleanup().await;
}

const HEADER_TIMEOUT: Duration = Duration::from_millis(300);

async fn serving(
    router: Router,
    header_read_timeout: Duration,
) -> (std::net::SocketAddr, CancellationToken) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(kaeso_server::serve::serve_until(
        listener,
        router,
        shutdown.clone(),
        Duration::from_secs(1),
        header_read_timeout,
        CancellationToken::new(),
    ));
    (addr, shutdown)
}

async fn until_closed(stream: &mut TcpStream) -> (Vec<u8>, Duration) {
    let started = tokio::time::Instant::now();
    let mut received = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut received))
        .await
        .expect("the gateway closed the connection")
        .ok();
    (received, started.elapsed())
}

#[tokio::test]
async fn a_client_that_never_finishes_its_headers_is_disconnected() {
    let (addr, shutdown) = serving(
        Router::new().route("/", get(|| async { "hi" })),
        HEADER_TIMEOUT,
    )
    .await;
    let mut stalled = TcpStream::connect(addr).await.unwrap();
    stalled
        .write_all(b"GET / HTTP/1.1\r\nHost: kaeso.test\r\nX-Slow: ")
        .await
        .unwrap();
    let (received, waited) = until_closed(&mut stalled).await;
    assert!(waited >= HEADER_TIMEOUT, "{waited:?}");
    assert!(
        !String::from_utf8_lossy(&received).contains("200 OK"),
        "{received:?}"
    );
    shutdown.cancel();
}

#[tokio::test]
async fn a_client_that_sends_nothing_is_disconnected() {
    let (addr, shutdown) = serving(
        Router::new().route("/", get(|| async { "hi" })),
        HEADER_TIMEOUT,
    )
    .await;
    let mut silent = TcpStream::connect(addr).await.unwrap();
    let (received, waited) = until_closed(&mut silent).await;
    assert!(received.is_empty(), "{received:?}");
    assert!(waited >= HEADER_TIMEOUT, "{waited:?}");
    shutdown.cancel();
}

#[tokio::test]
async fn a_client_that_stalls_in_the_http2_preface_is_disconnected() {
    let (addr, shutdown) = serving(
        Router::new().route("/", get(|| async { "hi" })),
        HEADER_TIMEOUT,
    )
    .await;
    let mut stalled = TcpStream::connect(addr).await.unwrap();
    stalled
        .write_all(b"PRI * HTTP/2.0\r\n\r\nSM")
        .await
        .unwrap();
    let (received, waited) = until_closed(&mut stalled).await;
    assert!(received.is_empty(), "{received:?}");
    assert!(waited >= HEADER_TIMEOUT, "{waited:?}");
    shutdown.cancel();
}

#[tokio::test]
async fn an_idle_keep_alive_connection_is_closed_after_the_timeout() {
    let (addr, shutdown) = serving(
        Router::new().route("/", get(|| async { "hi" })),
        HEADER_TIMEOUT,
    )
    .await;
    let mut client = TcpStream::connect(addr).await.unwrap();
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: kaeso.test\r\n\r\n")
        .await
        .unwrap();
    let (received, waited) = until_closed(&mut client).await;
    let text = String::from_utf8_lossy(&received);
    assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
    assert!(text.ends_with("hi"), "{text}");
    assert!(waited >= HEADER_TIMEOUT, "{waited:?}");
    shutdown.cancel();
}

#[tokio::test]
async fn a_request_may_run_longer_than_the_header_timeout() {
    let started = Arc::new(Notify::new());
    let (addr, shutdown) = serving(
        Router::new().route(
            "/slow",
            handler(&started, Duration::from_millis(900), "done"),
        ),
        HEADER_TIMEOUT,
    )
    .await;
    let reply = reqwest::get(format!("http://{addr}/slow"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(reply, "done");
    shutdown.cancel();
}

#[tokio::test]
async fn clients_are_told_apart_by_their_address() {
    let router =
        Router::new().route(
            "/",
            get(
                |axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<
                    std::net::SocketAddr,
                >| async move { peer.ip().to_string() },
            ),
        );
    let (addr, shutdown) = serving(router, Duration::from_secs(30)).await;
    let reply = reqwest::get(format!("http://{addr}/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(reply, "127.0.0.1");
    shutdown.cancel();
}

#[tokio::test]
async fn http2_clients_with_prior_knowledge_are_served() {
    let (addr, shutdown) = serving(
        Router::new().route("/", get(|| async { "hi" })),
        Duration::from_secs(30),
    )
    .await;
    let reply = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .unwrap();
    assert_eq!(reply.version(), reqwest::Version::HTTP_2);
    assert_eq!(reply.text().await.unwrap(), "hi");
    shutdown.cancel();
}
