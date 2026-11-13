mod synth;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use kaeso_codecs::gemini::{GeminiAction, split_model_action};
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
    google_tokens: AtomicUsize,
    streams_started: AtomicUsize,
    streams_finished: AtomicUsize,
    streams_dropped: AtomicUsize,
    drops: tokio::sync::Notify,
    closed: Mutex<HashMap<String, Instant>>,
    text_sent: Mutex<HashMap<String, u64>>,
    abandoned: AtomicUsize,
    gates: Mutex<HashMap<String, Arc<GateState>>>,
    gates_made: AtomicU64,
    video_polls: crate::mock_videos::Polls,
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
    steps: watch::Sender<usize>,
}

impl Default for GateState {
    fn default() -> Self {
        GateState {
            arrived: watch::Sender::new(0),
            open: watch::Sender::new(false),
            steps: watch::Sender::new(0),
        }
    }
}

impl GateState {
    async fn step(&self, n: usize) {
        let mut steps = self.steps.subscribe();
        let _ = steps.wait_for(|s| *s >= n).await;
    }

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

    pub fn steps_marker(&self) -> String {
        format!("[mock:steps={}]", self.name)
    }

    pub fn step(&self) {
        self.state.steps.send_modify(|n| *n += 1);
    }

    pub fn finish(&self) {
        self.state.steps.send_replace(usize::MAX);
    }
}

#[derive(Clone, Debug)]
pub struct MockProvider {
    pub base_url: String,
    shared: Arc<Shared>,
}

#[derive(Clone)]
pub(crate) struct Ctx {
    shared: Arc<Shared>,
    fixtures: PathBuf,
}

impl Ctx {
    pub(crate) fn record(&self, r: Received) {
        let mut list = self
            .shared
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if list.len() >= KEEP_RECEIVED {
            list.pop_front();
        }
        list.push_back(r);
    }
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
            .route(
                "/v1/text-to-speech/{voice}/stream-input",
                axum::routing::get(crate::mock_realtime::socket),
            )
            .route(
                "/v1/text-to-speech/{voice}/multi-stream-input",
                axum::routing::get(crate::mock_realtime::socket),
            )
            .route(
                "/v1/text-to-dialogue/stream-input",
                axum::routing::get(crate::mock_realtime::socket),
            )
            .route(
                "/v1/text-to-dialogue/multi-stream-input",
                axum::routing::get(crate::mock_realtime::socket),
            )
            .route(
                "/v1/speech-to-text/realtime",
                axum::routing::get(crate::mock_realtime::socket),
            )
            .fallback(handle)
            // Anthropic takes request bodies up to 32 MB, more than axum's default.
            .layer(axum::extract::DefaultBodyLimit::max(64 << 20))
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

    pub fn abandoned(&self) -> usize {
        self.shared.abandoned.load(Ordering::SeqCst)
    }

    pub async fn wait_for_dropped_streams(&self, n: usize) {
        loop {
            let dropped = self.shared.drops.notified();
            tokio::pin!(dropped);
            dropped.as_mut().enable();
            if self.shared.streams_dropped.load(Ordering::SeqCst) >= n {
                return;
            }
            dropped.await;
        }
    }

    pub fn closed_at(&self, tag: &str) -> Option<Instant> {
        self.shared
            .closed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(tag)
            .copied()
    }

    // The reply text an endless stream with this tag has sent so far.
    pub fn text_sent(&self, tag: &str) -> u64 {
        self.shared
            .text_sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(tag)
            .copied()
            .unwrap_or_default()
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

// `[mock:fail=<model> <fixture>]` fails only the requests for that model, so one request
// can fail on one model and succeed on the next; a body may carry several. The fixture
// `stall` never answers.
fn failure_for<'a>(body: &'a str, model: &str) -> Option<&'a str> {
    body.match_indices("[mock:fail=").find_map(|(i, start)| {
        let rest = body.get(i + start.len()..)?;
        let (named, fixture) = rest.get(..rest.find(']')?)?.split_once(' ')?;
        (named == model).then_some(fixture)
    })
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

struct Answering {
    shared: Arc<Shared>,
    answered: bool,
}

impl Drop for Answering {
    fn drop(&mut self) {
        if !self.answered {
            self.shared.abandoned.fetch_add(1, Ordering::SeqCst);
        }
    }
}

async fn handle(
    State(ctx): State<Ctx>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut answering = Answering {
        shared: ctx.shared.clone(),
        answered: false,
    };
    let response = answer(ctx, method, uri, headers, body).await;
    answering.answered = true;
    response
}

async fn answer(ctx: Ctx, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
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
    if method == Method::POST && path == GOOGLE_TOKEN_PATH {
        return google_token(&ctx.shared, &text);
    }
    let platform = agent_platform_path(&path);
    if let Some(p) = &platform {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with(&format!("Bearer {GOOGLE_TOKEN_PREFIX}")));
        let keyed = headers.get("x-goog-api-key").is_some_and(|v| !v.is_empty())
            && !headers.contains_key("authorization");
        let allowed = if p.project { bearer } else { keyed };
        if !allowed {
            return error_response(&ctx.fixtures.join("gemini"), "recorded-error-invalid-key");
        }
    }
    let path = platform.map_or(path, |p| p.as_ai_studio);
    // Cloudflare's REST API names the account: `/client/v4/accounts/{id}/ai/v1/…` serves the
    // dialects' `/v1/…` paths, and `…/ai/models/search` its Workers AI model search.
    let path =
        match cloudflare_path(&path) {
            Some(CloudflarePath::ModelSearch) if method == Method::GET => {
                return json_file(&ctx.fixtures.join(
                    "openai_chat/providers/cloudflare-ai-gateway/account/search-recorded.json",
                ));
            }
            Some(CloudflarePath::Api(rest)) => rest,
            _ => path,
        };
    if method == Method::GET && path == format!("/api/v1/tasks/{DASHSCOPE_IMAGE_TASK}") {
        return dashscope_json(&ctx.fixtures, "polls/images/succeeded.json")
            .unwrap_or_else(|| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    if let Some(resp) = crate::mock_videos::answer(
        &ctx.shared.video_polls,
        &ctx.fixtures,
        &method,
        &path,
        uri.query(),
        &headers,
        &text,
    ) {
        return resp;
    }
    let headers = if headers.contains_key("authorization") && path.starts_with(GEMINI_MODELS) {
        let mut h = headers;
        h.remove("authorization");
        h.insert(
            "x-goog-api-key",
            axum::http::HeaderValue::from_static("mock"),
        );
        h
    } else {
        headers
    };
    let anthropic = path.starts_with("/v1/messages")
        || (path.starts_with("/v1/models") && headers.contains_key("x-api-key"));
    let gemini = gemini_call(&path);
    let decision = crate::mock_decisions::dialect(&path, &body);
    let dialect = if let Some((_, action)) = &gemini {
        action.dialect().as_str()
    } else if let Some(d) = decision {
        d.as_str()
    } else if path.starts_with(GEMINI_MODELS) {
        "gemini"
    } else if anthropic {
        "anthropic_messages"
    } else if path.starts_with("/v1/responses") {
        "openai_responses"
    } else if path.starts_with("/v1/embeddings") {
        "openai_embeddings"
    } else if path == "/v1/image_generation" {
        "minimax_images"
    } else if path.starts_with("/v1/images") || path.starts_with("/api/paas/v4/images") {
        "openai_images"
    } else {
        "openai_chat"
    };
    let dir = ctx.fixtures.join(dialect);
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer ") && v.len() > 7);
    // Anthropic takes `x-api-key`; Anthropic-compatible servers such as Vercel AI Gateway
    // also take a Bearer token.
    let authed = if dialect.starts_with("gemini") {
        headers.get("x-goog-api-key").is_some_and(|v| !v.is_empty())
            && !headers.contains_key("authorization")
    } else {
        bearer
            || anthropic && headers.get("x-api-key").is_some_and(|v| !v.is_empty())
            || headers.get("xi-api-key").is_some_and(|v| !v.is_empty())
    };
    if !authed && dialect.starts_with("gemini") && decision.is_none() {
        return error_response(&ctx.fixtures.join("gemini"), key_refused(dialect));
    }
    if !authed {
        return error_response(&dir, key_refused(dialect));
    }
    let key = key_text(&headers);
    let asks_models =
        method == Method::GET && (path.starts_with(MODELS) || path.starts_with(GEMINI_MODELS));
    let hold =
        marker(&text, "gate").or_else(|| asks_models.then(|| marker(&key, "gate")).flatten());
    if let Some(name) = hold {
        ctx.shared.gate(name).pass().await;
    }
    if asks_models && answers_route_not_found(marker(&key, "models"), &path) {
        return route_not_found();
    }
    if asks_models
        && (path == MODELS || path == GEMINI_MODELS)
        && marker(&key, "models") == Some("list-refuses-key")
    {
        return error_response(&dir, key_refused(dialect));
    }
    if is_unserved(&path, &body) {
        let (dir, name) = match dialect {
            "anthropic_messages" => (dir.clone(), "not-found"),
            "gemini" | "gemini_embeddings" => {
                (ctx.fixtures.join("gemini"), "recorded-error-unknown-model")
            }
            "openai_responses" => (dir.clone(), "model-not-found-404"),
            _ => (ctx.fixtures.join("openai_chat"), "model-not-found"),
        };
        return error_response(&dir, name);
    }
    if let Some(name) = marker(&text, "error") {
        return error_response(&dir, name);
    }
    if method == Method::POST
        && let Some(resp) = dashscope(&ctx.fixtures, &path, &text, &headers)
    {
        return resp;
    }
    if method == Method::POST
        && let Some(resp) = audio(&ctx.fixtures, &path, &text)
    {
        return resp;
    }
    if method == Method::GET && (path == "/v1/voices" || path == "/v2/voices") {
        return json_file(&ctx.fixtures.join("elevenlabs/voices/recorded-search.json"));
    }
    let asked = match &gemini {
        Some((model, _)) => Some(model.clone()),
        None => serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| {
                v.get("model")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            }),
    };
    if let Some(name) = asked.as_deref().and_then(|m| failure_for(&text, m)) {
        if name == "stall" {
            std::future::pending::<()>().await;
        }
        return error_response(&dir, name);
    }
    let stream = match &gemini {
        Some((_, action)) => *action == GeminiAction::StreamGenerateContent,
        None => serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("stream").and_then(serde_json::Value::as_bool))
            .unwrap_or_else(|| text.contains("name=\"stream\"\r\n\r\ntrue")),
    };
    if marker(&text, "stall").is_some() && !stream {
        std::future::pending::<()>().await;
    }
    if method == Method::GET && dialect == "gemini" {
        return gemini_models(&dir.join("models"), &path, uri.query());
    }
    if let Some(d) = decision
        && method == Method::POST
        && marker(&text, "fixture").is_none()
    {
        return crate::mock_decisions::answer(d, &body, &headers);
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
    let counting = path.ends_with("/count_tokens")
        || path.ends_with("/input_tokens")
        || path.ends_with(":countTokens");
    let tag = marker(&text, "tag").map(str::to_owned);
    if !counting
        && dialect != "openai_embeddings"
        && decision.is_none()
        && let Some(shape) = synth::Shape::of(&text, body.len())
        && let Ok(id) = dialect.parse::<kaeso_kpp::DialectId>()
    {
        let seed = ctx.shared.made_up.fetch_add(1, Ordering::Relaxed);
        if stream && marker(&text, "endless").is_some() {
            if let Some((head, rest)) = synth::endless(id, &body, &headers, shape, seed) {
                return endless(ctx.shared.clone(), head, rest, tag);
            }
        } else if stream {
            if let Some(chunks) = synth::stream(id, &body, &headers, shape, seed) {
                let cut = marker(&text, "cut").is_some();
                return paced(ctx.shared.clone(), chunks, cut, tag);
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
            "gemini" => "recorded-stream-text".into(),
            "openai_images" if path.ends_with("/edits") => "edit-every-field".into(),
            "openai_images" => "generate".into(),
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
        let steps = marker(&text, "steps").map(|name| ctx.shared.gate(name));
        return sse(ctx.shared.clone(), bytes, mode, steps, tag);
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
        ("gemini", _) if counting => counting_fixture(
            &dir.join("responses/count_tokens"),
            fixture.as_deref(),
            "recorded-contents",
        ),
        ("gemini_embeddings", _) => {
            let default = if gemini.is_some_and(|(_, a)| a == GeminiAction::BatchEmbedContents) {
                "recorded-batch"
            } else {
                "recorded-embed-single"
            };
            fixture_file(
                &dir,
                "responses",
                fixture.as_deref().unwrap_or(default),
                "json",
            )
        }
        ("openai_embeddings", _) => {
            let default = if text.contains("\"encoding_format\":\"base64\"") {
                "base64-encoding"
            } else {
                "string-input"
            };
            fixture_file(
                &dir,
                "responses",
                fixture.as_deref().unwrap_or(default),
                "json",
            )
        }
        _ => {
            let default = match dialect {
                "anthropic_messages" => "text-simple",
                "openai_responses" => "text-input-string",
                "gemini" if text.contains("\"responseModalities\":[\"AUDIO\"]") => {
                    "recorded-speech-mp3"
                }
                "gemini" if text.contains("\"audioTranscriptionConfig\"") => {
                    "recorded-transcribe-diarized"
                }
                "gemini" => "recorded-text-simple",
                "openai_images" => "generate",
                "minimax_images" => "generate-urls",
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

fn bytes_file(path: &Path, content_type: &str) -> Response {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("mock: cannot read {}: {e}", path.display()));
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// `[mock:audio-cut=<n>]`: 200 with the first `n` bytes of the audio, then the connection
// breaks, as a provider that fails while it streams audio.
fn cut_audio(path: &Path, n: usize) -> Response {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("mock: cannot read {}: {e}", path.display()));
    let head = Bytes::copy_from_slice(bytes.get(..n).unwrap_or(&bytes));
    // The break waits until the bytes before it are on the wire; breaking at once would drop
    // them with the connection.
    let chunks = futures::stream::unfold(Some(head), |head| async move {
        match head {
            Some(head) if !head.is_empty() => Some((Ok(head), Some(Bytes::new()))),
            Some(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Some((Err(std::io::Error::other("cut")), None))
            }
            None => None,
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "audio/mpeg")
        .body(Body::from_stream(chunks))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn form_value<'a>(form: &'a str, name: &str) -> Option<&'a str> {
    let head = format!("name=\"{name}\"\r\n\r\n");
    let start = form.find(&head)? + head.len();
    let rest = form.get(start..)?;
    rest.get(..rest.find("\r\n")?)
}

// Speech and transcription paths of every provider form Kaeso speaks, answered from the
// `openai_speech`, `openai_transcriptions` and `minimax_speech` fixtures.
/// The mock's link for files a DashScope reply names, which is not one of the hosts Kaeso
/// fetches QwenCloud's results from.
pub const DASHSCOPE_LINK: &str = "https://files.example.com/dashscope/result.png";

// QwenCloud's DashScope API: images, Qwen3-TTS and Qwen-Audio-ASR share one path, told apart
// by the model; Qwen-Audio-TTS streams from its own. Links in replies point away from
// QwenCloud's result hosts, so a test never reaches them.
/// The task the mock's Wan text-to-image tasks are started as.
pub const DASHSCOPE_IMAGE_TASK: &str = "7d4ac4b2-2e1a-4c84-8c6d-xxxxxx";

fn dashscope_json(fixtures: &Path, rel: &str) -> Option<Response> {
    let bytes = std::fs::read(fixtures.join("dashscope").join(rel)).ok()?;
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    point_links_at(&mut v, DASHSCOPE_LINK);
    Response::builder()
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&v).ok()?))
        .ok()
}

fn dashscope(fixtures: &Path, path: &str, text: &str, headers: &HeaderMap) -> Option<Response> {
    let dir = fixtures.join("dashscope");
    let sse = headers
        .get("x-dashscope-sse")
        .is_some_and(|v| v.as_bytes() == b"enable");
    let local = |rel: &str| dashscope_json(fixtures, rel);
    match path {
        "/api/v1/services/aigc/text2image/image-synthesis"
            if headers
                .get("x-dashscope-async")
                .is_some_and(|v| v.as_bytes() == b"enable") =>
        {
            local("polls/images/started.json")
        }
        "/api/v1/services/audio/tts/SpeechSynthesizer" if text.contains("\"text_prompt\"") => {
            local("responses/speech/qwen-audio-next-link.json")
        }
        "/api/v1/services/audio/tts/SpeechSynthesizer" => Some(bytes_file(
            &dir.join("streams/speech/qwen-audio-tts.sse"),
            "text/event-stream",
        )),
        "/api/v1/services/aigc/multimodal-generation/generation" => {
            if text.contains("\"qwen3-tts") && sse {
                Some(bytes_file(
                    &dir.join("streams/speech/qwen3-tts-pcm.sse"),
                    "text/event-stream",
                ))
            } else if text.contains("\"qwen3-tts") {
                local("responses/speech/qwen3-tts-link.json")
            } else if text.contains("-asr-") {
                local("responses/transcriptions/asr.json")
            } else if text.contains("\"z-image") {
                local("responses/images/z-image.json")
            } else if text.contains("\"wan2.7") {
                local("responses/images/wan27.json")
            } else if text.contains("\"wan2.6-t2i") {
                local("responses/images/wan26-t2i.json")
            } else if text.contains("\"image\"") {
                local("responses/images/qwen-image-edit.json")
            } else {
                local("responses/images/qwen-image-generate.json")
            }
        }
        _ => None,
    }
}

fn point_links_at(v: &mut serde_json::Value, link: &str) {
    match v {
        serde_json::Value::String(s) if s.starts_with("http") && s.contains("dashscope-result") => {
            *s = link.to_owned();
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|i| point_links_at(i, link)),
        serde_json::Value::Object(map) => map.values_mut().for_each(|i| point_links_at(i, link)),
        _ => {}
    }
}

fn audio(fixtures: &Path, path: &str, text: &str) -> Option<Response> {
    let speech = fixtures.join("openai_speech");
    let transcripts = fixtures.join("openai_transcriptions");
    let json = |dir: &Path, name: &str| Some(json_file(&dir.join(format!("{name}.json"))));
    let events = |file: PathBuf| Some(bytes_file(&file, "text/event-stream"));
    let elevenlabs = fixtures.join("elevenlabs/responses");
    let el_audio = |name: &str, ctype: &str| {
        let mut r = bytes_file(&elevenlabs.join(name), ctype);
        r.headers_mut()
            .insert("character-cost", axum::http::HeaderValue::from_static("2"));
        Some(r)
    };
    if path.starts_with("/v1/text-to-speech/") || path.starts_with("/v1/text-to-dialogue") {
        return if path.ends_with("/stream/with-timestamps") {
            Some(bytes_file(
                &fixtures.join("elevenlabs/streams/recorded-speech-stream-with-timestamps.ndjson"),
                "application/json",
            ))
        } else if path.ends_with("/with-timestamps") {
            json(&elevenlabs, "recorded-speech-with-timestamps")
        } else if path.ends_with("/stream") {
            el_audio("recorded-speech-stream.pcm", "audio/pcm")
        } else {
            el_audio("recorded-speech.mp3", "audio/mpeg")
        };
    }
    if path.starts_with("/v1/voice-changer/") {
        return el_audio("recorded-voice-change.mp3", "audio/mpeg");
    }
    match path {
        "/v1/sound-generation" => el_audio("recorded-sound.mp3", "audio/mpeg"),
        "/v1/music" | "/v1/music/stream" => Some(error_response(
            &fixtures.join("elevenlabs"),
            "recorded-music-402",
        )),
        "/v1/speech-to-text" => json(&elevenlabs, "recorded-transcribe"),
        "/v1/t2a_v2" => json(&fixtures.join("minimax_speech/responses"), "speech"),
        "/v4/ai/speech-model" => json(&speech.join("providers/vercel/responses"), "speech"),
        "/v1/audio/speech" | "/v1/tts" => {
            if let Some(n) = marker(text, "audio-cut").and_then(|n| n.parse::<usize>().ok()) {
                return Some(cut_audio(&speech.join("responses/speech.mp3"), n));
            }
            let mistral = text.contains("\"voice_id\"") && !path.ends_with("/tts");
            if mistral && text.contains("\"stream\":true") {
                events(speech.join("providers/mistral/streams/stream-speech.sse"))
            } else if mistral {
                json(&speech.join("providers/mistral/responses"), "speech")
            } else if text.contains("\"stream_format\":\"sse\"") {
                events(speech.join("streams/stream-speech.sse"))
            } else {
                Some(bytes_file(
                    &speech.join("responses/speech.mp3"),
                    "audio/mpeg",
                ))
            }
        }
        "/v4/ai/transcription-model" => {
            json(&transcripts.join("providers/vercel/responses"), "json")
        }
        "/v1/stt" => json(&transcripts.join("providers/xai/responses"), "json"),
        "/v2/audio/transcriptions" => json(&transcripts.join("providers/cohere/responses"), "json"),
        "/api/paas/v4/audio/transcriptions" => {
            json(&transcripts.join("providers/zai/responses"), "json")
        }
        "/v1/speech_to_text" => json(&transcripts.join("providers/minimax/responses"), "json"),
        "/v1/audio/transcriptions" | "/v1/audio/translations" => {
            if form_value(text, "stream") == Some("true") {
                return events(transcripts.join("streams/stream-transcribe.sse"));
            }
            let plain = |name: &str| {
                Some(bytes_file(
                    &transcripts.join(format!("responses/{name}")),
                    "text/plain",
                ))
            };
            match form_value(text, "response_format") {
                Some("text") => plain("text.txt"),
                Some("srt") => plain("subtitles.srt"),
                Some("vtt") => plain("subtitles.vtt"),
                Some("verbose_json") => json(&transcripts.join("responses"), "verbose-segments"),
                Some("diarized_json") => json(&transcripts.join("responses"), "diarized"),
                _ if path.ends_with("/translations") => {
                    json(&transcripts.join("responses"), "translation")
                }
                _ => json(&transcripts.join("responses"), "json"),
            }
        }
        _ => None,
    }
}

pub const UNSERVED_PREFIX: &str = "unserved-";

/// Where the mock answers Google's OAuth token endpoint; point the gateway's Google token
/// endpoint at `{base_url}/token`.
pub const GOOGLE_TOKEN_PATH: &str = "/token";

/// How the access tokens the mock's token endpoint grants start.
pub const GOOGLE_TOKEN_PREFIX: &str = "ya29.mock-";

struct PlatformPath {
    project: bool,
    as_ai_studio: String,
}

// Gemini Enterprise Agent Platform serves Gemini models under
// `/v1/publishers/google/models` with an express mode key and under
// `/v1/projects/{project}/locations/{location}/publishers/google/models` with a token; the
// mock answers both like AI Studio's `/v1beta/models`. With a token it also serves Claude at
// `…/publishers/anthropic/models/{model}:rawPredict` (`:streamRawPredict`,
// `count-tokens:rawPredict`) and other models at `…/endpoints/openapi/chat/completions`,
// which the mock answers like Anthropic's and OpenAI's own paths. Gemini Omni's interactions,
// at `/v1beta1/projects/{project}/locations/global/interactions`, answer like AI Studio's.
enum CloudflarePath {
    Api(String),
    ModelSearch,
}

fn cloudflare_path(path: &str) -> Option<CloudflarePath> {
    let rest = path.strip_prefix("/client/v4/accounts/")?;
    let (account, rest) = rest.split_once('/')?;
    if account.len() != 32 || !account.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rest = rest.strip_prefix("ai")?;
    if rest == "/models/search" {
        return Some(CloudflarePath::ModelSearch);
    }
    rest.starts_with("/v1/")
        .then(|| CloudflarePath::Api(rest.to_owned()))
}

fn agent_platform_path(path: &str) -> Option<PlatformPath> {
    const MODELS: &str = "publishers/google/models";
    if let Some(rest) = path
        .strip_prefix("/v1/")
        .and_then(|p| p.strip_prefix(MODELS))
    {
        return Some(PlatformPath {
            project: false,
            as_ai_studio: format!("{GEMINI_MODELS}{rest}"),
        });
    }
    if let Some(rest) = path.strip_prefix("/v1beta1/projects/") {
        let mut parts = rest.splitn(4, '/');
        let (_project, locations, location, tail) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        let rest = tail.strip_prefix("interactions")?;
        if locations != "locations" || location != "global" {
            return None;
        }
        return Some(PlatformPath {
            project: true,
            as_ai_studio: format!("/v1beta/interactions{rest}"),
        });
    }
    let rest = path.strip_prefix("/v1/projects/")?;
    let mut parts = rest.splitn(4, '/');
    let (_project, locations, _location, tail) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if locations != "locations" {
        return None;
    }
    if tail == "endpoints/openapi/chat/completions" {
        return Some(PlatformPath {
            project: true,
            as_ai_studio: "/v1/chat/completions".to_owned(),
        });
    }
    if let Some(call) = tail.strip_prefix("publishers/anthropic/models/") {
        let as_anthropic = match call.rsplit_once(':')? {
            ("count-tokens", "rawPredict") => "/v1/messages/count_tokens",
            (_, "rawPredict" | "streamRawPredict") => "/v1/messages",
            _ => return None,
        };
        return Some(PlatformPath {
            project: true,
            as_ai_studio: as_anthropic.to_owned(),
        });
    }
    let rest = tail.strip_prefix(MODELS)?;
    Some(PlatformPath {
        project: true,
        as_ai_studio: format!("{GEMINI_MODELS}{rest}"),
    })
}

fn claims_of(form: &str) -> serde_json::Value {
    use base64::Engine as _;
    url::form_urlencoded::parse(form.as_bytes())
        .find(|(k, _)| k == "assertion")
        .and_then(|(_, jwt)| {
            let payload = jwt.split('.').nth(1)?.to_owned();
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(payload)
                .ok()
        })
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(serde_json::Value::Null)
}

// Grants a new token for every assertion, `ya29.mock-<n>`. The assertion's `iss` chooses
// another answer: one containing `refused` gets Google's 400 `invalid_grant`, `unavailable`
// a 503, and `short` a token that lasts 400 seconds.
fn google_token(shared: &Shared, form: &str) -> Response {
    let claims = claims_of(form);
    let iss = claims
        .get("iss")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let grant = url::form_urlencoded::parse(form.as_bytes())
        .find(|(k, _)| k == "grant_type")
        .map(|(_, v)| v.into_owned());
    let (status, body) = if grant.as_deref() != Some("urn:ietf:params:oauth:grant-type:jwt-bearer")
        || iss.contains("refused")
    {
        (
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error": "invalid_grant", "error_description": "Invalid JWT Signature."}),
        )
    } else if iss.contains("unavailable") {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error": "internal_failure"}),
        )
    } else {
        let n = shared.google_tokens.fetch_add(1, Ordering::Relaxed) + 1;
        let lifetime = if iss.contains("short") { 400 } else { 3599 };
        (
            StatusCode::OK,
            serde_json::json!({
                "access_token": format!("{GOOGLE_TOKEN_PREFIX}{n}"),
                "expires_in": lifetime,
                "token_type": "Bearer"
            }),
        )
    };
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

const MODELS: &str = "/v1/models";

const GEMINI_MODELS: &str = "/v1beta/models";

// Three pages: the first, then the recorded next page, then that page again without a
// `nextPageToken`, for the token the second page gave.
fn gemini_models(dir: &Path, path: &str, query: Option<&str>) -> Response {
    if path != GEMINI_MODELS {
        return json_file(&dir.join("recorded-get-gemini-3.5-flash.json"));
    }
    let token = query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .find_map(|pair| pair.strip_prefix("pageToken="));
    let Some(token) = token else {
        return json_file(&dir.join("recorded-list-page-size-5.json"));
    };
    let next = dir.join("recorded-list-next-page.json");
    let mut page: serde_json::Value = std::fs::read(&next)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| panic!("mock: no fixture {}", next.display()));
    if page
        .get("nextPageToken")
        .and_then(serde_json::Value::as_str)
        == Some(token)
        && let Some(map) = page.as_object_mut()
    {
        map.remove("nextPageToken");
    }
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(page.to_string()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn gemini_call(path: &str) -> Option<(String, GeminiAction)> {
    split_model_action(path.strip_prefix(GEMINI_MODELS)?.strip_prefix('/')?)
}

fn key_text(headers: &HeaderMap) -> String {
    ["x-api-key", "x-goog-api-key", "authorization"]
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
        "gemini" | "gemini_embeddings" => "recorded-error-invalid-key",
        "gemini_decisions" => "invalid-api-key-400",
        "system_one" => "unauthorized-401",
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
        .or_else(|| path.strip_prefix("/v1beta/models/"))
        .map(|rest| rest.split(':').next().unwrap_or(rest).to_owned())
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
    tag: Option<String>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(tag) = self.tag.take() {
                self.shared
                    .closed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(tag, Instant::now());
            }
            self.shared.streams_dropped.fetch_add(1, Ordering::SeqCst);
            self.shared.drops.notify_waiters();
        }
    }
}

fn sse(
    shared: Arc<Shared>,
    bytes: Vec<u8>,
    mode: StreamMode,
    steps: Option<Arc<GateState>>,
    tag: Option<String>,
) -> Response {
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
        tag,
    };
    let state = (events.into_iter(), guard, 0usize);
    let stream = futures::stream::unfold(state, move |(mut it, mut guard, n)| {
        let steps = steps.clone();
        async move {
            if guard.finished {
                return None;
            }
            if let Some(gate) = steps.filter(|_| n > 0) {
                gate.step(n).await;
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
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("x-request-id", "req_mock_stream")
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn paced(
    shared: Arc<Shared>,
    mut chunks: Vec<(Duration, Bytes)>,
    cut: bool,
    tag: Option<String>,
) -> Response {
    shared.streams_started.fetch_add(1, Ordering::SeqCst);
    if cut {
        chunks.truncate((chunks.len() / 2).max(1));
    }
    let guard = Guard {
        shared,
        finished: false,
        tag,
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

fn endless(
    shared: Arc<Shared>,
    head: Vec<(Duration, Bytes)>,
    rest: synth::Endless,
    tag: Option<String>,
) -> Response {
    shared.streams_started.fetch_add(1, Ordering::SeqCst);
    let guard = Guard {
        shared,
        finished: false,
        tag,
    };
    let stream = futures::stream::unfold(
        (head.into_iter(), rest, guard),
        |(mut head, mut rest, guard)| async move {
            let (pause, chunk) = match head.next() {
                Some(next) => next,
                None => {
                    let next = rest.chunk()?;
                    if let Some(tag) = &guard.tag {
                        guard
                            .shared
                            .text_sent
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .insert(tag.clone(), rest.sent());
                    }
                    next
                }
            };
            if !pause.is_zero() {
                tokio::time::sleep(pause).await;
            }
            Some((Ok::<_, std::io::Error>(chunk), (head, rest, guard)))
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
    ctx.shared.video_polls.clear();
    for c in [
        &ctx.shared.streams_started,
        &ctx.shared.streams_finished,
        &ctx.shared.streams_dropped,
    ] {
        c.store(0, Ordering::SeqCst);
    }
    StatusCode::NO_CONTENT
}
