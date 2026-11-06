use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};

pub const MODERN: &str = "2026-07-28";
const LEGACY: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Era {
    Modern,
    Legacy,
    #[default]
    Both,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub era: Era,
    pub token: Option<String>,
    pub stream_calls: bool,
    pub page_size: usize,
    pub tools: Vec<Value>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            era: Era::Both,
            token: None,
            stream_calls: false,
            page_size: 50,
            tools: default_tools(),
        }
    }
}

pub fn default_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "create_post",
            "title": "Create post",
            "description": "Posts a message to X as the signed-in account.",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
            "annotations": { "destructiveHint": true, "openWorldHint": true }
        }),
        json!({
            "name": "search_posts",
            "title": "Search posts",
            "description": "Finds recent posts that match a query.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] },
            "outputSchema": { "type": "object", "properties": { "posts": { "type": "array" } } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "get_me",
            "description": "Shows the signed-in account.",
            "inputSchema": { "type": "object" },
            "annotations": { "readOnlyHint": true },
            "_meta": { "io.modelcontextprotocol/ui": { "resourceUri": "ui://x/me" } }
        }),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub method: String,
    pub protocol_version: Option<String>,
    pub mcp_method: Option<String>,
    pub mcp_name: Option<String>,
    pub session: Option<String>,
    pub authorization: Option<String>,
    pub body: Value,
}

struct Inner {
    options: Options,
    tools: Mutex<Vec<Value>>,
    sessions: Mutex<HashSet<String>>,
    next: AtomicUsize,
    seen: Mutex<Vec<Seen>>,
    base: String,
}

pub struct MockMcp {
    pub addr: SocketAddr,
    inner: Arc<Inner>,
}

impl MockMcp {
    pub async fn start(options: Options) -> MockMcp {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            tools: Mutex::new(options.tools.clone()),
            options,
            sessions: Mutex::default(),
            next: AtomicUsize::new(1),
            seen: Mutex::default(),
            base: format!("http://{addr}"),
        });
        let app = Router::new()
            .route("/mcp", post(handle).get(not_allowed).delete(not_allowed))
            .route("/moved", post(moved).get(moved))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockMcp { addr, inner }
    }

    pub fn url(&self) -> String {
        format!("{}/mcp", self.inner.base)
    }

    pub fn moved_url(&self) -> String {
        format!("{}/moved", self.inner.base)
    }

    pub fn set_tools(&self, tools: Vec<Value>) {
        *self
            .inner
            .tools
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = tools;
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.inner
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn end_sessions(&self) {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

fn header_of(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

async fn not_allowed() -> Response {
    let mut r = StatusCode::METHOD_NOT_ALLOWED.into_response();
    r.headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST"));
    r
}

async fn moved() -> Response {
    let mut r = StatusCode::TEMPORARY_REDIRECT.into_response();
    r.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_static("http://169.254.169.254/latest/meta-data/"),
    );
    r
}

fn json_reply(status: StatusCode, body: &Value, session: Option<&str>) -> Response {
    let mut r = (status, axum::Json(body.clone())).into_response();
    if let Some(s) = session.and_then(|s| HeaderValue::from_str(s).ok()) {
        r.headers_mut().insert("mcp-session-id", s);
    }
    r
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn events(messages: &[Value]) -> Response {
    let mut body = String::new();
    for m in messages {
        body.push_str("event: message\ndata: ");
        body.push_str(&m.to_string());
        body.push_str("\n\n");
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from(body))
        .expect("response")
}

async fn handle(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    if let Some(token) = &inner.options.token {
        let sent = header_of(&headers, "authorization");
        if sent.as_deref() != Some(&format!("Bearer {token}")) {
            let mut r = StatusCode::UNAUTHORIZED.into_response();
            let challenge = format!(
                "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
                inner.base
            );
            if let Ok(v) = HeaderValue::from_str(&challenge) {
                r.headers_mut().insert(header::WWW_AUTHENTICATE, v);
            }
            return r;
        }
    }
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return json_reply(
            StatusCode::BAD_REQUEST,
            &rpc_error(&Value::Null, -32700, "Parse error"),
            None,
        );
    };
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    inner
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Seen {
            method: method.clone(),
            protocol_version: header_of(&headers, "mcp-protocol-version"),
            mcp_method: header_of(&headers, "mcp-method"),
            mcp_name: header_of(&headers, "mcp-name"),
            session: header_of(&headers, "mcp-session-id"),
            authorization: header_of(&headers, "authorization"),
            body: message.clone(),
        });
    let Some(id) = message.get("id").cloned() else {
        return StatusCode::ACCEPTED.into_response();
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let declared = params
        .pointer("/_meta/io.modelcontextprotocol~1protocolVersion")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let era = inner.options.era;
    let server_info = json!({ "name": "mock-x", "version": "2.0.0" });
    if method == "initialize" {
        if era == Era::Modern {
            let mut e = rpc_error(&id, -32022, "Unsupported protocol version");
            e["error"]["data"] =
                json!({ "supported": [MODERN], "requested": params.get("protocolVersion") });
            return json_reply(StatusCode::BAD_REQUEST, &e, None);
        }
        let asked = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = LEGACY.iter().find(|v| **v == asked).unwrap_or(&LEGACY[0]);
        let session = format!("s-{}", inner.next.fetch_add(1, Ordering::SeqCst));
        inner
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.clone());
        let result = json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": server_info,
            "instructions": "Posts to X."
        });
        return json_reply(
            StatusCode::OK,
            &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Some(&session),
        );
    }
    let modern = declared.is_some();
    if modern {
        if era == Era::Legacy {
            return json_reply(
                StatusCode::BAD_REQUEST,
                &rpc_error(
                    &Value::Null,
                    -32000,
                    "Bad Request: No valid session ID provided",
                ),
                None,
            );
        }
        let version_header = header_of(&headers, "mcp-protocol-version");
        if declared.as_deref() != Some(MODERN) {
            let mut e = rpc_error(&id, -32022, "Unsupported protocol version");
            e["error"]["data"] = json!({ "supported": [MODERN], "requested": declared });
            return json_reply(StatusCode::BAD_REQUEST, &e, None);
        }
        if version_header != declared
            || header_of(&headers, "mcp-method").as_deref() != Some(method.as_str())
        {
            return json_reply(
                StatusCode::BAD_REQUEST,
                &rpc_error(&id, -32020, "Header mismatch"),
                None,
            );
        }
    } else if method != "server/discover" || era == Era::Legacy {
        let session = header_of(&headers, "mcp-session-id");
        let known = session.as_ref().is_some_and(|s| {
            inner
                .sessions
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(s)
        });
        if session.is_none() {
            return json_reply(
                StatusCode::BAD_REQUEST,
                &rpc_error(
                    &Value::Null,
                    -32000,
                    "Bad Request: No valid session ID provided",
                ),
                None,
            );
        }
        if !known {
            return json_reply(
                StatusCode::NOT_FOUND,
                &rpc_error(&id, -32001, "Session not found"),
                None,
            );
        }
    }
    let ok = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    match method.as_str() {
        "server/discover" => {
            let supported: Vec<&str> = match era {
                Era::Modern => vec![MODERN],
                _ => std::iter::once(MODERN).chain(LEGACY).collect(),
            };
            json_reply(
                StatusCode::OK,
                &ok(json!({
                    "resultType": "complete",
                    "supportedVersions": supported,
                    "capabilities": { "tools": {} },
                    "_meta": { "io.modelcontextprotocol/serverInfo": server_info },
                    "instructions": "Posts to X.",
                    "ttlMs": 3_600_000,
                    "cacheScope": "public"
                })),
                None,
            )
        }
        "tools/list" => {
            let tools = inner
                .tools
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            let start = params
                .get("cursor")
                .and_then(Value::as_str)
                .and_then(|c| c.parse::<usize>().ok())
                .unwrap_or(0);
            let size = inner.options.page_size.max(1);
            let page: Vec<Value> = tools.iter().skip(start).take(size).cloned().collect();
            let mut result = json!({ "tools": page });
            if start + size < tools.len() {
                result["nextCursor"] = json!((start + size).to_string());
            }
            if modern {
                result["resultType"] = json!("complete");
                result["ttlMs"] = json!(300_000);
                result["cacheScope"] = json!("private");
            }
            json_reply(StatusCode::OK, &ok(result), None)
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let known = inner
                .tools
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .any(|t| t.get("name").and_then(Value::as_str) == Some(name));
            if !known {
                return json_reply(
                    StatusCode::OK,
                    &rpc_error(&id, -32602, &format!("Unknown tool: {name}")),
                    None,
                );
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let mut result = json!({
                "content": [{ "type": "text", "text": format!("{name} ran with {arguments}") }],
                "isError": false
            });
            if modern {
                result["resultType"] = json!("complete");
            }
            if inner.options.stream_calls {
                let progress = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/progress",
                    "params": { "progressToken": "p", "progress": 1, "total": 2 }
                });
                events(&[progress, ok(result)])
            } else {
                json_reply(StatusCode::OK, &ok(result), None)
            }
        }
        "ping" => json_reply(StatusCode::OK, &ok(json!({})), None),
        _ => json_reply(
            StatusCode::NOT_FOUND,
            &rpc_error(&id, -32601, "Method not found"),
            None,
        ),
    }
}
