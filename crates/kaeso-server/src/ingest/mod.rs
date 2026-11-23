//! Where sources send what they observed: OTLP/HTTP for clients that speak OpenTelemetry
//! (`/ingest/otlp/v1/logs`, `/traces`, `/metrics`) and JSON for gateways and scripts
//! (`/ingest/v1/events`, `/ingest/v1/litellm`, `/ingest/v1/antigravity`). Each request carries a
//! source key; nothing here ever reaches a provider.

pub mod content;
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

/// The most a request body may be as sent, and once gzip is undone; JSON takes several times
/// its size in memory once parsed, so it may be less.
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_INFLATED: u64 = 16 * 1024 * 1024;
const MAX_INFLATED_JSON: u64 = 8 * 1024 * 1024;
/// The most records one batch may hold. OpenTelemetry SDKs export at most 512 at a time.
const MAX_RECORDS: usize = 1_000;
/// Batches and records a source may send a minute; a busy coding agent exports a few batches
/// each minute, and a team's proxy a few records a second.
const BATCHES_PER_MINUTE: i64 = 600;
const RECORDS_PER_MINUTE: i64 = 6_000;
/// Records a source may send a day, which also bounds the ids Kaeso remembers for it.
const RECORDS_PER_DAY: i64 = 200_000;
/// Batches of one source in progress at the same time, fewer than the decoding turns: a client
/// exports its logs, traces and metrics side by side.
const BUSY_PER_SOURCE: usize = 3;
const VALKEY_DEADLINE: Duration = Duration::from_secs(2);
/// How long a source that found Kaeso unavailable is asked to wait.
const RETRY_LATER: u64 = 30;

// Bodies are unzipped and decoded a few at a time, off the async workers, so a burst of large
// batches costs a bounded amount of memory; a slow upload is read before it takes a turn.
static DECODING: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(4));
static BUSY: LazyLock<std::sync::Mutex<std::collections::HashMap<uuid::Uuid, usize>>> =
    LazyLock::new(Default::default);

// One of a source's batches in progress; the source may have `BUSY_PER_SOURCE` at once, so one
// source can't hold every decoding turn.
struct Busy(uuid::Uuid);

impl Busy {
    fn take(source: uuid::Uuid) -> Result<Busy, Refused> {
        let mut busy = BUSY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let n = busy.entry(source).or_insert(0);
        if *n >= BUSY_PER_SOURCE {
            return Err(Refused {
                retry_after: Some(1),
                ..Refused::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    RESOURCE_EXHAUSTED,
                    "This source has three batches in progress; send the next one when one ends.",
                )
            });
        }
        *n += 1;
        Ok(Busy(source))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        let mut busy = BUSY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(n) = busy.get_mut(&self.0) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                busy.remove(&self.0);
            }
        }
    }
}

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
        "A batch may be at most 8 MiB, 16 MiB unzipped (8 MiB for JSON), and 1,000 records.",
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
                .and_then(|v| crate::auth::keys::bearer(v).or_else(|| encoded_bearer(v)))
        })
        .map(str::trim)
}

// `OTEL_EXPORTER_OTLP_HEADERS` takes URL-encoded values, so guides write the space as `%20`;
// some exporters send the value without decoding it.
fn encoded_bearer(v: &str) -> Option<&str> {
    let (scheme, key) = v.split_at_checked("Bearer%20".len())?;
    scheme.eq_ignore_ascii_case("Bearer%20").then_some(key)
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

// Each record costs a log row, a usage row and a limit charge, so a source's records count too,
// not only its batches.
async fn within_record_rate(
    state: &AppState,
    source: &Source,
    records: usize,
) -> Result<(), Refused> {
    let now = state.clock.now().unix_timestamp();
    let minute = state
        .valkey
        .key(&format!("obsrecords:{}:{}", source.id, now / 60));
    let day = state
        .valkey
        .key(&format!("obsrecords:{}:d{}", source.id, now / 86_400));
    let n = i64::try_from(records).unwrap_or(i64::MAX);
    let mut conn = state.valkey.conn();
    let counted: Result<Result<(i64, i64), redis::RedisError>, _> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::pipe()
            .atomic()
            .cmd("INCRBY")
            .arg(&minute)
            .arg(n)
            .cmd("EXPIRE")
            .arg(&minute)
            .arg(120)
            .ignore()
            .cmd("INCRBY")
            .arg(&day)
            .arg(n)
            .cmd("EXPIRE")
            .arg(&day)
            .arg(2 * 86_400)
            .ignore()
            .query_async::<(i64, i64)>(&mut conn),
    )
    .await;
    let refused = |retry: i64, message: &str| Refused {
        retry_after: Some(u64::try_from(retry).unwrap_or(60)),
        ..Refused::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            RESOURCE_EXHAUSTED,
            message,
        )
    };
    let refusal = match counted {
        Ok(Ok((m, _))) if m > RECORDS_PER_MINUTE => refused(
            60 - now % 60,
            "This source sent more than 6,000 records this minute; send the rest shortly.",
        ),
        Ok(Ok((_, d))) if d > RECORDS_PER_DAY => refused(
            86_400 - now % 86_400,
            "This source sent more than 200,000 records today; send the rest tomorrow.",
        ),
        _ => return Ok(()),
    };
    // A refused batch keeps none of its records, so its retries don't use up the day.
    let _: Result<Result<(), redis::RedisError>, _> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::pipe()
            .cmd("DECRBY")
            .arg(&minute)
            .arg(n)
            .ignore()
            .cmd("EXPIRE")
            .arg(&minute)
            .arg(120)
            .ignore()
            .cmd("DECRBY")
            .arg(&day)
            .arg(n)
            .ignore()
            .cmd("EXPIRE")
            .arg(&day)
            .arg(2 * 86_400)
            .ignore()
            .query_async::<()>(&mut conn),
    )
    .await;
    Err(refusal)
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

fn inflate(headers: &HeaderMap, raw: Bytes, most: u64) -> Result<Bytes, Refused> {
    let encoding = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase());
    match encoding.as_deref() {
        None | Some("" | "identity") => Ok(raw),
        Some("gzip" | "x-gzip") => {
            let mut out = Vec::new();
            flate2::read::MultiGzDecoder::new(raw.as_ref())
                .take(most + 1)
                .read_to_end(&mut out)
                .map_err(|_| unreadable())?;
            if out.len() as u64 > most {
                return Err(too_large());
            }
            Ok(Bytes::from(out))
        }
        Some(_) => Err(media_type()),
    }
}

/// Reads a source's body, then unzips and decodes it in its turn, off the async workers.
async fn decoded<T, F>(source: &Source, req: Request, most: u64, read: F) -> Result<T, Refused>
where
    T: Send + 'static,
    F: FnOnce(&HeaderMap, &[u8]) -> Result<T, Refused> + Send + 'static,
{
    let _busy = Busy::take(source.id)?;
    let (headers, raw) = raw_body(req).await?;
    let _turn = DECODING.acquire().await.map_err(|_| unavailable())?;
    tokio::task::spawn_blocking(move || {
        let bytes = inflate(&headers, raw, most)?;
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
    let records = batch.requests.len() + batch.tool_calls.len();
    if records > MAX_RECORDS {
        return Err(too_large());
    }
    within_record_rate(state, source, records).await?;
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
    let most = match encoding {
        Encoding::Protobuf => MAX_INFLATED,
        Encoding::Json => MAX_INFLATED_JSON,
    };
    let read = decoded(&source, req, most, move |_, bytes| {
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
        // Metrics count nothing that the client's events don't, so they are taken and left: a
        // source is connected once its requests or tool calls arrive, not its metrics alone.
        Decoded::Metrics => Ok(otlp_answer(signal.name(), encoding, 0, "")),
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
    F: FnOnce(&[u8]) -> Result<Batch, Refused> + Send + 'static,
{
    let source = source(state, req.headers(), ip).await?;
    within_rate(state, &source).await?;
    let batch = decoded(&source, req, MAX_INFLATED_JSON, move |_, bytes| read(bytes)).await;
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
        if otlp::too_many_json_values(bytes) {
            return Err(too_large());
        }
        let e = serde_json::from_slice::<events::Events>(bytes).map_err(|_| unreadable())?;
        if e.requests.len() + e.tool_calls.len() > MAX_RECORDS {
            return Err(too_large());
        }
        Ok(events::batch(e))
    })
    .await
    .unwrap_or_else(IntoResponse::into_response)
}

async fn json_litellm(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    json_batch(&state, ip, req, |bytes| {
        if otlp::too_many_json_values(bytes) {
            return Err(too_large());
        }
        let items = json_values(bytes).ok_or_else(unreadable)?;
        if items.len() > MAX_RECORDS {
            return Err(too_large());
        }
        Ok(map::litellm(&items))
    })
    .await
    .unwrap_or_else(IntoResponse::into_response)
}

async fn json_antigravity(State(state): State<AppState>, ip: ClientIp, req: Request) -> Response {
    json_batch(&state, ip, req, |bytes| {
        if otlp::too_many_json_values(bytes) {
            return Err(too_large());
        }
        let Ok(Json::Object(mut body)) = serde_json::from_slice::<Json>(bytes) else {
            return Err(unreadable());
        };
        let hook = body.remove("hook").unwrap_or(Json::Null);
        let steps = match body.remove("steps") {
            Some(Json::Array(steps)) => steps,
            _ => Vec::new(),
        };
        if steps.len() > MAX_RECORDS {
            return Err(too_large());
        }
        Ok(map::antigravity(&hook, &steps))
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
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer%20sk-kso-src-z"),
        );
        assert_eq!(
            presented(&h),
            Some("sk-kso-src-z"),
            "as an exporter that doesn't decode `%20` sends it"
        );
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

    // One resource with an attribute of `attributes` bytes, `empty` scopes without records and
    // `full` scopes with one record each.
    fn resource_with_scopes(attributes: usize, empty: usize, full: usize) -> Vec<u8> {
        let value = vec![b'x'; attributes];
        let mut any = vec![0x0a];
        any.extend(varint(value.len()));
        any.extend(&value);
        let mut kv = vec![0x0a, 0x01, b'k', 0x12];
        kv.extend(varint(any.len()));
        kv.extend(&any);
        let mut resource = vec![0x0a];
        resource.extend(varint(kv.len()));
        resource.extend(&kv);
        let mut inner = vec![0x0a];
        inner.extend(varint(resource.len()));
        inner.extend(&resource);
        for _ in 0..empty {
            inner.extend([0x12, 0x00]);
        }
        for _ in 0..full {
            inner.extend([0x12, 0x02, 0x12, 0x00]);
        }
        let mut resource_logs = vec![0x0a];
        resource_logs.extend(varint(inner.len()));
        resource_logs.extend(&inner);
        resource_logs
    }

    #[test]
    fn a_body_of_many_empty_scopes_is_refused_and_empty_scopes_are_left_out() {
        assert_eq!(
            otlp::logs_from_protobuf(&resource_with_scopes(64, MAX_RECORDS + 1, 1), MAX_RECORDS)
                .map(|g| g.len()),
            Err(otlp::DecodeError::TooMany),
            "every scope counts, not only its records"
        );
        let groups =
            otlp::logs_from_protobuf(&resource_with_scopes(64, 3, 1), MAX_RECORDS).unwrap();
        assert_eq!(groups.len(), 1, "scopes without records are left out");
        assert_eq!(groups[0].resource.str("k").map(str::len), Some(64));

        let scopes: Vec<Json> = (0..=MAX_RECORDS).map(|_| json!({})).collect();
        let body = serde_json::to_vec(&json!({
            "resourceLogs": [{ "resource": { "attributes": [] }, "scopeLogs": scopes }]
        }))
        .unwrap();
        assert_eq!(
            otlp::logs_from_json(&body, MAX_RECORDS).map(|g| g.len()),
            Err(otlp::DecodeError::TooMany)
        );
    }

    #[test]
    fn a_resource_keeps_only_short_values_as_they_are_copied_into_every_record() {
        let long =
            otlp::logs_from_protobuf(&resource_with_scopes(2_000, 0, 1), MAX_RECORDS).unwrap();
        assert_eq!(
            long[0].resource.get("k"),
            None,
            "2,000 bytes can't be a name"
        );
        let short =
            otlp::logs_from_protobuf(&resource_with_scopes(1_000, 0, 1), MAX_RECORDS).unwrap();
        assert_eq!(short[0].resource.str("k").map(str::len), Some(1_000));
    }

    #[test]
    fn a_body_of_millions_of_tiny_fields_is_refused_before_it_is_decoded() {
        // One record whose attributes are 300,000 empty key-values: two bytes each on the wire.
        let mut attributes = Vec::new();
        for _ in 0..300_000 {
            attributes.extend([0x32, 0x00]);
        }
        let mut record = vec![0x12];
        record.extend(varint(attributes.len()));
        record.extend(&attributes);
        let mut scope = vec![0x12];
        scope.extend(varint(record.len()));
        scope.extend(&record);
        let mut body = vec![0x0a];
        body.extend(varint(scope.len()));
        body.extend(&scope);
        assert_eq!(
            otlp::logs_from_protobuf(&body, MAX_RECORDS).map(|g| g.len()),
            Err(otlp::DecodeError::TooMany)
        );

        let objects = "{\"a\":0},".repeat(300_000);
        let json = format!("{{\"resourceLogs\":[{objects}{{}}]}}");
        assert_eq!(
            otlp::logs_from_json(json.as_bytes(), MAX_RECORDS).map(|g| g.len()),
            Err(otlp::DecodeError::TooMany)
        );
        assert_eq!(otlp::count_json_values(br#"{"a":"{[}]","b":[1,{}]}"#), 3);
    }

    #[test]
    fn fields_nested_deeper_than_the_count_reads_still_count() {
        // Empty key-values inside twenty kvlist values, inside one record attribute.
        let mut inner = Vec::new();
        for _ in 0..300_000 {
            inner.extend([0x0a, 0x00]);
        }
        for _ in 0..20 {
            // KeyValueList { values = 1 } inside AnyValue { kvlist_value = 6 }.
            let mut list = vec![0x32];
            list.extend(varint(inner.len()));
            list.extend(&inner);
            inner = list;
        }
        assert!(otlp::count_fields(&inner, 12, otlp::MAX_FIELDS) > otlp::MAX_FIELDS);

        let scalars = format!("[{}0]", "0,".repeat(1_100_000));
        assert!(otlp::too_many_json_values(scalars.as_bytes()));
        assert!(!otlp::too_many_json_values(br#"{"a":[1,2,3],"b":"x,y:z"}"#));
    }

    #[test]
    fn a_resource_is_shared_by_its_scopes_instead_of_copied() {
        let groups =
            otlp::logs_from_protobuf(&resource_with_scopes(64, 0, 2), MAX_RECORDS).unwrap();
        assert_eq!(groups.len(), 2);
        assert!(std::sync::Arc::ptr_eq(
            &groups[0].resource,
            &groups[1].resource
        ));
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
