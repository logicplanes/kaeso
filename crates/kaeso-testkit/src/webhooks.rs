//! A webhook receiver that keeps every request it gets and answers with the status a test sets.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;

/// One request as the receiver got it.
#[derive(Clone, Debug)]
pub struct Delivered {
    pub headers: HeaderMap,
    pub body: String,
}

impl Delivered {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Default)]
struct Inner {
    got: Mutex<Vec<Delivered>>,
    status: AtomicU16,
}

#[derive(Clone)]
pub struct MockReceiver {
    /// The receiver's address, for a webhook's URL: `http://127.0.0.1:<port>/hook`.
    pub url: String,
    inner: Arc<Inner>,
}

impl MockReceiver {
    pub async fn start() -> MockReceiver {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            status: AtomicU16::new(204),
            ..Inner::default()
        });
        let app = Router::new()
            .route("/hook", post(hook))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockReceiver {
            url: format!("http://{addr}/hook"),
            inner,
        }
    }

    /// Answers every request with `status` from now on, such as 503 for a receiver that is down.
    pub fn answer(&self, status: u16) {
        self.inner.status.store(status, Ordering::SeqCst);
    }

    /// Every request it got, failed ones included, in order.
    pub fn got(&self) -> Vec<Delivered> {
        self.inner
            .got
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

async fn hook(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    inner
        .got
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Delivered {
            headers,
            body: String::from_utf8_lossy(&body).into_owned(),
        });
    StatusCode::from_u16(inner.status.load(Ordering::SeqCst))
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        .into_response()
}
