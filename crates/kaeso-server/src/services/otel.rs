//! Traces of model requests and MCP tool calls for an OpenTelemetry collector, sent over
//! OTLP/HTTP as JSON. A request is a server span, `kaeso.request`, with a client span for each
//! step of its route that called a provider. Attributes follow the GenAI semantic conventions at
//! commit `e6658f14` of open-telemetry/semantic-conventions-genai (2026-11-06), which build on the
//! core conventions v1.45.0; Kaeso's own are under `kaeso.*`.
//!
//! The export never delays or fails a request: spans wait in a bounded buffer, a full buffer
//! drops its oldest, and a collector that can't be reached is tried again on the next interval.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::http::HeaderMap;
use bytes::Bytes;
use kaeso_kpp::{Block, DialectId, Message, RequestKind, Response, Role, StopReason};
use serde_json::{Value, json};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use kaeso_core::id::{ApiKeyId, McpServerId, OrgId, ProjectId, ServiceAccountId, UserId};

use crate::config::OtelConfig;
use crate::model_api::hooks::{FellBack, Outcome};
use crate::services::response_cache::Verdict;
use crate::state::AppState;

/// The GenAI semantic conventions Kaeso follows, as the docs name them.
pub const SEMCONV_GENAI_COMMIT: &str = "e6658f14c9d9ea0760825f029cb82e7673f48fab";
const SCHEMA_URL: &str = "https://opentelemetry.io/schemas/1.45.0";
const SCOPE: &str = "kaeso-gateway";

/// Spans sent in one export, at most.
const BATCH: usize = 512;
const EXPORT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A content attribute larger than this is left out of the span, not cut: cut JSON can't be read.
const CONTENT_LIMIT: usize = 128 * 1024;
/// W3C Trace Context lets a `tracestate` grow to 512 characters before it may be cut.
const TRACESTATE_LIMIT: usize = 512;

const KIND_SERVER: u8 = 2;
const KIND_CLIENT: u8 = 3;
const STATUS_ERROR: u8 = 2;

/// The app's span a request continues, from its `traceparent` and `tracestate` headers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceParent {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub sampled: bool,
    pub state: Option<String>,
}

impl TraceParent {
    pub fn from_headers(headers: &HeaderMap) -> Option<TraceParent> {
        let mut parents = headers.get_all("traceparent").iter();
        let parent = parents.next()?.to_str().ok()?;
        // Two of them can't both be the parent; W3C Trace Context ignores both.
        if parents.next().is_some() {
            return None;
        }
        let state = headers
            .get_all("tracestate")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join(",");
        TraceParent::parse(parent, Some(state.as_str()))
    }

    /// A `traceparent` as W3C Trace Context writes it: `00-<trace id>-<span id>-<flags>`, or a
    /// later version with more fields after them.
    pub fn parse(traceparent: &str, tracestate: Option<&str>) -> Option<TraceParent> {
        let mut fields = traceparent.trim().split('-');
        let version = fields.next()?;
        let trace_id = hex_array::<16>(fields.next()?)?;
        let span_id = hex_array::<8>(fields.next()?)?;
        let flags = hex_array::<1>(fields.next()?)?;
        let more = fields.next().is_some();
        let [version] = hex_array::<1>(version)?;
        if version == 0xff || (version == 0 && more) {
            return None;
        }
        if trace_id == [0; 16] || span_id == [0; 8] {
            return None;
        }
        let state = tracestate
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= TRACESTATE_LIMIT && s.is_ascii())
            .map(str::to_owned);
        Some(TraceParent {
            trace_id,
            span_id,
            sampled: flags[0] & 1 == 1,
            state,
        })
    }
}

fn hex_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut out = [0u8; N];
    for (byte, pair) in out.iter_mut().zip(text.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

pub struct Exporter {
    url: url::Url,
    headers: reqwest::header::HeaderMap,
    client: reqwest::Client,
    resource: Value,
    content: bool,
    ratio: f64,
    capacity: usize,
    interval: Duration,
    queue: Mutex<VecDeque<Value>>,
    wake: Notify,
}

impl std::fmt::Debug for Exporter {
    // The headers may carry the collector's token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exporter")
            .field("url", &self.url.as_str())
            .field("waiting", &self.waiting())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Exported {
    /// The collector took this many spans.
    Sent(usize),
    /// It couldn't be reached or asked to wait: the spans wait for the next try.
    Again,
}

impl Exporter {
    /// The exporter of `otel`, or none when no endpoint is set.
    pub fn new(otel: &OtelConfig) -> Option<Exporter> {
        let url = otel.traces_url()?;
        let headers = otel
            .header_map()
            .inspect_err(|e| tracing::error!(error = %e, "otel.headers can't be read; traces are not exported"))
            .ok()?;
        let client = reqwest::Client::builder()
            .timeout(EXPORT_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .inspect_err(|e| tracing::error!(error = %e, "cannot build the trace exporter's client; traces are not exported"))
            .ok()?;
        let resource = json!({
            "attributes": [
                string("service.name", otel.service_name.trim()),
                string("service.version", env!("CARGO_PKG_VERSION")),
                string("service.instance.id", uuid::Uuid::now_v7().to_string()),
                string("telemetry.sdk.name", "kaeso"),
                string("telemetry.sdk.language", "rust"),
                string("telemetry.sdk.version", env!("CARGO_PKG_VERSION")),
            ]
        });
        Some(Exporter {
            url,
            headers,
            client,
            resource,
            content: otel.content,
            ratio: otel.sample_ratio,
            capacity: usize::try_from(otel.buffer_size)
                .unwrap_or(usize::MAX)
                .max(1),
            interval: otel.flush_interval,
            queue: Mutex::new(VecDeque::new()),
            wake: Notify::new(),
        })
    }

    fn queue(&self) -> MutexGuard<'_, VecDeque<Value>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Spans waiting for the collector.
    pub fn waiting(&self) -> usize {
        self.queue().len()
    }

    // Parent-based: a request with a `traceparent` follows the app's choice; one without is
    // kept by its trace id's low 8 bytes, as OpenTelemetry's TraceIdRatioBased sampler does.
    fn sampled(&self, trace_id: &[u8; 16], parent: Option<&TraceParent>) -> bool {
        if let Some(p) = parent {
            return p.sampled;
        }
        if self.ratio >= 1.0 {
            return true;
        }
        let mut low = [0u8; 8];
        low.copy_from_slice(trace_id.get(8..).unwrap_or(&[0; 8]));
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let bound = (self.ratio * (u64::MAX as f64)) as u64;
        u64::from_be_bytes(low) < bound
    }

    fn push(&self, spans: Vec<Value>) {
        let mut dropped = 0u64;
        {
            let mut q = self.queue();
            for span in spans {
                if q.len() >= self.capacity {
                    q.pop_front();
                    dropped += 1;
                }
                q.push_back(span);
            }
            if q.len() >= BATCH {
                self.wake.notify_one();
            }
        }
        count("dropped", dropped);
    }

    // A batch that couldn't be sent goes back before the newer spans, as far as there is room.
    fn put_back(&self, batch: Vec<Value>) {
        let mut q = self.queue();
        let room = self.capacity.saturating_sub(q.len());
        let lost = batch.len().saturating_sub(room);
        for span in batch.into_iter().skip(lost).rev() {
            q.push_front(span);
        }
        drop(q);
        count("dropped", lost as u64);
    }

    async fn export_once(&self) -> Exported {
        let batch: Vec<Value> = {
            let mut q = self.queue();
            let n = q.len().min(BATCH);
            q.drain(..n).collect()
        };
        if batch.is_empty() {
            return Exported::Sent(0);
        }
        let n = batch.len();
        let body = json!({
            "resourceSpans": [{
                "resource": self.resource,
                "scopeSpans": [{
                    "scope": { "name": SCOPE, "version": env!("CARGO_PKG_VERSION") },
                    "schemaUrl": SCHEMA_URL,
                    "spans": batch,
                }],
                "schemaUrl": SCHEMA_URL,
            }]
        });
        let sent = self
            .client
            .post(self.url.clone())
            .headers(self.headers.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await;
        let spans = || {
            body.pointer("/resourceSpans/0/scopeSpans/0/spans")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        match sent {
            Ok(r) if r.status().is_success() => {
                // A collector that keeps only some says how many it rejected.
                let rejected = r
                    .bytes()
                    .await
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .and_then(|v| {
                        let n = v.pointer("/partialSuccess/rejectedSpans")?;
                        n.as_u64().or_else(|| n.as_str()?.parse().ok())
                    })
                    .unwrap_or(0)
                    .min(n as u64);
                if rejected > 0 {
                    tracing::warn!(rejected, "the trace collector rejected some spans");
                }
                count("exported", n as u64 - rejected);
                count("dropped", rejected);
                Exported::Sent(n)
            }
            Ok(r) => {
                let status = r.status();
                count("failed", n as u64);
                if matches!(status.as_u16(), 429 | 502 | 503 | 504) {
                    tracing::warn!(%status, spans = n, "the trace collector asked to wait; its spans are sent again");
                    self.put_back(spans());
                    Exported::Again
                } else {
                    tracing::warn!(%status, spans = n, "the trace collector refused spans; they are dropped");
                    count("dropped", n as u64);
                    Exported::Sent(n)
                }
            }
            Err(e) => {
                count("failed", n as u64);
                tracing::warn!(error = %e.without_url(), spans = n, "cannot reach the trace collector; its spans are sent again");
                self.put_back(spans());
                Exported::Again
            }
        }
    }
}

fn count(outcome: &'static str, n: u64) {
    if n > 0 {
        metrics::counter!(crate::metrics::OTEL_SPANS, "outcome" => outcome).increment(n);
    }
}

/// Sends buffered spans every `otel.flush_interval`, and at once when a batch is full. After a
/// failed export it waits a whole interval, so a collector that is down isn't asked at every
/// request.
pub fn spawn_exporter(state: AppState, draining: CancellationToken, stop: CancellationToken) {
    let Some(exporter) = state.otel.clone() else {
        return;
    };
    tokio::spawn(async move {
        let mut wait = false;
        loop {
            let every = if draining.is_cancelled() {
                crate::serve::SHUTDOWN_FLUSH_INTERVAL.min(exporter.interval)
            } else {
                exporter.interval
            };
            tokio::select! {
                () = stop.cancelled() => return,
                () = draining.cancelled(), if !draining.is_cancelled() && !wait => {}
                () = tokio::time::sleep(every) => {}
                () = exporter.wake.notified(), if !wait => {}
            }
            wait = false;
            loop {
                match exporter.export_once().await {
                    Exported::Sent(n) if n >= BATCH => {}
                    Exported::Sent(_) => break,
                    Exported::Again => {
                        wait = true;
                        break;
                    }
                }
            }
        }
    });
}

/// Sends what is left at shutdown, within `wait`; the spans it couldn't send count as dropped.
pub async fn flush_all(state: &AppState, wait: Duration) -> usize {
    let Some(exporter) = state.otel.as_deref() else {
        return 0;
    };
    let mut sent = 0;
    let _ = tokio::time::timeout(wait, async {
        loop {
            match exporter.export_once().await {
                Exported::Sent(0) | Exported::Again => break,
                Exported::Sent(n) => sent += n,
            }
        }
    })
    .await;
    let left = exporter.waiting();
    if left > 0 {
        exporter.queue().clear();
        count("dropped", left as u64);
        tracing::error!(sent, left, "traces could not all be exported at shutdown");
    }
    sent
}

/// Exports a finished model request's trace when the request is sampled.
pub async fn record(state: &AppState, o: &Outcome) {
    let Some(exporter) = state.otel.as_deref() else {
        return;
    };
    let trace_id = o
        .trace
        .as_ref()
        .map_or_else(kaeso_crypto::random_bytes::<16>, |p| p.trace_id);
    if !exporter.sampled(&trace_id, o.trace.as_ref()) {
        return;
    }
    let mut trace = request_trace(o, trace_id);
    if exporter.content && o.content_kept && matches!(o.kind, RequestKind::Generate) {
        let (ingress, body, reply) = (o.ingress, o.request_body.clone(), o.reply.clone());
        let found = tokio::task::spawn_blocking(move || content(ingress, body, reply.as_ref()))
            .await
            .unwrap_or_default();
        trace.carrier().add_content(found);
    }
    exporter.push(trace.into_spans());
}

/// What the trace of an MCP tool call needs, as the portal records it.
#[derive(Clone, Debug)]
pub struct ToolCall<'a> {
    pub parent: Option<&'a TraceParent>,
    pub at: OffsetDateTime,
    pub duration: Duration,
    pub tool: &'a str,
    pub org_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub server_id: uuid::Uuid,
    pub key_id: Option<uuid::Uuid>,
    pub client: &'a str,
    pub status: &'a str,
    pub error_type: &'a str,
    pub error_message: &'a str,
}

/// Exports a tool call's span, `execute_tool <tool>`, when it is sampled.
pub fn record_tool_call(state: &AppState, call: &ToolCall<'_>) {
    let Some(exporter) = state.otel.as_deref() else {
        return;
    };
    let trace_id = call
        .parent
        .map_or_else(kaeso_crypto::random_bytes::<16>, |p| p.trace_id);
    if !exporter.sampled(&trace_id, call.parent) {
        return;
    }
    let start = call.at;
    let mut span = Span::new(
        trace_id,
        format!("execute_tool {}", call.tool),
        KIND_SERVER,
        start,
        start + call.duration,
    );
    if let Some(p) = call.parent {
        span.parent = Some(p.span_id);
        span.state.clone_from(&p.state);
    }
    let a = &mut span.attributes;
    a.push(string("gen_ai.operation.name", "execute_tool"));
    a.push(string("gen_ai.tool.name", call.tool));
    a.push(string("gen_ai.tool.type", "extension"));
    a.push(string(
        "kaeso.organization.id",
        OrgId::from_uuid(call.org_id).to_string(),
    ));
    a.push(string(
        "kaeso.project.id",
        ProjectId::from_uuid(call.project_id).to_string(),
    ));
    a.push(string(
        "kaeso.mcp_server.id",
        McpServerId::from_uuid(call.server_id).to_string(),
    ));
    if let Some(key) = call.key_id {
        a.push(string("kaeso.key.id", ApiKeyId::from_uuid(key).to_string()));
    }
    if !call.client.is_empty() {
        a.push(string("kaeso.client", call.client));
    }
    a.push(string("kaeso.tool_call.status", call.status));
    if !call.error_type.is_empty() {
        a.push(string("error.type", call.error_type));
        span.error = Some(call.error_message.to_owned());
    }
    exporter.push(vec![span.into_value()]);
}

struct Span {
    trace_id: [u8; 16],
    id: [u8; 8],
    parent: Option<[u8; 8]>,
    state: Option<String>,
    name: String,
    kind: u8,
    start: OffsetDateTime,
    end: OffsetDateTime,
    attributes: Vec<Value>,
    error: Option<String>,
}

impl Span {
    fn new(
        trace_id: [u8; 16],
        name: String,
        kind: u8,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> Span {
        Span {
            trace_id,
            id: span_id(),
            parent: None,
            state: None,
            name,
            kind,
            start,
            end: end.max(start),
            attributes: Vec::new(),
            error: None,
        }
    }

    fn add_usage(&mut self, o: &Outcome) {
        let Some(u) = &o.usage else {
            return;
        };
        let a = &mut self.attributes;
        // Kaeso counts cache reads and writes apart from the input; the conventions count them in it.
        let input = u
            .input_tokens
            .saturating_add(u.cache_read_tokens)
            .saturating_add(u.cache_write_tokens);
        a.push(int("gen_ai.usage.input_tokens", input));
        a.push(int("gen_ai.usage.output_tokens", u.output_tokens));
        if u.cache_read_tokens > 0 {
            a.push(int(
                "gen_ai.usage.cache_read.input_tokens",
                u.cache_read_tokens,
            ));
        }
        if u.cache_write_tokens > 0 {
            a.push(int(
                "gen_ai.usage.cache_write.input_tokens",
                u.cache_write_tokens,
            ));
        }
        if let Some(r) = u.reasoning_tokens.filter(|r| *r > 0) {
            a.push(int("gen_ai.usage.reasoning.output_tokens", r));
        }
        if o.usage_estimated {
            a.push(boolean("kaeso.usage_estimated", true));
        }
    }

    fn add_content(&mut self, found: Content) {
        let mut omitted = false;
        for (key, value) in [
            ("gen_ai.system_instructions", found.system),
            ("gen_ai.input.messages", found.input),
            ("gen_ai.output.messages", found.output),
        ] {
            match value {
                Some(v) if v.len() <= CONTENT_LIMIT => self.attributes.push(string(key, v)),
                Some(_) => omitted = true,
                None => {}
            }
        }
        if omitted {
            self.attributes.push(boolean("kaeso.content_omitted", true));
        }
    }

    fn into_value(self) -> Value {
        let mut span = serde_json::Map::new();
        span.insert("traceId".into(), hex(&self.trace_id).into());
        span.insert("spanId".into(), hex(&self.id).into());
        if let Some(parent) = self.parent {
            span.insert("parentSpanId".into(), hex(&parent).into());
        }
        if let Some(state) = self.state {
            span.insert("traceState".into(), state.into());
        }
        span.insert("name".into(), self.name.into());
        span.insert("kind".into(), self.kind.into());
        span.insert("startTimeUnixNano".into(), nanos(self.start).into());
        span.insert("endTimeUnixNano".into(), nanos(self.end).into());
        span.insert("attributes".into(), self.attributes.into());
        if let Some(message) = self.error {
            span.insert(
                "status".into(),
                json!({ "code": STATUS_ERROR, "message": message }),
            );
        }
        Value::Object(span)
    }
}

struct Trace {
    server: Span,
    clients: Vec<Span>,
    /// Whether the last client span is the step that answered, which carries the tokens.
    answered: bool,
}

impl Trace {
    // The span that carries the tokens and the content: the step that called the provider last,
    // or the request's own when none did, such as for a cached reply.
    fn carrier(&mut self) -> &mut Span {
        match self.clients.last_mut() {
            Some(last) if self.answered => last,
            _ => &mut self.server,
        }
    }

    fn into_spans(self) -> Vec<Value> {
        std::iter::once(self.server)
            .chain(self.clients)
            .map(Span::into_value)
            .collect()
    }
}

fn span_id() -> [u8; 8] {
    loop {
        let id = kaeso_crypto::random_bytes::<8>();
        if id != [0; 8] {
            return id;
        }
    }
}

fn nanos(at: OffsetDateTime) -> String {
    u64::try_from(at.unix_timestamp_nanos().max(0))
        .unwrap_or(u64::MAX)
        .to_string()
}

fn attribute(key: &str, value: Value) -> Value {
    json!({ "key": key, "value": value })
}

fn string(key: &str, value: impl Into<String>) -> Value {
    attribute(key, json!({ "stringValue": value.into() }))
}

// OTLP's JSON writes 64-bit integers as strings.
fn int(key: &str, value: impl Into<i128>) -> Value {
    let value = i64::try_from(value.into()).unwrap_or(i64::MAX);
    attribute(key, json!({ "intValue": value.to_string() }))
}

fn boolean(key: &str, value: bool) -> Value {
    attribute(key, json!({ "boolValue": value }))
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// The conventions' operation for what a request asked: `chat`, `generate_content` and
/// `embeddings` where they apply, otherwise the endpoint the request log names.
fn operation(o: &Outcome) -> &'static str {
    match o.kind {
        RequestKind::Generate if o.ingress == DialectId::Gemini => "generate_content",
        RequestKind::Generate if o.realtime.is_none() => "chat",
        RequestKind::Embed => "embeddings",
        _ => crate::services::logs::endpoint(o),
    }
}

/// A provider by the conventions' name where they have one, otherwise by Kaeso's id.
pub fn provider_name(id: &str) -> &str {
    match id {
        "amazon-bedrock" => "aws.bedrock",
        "azure" => "azure.ai.openai",
        "google" => "gcp.gemini",
        "gemini-agent-platform" => "gcp.vertex_ai",
        "mistral" => "mistral",
        "moonshotai" => "moonshot_ai",
        "ollama-cloud" => "ollama",
        "xai" => "x_ai",
        other => other,
    }
}

fn request_trace(o: &Outcome, trace_id: [u8; 16]) -> Trace {
    let start = o.started_at;
    let end = start + o.duration;
    let op = operation(o);
    let mut server = Span::new(
        trace_id,
        "kaeso.request".to_owned(),
        KIND_SERVER,
        start,
        end,
    );
    if let Some(p) = &o.trace {
        server.parent = Some(p.span_id);
        server.state.clone_from(&p.state);
    }
    let provider = o
        .custom_provider
        .as_deref()
        .or(o.provider)
        .map(provider_name);
    let a = &mut server.attributes;
    a.push(string("gen_ai.operation.name", op));
    if let Some(m) = &o.requested_model {
        a.push(string("gen_ai.request.model", m.clone()));
    }
    if let Some(p) = provider {
        a.push(string("gen_ai.provider.name", p));
    }
    if let Some(m) = &o.response_model {
        a.push(string("gen_ai.response.model", m.clone()));
    }
    a.push(boolean("gen_ai.request.stream", o.stream));
    a.push(int("http.response.status_code", o.status));
    if o.status >= 400 {
        a.push(string(
            "error.type",
            o.error_kind.clone().unwrap_or_else(|| o.status.to_string()),
        ));
    }
    if o.status >= 500 {
        server.error = Some(o.error_message.clone().unwrap_or_default());
    }
    let a = &mut server.attributes;
    a.push(string("kaeso.request_id", o.request_id.to_string()));
    a.push(string("kaeso.endpoint", crate::services::logs::endpoint(o)));
    a.push(string("kaeso.source", o.source.as_str()));
    if let Some(c) = &o.caller {
        a.push(string(
            "kaeso.organization.id",
            OrgId::from_uuid(c.org_id).to_string(),
        ));
        a.push(string(
            "kaeso.project.id",
            ProjectId::from_uuid(c.project_id).to_string(),
        ));
        if let Some(k) = c.key_id {
            a.push(string("kaeso.key.id", ApiKeyId::from_uuid(k).to_string()));
        }
        if let Some(u) = c.user_id {
            a.push(string("kaeso.user.id", UserId::from_uuid(u).to_string()));
        }
        if let Some(s) = c.service_account_id {
            a.push(string(
                "kaeso.service_account.id",
                ServiceAccountId::from_uuid(s).to_string(),
            ));
        }
    }
    if let Some(alias) = &o.alias {
        a.push(string("kaeso.alias", alias.clone()));
    }
    if let Some(v) = o.response_cache {
        a.push(string("kaeso.response_cache", v.as_str()));
    }
    if crate::services::usage::cost_known(o) {
        let (_, cost) = crate::services::usage::costed(o);
        let micro = (cost * rust_decimal::Decimal::from(1_000_000))
            .round()
            .try_into()
            .unwrap_or(i64::MAX);
        a.push(int("kaeso.cost_microusd", micro));
    }
    if let Some(t) = o.ttfb {
        a.push(int("kaeso.ttfb_ms", millis(t)));
    }
    if let Some(t) = o.overhead {
        a.push(int("kaeso.overhead_ms", millis(t)));
    }
    if let Some(id) = &o.upstream_request_id {
        a.push(string("kaeso.upstream_request_id", id.clone()));
    }
    if o.cancelled {
        a.push(boolean("kaeso.cancelled", true));
    }
    if !o.tags.is_empty() {
        let tags: Vec<Value> = o.tags.iter().map(|t| json!({ "stringValue": t })).collect();
        a.push(attribute(
            "kaeso.tags",
            json!({ "arrayValue": { "values": tags } }),
        ));
    }
    if let Some(agent) = &o.user_agent {
        a.push(string("user_agent.original", agent.clone()));
    }

    // Client spans, in the order the steps were tried. The log keeps each failed step's
    // duration, not its start: Kaeso's own time comes first, then the steps one after another.
    let mut cursor = start + o.upload.unwrap_or_default() + o.overhead.unwrap_or_default();
    cursor = cursor.min(end);
    let mut clients = Vec::with_capacity(o.fallbacks.len() + 1);
    for f in &o.fallbacks {
        let took = if f.skipped {
            Duration::ZERO
        } else {
            Duration::from_millis(f.duration_ms)
        };
        let stop = (cursor + took).min(end);
        clients.push(fallback_span(trace_id, server.id, op, f, cursor, stop));
        cursor = stop;
    }
    let called = o.overhead.is_some()
        && o.response_cache != Some(Verdict::Hit)
        && provider.is_some()
        && o.model_id.is_some();
    if called {
        let model = o.model_id.clone().unwrap_or_default();
        let mut step = Span::new(trace_id, format!("{op} {model}"), KIND_CLIENT, cursor, end);
        step.parent = Some(server.id);
        let a = &mut step.attributes;
        a.push(string("gen_ai.operation.name", op));
        if let Some(p) = provider {
            a.push(string("gen_ai.provider.name", p));
        }
        a.push(string("gen_ai.request.model", model));
        if let Some(m) = &o.response_model {
            a.push(string("gen_ai.response.model", m.clone()));
        }
        a.push(boolean("gen_ai.request.stream", o.stream));
        a.push(int("http.response.status_code", o.status));
        if o.status >= 400 {
            a.push(string(
                "error.type",
                o.error_kind.clone().unwrap_or_else(|| o.status.to_string()),
            ));
            step.error = Some(o.error_message.clone().unwrap_or_default());
        }
        clients.push(step);
    }
    let mut trace = Trace {
        server,
        clients,
        answered: called,
    };
    trace.carrier().add_usage(o);
    trace
}

fn fallback_span(
    trace_id: [u8; 16],
    parent: [u8; 8],
    op: &str,
    f: &FellBack,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Span {
    let (provider, model) = f.model.split_once('/').unwrap_or(("", f.model.as_str()));
    let mut span = Span::new(trace_id, format!("{op} {model}"), KIND_CLIENT, start, end);
    span.parent = Some(parent);
    let a = &mut span.attributes;
    a.push(string("gen_ai.operation.name", op));
    if !provider.is_empty() {
        a.push(string("gen_ai.provider.name", provider_name(provider)));
    }
    a.push(string("gen_ai.request.model", model));
    if f.status > 0 {
        a.push(int("http.response.status_code", f.status));
    }
    a.push(string("error.type", f.error_kind.clone()));
    if !f.path.is_empty() {
        a.push(string("kaeso.route.path", f.path.clone()));
    }
    if f.skipped {
        a.push(boolean("kaeso.skipped", true));
    }
    if f.retried {
        a.push(boolean("kaeso.retried", true));
    }
    span.error = Some(f.error_message.clone());
    span
}

/// The prompt and the reply as the conventions' content attributes, each as JSON.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Content {
    pub system: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
}

fn content(ingress: DialectId, body: Option<Bytes>, reply: Option<&Response>) -> Content {
    let mut found = Content::default();
    let request = body.as_deref().and_then(|b| {
        kaeso_codecs::dispatch::decode_request(
            ingress,
            RequestKind::Generate,
            b,
            &kaeso_codecs::HeaderList::new(),
        )
        .ok()
    });
    if let Some(kaeso_kpp::Request::Generate(r)) = request {
        let system: Vec<Value> = r.system.iter().filter_map(part).collect();
        if !system.is_empty() {
            found.system = serde_json::to_string(&system).ok();
        }
        let input: Vec<Value> = r.messages.iter().filter_map(message).collect();
        if !input.is_empty() {
            found.input = serde_json::to_string(&input).ok();
        }
    }
    if let Some(r) = reply {
        let parts: Vec<Value> = r.blocks.iter().filter_map(part).collect();
        let output = json!([{
            "role": "assistant",
            "parts": parts,
            "finish_reason": finish_reason(&r.stop),
        }]);
        found.output = serde_json::to_string(&output).ok();
    }
    found
}

fn message(m: &Message) -> Option<Value> {
    let parts: Vec<Value> = m.blocks.iter().filter_map(part).collect();
    if parts.is_empty() {
        return None;
    }
    // A user turn of tool results only is the conventions' `tool` message.
    let tool_results = m.blocks.iter().all(|b| matches!(b, Block::ToolResult(_)));
    let role = match m.role {
        Role::User if tool_results => "tool",
        other => other.as_str(),
    };
    Some(json!({ "role": role, "parts": parts }))
}

// A block as a message part; media is never exported, and an unknown block is left out.
fn part(b: &Block) -> Option<Value> {
    Some(match b {
        Block::Text(t) => json!({ "type": "text", "content": t.text }),
        Block::Refusal(r) => json!({ "type": "text", "content": r.text }),
        Block::ToolCall(c) => {
            let arguments: Value = serde_json::from_str(c.arguments.get()).unwrap_or(Value::Null);
            json!({ "type": "tool_call", "id": c.id, "name": c.name, "arguments": arguments })
        }
        Block::ToolResult(r) => {
            let text: Vec<&str> = r
                .content
                .iter()
                .filter_map(|b| match b {
                    Block::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect();
            json!({ "type": "tool_call_response", "id": r.call_id, "response": text.join("\n") })
        }
        Block::Reasoning(r) => {
            let text = if r.text.is_empty() {
                &r.summary
            } else {
                &r.text
            };
            if text.is_empty() {
                return None;
            }
            json!({ "type": "reasoning", "content": text.join("\n") })
        }
        Block::Json(j) => {
            let data: Value = serde_json::from_str(j.data.get()).unwrap_or(Value::Null);
            json!({ "type": "text", "content": data.to_string() })
        }
        _ => return None,
    })
}

fn finish_reason(stop: &StopReason) -> &str {
    match stop {
        StopReason::MaxTokens => "length",
        StopReason::ToolUse => "tool_call",
        StopReason::ContentFilter | StopReason::Refusal => "content_filter",
        StopReason::Other(other) => other.as_str(),
        _ => "stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";

    #[test]
    fn a_traceparent_is_read_as_w3c_trace_context_writes_it() {
        let p = TraceParent::parse(PARENT, Some("vendor=abc")).unwrap();
        assert_eq!(hex(&p.trace_id), "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(hex(&p.span_id), "b7ad6b7169203331");
        assert!(p.sampled);
        assert_eq!(p.state.as_deref(), Some("vendor=abc"));
        let unsampled = TraceParent::parse(&PARENT.replace("-01", "-00"), None).unwrap();
        assert!(!unsampled.sampled);
        // A later version may add fields; version 00 may not.
        assert!(TraceParent::parse(&format!("01{}-extra", &PARENT[2..]), None).is_some());
        for bad in [
            format!("{PARENT}-extra"),
            PARENT.replace("00-", "ff-"),
            PARENT.to_uppercase(),
            "00-00000000000000000000000000000000-b7ad6b7169203331-01".to_owned(),
            "00-0af7651916cd43dd8448eb211c80319c-0000000000000000-01".to_owned(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b716920333-01".to_owned(),
            "00-0af7651916cd43dd8448eb211c80319c".to_owned(),
            String::new(),
        ] {
            assert_eq!(TraceParent::parse(&bad, None), None, "{bad}");
        }
        let long = "a=".to_owned() + &"b".repeat(TRACESTATE_LIMIT);
        assert_eq!(TraceParent::parse(PARENT, Some(&long)).unwrap().state, None);
    }

    #[test]
    fn two_traceparents_name_no_parent() {
        let mut h = HeaderMap::new();
        h.append("traceparent", PARENT.parse().unwrap());
        h.append("tracestate", "a=1".parse().unwrap());
        h.append("tracestate", "b=2".parse().unwrap());
        assert_eq!(
            TraceParent::from_headers(&h).unwrap().state.as_deref(),
            Some("a=1,b=2")
        );
        h.append("traceparent", PARENT.parse().unwrap());
        assert_eq!(TraceParent::from_headers(&h), None);
    }

    fn exporter(ratio: f64, capacity: u32) -> Exporter {
        let otel = OtelConfig {
            endpoint: Some("http://collector.example.com:4318".parse().unwrap()),
            headers: None,
            service_name: "kaeso-gateway".into(),
            content: false,
            sample_ratio: ratio,
            buffer_size: capacity,
            flush_interval: Duration::from_secs(5),
        };
        Exporter::new(&otel).unwrap()
    }

    #[test]
    fn sampling_follows_the_parent_or_the_trace_ids_ratio() {
        let none = exporter(0.0, 10);
        let all = exporter(1.0, 10);
        let half = exporter(0.5, 10);
        let id = |low: u64| {
            let mut t = [0u8; 16];
            t[8..].copy_from_slice(&low.to_be_bytes());
            t
        };
        assert!(all.sampled(&id(u64::MAX), None));
        assert!(!none.sampled(&id(0), None));
        assert!(half.sampled(&id(u64::MAX / 4), None));
        assert!(!half.sampled(&id(u64::MAX / 4 * 3), None));
        let parent = TraceParent::parse(PARENT, None).unwrap();
        assert!(
            none.sampled(&parent.trace_id, Some(&parent)),
            "the app's choice wins"
        );
        let unsampled = TraceParent {
            sampled: false,
            ..parent
        };
        assert!(!all.sampled(&unsampled.trace_id, Some(&unsampled)));
    }

    #[test]
    fn a_full_buffer_keeps_the_newest_spans_and_a_batch_sent_again_goes_first() {
        let x = exporter(1.0, 3);
        x.push((0..5).map(Value::from).collect());
        assert_eq!(Vec::from(x.queue().clone()), [json!(2), json!(3), json!(4)]);
        let batch: Vec<Value> = x.queue().drain(..2).collect();
        x.push(vec![json!(5)]);
        x.put_back(batch);
        // One of the two fits back, and it is the newer of them.
        assert_eq!(Vec::from(x.queue().clone()), [json!(3), json!(4), json!(5)]);
    }

    #[test]
    fn the_endpoint_gets_the_traces_path_unless_it_names_it() {
        let mut otel = OtelConfig {
            endpoint: Some("http://collector.example.com:4318".parse().unwrap()),
            headers: None,
            service_name: "kaeso-gateway".into(),
            content: false,
            sample_ratio: 1.0,
            buffer_size: 1,
            flush_interval: Duration::from_secs(5),
        };
        assert_eq!(
            otel.traces_url().unwrap().as_str(),
            "http://collector.example.com:4318/v1/traces"
        );
        otel.endpoint = Some("https://otel.example.com/otlp/".parse().unwrap());
        assert_eq!(
            otel.traces_url().unwrap().as_str(),
            "https://otel.example.com/otlp/v1/traces"
        );
        otel.endpoint = Some("https://otel.example.com/v1/traces".parse().unwrap());
        assert_eq!(
            otel.traces_url().unwrap().as_str(),
            "https://otel.example.com/v1/traces"
        );
    }

    #[test]
    fn headers_are_name_value_pairs_with_percent_encoded_values() {
        let otel = |h: &str| OtelConfig {
            endpoint: None,
            headers: Some(h.to_owned().into()),
            service_name: "kaeso-gateway".into(),
            content: false,
            sample_ratio: 1.0,
            buffer_size: 1,
            flush_interval: Duration::from_secs(5),
        };
        let map = otel("authorization=Bearer%20abc, x-tenant = one ")
            .header_map()
            .unwrap();
        assert_eq!(map["authorization"], "Bearer abc");
        assert!(map["authorization"].is_sensitive());
        assert_eq!(map["x-tenant"], "one");
        assert!(otel("authorization").header_map().is_err());
        assert!(otel("bad name=1").header_map().is_err());
    }

    #[test]
    fn content_uses_the_conventions_parts_and_leaves_media_out() {
        let body = Bytes::from_static(
            br#"{"model":"claude-haiku-4-5","max_tokens":10,"system":"Be brief.","messages":[
                {"role":"user","content":[{"type":"text","text":"What is in it?"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo="}}]},
                {"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"look","input":{"q":1}}]},
                {"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"a cat"}]}
            ]}"#,
        );
        let mut reply = Response::new("msg_1", "claude-haiku-4-5", vec![Block::text("A cat.")]);
        reply.stop = StopReason::MaxTokens;
        let found = content(DialectId::AnthropicMessages, Some(body), Some(&reply));
        let parse = |s: Option<String>| serde_json::from_str::<Value>(&s.unwrap()).unwrap();
        assert_eq!(
            parse(found.system),
            json!([{ "type": "text", "content": "Be brief." }])
        );
        assert_eq!(
            parse(found.input),
            json!([
                { "role": "user", "parts": [{ "type": "text", "content": "What is in it?" }] },
                { "role": "assistant", "parts": [{ "type": "tool_call", "id": "t1", "name": "look", "arguments": { "q": 1 } }] },
                { "role": "tool", "parts": [{ "type": "tool_call_response", "id": "t1", "response": "a cat" }] }
            ])
        );
        assert_eq!(
            parse(found.output),
            json!([{ "role": "assistant", "parts": [{ "type": "text", "content": "A cat." }], "finish_reason": "length" }])
        );
        let mut span = Span::new(
            [1; 16],
            "x".into(),
            KIND_CLIENT,
            OffsetDateTime::UNIX_EPOCH,
            OffsetDateTime::UNIX_EPOCH,
        );
        span.add_content(Content {
            system: None,
            input: Some("x".repeat(CONTENT_LIMIT + 1)),
            output: Some("[]".into()),
        });
        let keys: Vec<&str> = span
            .attributes
            .iter()
            .filter_map(|a| a["key"].as_str())
            .collect();
        assert_eq!(keys, ["gen_ai.output.messages", "kaeso.content_omitted"]);
    }

    #[test]
    fn providers_take_the_conventions_names() {
        assert_eq!(provider_name("amazon-bedrock"), "aws.bedrock");
        assert_eq!(provider_name("google"), "gcp.gemini");
        assert_eq!(provider_name("openai"), "openai");
        assert_eq!(provider_name("fireworks-ai"), "fireworks-ai");
    }
}
