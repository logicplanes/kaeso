use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};

/// What a guardrail was asked: the Standard Webhooks headers and the body.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub path: String,
    pub id: Option<String>,
    pub timestamp: Option<String>,
    pub signature: Option<String>,
    pub raw: Vec<u8>,
    pub body: Value,
}

struct Inner {
    seen: Mutex<Vec<Asked>>,
}

/// A guardrail for tests. `/check` blocks any text with `forbidden`, replaces `secret` with
/// `[REDACTED]` and allows the rest; `?block=` and `?replace=` choose other words. `/slow`
/// answers after three seconds, `/fail` with 500, `/moved` with a redirect to `/check` and
/// `/nonsense` with JSON that is no verdict.
pub struct MockGuardrail {
    pub addr: SocketAddr,
    inner: Arc<Inner>,
}

impl MockGuardrail {
    pub async fn start() -> MockGuardrail {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            seen: Mutex::default(),
        });
        let app = Router::new()
            .route("/check", post(check))
            .route("/slow", post(slow))
            .route("/fail", post(fail))
            .route("/moved", post(moved))
            .route("/nonsense", post(nonsense))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockGuardrail { addr, inner }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    pub fn seen(&self) -> Vec<Asked> {
        self.inner
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

fn remember(inner: &Inner, path: &str, headers: &HeaderMap, raw: &[u8]) -> Value {
    let header = |n: &str| {
        headers
            .get(n)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let body: Value = serde_json::from_slice(raw).unwrap_or(Value::Null);
    inner
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Asked {
            path: path.to_owned(),
            id: header("webhook-id"),
            timestamp: header("webhook-timestamp"),
            signature: header("webhook-signature"),
            raw: raw.to_vec(),
            body: body.clone(),
        });
    body
}

fn texts(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(t)) = o.get("text") {
                out.push(t.clone());
            }
            if let Some(Value::String(a)) = o.get("arguments") {
                out.push(a.clone());
            }
            for (k, child) in o {
                if k != "text" && k != "arguments" {
                    texts(child, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| texts(x, out)),
        _ => {}
    }
}

fn redact(v: &mut Value, word: &str) {
    match v {
        Value::Object(o) => {
            for (k, child) in o.iter_mut() {
                match child {
                    Value::String(s) if k == "text" => *s = s.replace(word, "[REDACTED]"),
                    _ => redact(child, word),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| redact(x, word)),
        _ => {}
    }
}

async fn check(
    State(inner): State<Arc<Inner>>,
    Query(words): Query<HashMap<String, String>>,
    headers: HeaderMap,
    raw: axum::body::Bytes,
) -> Json<Value> {
    let body = remember(&inner, "/check", &headers, &raw);
    let block = words.get("block").map_or("forbidden", String::as_str);
    let replace = words.get("replace").map_or("secret", String::as_str);
    let reply_phase = body.get("phase").and_then(Value::as_str) == Some("reply");
    let checked = if reply_phase {
        body.get("reply").cloned().unwrap_or(Value::Null)
    } else {
        body.get("messages").cloned().unwrap_or(Value::Null)
    };
    let mut found = Vec::new();
    texts(&checked, &mut found);
    if found.iter().any(|t| t.contains(block)) {
        return Json(json!({
            "action": "block",
            "message": "Forbidden words are not allowed",
            "category": "words"
        }));
    }
    if found.iter().any(|t| t.contains(replace)) {
        let mut messages = checked;
        redact(&mut messages, replace);
        let messages = if reply_phase {
            json!([messages])
        } else {
            messages
        };
        return Json(json!({"action": "replace", "category": "secrets", "messages": messages}));
    }
    Json(json!({"action": "allow"}))
}

async fn slow(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    raw: axum::body::Bytes,
) -> Json<Value> {
    remember(&inner, "/slow", &headers, &raw);
    tokio::time::sleep(Duration::from_secs(3)).await;
    Json(json!({"action": "allow"}))
}

async fn fail(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    raw: axum::body::Bytes,
) -> Response {
    remember(&inner, "/fail", &headers, &raw);
    (StatusCode::INTERNAL_SERVER_ERROR, "down").into_response()
}

async fn moved(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    raw: axum::body::Bytes,
) -> Response {
    remember(&inner, "/moved", &headers, &raw);
    (StatusCode::TEMPORARY_REDIRECT, [("location", "/check")], "").into_response()
}

async fn nonsense(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    raw: axum::body::Bytes,
) -> Json<Value> {
    remember(&inner, "/nonsense", &headers, &raw);
    Json(json!({"verdict": "fine"}))
}
