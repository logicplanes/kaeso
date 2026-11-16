//! An OpenTelemetry collector that takes OTLP/HTTP JSON at `/v1/traces` and keeps what it got.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::Value;
use tokio::sync::Notify;

/// One export as the collector got it.
#[derive(Clone, Debug)]
pub struct Export {
    pub headers: HeaderMap,
    pub body: Value,
}

#[derive(Default)]
struct Inner {
    exports: Mutex<Vec<Export>>,
    /// The status it answers with: 200 unless a test sets another.
    status: AtomicU16,
    arrived: Notify,
}

#[derive(Clone)]
pub struct MockCollector {
    pub base_url: String,
    inner: Arc<Inner>,
}

impl MockCollector {
    pub async fn start() -> MockCollector {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            status: AtomicU16::new(200),
            ..Inner::default()
        });
        let app = Router::new()
            .route("/v1/traces", post(traces))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockCollector {
            base_url: format!("http://{addr}"),
            inner,
        }
    }

    /// Answers every export with `status` from now on, such as 503 for a collector that asks
    /// to wait or 400 for one that refuses.
    pub fn answer(&self, status: u16) {
        self.inner.status.store(status, Ordering::SeqCst);
    }

    pub fn exports(&self) -> Vec<Export> {
        self.inner
            .exports
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every span the collector took, in the order they came.
    pub fn spans(&self) -> Vec<Value> {
        self.exports()
            .iter()
            .flat_map(|e| {
                e.body["resourceSpans"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|r| r["scopeSpans"].as_array().into_iter().flatten())
                    .flat_map(|s| s["spans"].as_array().into_iter().flatten())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Waits until the collector has taken at least `n` spans, at most `within`.
    pub async fn wait_for(&self, n: usize, within: Duration) -> Vec<Value> {
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let arrived = self.inner.arrived.notified();
            tokio::pin!(arrived);
            arrived.as_mut().enable();
            let spans = self.spans();
            if spans.len() >= n {
                return spans;
            }
            if tokio::time::timeout_at(deadline, arrived).await.is_err() {
                return self.spans();
            }
        }
    }
}

async fn traces(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    let status = StatusCode::from_u16(inner.status.load(Ordering::SeqCst))
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if !status.is_success() {
        return status.into_response();
    }
    let body: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    inner
        .exports
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Export { headers, body });
    inner.arrived.notify_waiters();
    (
        [("content-type", "application/json")],
        "{\"partialSuccess\":{}}",
    )
        .into_response()
}
