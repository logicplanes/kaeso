//! Where sources send what they observed: OTLP/HTTP for clients that speak OpenTelemetry
//! (`/ingest/otlp/v1/logs`, `/traces`, `/metrics`) and JSON for gateways and scripts
//! (`/ingest/v1/events`, `/ingest/v1/litellm`, `/ingest/v1/antigravity`). Each request carries a
//! source key; nothing here ever reaches a provider.

pub mod events;
pub mod map;
pub mod otlp;

use std::io::Read;
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

use crate::services::observed::{self, Batch, Recorded, Refusal};
use crate::state::AppState;

pub const BASE: &str = "/ingest";

/// The most a request body may be as sent, and once gzip is undone.
const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_INFLATED: u64 = 32 * 1024 * 1024;
/// The most records one batch may hold. OpenTelemetry SDKs export at most 512 at a time.
const MAX_RECORDS: usize = 5_000;
/// Batches a source may send a minute; a busy coding agent exports a few each minute.
const BATCHES_PER_MINUTE: i64 = 600;
const VALKEY_DEADLINE: Duration = Duration::from_secs(2);

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
    message: &'static str,
    retry_after: Option<u64>,
}

impl Refused {
    const fn new(status: StatusCode, kind: &'static str, message: &'static str) -> Self {
        Refused {
            status,
            kind,
            message,
            retry_after: None,
        }
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        let mut r = (
            self.status,
            axum::Json(json!({ "error": { "type": self.kind, "message": self.message } })),
        )
            .into_response();
        if let Some(s) = self.retry_after
            && let Ok(v) = HeaderValue::from_str(&s.to_string())
        {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
        r
    }
}

const NO_KEY: Refused = Refused::new(
    StatusCode::UNAUTHORIZED,
    "authentication",
    "Send the source's key as `Authorization: Bearer sk-kso-src-…`.",
);
const UNKNOWN_KEY: Refused = Refused::new(
    StatusCode::UNAUTHORIZED,
    "authentication",
    "No source has this key; it may have been replaced.",
);
const INACTIVE: Refused = Refused::new(
    StatusCode::FORBIDDEN,
    "permission_denied",
    "This source is disabled or archived, or its project is.",
);
const TOO_LARGE: Refused = Refused::new(
    StatusCode::PAYLOAD_TOO_LARGE,
    "request_too_large",
    "A batch may be at most 8 MiB, 32 MiB unzipped, and 5,000 records.",
);
const UNREADABLE: Refused = Refused::new(
    StatusCode::BAD_REQUEST,
    "invalid_request",
    "The body isn't what this endpoint reads; see the setup guide for the source.",
);
const ENCODING: Refused = Refused::new(
    StatusCode::UNSUPPORTED_MEDIA_TYPE,
    "invalid_request",
    "Send the body as it is or gzipped.",
);
const UNAVAILABLE: Refused = Refused::new(
    StatusCode::SERVICE_UNAVAILABLE,
    "unavailable",
    "Kaeso can't record this batch right now; send it again shortly.",
);

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

async fn source(state: &AppState, headers: &HeaderMap) -> Result<Source, Refused> {
    let key = presented(headers).ok_or(NO_KEY)?;
    if !kaeso_core::sources::is_key(key) {
        return Err(UNKNOWN_KEY);
    }
    let hash = kaeso_core::secret::hash(key);
    let found = kaeso_store::pg::sources::by_secret(&state.pg, &hash)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot look up a source key");
            UNAVAILABLE
        })?
        .ok_or(UNKNOWN_KEY)?;
    if found.source.disabled_at.is_some()
        || found.source.archived_at.is_some()
        || found.project_archived
        || !found.org_active
    {
        return Err(INACTIVE);
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
                "This source sent more than 600 batches this minute; send the rest shortly.",
            )
        }),
        _ => Ok(()),
    }
}

async fn body(req: Request) -> Result<(HeaderMap, Bytes), Refused> {
    let (parts, body) = req.into_parts();
    let raw = http_body_util::Limited::new(body, MAX_BODY)
        .collect()
        .await
        .map_err(|_| TOO_LARGE)?
        .to_bytes();
    let encoding = parts
        .headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase());
    let bytes = match encoding.as_deref() {
        None | Some("" | "identity") => raw,
        Some("gzip" | "x-gzip") => {
            let mut out = Vec::new();
            flate2::read::MultiGzDecoder::new(raw.as_ref())
                .take(MAX_INFLATED + 1)
                .read_to_end(&mut out)
                .map_err(|_| UNREADABLE)?;
            if out.len() as u64 > MAX_INFLATED {
                return Err(TOO_LARGE);
            }
            Bytes::from(out)
        }
        Some(_) => return Err(ENCODING),
    };
    Ok((parts.headers, bytes))
}

fn protobuf(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| {
            t.starts_with("application/x-protobuf") || t.starts_with("application/protobuf")
        })
}

async fn refuse_unreadable(state: &AppState, source: &Source) -> Refused {
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
    UNREADABLE
}

async fn recorded(state: &AppState, source: &Source, batch: Batch) -> Result<Recorded, Refused> {
    if batch.requests.len() + batch.tool_calls.len() > MAX_RECORDS {
        return Err(TOO_LARGE);
    }
    observed::record(state, source, batch).await.map_err(|e| {
        tracing::error!(error = %e, source = %source.id, "cannot record a source's batch");
        UNAVAILABLE
    })
}

/// The answer to an OTLP export: `partial_success` names what Kaeso didn't keep.
fn otlp_answer(signal: &str, as_protobuf: bool, rejected: usize, message: &str) -> Response {
    let rejected = i64::try_from(rejected).unwrap_or(i64::MAX);
    if as_protobuf {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/x-protobuf")],
            otlp::response_protobuf(rejected, message),
        )
            .into_response()
    } else {
        (
            StatusCode::OK,
            axum::Json(otlp::response_json(signal, rejected, message)),
        )
            .into_response()
    }
}

fn refusal_text(r: &Recorded) -> String {
    r.refusal
        .map(Refusal::as_str)
        .unwrap_or_default()
        .to_owned()
}

async fn otlp_logs(State(state): State<AppState>, req: Request) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (headers, bytes) = body(req).await?;
    let pb = protobuf(&headers);
    let groups = if pb {
        otlp::logs_from_protobuf(&bytes)
    } else {
        otlp::logs_from_json(&bytes)
    };
    let Ok(groups) = groups else {
        return Err(refuse_unreadable(&state, &source).await);
    };
    let done = recorded(&state, &source, map::logs(&groups)).await?;
    Ok(otlp_answer("logs", pb, done.refused, &refusal_text(&done)))
}

async fn otlp_traces(State(state): State<AppState>, req: Request) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (headers, bytes) = body(req).await?;
    let pb = protobuf(&headers);
    let groups = if pb {
        otlp::spans_from_protobuf(&bytes)
    } else {
        otlp::spans_from_json(&bytes)
    };
    let Ok(groups) = groups else {
        return Err(refuse_unreadable(&state, &source).await);
    };
    let done = recorded(&state, &source, map::spans(&groups)).await?;
    Ok(otlp_answer(
        "traces",
        pb,
        done.refused,
        &refusal_text(&done),
    ))
}

// Metrics only say a client is alive: every request they count, its log events report.
async fn otlp_metrics(State(state): State<AppState>, req: Request) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (headers, bytes) = body(req).await?;
    let pb = protobuf(&headers);
    let read = if pb {
        otlp::metrics_from_protobuf(&bytes)
    } else {
        otlp::metrics_from_json(&bytes)
    };
    if read.is_err() {
        return Err(refuse_unreadable(&state, &source).await);
    }
    let now = state.clock.now();
    if let Err(e) = kaeso_store::pg::sources::touch(&state.pg, source.id, now, None).await {
        tracing::warn!(error = %e, "cannot note a source's metrics");
    }
    Ok(otlp_answer("metrics", pb, 0, ""))
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

async fn json_events(State(state): State<AppState>, req: Request) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (_, bytes) = body(req).await?;
    let Ok(batch) = serde_json::from_slice::<events::Events>(&bytes) else {
        return Err(refuse_unreadable(&state, &source).await);
    };
    let done = recorded(&state, &source, events::batch(batch)).await?;
    Ok(json_answer(&done))
}

async fn json_litellm(State(state): State<AppState>, req: Request) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (_, bytes) = body(req).await?;
    let Some(items) = json_values(&bytes) else {
        return Err(refuse_unreadable(&state, &source).await);
    };
    let done = recorded(&state, &source, map::litellm(&items)).await?;
    Ok(json_answer(&done))
}

async fn json_antigravity(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, Refused> {
    let source = source(&state, req.headers()).await?;
    within_rate(&state, &source).await?;
    let (_, bytes) = body(req).await?;
    let Ok(Json::Object(body)) = serde_json::from_slice::<Json>(&bytes) else {
        return Err(refuse_unreadable(&state, &source).await);
    };
    let hook = body.get("hook").cloned().unwrap_or(Json::Null);
    let steps = body
        .get("steps")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();
    let done = recorded(&state, &source, map::antigravity(&hook, &steps)).await?;
    Ok(json_answer(&done))
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
    fn protobuf_is_told_by_its_content_type() {
        let mut h = HeaderMap::new();
        assert!(!protobuf(&h));
        h.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-protobuf"),
        );
        assert!(protobuf(&h));
    }

    #[test]
    fn an_export_kaeso_took_whole_answers_with_an_empty_message() {
        assert!(otlp::response_protobuf(0, "").is_empty());
        assert!(!otlp::response_protobuf(2, "no_model").is_empty());
        assert_eq!(otlp::response_json("logs", 0, ""), json!({}));
    }
}
