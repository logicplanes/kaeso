mod synth;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
}

impl Received {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

pub const KEEP_RECEIVED: usize = 10_000;

#[derive(Debug, Default)]
struct Shared {
    received: Mutex<VecDeque<Received>>,
    made_up: AtomicU64,
    streams_started: AtomicUsize,
    streams_finished: AtomicUsize,
    streams_dropped: AtomicUsize,
    gates: Mutex<HashMap<String, Arc<GateState>>>,
    gates_made: AtomicU64,
}

impl Shared {
    fn gate(&self, name: &str) -> Arc<GateState> {
        self.gates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(name.to_owned())
            .or_default()
            .clone()
    }
}

#[derive(Debug)]
struct GateState {
    arrived: watch::Sender<usize>,
    open: watch::Sender<bool>,
}

impl Default for GateState {
    fn default() -> Self {
        GateState {
            arrived: watch::Sender::new(0),
            open: watch::Sender::new(false),
        }
    }
}

impl GateState {
    async fn pass(&self) {
        let mut open = self.open.subscribe();
        self.arrived.send_modify(|n| *n += 1);
        let _ = open.wait_for(|open| *open).await;
    }
}

#[derive(Debug)]
pub struct Gate {
    name: String,
    state: Arc<GateState>,
    arrivals: watch::Receiver<usize>,
}

impl Gate {
    pub fn marker(&self) -> String {
        format!("[mock:gate={}]", self.name)
    }

    pub fn arrived(&mut self) -> usize {
        *self.arrivals.borrow_and_update()
    }

    pub async fn next_arrival(&mut self) {
        let _ = self.arrivals.changed().await;
    }

    pub async fn wait_for(&mut self, n: usize) {
        let _ = self.arrivals.wait_for(|arrived| *arrived >= n).await;
    }

    pub fn open(&self) {
        self.state.open.send_replace(true);
    }
}

#[derive(Clone, Debug)]
pub struct MockProvider {
    pub base_url: String,
    shared: Arc<Shared>,
}

#[derive(Clone)]
struct Ctx {
    shared: Arc<Shared>,
    fixtures: PathBuf,
}

impl MockProvider {
    pub async fn start() -> MockProvider {
        MockProvider::start_on("127.0.0.1:0").await
    }

    pub async fn start_on(addr: &str) -> MockProvider {
        let shared = Arc::new(Shared::default());
        let ctx = Ctx {
            shared: shared.clone(),
            fixtures: kpp_fixtures(),
        };
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .unwrap_or_else(|e| panic!("cannot bind the mock provider: {e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("mock provider address: {e}"));
        let app = Router::new()
            .route("/__mock/received", axum::routing::get(control_received))
            .route("/__mock/streams", axum::routing::get(control_streams))
            .route("/__mock/reset", axum::routing::post(control_reset))
            .fallback(handle)
            .with_state(ctx);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockProvider {
            base_url: format!("http://{addr}"),
            shared,
        }
    }

    pub fn received(&self) -> Vec<Received> {
        self.shared
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    pub fn last(&self) -> Option<Received> {
        self.received().pop()
    }

    pub fn clear(&self) {
        self.shared
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    pub fn gate(&self) -> Gate {
        let name = format!(
            "gate-{}",
            self.shared.gates_made.fetch_add(1, Ordering::Relaxed)
        );
        let state = self.shared.gate(&name);
        let arrivals = state.arrived.subscribe();
        Gate {
            name,
            state,
            arrivals,
        }
    }

    pub fn stream_counts(&self) -> (usize, usize, usize) {
        (
            self.shared.streams_started.load(Ordering::SeqCst),
            self.shared.streams_finished.load(Ordering::SeqCst),
            self.shared.streams_dropped.load(Ordering::SeqCst),
        )
    }
}

pub fn kpp_fixtures() -> PathBuf {
    std::env::var_os("KAESO_KPP_FIXTURES").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-codecs/fixtures"),
        PathBuf::from,
    )
}

fn marker<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let start = format!("[mock:{name}");
    let i = body.find(&start)?;
    let rest = body.get(i + start.len()..)?;
    let end = rest.find(']')?;
    Some(rest.get(..end)?.trim_start_matches('='))
}

fn fixture_file(dir: &Path, folder: &str, name: &str, extension: &str) -> PathBuf {
    match name
        .strip_prefix("providers/")
        .and_then(|rest| rest.split_once('/'))
    {
        Some((provider, stem)) => dir
            .join("providers")
            .join(provider)
            .join(folder)
            .join(format!("{stem}.{extension}")),
        None => dir.join(folder).join(format!("{name}.{extension}")),
    }
}

fn error_response(dir: &Path, name: &str) -> Response {
    let body = std::fs::read(fixture_file(dir, "errors", name, "json"))
        .unwrap_or_else(|e| panic!("mock: no error fixture {name}: {e}"));
    let meta: serde_json::Value = std::fs::read(fixture_file(dir, "errors", name, "meta.json"))
        .ok()
        .and_then(|m| serde_json::from_slice(&m).ok())
        .unwrap_or_default();
    let status = meta
        .get("status")
        .and_then(serde_json::Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .unwrap_or(500);
    let mut resp = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", "application/json");
    if let Some(h) = meta.get("headers").and_then(serde_json::Value::as_object) {
        for (k, v) in h {
            if let Some(v) = v.as_str() {
                resp = resp.header(k, v);
            }
        }
    }
    resp.body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn handle(
    State(ctx): State<Ctx>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_owned();
    let text = String::from_utf8_lossy(&body).into_owned();
    {
        let mut received = ctx
            .shared
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if received.len() >= KEEP_RECEIVED {
            received.pop_front();
        }
        received.push_back(Received {
            method: method.to_string(),
            path: path.clone(),
            query: uri.query().map(str::to_owned),
            headers: headers
                .iter()
                .filter_map(|(n, v)| {
                    v.to_str()
                        .ok()
                        .map(|v| (n.as_str().to_owned(), v.to_owned()))
                })
                .collect(),
            body: body.clone(),
        });
    }
    let anthropic = path.starts_with("/v1/messages")
        || (path.starts_with("/v1/models") && headers.contains_key("x-api-key"));
    let dialect = if anthropic {
        "anthropic_messages"
    } else if path.starts_with("/v1/responses") {
        "openai_responses"
    } else if path.starts_with("/v1/embeddings") {
        "openai_embeddings"
    } else {
        "openai_chat"
    };
    let dir = ctx.fixtures.join(dialect);
    let authed = if anthropic {
        headers.get("x-api-key").is_some_and(|v| !v.is_empty())
    } else {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("Bearer ") && v.len() > 7)
    };
    if !authed {
        return error_response(&dir, key_refused(dialect));
    }
    let key = key_text(&headers);
    let asks_models = method == Method::GET && path.starts_with(MODELS);
    let hold =
        marker(&text, "gate").or_else(|| asks_models.then(|| marker(&key, "gate")).flatten());
    if let Some(name) = hold {
        ctx.shared.gate(name).pass().await;
    }
    if asks_models && answers_route_not_found(marker(&key, "models"), &path) {
        return route_not_found();
    }
    if asks_models && path == MODELS && marker(&key, "models") == Some("list-refuses-key") {
        return error_response(&dir, key_refused(dialect));
    }
    if is_unserved(&path, &body) {
        let (dir, name) = match dialect {
            "anthropic_messages" => (dir.clone(), "not-found"),
            "openai_responses" => (dir.clone(), "model-not-found-404"),
            _ => (ctx.fixtures.join("openai_chat"), "model-not-found"),
        };
        return error_response(&dir, name);
    }
    if let Some(name) = marker(&text, "error") {
        return error_response(&dir, name);
    }
    let stream = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if marker(&text, "stall").is_some() && !stream {
        std::future::pending::<()>().await;
    }
    if method == Method::GET {
        let endless = marker(&key, "models") == Some("endless");
        let file = match (path == MODELS, anthropic, uri.query().is_none() || endless) {
            (true, true, true) => "list-page.json",
            (true, _, _) => "list.json",
            (false, _, _) => "retrieve.json",
        };
        let models = if anthropic {
            dir.join("models")
        } else {
            ctx.fixtures.join("openai_chat/models")
        };
        return json_file(&models.join(file));
    }
    let counting = path.ends_with("/count_tokens") || path.ends_with("/input_tokens");
    if !counting
        && dialect != "openai_embeddings"
        && let Some(shape) = synth::Shape::of(&text, body.len())
        && let Ok(id) = dialect.parse::<kaeso_kpp::DialectId>()
    {
        let seed = ctx.shared.made_up.fetch_add(1, Ordering::Relaxed);
        if stream {
            if let Some(chunks) = synth::stream(id, &body, &headers, shape, seed) {
                let cut = marker(&text, "cut").is_some();
                return paced(ctx.shared.clone(), chunks, cut);
            }
        } else if let Some(bytes) = synth::complete(id, &body, &headers, shape, seed) {
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("x-request-id", format!("req_mock_{seed}"))
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }
    let fixture = marker(&text, "fixture").map(str::to_owned);
    if stream {
        let name = fixture.unwrap_or_else(|| match dialect {
            "anthropic_messages" => "text-simple".into(),
            "openai_responses" => "text-basic-stream".into(),
            _ => "stream-include-usage".into(),
        });
        let bytes = std::fs::read(fixture_file(&dir, "streams", &name, "sse"))
            .unwrap_or_else(|e| panic!("mock: no stream fixture {dialect}/{name}: {e}"));
        let mode = if marker(&text, "stall").is_some() {
            StreamMode::Stall
        } else if marker(&text, "cut").is_some() {
            StreamMode::Cut
        } else if let Some(ms) = marker(&text, "slow") {
            StreamMode::Slow(Duration::from_millis(ms.parse().unwrap_or(50)))
        } else if let Some(ms) = marker(&text, "silence").and_then(|m| m.parse::<u64>().ok()) {
            StreamMode::Silence(Duration::from_millis(ms))
        } else {
            StreamMode::Normal
        };
        return sse(ctx.shared.clone(), bytes, mode);
    }
    let file = match (dialect, path.as_str()) {
        ("anthropic_messages", "/v1/messages/count_tokens") => counting_fixture(
            &dir.join("responses/count_tokens"),
            fixture.as_deref(),
            "basic-system-string",
        ),
        ("openai_responses", "/v1/responses/input_tokens") => counting_fixture(
            &dir.join("responses/input_tokens"),
            fixture.as_deref(),
            "string-input",
        ),
        ("openai_embeddings", _) => {
            let default = if text.contains("\"encoding_format\":\"base64\"") {
                "base64-encoding"
            } else {
                "string-input"
            };
            dir.join("responses")
                .join(format!("{}.json", fixture.as_deref().unwrap_or(default)))
        }
        _ => {
            let default = match dialect {
                "anthropic_messages" => "text-simple",
                "openai_responses" => "text-input-string",
                _ => "plain-developer-string",
            };
            fixture_file(
                &dir,
                "responses",
                fixture.as_deref().unwrap_or(default),
                "json",
            )
        }
    };
    json_file(&file)
}

pub const UNSERVED_PREFIX: &str = "unserved-";

const MODELS: &str = "/v1/models";

fn key_text(headers: &HeaderMap) -> String {
    ["x-api-key", "authorization"]
        .into_iter()
        .filter_map(|name| headers.get(name).and_then(|v| v.to_str().ok()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn answers_route_not_found(models: Option<&str>, path: &str) -> bool {
    match models {
        Some("absent") => true,
        Some("list-only" | "list-refuses-key") => path != MODELS,
        _ => false,
    }
}

fn key_refused(dialect: &str) -> &'static str {
    match dialect {
        "anthropic_messages" => "authentication",
        "openai_chat" => "invalid-api-key",
        _ => "invalid-api-key-401",
    }
}

fn route_not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"detail":"Not Found"}"#))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn is_unserved(path: &str, body: &[u8]) -> bool {
    let named = path
        .strip_prefix("/v1/models/")
        .map(str::to_owned)
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(body)
                .ok()?
                .get("model")?
                .as_str()
                .map(str::to_owned)
        });
    named.is_some_and(|m| m.starts_with(UNSERVED_PREFIX))
}

fn counting_fixture(dir: &Path, named: Option<&str>, default: &str) -> PathBuf {
    named
        .map(|n| dir.join(format!("{n}.json")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| dir.join(format!("{default}.json")))
}

fn json_file(path: &Path) -> Response {
    match std::fs::read(path) {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-request-id", "req_mock")
            .body(Body::from(body))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(e) => panic!("mock: no fixture {}: {e}", path.display()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamMode {
    Normal,
    Slow(Duration),
    Stall,
    Cut,
    Silence(Duration),
}

struct Guard {
    shared: Arc<Shared>,
    finished: bool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            self.shared.streams_dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn sse(shared: Arc<Shared>, bytes: Vec<u8>, mode: StreamMode) -> Response {
    shared.streams_started.fetch_add(1, Ordering::SeqCst);
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    let mut events: Vec<Bytes> = text
        .split_inclusive("\n\n")
        .map(|e| Bytes::from(e.to_owned()))
        .collect();
    if mode == StreamMode::Cut {
        let keep = events.len() / 2;
        events.truncate(keep.max(1));
    }
    let guard = Guard {
        shared,
        finished: false,
    };
    let state = (events.into_iter(), guard, 0usize);
    let stream = futures::stream::unfold(state, move |(mut it, mut guard, n)| async move {
        if guard.finished {
            return None;
        }
        if n > 0 {
            match mode {
                StreamMode::Slow(every) => tokio::time::sleep(every).await,
                StreamMode::Cut => tokio::time::sleep(Duration::from_millis(20)).await,
                StreamMode::Stall => std::future::pending::<()>().await,
                StreamMode::Silence(quiet) if n == 1 => tokio::time::sleep(quiet).await,
                _ => {}
            }
        }
        match it.next() {
            Some(chunk) => Some((Ok::<_, std::io::Error>(chunk), (it, guard, n + 1))),
            None => {
                if mode == StreamMode::Cut {
                    guard.finished = true;
                    guard.shared.streams_finished.fetch_add(1, Ordering::SeqCst);
                    return Some((Err(std::io::Error::other("cut")), (it, guard, n + 1)));
                }
                guard.finished = true;
                guard.shared.streams_finished.fetch_add(1, Ordering::SeqCst);
                None
            }
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("x-request-id", "req_mock_stream")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn paced(shared: Arc<Shared>, mut chunks: Vec<(Duration, Bytes)>, cut: bool) -> Response {
    shared.streams_started.fetch_add(1, Ordering::SeqCst);
    if cut {
        chunks.truncate((chunks.len() / 2).max(1));
    }
    let guard = Guard {
        shared,
        finished: false,
    };
    let stream = futures::stream::unfold(
        (chunks.into_iter(), guard),
        move |(mut it, mut guard)| async move {
            if guard.finished {
                return None;
            }
            match it.next() {
                Some((pause, chunk)) => {
                    if !pause.is_zero() {
                        tokio::time::sleep(pause).await;
                    }
                    Some((Ok::<_, std::io::Error>(chunk), (it, guard)))
                }
                None => {
                    guard.finished = true;
                    guard.shared.streams_finished.fetch_add(1, Ordering::SeqCst);
                    cut.then(|| (Err(std::io::Error::other("cut")), (it, guard)))
                }
            }
        },
    );
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("x-request-id", "req_mock_stream")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn control_received(State(ctx): State<Ctx>) -> Response {
    let list: Vec<serde_json::Value> = ctx
        .shared
        .received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|r| {
            serde_json::json!({
                "method": r.method,
                "path": r.path,
                "headers": r.headers.iter().cloned().collect::<std::collections::BTreeMap<_, _>>(),
                "body": serde_json::from_slice::<serde_json::Value>(&r.body)
                    .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&r.body).into_owned())),
            })
        })
        .collect();
    axum::Json(list).into_response()
}

async fn control_streams(State(ctx): State<Ctx>) -> Response {
    axum::Json(serde_json::json!({
        "started": ctx.shared.streams_started.load(Ordering::SeqCst),
        "finished": ctx.shared.streams_finished.load(Ordering::SeqCst),
        "dropped": ctx.shared.streams_dropped.load(Ordering::SeqCst),
    }))
    .into_response()
}

async fn control_reset(State(ctx): State<Ctx>) -> StatusCode {
    ctx.shared
        .received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    for c in [
        &ctx.shared.streams_started,
        &ctx.shared.streams_finished,
        &ctx.shared.streams_dropped,
    ] {
        c.store(0, Ordering::SeqCst);
    }
    StatusCode::NO_CONTENT
}
