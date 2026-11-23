//! Where sources send what they observed: OTLP/HTTP for clients that speak OpenTelemetry
//! (`/ingest/otlp/v1/logs`, `/traces`, `/metrics`) and JSON for gateways and scripts
//! (`/ingest/v1/events`, `/ingest/v1/litellm`, `/ingest/v1/antigravity`). Each request carries a
//! source key; nothing here ever reaches a provider.

pub mod events;
pub mod map;
pub mod otlp;

use std::io::Read;
use std::sync::LazyLock;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use http_body_util::BodyExt;
use kaeso_store::pg::sources::Source;
use serde_json::{Value as Json, json};
use tokio::sync::Semaphore;

use crate::auth::key_failures::Gate;
use crate::http::client_ip::ClientIp;
use crate::services::observed::{self, Batch, Recorded, Refusal};
use crate::state::AppState;

/// The most a request body may be as sent, and once gzip is undone.
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_INFLATED: u64 = 16 * 1024 * 1024;
/// The most records one batch may hold. OpenTelemetry SDKs export at most 512 at a time.
const MAX_RECORDS: usize = 5_000;
/// Batches a source may send a minute; a busy coding agent exports a few each minute.
const BATCHES_PER_MINUTE: i64 = 600;
const VALKEY_DEADLINE: Duration = Duration::from_secs(2);
/// How long a source that found Kaeso unavailable is asked to wait.
const RETRY_LATER: u64 = 30;

// Bodies are unzipped and decoded off the async workers, a few at a time, so a burst of large
// batches costs a bounded amount of memory.
static DECODING: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(4));

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ingest/otlp/v1/logs", post(otlp_logs))
        .route("/ingest/otlp/v1/traces", post(otlp_traces))
        .route("/ingest/otlp/v1/metrics", post(otlp_metrics))
        .route("/ingest/v1/events", post(json_events))
        .route("/ingest/v1/litellm", post(json_litellm))
        .route("/ingest/v1/antigravity", post(json_antigravity))
}

/// An answer that ends a request before anything was recorded.
#[derive(Debug)]
pub struct Refused {
    status: StatusCode,
    kind: &'static str,
    /// The gRPC code a protobuf answer names, as OTLP/HTTP asks.
    code: i32,
    message: String,
    retry_after: Option<u64>,
}

impl Refused {
    fn new(status: StatusCode, kind: &'static str, code: i32, message: &str) -> Self {
        Refused {
            status,
            kind,
            code,
            message: message.to_owned(),
            retry_after: None,
        }
    }

    /// The answer in the body's own encoding: protobuf `google.rpc.Status` for a protobuf
    /// export, JSON otherwise.
    fn answer(self, as_protobuf: bool) -> Response {
        let mut r = if as_protobuf {
            (
                self.status,
                [(header::CONTENT_TYPE, "application/x-protobuf")],
                otlp::status_protobuf(self.code, &self.message),
            )
                .into_response()
        } else {
            (
                self.status,
                axum::Json(json!({ "error": { "type": self.kind, "message": self.message } })),
            )
                .into_response()
        };
        if let Some(s) = self.retry_after
            && let Ok(v) = HeaderValue::from_str(&s.to_string())
        {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
        r
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        self.answer(false)
    }
}

// gRPC status codes (google.rpc.Code).
const INVALID_ARGUMENT: i32 = 3;
const PERMISSION_DENIED: i32 = 7;
const RESOURCE_EXHAUSTED: i32 = 8;
const UNAVAILABLE_CODE: i32 = 14;
const UNAUTHENTICATED: i32 = 16;

fn no_key() -> Refused {
    Refused::new(
        StatusCode::UNAUTHORIZED,
        "authentication",
        UNAUTHENTICATED,
        "Send the source's key as `Authorization: Bearer sk-kso-src-…`.",
    )
}

fn unknown_key() -> Refused {
    Refused::new(
        StatusCode::UNAUTHORIZED,
        "authentication",
        UNAUTHENTICATED,
        "No source has this key; it may have been replaced.",
    )
}

fn inactive() -> Refused {
    Refused::new(
        StatusCode::FORBIDDEN,
        "permission_denied",
        PERMISSION_DENIED,
        "This source is disabled or archived, or its project is.",
    )
}

fn too_large() -> Refused {
    Refused::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        RESOURCE_EXHAUSTED,
        "A batch may be at most 8 MiB, 16 MiB unzipped, and 5,000 records.",
    )
}

fn unreadable() -> Refused {
    Refused::new(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        INVALID_ARGUMENT,
        "The body isn't what this endpoint reads; see the setup guide for the source.",
    )
}

fn media_type() -> Refused {
    Refused::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_request",
        INVALID_ARGUMENT,
        "Send `application/x-protobuf` or `application/json`, as it is or gzipped.",
    )
}

fn unavailable() -> Refused {
    Refused {
        retry_after: Some(RETRY_LATER),
        ..Refused::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            UNAVAILABLE_CODE,
            "Kaeso can't record this batch right now; send it again shortly.",
        )
    }
}

fn presented(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-kaeso-source-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(crate::auth::keys::bearer)
        })
        .map(str::trim)
}

fn throttled(t: crate::auth::key_failures::Throttled) -> Refused {
    Refused {
        retry_after: Some(t.retry_after),
        ..Refused::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            RESOURCE_EXHAUSTED,
            "Too many unknown source keys from your address; try again later.",
        )
    }
}

// An unknown key counts against the sender's address like an unknown API key, so guessing keys
// is throttled where the model API throttles it.
async fn source(state: &AppState, headers: &HeaderMap, ip: ClientIp) -> Result<Source, Refused> {
    let gate = Gate::new(state, ip.0);
    if let Some(g) = &gate {
        g.check().await.map_err(throttled)?;
    }
    let key = presented(headers).ok_or_else(no_key)?;
    let found = if kaeso_core::sources::is_key(key) {
        kaeso_store::pg::sources::by_secret(&state.pg, &kaeso_core::secret::hash(key))
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "cannot look up a source key");
                unavailable()
            })?
    } else {
        None
    };
    let Some(found) = found else {
        if let Some(g) = &gate {
            g.failed().await.map_err(throttled)?;
        }
        return Err(unknown_key());
    };
    if found.source.disabled_at.is_some()
        || found.source.archived_at.is_some()
        || found.project_archived
        || !found.org_active
    {
        return Err(inactive());
    }
    Ok(found.source)
}

// Counts the source's batches in the current minute; Valkey being away lets the batch through,
// as a source is never in anyone's path and a lost count harms nothing.
async fn within_rate(state: &AppState, source: &Source) -> Result<(), Refused> {
    let now = state.clock.now().unix_timestamp();
    let key = state
        .valkey
        .key(&format!("obsrate:{}:{}", source.id, now / 60));
    let mut conn = state.valkey.conn();
    let counted: Result<Result<i64, redis::RedisError>, _> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::pipe()
            .atomic()
            .cmd("INCR")
            .arg(&key)
            .cmd("EXPIRE")
            .arg(&key)
            .arg(120)
            .ignore()
            .query_async::<(i64,)>(&mut conn),
    )
    .await
    .map(|r| r.map(|(n,)| n));
    match counted {
        Ok(Ok(n)) if n > BATCHES_PER_MINUTE => Err(Refused {
            retry_after: Some(u64::try_from(60 - now % 60).unwrap_or(60)),
            ..Refused::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                RESOURCE_EXHAUSTED,
                "This source sent more than 600 batches this minute; send the rest shortly.",
            )
        }),
        _ => Ok(()),
    }
}

async fn raw_body(req: Request) -> Result<(HeaderMap, Bytes), Refused> {
    let (parts, body) = req.into_parts();
    let raw = http_body_util::Limited::new(body, MAX_BODY)
        .collect()
        .await
        .map_err(|_| too_large())?
        .to_bytes();
    Ok((parts.headers, raw))
}

fn inflate(headers: &HeaderMap, raw: Bytes) -> Result<Bytes, Refused> {
    let encoding = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase());
    match encoding.as_deref() {
        None | Some("" | "identity") => Ok(raw),
        Some("gzip" | "x-gzip") => {
            let mut out = Vec::new();
            flate2::read::MultiGzDecoder::new(raw.as_ref())
                .take(MAX_INFLATED + 1)
                .read_to_end(&mut out)
                .map_err(|_| unreadable())?;
            if out.len() as u64 > MAX_INFLATED {
                return Err(too_large());
            }
            Ok(Bytes::from(out))
        }
        Some(_) => Err(media_type()),
    }
}

/// Unzips and reads a body off the async workers, a few at a time.
async fn decoded<T, F>(headers: HeaderMap, raw: Bytes, read: F) -> Result<T, Refused>
where
    T: Send + 'static,
    F: FnOnce(&HeaderMap, &[u8]) -> Result<T, Refused> + Send + 'static,
{
    let _turn = DECODING.acquire().await.map_err(|_| unavailable())?;
    tokio::task::spawn_blocking(move || {
        let bytes = inflate(&headers, raw)?;
        read(&headers, &bytes)
    })
    .await
    .map_err(|_| unavailable())?
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Protobuf,
    Json,
}

fn otlp_encoding(headers: &HeaderMap) -> Option<Encoding> {
    let ct = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())?;
    if ct.starts_with("application/x-protobuf") || ct.starts_with("application/protobuf") {
        Some(Encoding::Protobuf)
    } else if ct.starts_with("application/json") {
        Some(Encoding::Json)
    } else {
        None
    }
}

fn decode_error(e: &otlp::DecodeError) -> Refused {
    match e {
        otlp::DecodeError::TooMany => too_large(),
        _ => unreadable(),
    }
}

async fn note_unreadable(state: &AppState, source: &Source) {
    let _ = observed::record(
        state,
        source,
        Batch {
            refused: 1,
            refusal: Some(Refusal::Unreadable),
            ..Batch::default()
        },
    )
    .await;
}

async fn recorded(state: &AppState, source: &Source, batch: Batch) -> Result<Recorded, Refused> {
    if batch.requests.len() + batch.tool_calls.len() > MAX_RECORDS {
        return Err(too_large());
    }
    observed::record(state, source, batch).await.map_err(|e| {
        tracing::error!(error = %e, source = %source.id, "cannot record a source's batch");
        unavailable()
    })
}

/// The answer to an OTLP export: `partial_success` names what Kaeso didn't keep.
fn otlp_answer(signal: &str, encoding: Encoding, rejected: usize, message: &str) -> Response {
    let rejected = i64::try_from(rejected).unwrap_or(i64::MAX);
    match encoding {
        Encoding::Protobuf => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/x-protobuf")],
            otlp::response_protobuf(rejected, message),
        )
            .into_response(),
        Encoding::Json => (
            StatusCode::OK,
            axum::Json(otlp::response_json(signal, rejected, message)),
        )
            .into_response(),
    }
}

fn refusal_text(r: &Recorded) -> String {
    r.refusal
        .map(Refusal::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[derive(Clone, Copy)]
enum Signal {
    Logs,
    Traces,
    Metrics,
}

impl Signal {
    const fn name(self) -> &'static str {
        match self {
            Signal::Logs => "logs",
            Signal::Traces => "traces",
            Signal::Metrics => "metrics",
        }
    }
}

enum Decoded {
    Batch(Batch),
    Metrics,
}

fn read_otlp(signal: Signal, encoding: Encoding, bytes: &[u8]) -> Result<Decoded, Refused> {
    let pb = encoding == Encoding::Protobuf;
    match signal {
        Signal::Logs => {
            let groups = if pb {
                otlp::logs_from_protobuf(bytes, MAX_RECORDS)
            } else {
                otlp::logs_from_json(bytes, MAX_RECORDS)
            };
            groups
                .map(|g| Decoded::Batch(map::logs(&g)))
                .map_err(|e| decode_error(&e))
        }
        Signal::Traces => {
            let groups = if pb {
                otlp::spans_from_protobuf(bytes, MAX_RECORDS)
            } else {
                otlp::spans_from_json(bytes, MAX_RECORDS)
            };
            groups
                .map(|g| Decoded::Batch(map::spans(&g)))
                .map_err(|e| decode_error(&e))
        }
        Signal::Metrics => {
            let read = if pb {
                otlp::metrics_from_protobuf(bytes)
            } else {
                otlp::metrics_from_json(bytes)
            };
            read.map(|_| Decoded::Metrics).map_err(|e| decode_error(&e))
        }
    }
}

async fn otlp(state: AppState, ip: ClientIp, signal: Signal, req: Request) -> Response {
    let Some(encoding) = otlp_encoding(req.headers()) else {
        return media_type().answer(false);
    };
    let pb = encoding == Encoding::Protobuf;
    match otlp_export(&state, ip, signal, encoding, req).await {
        Ok(r) => r,
        Err(refused) => refused.answer(pb),
    }
}

async fn otlp_export(
    state: &AppState,
    ip: ClientIp,
    signal: Signal,
    encoding: Encoding,
    req: Request,
) -> Result<Response, Refused> {
    let source = source(state, req.headers(), ip).await?;
    within_rate(state, &source).await?;
    let (headers, raw) = raw_body(req).await?;
    let read = decoded(headers, raw, move |_, bytes| {
        read_otlp(signal, encoding, bytes)
    })
    .await;
    let read = match read {
        Ok(read) => read,
        Err(refused) => {
            if refused.status == StatusCode::BAD_REQUEST {
                note_unreadable(state, &source).await;
            }
            return Err(refused);
        }
    };
    match read {
        Decoded::Batch(batch) => {
            let done = recorded(state, &source, batch).await?;
            Ok(otlp_answer(
                signal.name(),
                encoding,
                done.refused,
                &refusal_text(&done),
            ))
        }
        // Metrics only say a client is alive: every request they count, its events report.
        Decoded::Metrics => {
            let now = state.clock.now();
            if let Err(e) = kaeso_store::pg::sources::touch(&state.pg, source.id, now, None).await {
                tracing::warn!(error = %e, "cannot note a source's metrics");
            }
            Ok(otlp_answer(signal.name(), encoding, 0, ""))
        }
    }
}

async fn otlp_logs(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    otlp(state, ip, Signal::Logs, req).await
}

async fn otlp_traces(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    otlp(state, ip, Signal::Traces, req).await
}

async fn otlp_metrics(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    otlp(state, ip, Signal::Metrics, req).await
}

fn json_answer(r: &Recorded) -> Response {
    (
        StatusCode::OK,
        axum::Json(json!({
            "requests": r.requests,
            "tool_calls": r.tool_calls,
            "duplicates": r.duplicates,
            "refused": r.refused,
            "refusal": r.refusal.map(Refusal::as_str),
        })),
    )
        .into_response()
}

/// A JSON body: one value, or newline-delimited values.
fn json_values(bytes: &[u8]) -> Option<Vec<Json>> {
    if let Ok(v) = serde_json::from_slice::<Json>(bytes) {
        return Some(match v {
            Json::Array(items) => items,
            other => vec![other],
        });
    }
    let text = std::str::from_utf8(bytes).ok()?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Json>(l).ok())
        .collect()
}

/// Reads a JSON endpoint's body and records what `read` makes of it.
async fn json_batch<F>(
    state: &AppState,
    ip: ClientIp,
    req: Request,
    read: F,
) -> Result<Response, Refused>
where
    F: FnOnce(&[u8]) -> Option<Batch> + Send + 'static,
{
    let source = source(state, req.headers(), ip).await?;
    within_rate(state, &source).await?;
    let (headers, raw) = raw_body(req).await?;
    let batch = decoded(headers, raw, move |_, bytes| {
        read(bytes).ok_or_else(unreadable)
    })
    .await;
    let batch = match batch {
        Ok(b) => b,
        Err(refused) => {
            if refused.status == StatusCode::BAD_REQUEST {
                note_unreadable(state, &source).await;
            }
            return Err(refused);
        }
    };
    let done = recorded(state, &source, batch).await?;
    Ok(json_answer(&done))
}

async fn json_events(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    json_batch(&state, ip, req, |bytes| {
        let e = serde_json::from_slice::<events::Events>(bytes).ok()?;
        (e.requests.len() + e.tool_calls.len() <= MAX_RECORDS).then(|| events::batch(e))
    })
    .await
    .unwrap_or_else(IntoResponse::into_response)
}

async fn json_litellm(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    json_batch(&state, ip, req, |bytes| {
        let items = json_values(bytes)?;
        (items.len() <= MAX_RECORDS).then(|| map::litellm(&items))
    })
    .await
    .unwrap_or_else(IntoResponse::into_response)
}

async fn json_antigravity(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    json_batch(&state, ip, req, |bytes| {
        let Ok(Json::Object(body)) = serde_json::from_slice::<Json>(bytes) else {
            return None;
        };
        let hook = body.get("hook").cloned().unwrap_or(Json::Null);
        let steps = body
            .get("steps")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        (steps.len() <= MAX_RECORDS).then(|| map::antigravity(&hook, &steps))
    })
    .await
    .unwrap_or_else(IntoResponse::into_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_body_is_one_value_a_list_or_lines_of_values() {
        assert_eq!(
            json_values(b"[{\"a\":1},{\"a\":2}]").map(|v| v.len()),
            Some(2)
        );
        assert_eq!(json_values(b"{\"a\":1}").map(|v| v.len()), Some(1));
        assert_eq!(
            json_values(b"{\"a\":1}\n\n{\"a\":2}\n").map(|v| v.len()),
            Some(2)
        );
        assert_eq!(json_values(b"{\"a\":1}\nnot json"), None);
    }

    #[test]
    fn the_key_comes_from_its_own_header_or_a_bearer_token() {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer sk-kso-src-x"),
        );
        assert_eq!(presented(&h), Some("sk-kso-src-x"));
        h.insert(
            "x-kaeso-source-key",
            HeaderValue::from_static("sk-kso-src-y"),
        );
        assert_eq!(presented(&h), Some("sk-kso-src-y"));
    }

    #[test]
    fn an_export_is_protobuf_or_json_by_its_content_type_and_nothing_else() {
        let with = |ct: &'static str| {
            let mut h = HeaderMap::new();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
            otlp_encoding(&h)
        };
        assert!(otlp_encoding(&HeaderMap::new()).is_none());
        assert!(with("application/x-protobuf") == Some(Encoding::Protobuf));
        assert!(with("application/json; charset=utf-8") == Some(Encoding::Json));
        assert!(with("text/plain").is_none());
    }

    #[test]
    fn an_export_kaeso_took_whole_answers_with_an_empty_message() {
        assert!(otlp::response_protobuf(0, "").is_empty());
        assert!(!otlp::response_protobuf(2, "no_model").is_empty());
        assert_eq!(otlp::response_json("logs", 0, ""), json!({}));
        assert_eq!(
            otlp::response_json("traces", 2, "no_model")["partialSuccess"]["rejectedSpans"],
            "2"
        );
    }

    #[test]
    fn a_body_of_millions_of_tiny_records_is_refused_before_it_is_decoded() {
        let record = [0x12u8, 0x00];
        let scope: Vec<u8> = record.iter().copied().cycle().take(2 * 6_000).collect();
        let mut scope_logs = vec![0x12];
        scope_logs.extend(varint(scope.len()));
        scope_logs.extend(&scope);
        let mut resource_logs = vec![0x0a];
        resource_logs.extend(varint(scope_logs.len()));
        resource_logs.extend(&scope_logs);
        assert_eq!(otlp::count_protobuf(&resource_logs, &[1, 2, 2]), Ok(6_000));
        assert_eq!(
            otlp::logs_from_protobuf(&resource_logs, MAX_RECORDS).map(|g| g.len()),
            Err(otlp::DecodeError::TooMany)
        );
        assert!(otlp::count_protobuf(&[0x0a, 0x05, 0x12], &[1, 2, 2]).is_err());
    }

    fn varint(mut n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }
}
