use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
pub struct OAuth {
    pub cimd: bool,
    pub dcr: bool,
    pub confidential: bool,
    pub iss: bool,
    pub expires_in: i64,
    pub label: Option<String>,
}

impl Default for OAuth {
    fn default() -> OAuth {
        OAuth {
            cimd: true,
            dcr: true,
            confidential: false,
            iss: true,
            expires_in: 3600,
            label: Some("kaesoapp".to_owned()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub era: Era,
    pub token: Option<String>,
    pub oauth: Option<OAuth>,
    pub stream_calls: bool,
    pub page_size: usize,
    pub tools: Vec<Value>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            era: Era::Both,
            token: None,
            oauth: None,
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

struct Code {
    client_id: String,
    redirect_uri: String,
    challenge: String,
    resource: String,
}

#[derive(Default)]
struct Auth {
    codes: HashMap<String, Code>,
    access: HashSet<String>,
    refresh: HashSet<String>,
    clients: HashMap<String, Option<String>>,
    token_requests: Vec<HashMap<String, String>>,
    issued: usize,
}

struct Inner {
    options: Options,
    auth: Mutex<Auth>,
    tools: Mutex<Vec<Value>>,
    sessions: Mutex<HashSet<String>>,
    next: AtomicUsize,
    seen: Mutex<Vec<Seen>>,
    base: String,
    held: AtomicBool,
    release: tokio::sync::Notify,
}

pub struct MockMcp {
    pub addr: SocketAddr,
    inner: Arc<Inner>,
}

impl MockMcp {
    pub async fn start(options: Options) -> MockMcp {
        MockMcp::start_on("127.0.0.1:0", options).await
    }

    pub async fn start_on(addr: &str, options: Options) -> MockMcp {
        let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            held: AtomicBool::new(false),
            release: tokio::sync::Notify::new(),
            tools: Mutex::new(options.tools.clone()),
            options,
            sessions: Mutex::default(),
            auth: Mutex::default(),
            next: AtomicUsize::new(1),
            seen: Mutex::default(),
            base: format!("http://{addr}"),
        });
        let app = Router::new()
            .route("/mcp", post(handle).get(not_allowed).delete(not_allowed))
            .route("/moved", post(moved).get(moved))
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(resource_metadata),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(server_metadata),
            )
            .route("/token", post(token))
            .route("/register", post(register))
            .route("/userinfo", get(userinfo))
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

    // With `stream_calls`, a call's answer waits after its first event until `release`.
    pub fn hold_calls(&self) {
        self.inner.held.store(true, Ordering::SeqCst);
    }

    pub fn release(&self) {
        self.inner.held.store(false, Ordering::SeqCst);
        self.inner.release.notify_waiters();
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.inner
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn base(&self) -> String {
        self.inner.base.clone()
    }

    // Plays the person's browser at the authorization endpoint: approves at once and returns
    // the address the server sends them back to, with the code, the state and the issuer.
    pub fn authorize(&self, authorization_url: &str) -> String {
        let url = url::Url::parse(authorization_url).expect("authorization URL");
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        let get = |k: &str| q.get(k).cloned().unwrap_or_default();
        let code = format!("code-{}", self.inner.next.fetch_add(1, Ordering::SeqCst));
        self.inner.lock_auth().codes.insert(
            code.clone(),
            Code {
                client_id: get("client_id"),
                redirect_uri: get("redirect_uri"),
                challenge: get("code_challenge"),
                resource: get("resource"),
            },
        );
        let mut back = url::Url::parse(&get("redirect_uri")).expect("redirect URI");
        {
            let mut p = back.query_pairs_mut();
            p.append_pair("code", &code)
                .append_pair("state", &get("state"));
            if self.inner.options.oauth.as_ref().is_some_and(|o| o.iss) {
                p.append_pair("iss", &self.inner.base);
            }
        }
        back.to_string()
    }

    pub fn revoke_refresh_tokens(&self) {
        let mut auth = self.inner.lock_auth();
        auth.refresh.clear();
        auth.access.clear();
    }

    pub fn token_requests(&self) -> Vec<HashMap<String, String>> {
        self.inner.lock_auth().token_requests.clone()
    }

    pub fn registered_clients(&self) -> Vec<String> {
        self.inner.lock_auth().clients.keys().cloned().collect()
    }

    pub fn end_sessions(&self) {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

impl Inner {
    fn lock_auth(&self) -> std::sync::MutexGuard<'_, Auth> {
        self.auth.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn challenge(&self) -> Response {
        let mut r = StatusCode::UNAUTHORIZED.into_response();
        let challenge = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
            self.base
        );
        if let Ok(v) = HeaderValue::from_str(&challenge) {
            r.headers_mut().insert(header::WWW_AUTHENTICATE, v);
        }
        r
    }
}

async fn resource_metadata(State(inner): State<Arc<Inner>>) -> Response {
    if inner.options.oauth.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    axum::Json(json!({
        "resource": format!("{}/mcp", inner.base),
        "authorization_servers": [inner.base],
        "scopes_supported": ["posts.read", "posts.write"]
    }))
    .into_response()
}

async fn server_metadata(State(inner): State<Arc<Inner>>) -> Response {
    let Some(o) = &inner.options.oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let b = &inner.base;
    let mut meta = json!({
        "issuer": b,
        "authorization_endpoint": format!("{b}/authorize"),
        "token_endpoint": format!("{b}/token"),
        "userinfo_endpoint": format!("{b}/userinfo"),
        "code_challenge_methods_supported": ["S256"],
        "client_id_metadata_document_supported": o.cimd,
        "authorization_response_iss_parameter_supported": o.iss
    });
    if o.dcr {
        meta["registration_endpoint"] = json!(format!("{b}/register"));
    }
    axum::Json(meta).into_response()
}

fn oauth_error(error: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(json!({ "error": error })),
    )
        .into_response()
}

fn basic_client(headers: &HeaderMap) -> Option<(String, String)> {
    let value = header_of(headers, "authorization")?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    let unescape = |s: &str| {
        url::form_urlencoded::parse(format!("x={s}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    };
    Some((unescape(id), unescape(secret)))
}

async fn token(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let Some(o) = inner.options.oauth.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut auth = inner.lock_auth();
    auth.token_requests.push(form.clone());
    let get = |k: &str| form.get(k).cloned().unwrap_or_default();
    let client_id = get("client_id");
    if let Some(Some(secret)) = auth.clients.get(&client_id) {
        let sent = basic_client(&headers)
            .filter(|(id, _)| *id == client_id)
            .map(|(_, s)| s)
            .or_else(|| form.get("client_secret").cloned());
        if sent.as_deref() != Some(secret.as_str()) {
            return oauth_error("invalid_client");
        }
    }
    match get("grant_type").as_str() {
        "authorization_code" => {
            let Some(code) = auth.codes.remove(&get("code")) else {
                return oauth_error("invalid_grant");
            };
            let verifier = get("code_verifier");
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            if code.client_id != client_id
                || code.redirect_uri != get("redirect_uri")
                || code.challenge != challenge
                || code.resource != get("resource")
            {
                return oauth_error("invalid_grant");
            }
        }
        "refresh_token" => {
            if !auth.refresh.remove(&get("refresh_token")) {
                return oauth_error("invalid_grant");
            }
        }
        _ => return oauth_error("unsupported_grant_type"),
    }
    auth.issued += 1;
    let n = auth.issued;
    let access = format!("at-{n}");
    let refresh = format!("rt-{n}");
    auth.access.insert(access.clone());
    auth.refresh.insert(refresh.clone());
    axum::Json(json!({
        "access_token": access,
        "refresh_token": refresh,
        "token_type": "Bearer",
        "expires_in": o.expires_in
    }))
    .into_response()
}

async fn register(State(inner): State<Arc<Inner>>, body: Bytes) -> Response {
    let Some(o) = inner.options.oauth.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !o.dcr {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        return oauth_error("invalid_client_metadata");
    };
    if request
        .get("application_type")
        .and_then(Value::as_str)
        .is_none()
    {
        return oauth_error("invalid_client_metadata");
    }
    let id = format!("dcr-{}", inner.next.fetch_add(1, Ordering::SeqCst));
    let secret = o.confidential.then(|| format!("secret-{id}"));
    inner.lock_auth().clients.insert(id.clone(), secret.clone());
    let mut answer = json!({ "client_id": id, "redirect_uris": request.get("redirect_uris") });
    if let Some(s) = secret {
        answer["client_secret"] = json!(s);
        answer["token_endpoint_auth_method"] = json!("client_secret_basic");
    }
    (StatusCode::CREATED, axum::Json(answer)).into_response()
}

async fn userinfo(State(inner): State<Arc<Inner>>, headers: HeaderMap) -> Response {
    let Some(o) = inner.options.oauth.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let token = header_of(&headers, "authorization")
        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned))
        .unwrap_or_default();
    if !inner.lock_auth().access.contains(&token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    axum::Json(json!({ "preferred_username": o.label })).into_response()
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

fn held_events(inner: Arc<Inner>, first: Value, last: Value) -> Response {
    let frame = |m: &Value| Bytes::from(format!("event: message\ndata: {m}\n\n"));
    let first = frame(&first);
    let last = frame(&last);
    let rest = async move {
        let released = inner.release.notified();
        if inner.held.load(Ordering::SeqCst) {
            released.await;
        }
        Ok::<_, std::io::Error>(last)
    };
    let body = futures::StreamExt::chain(
        futures::stream::once(async move { Ok::<_, std::io::Error>(first) }),
        futures::stream::once(rest),
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(body))
        .expect("response")
}

async fn handle(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    if inner.options.oauth.is_some() {
        let token = header_of(&headers, "authorization")
            .and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned))
            .unwrap_or_default();
        if !inner.lock_auth().access.contains(&token) {
            return inner.challenge();
        }
    }
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
                if inner.held.load(Ordering::SeqCst) {
                    held_events(inner.clone(), progress, ok(result))
                } else {
                    events(&[progress, ok(result)])
                }
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
