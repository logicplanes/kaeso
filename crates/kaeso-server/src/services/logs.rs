use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use futures::future::BoxFuture;
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, RequestKind, Response};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::logs::{self as ch, ContentRow, LogRow};
use kaeso_store::pg::log_jobs::{self, LogSettings};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::model_api::hooks::{Outcome, Sink};
use crate::state::AppState;

const WRITE_RETRY_WAIT: Duration = Duration::from_secs(2);

struct Entry {
    log: Option<LogRow>,
    content: Option<ContentRow>,
    size: u64,
}

impl Entry {
    fn content_only(self) -> Entry {
        Entry { log: None, ..self }
    }
}

#[derive(Default)]
pub struct Buffer {
    queue: Mutex<VecDeque<Entry>>,
    bytes: AtomicU64,
    dropped: AtomicU64,
    wake: Notify,
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("bytes", &self.bytes.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Buffer {
    fn push(&self, entry: Entry, cap: u64, flush_rows: usize) {
        let mut q = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        self.bytes.fetch_add(entry.size, Ordering::Relaxed);
        q.push_back(entry);
        self.trim(&mut q, cap);
        let len = q.len();
        drop(q);
        self.gauges(len);
        if len >= flush_rows {
            self.wake.notify_one();
        }
    }

    fn trim(&self, q: &mut VecDeque<Entry>, cap: u64) {
        while self.bytes.load(Ordering::Relaxed) > cap {
            let Some(old) = q.pop_front() else { break };
            self.bytes.fetch_sub(old.size, Ordering::Relaxed);
            self.dropped.fetch_add(1, Ordering::Relaxed);
            metrics::counter!(crate::metrics::LOG_ROWS_DROPPED).increment(1);
        }
    }

    fn gauges(&self, len: usize) {
        metrics::gauge!(crate::metrics::LOG_BUFFER_ROWS).set(len as f64);
        metrics::gauge!(crate::metrics::LOG_BUFFER_BYTES)
            .set(self.bytes.load(Ordering::Relaxed) as f64);
    }

    fn take(&self, max: usize) -> Vec<Entry> {
        let mut q = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        let n = q.len().min(max);
        let batch: Vec<Entry> = q.drain(..n).collect();
        for e in &batch {
            self.bytes.fetch_sub(e.size, Ordering::Relaxed);
        }
        let len = q.len();
        drop(q);
        self.gauges(len);
        batch
    }

    fn put_back(&self, batch: Vec<Entry>, cap: u64) {
        let mut q = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        for e in batch.into_iter().rev() {
            self.bytes.fetch_add(e.size, Ordering::Relaxed);
            q.push_front(e);
        }
        self.trim(&mut q, cap);
        let len = q.len();
        drop(q);
        self.gauges(len);
    }

    pub fn len(&self) -> usize {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub async fn flush_once(state: &AppState) -> Result<usize, StoreError> {
    match flush_by(state, None).await {
        Flushed::Wrote(n) => Ok(n),
        Flushed::Failed(e) => Err(e),
        Flushed::OutOfTime => Ok(0),
    }
}

enum Flushed {
    Wrote(usize),
    Failed(StoreError),
    OutOfTime,
}

async fn within<F>(
    deadline: Option<tokio::time::Instant>,
    write: F,
) -> Option<Result<(), StoreError>>
where
    F: Future<Output = Result<(), StoreError>>,
{
    match deadline {
        Some(d) => tokio::time::timeout_at(d, write).await.ok(),
        None => Some(write.await),
    }
}

async fn flush_by(state: &AppState, deadline: Option<tokio::time::Instant>) -> Flushed {
    let batch = state.logs.take(state.config.logs.flush_rows as usize);
    if batch.is_empty() {
        return Flushed::Wrote(0);
    }
    let taken = batch.len();
    let logs: Vec<&LogRow> = batch.iter().filter_map(|e| e.log.as_ref()).collect();
    match within(deadline, ch::insert_logs(&state.clickhouse, logs)).await {
        Some(Ok(())) => {}
        Some(Err(e)) => return Flushed::Failed(keep_unwritten(state, batch, e)),
        None => {
            put_back(state, batch);
            return Flushed::OutOfTime;
        }
    }
    let unwritten: Vec<Entry> = batch
        .into_iter()
        .filter(|e| e.content.is_some())
        .map(Entry::content_only)
        .collect();
    let contents: Vec<&ContentRow> = unwritten
        .iter()
        .filter_map(|e| e.content.as_ref())
        .collect();
    match within(deadline, ch::insert_contents(&state.clickhouse, contents)).await {
        Some(Ok(())) => Flushed::Wrote(taken),
        Some(Err(e)) => Flushed::Failed(keep_unwritten(state, unwritten, e)),
        None => {
            put_back(state, unwritten);
            Flushed::OutOfTime
        }
    }
}

fn put_back(state: &AppState, entries: Vec<Entry>) {
    state
        .logs
        .put_back(entries, state.config.logs.buffer_size.0);
}

fn keep_unwritten(state: &AppState, entries: Vec<Entry>, error: StoreError) -> StoreError {
    metrics::counter!(crate::metrics::CLICKHOUSE_WRITE_FAILURES).increment(1);
    put_back(state, entries);
    error
}

pub async fn flush_all(state: &AppState, timeout: Duration) -> usize {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut written = 0;
    while !state.logs.is_empty() && tokio::time::Instant::now() < deadline {
        match flush_by(state, Some(deadline)).await {
            Flushed::Wrote(n) => written += n,
            Flushed::Failed(e) => {
                tracing::warn!(error = %e, rows = state.logs.len(), "cannot write request logs at shutdown; retrying");
                let retry = (tokio::time::Instant::now() + WRITE_RETRY_WAIT).min(deadline);
                tokio::time::sleep_until(retry).await;
            }
            Flushed::OutOfTime => break,
        }
    }
    written
}

pub fn spawn_writer(state: AppState, shutdown: CancellationToken) {
    tokio::spawn(async move {
        let interval = state.config.logs.flush_interval;
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(interval) => {}
                () = state.logs.wake.notified() => {}
            }
            loop {
                match flush_once(&state).await {
                    Ok(n) if n >= state.config.logs.flush_rows as usize => continue,
                    Ok(_) => break,
                    Err(e) => {
                        tracing::warn!(error = %e, rows = state.logs.len(), "cannot write request logs; keeping them in memory");
                        tokio::select! {
                            () = shutdown.cancelled() => return,
                            () = tokio::time::sleep(WRITE_RETRY_WAIT) => {}
                        }
                        break;
                    }
                }
            }
        }
    });
}

async fn settings(state: &AppState, project: Uuid) -> Result<Option<LogSettings>, StoreError> {
    let pg = state.pg.clone();
    let found = state
        .caches
        .log_settings
        .get_or_load(&project.to_string(), || async move {
            log_jobs::settings(&pg, project)
                .await
                .map(|s| s.map(Cached::from))
        })
        .await?;
    Ok(found.map(|c| (*c).into()))
}

pub async fn keeps_content(state: &AppState, project: Uuid) -> bool {
    match settings(state, project).await {
        Ok(Some(cfg)) => cfg.org_content && cfg.project_content,
        Ok(None) => false,
        Err(_) => true,
    }
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .log_settings
        .invalidate(&project.to_string())
        .await;
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Cached {
    org_content: bool,
    project_content: bool,
    metadata_days: i32,
    content_days: i32,
}

impl From<LogSettings> for Cached {
    fn from(s: LogSettings) -> Self {
        Cached {
            org_content: s.org_content,
            project_content: s.project_content,
            metadata_days: s.metadata_days,
            content_days: s.content_days,
        }
    }
}

impl From<Cached> for LogSettings {
    fn from(c: Cached) -> Self {
        LogSettings {
            org_content: c.org_content,
            project_content: c.project_content,
            metadata_days: c.metadata_days,
            content_days: c.content_days,
        }
    }
}

pub fn endpoint(ingress: DialectId, kind: RequestKind) -> &'static str {
    match (ingress, kind) {
        (DialectId::AnthropicMessages, RequestKind::CountTokens) => "messages/count_tokens",
        (DialectId::AnthropicMessages, _) => "messages",
        (DialectId::OpenAiResponses, RequestKind::CountTokens) => "responses/input_tokens",
        (DialectId::OpenAiResponses, _) => "responses",
        (DialectId::OpenAiEmbeddings, _) => "embeddings",
        _ => "chat/completions",
    }
}

fn logged_cost(cost: Decimal, request_id: RequestId) -> i128 {
    kaeso_core::cost::cost_raw(cost).unwrap_or_else(|| {
        tracing::warn!(%cost, %request_id, "a request's cost is beyond what the request log holds; it is logged as the largest cost the log holds");
        kaeso_core::cost::saturated_cost_raw(cost)
    })
}

const LAST_EXPIRY: OffsetDateTime = time::macros::datetime!(2106-02-07 06:28:15 UTC);

fn days(at: OffsetDateTime, days: i32) -> OffsetDateTime {
    at.checked_add(time::Duration::days(i64::from(days)))
        .map_or(LAST_EXPIRY, |t| t.min(LAST_EXPIRY))
}

fn ms(d: Duration) -> u32 {
    u32::try_from(d.as_millis()).unwrap_or(u32::MAX)
}

fn reply_json(reply: &Response) -> Result<String, serde_json::Error> {
    serde_json::to_string(reply)
}

pub async fn record(state: &AppState, o: &Outcome) {
    let Some(caller) = &o.caller else { return };
    let cfg = match settings(state, caller.project_id).await {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return,
        Err(e) => {
            metrics::counter!(crate::metrics::LOG_ROWS_DROPPED).increment(1);
            tracing::error!(error = %e, request_id = %o.request_id, "cannot load the project's log settings; the request log is lost");
            return;
        }
    };
    let usage = o.usage.clone().unwrap_or_default();
    let (_, cost) = crate::services::usage::costed(o);
    let progress = o.stream_progress.unwrap_or_default();
    let content = o.content_kept && cfg.org_content && cfg.project_content;
    let request_body = o
        .request_body
        .as_ref()
        .filter(|_| content)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let reply = o
        .reply
        .as_ref()
        .filter(|_| content)
        .and_then(|r| {
            reply_json(r)
                .map_err(|e| tracing::warn!(error = %e, request_id = %o.request_id, "cannot serialize a reply for the request log; it is logged without it"))
                .ok()
        })
        .unwrap_or_default();
    let logical = if content {
        (request_body.len() + reply.len()) as u64
    } else {
        0
    };
    let principal = caller
        .user_id
        .or(caller.service_account_id)
        .unwrap_or(Uuid::nil());
    let log = LogRow {
        request_id: o.request_id.uuid(),
        org_id: caller.org_id,
        project_id: caller.project_id,
        key_id: caller.key_id.unwrap_or(Uuid::nil()),
        principal_kind: if caller.user_id.is_some() {
            "user"
        } else {
            "service_account"
        }
        .into(),
        principal_id: principal,
        started_at: o.started_at,
        finished_at: o.started_at + o.duration,
        duration_ms: ms(o.duration),
        ttfb_ms: o.ttfb.map(ms),
        endpoint: endpoint(o.ingress, o.kind).into(),
        ingress_dialect: o.ingress.as_str().into(),
        egress_dialect: o.egress.map(|d| d.as_str().to_owned()).unwrap_or_default(),
        provider: o
            .custom_provider
            .clone()
            .unwrap_or_else(|| o.provider.unwrap_or_default().into()),
        requested_model: o.requested_model.clone().unwrap_or_default(),
        resolved_model: o.model_id.clone().unwrap_or_default(),
        stream: o.stream,
        status: o.status,
        error_kind: o.error_kind.clone().unwrap_or_default(),
        error_message: o
            .error_message
            .as_deref()
            .map(kaeso_core::secret::redact_tokens)
            .unwrap_or_default(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        reasoning_tokens: usage.reasoning_tokens.unwrap_or(0),
        cost: logged_cost(cost, o.request_id),
        user_agent: o.user_agent.clone().unwrap_or_default(),
        client_ip: o.client_ip.map(|i| i.to_string()).unwrap_or_default(),
        upstream_request_id: o.upstream_request_id.clone().unwrap_or_default(),
        has_content: content,
        logical_bytes: logical,
        expires_at: days(o.started_at, cfg.metadata_days),
        stream_events: progress.events,
        stream_bytes: progress.bytes,
        stream_first_byte_ms: progress.first_byte.map(ms),
        stream_keepalives: progress.keepalives,
        usage_estimated: o.usage_estimated,
        source: o.source.as_str().into(),
        overhead_ms: o.overhead.map(ms),
        stream_processing_ms: o.stream_processing.map(ms),
        cache_breakpoints: o
            .cache_breakpoints
            .map(|n| u16::try_from(n).unwrap_or(u16::MAX)),
    };
    let content = content.then(|| ContentRow {
        request_id: log.request_id,
        org_id: log.org_id,
        project_id: log.project_id,
        started_at: log.started_at,
        request_body,
        reply,
        expires_at: days(o.started_at, cfg.content_days.min(cfg.metadata_days)),
    });
    let size = 512 + logical + text_bytes(&log);
    let cfg = &state.config.logs;
    state.logs.push(
        Entry {
            log: Some(log),
            content,
            size,
        },
        cfg.buffer_size.0,
        cfg.flush_rows as usize,
    );
}

fn text_bytes(log: &LogRow) -> u64 {
    [
        &log.requested_model,
        &log.resolved_model,
        &log.error_kind,
        &log.error_message,
        &log.user_agent,
        &log.upstream_request_id,
        &log.egress_dialect,
        &log.provider,
    ]
    .iter()
    .map(|s| s.len() as u64)
    .sum()
}

#[derive(Debug, Default)]
pub struct Recorder;

impl Sink for Recorder {
    fn finish<'a>(&'a self, state: &'a AppState, outcome: Outcome) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            crate::services::usage::account(state, &outcome).await;
            record(state, &outcome).await;
        })
    }
}

pub async fn run_jobs(state: &AppState) -> Result<String, StoreError> {
    let jobs = log_jobs::pending(&state.pg, 20).await?;
    let mut done = 0;
    for job in jobs {
        let result = ch::delete_range(
            &state.clickhouse,
            job.org_id,
            job.project_id,
            job.from_at,
            job.to_at,
            job.kind == "shrink_content",
        )
        .await;
        let error = result.as_ref().err().map(ToString::to_string);
        log_jobs::finish(&state.pg, job.id, state.clock.now(), error.as_deref()).await?;
        if error.is_none() {
            done += 1;
        }
    }
    Ok(format!("{done} log jobs done"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_dates_stop_at_the_last_second_the_log_stores() {
        let at = time::macros::datetime!(2026-10-16 12:00 UTC);
        assert_eq!(days(at, 30), time::macros::datetime!(2026-11-15 12:00 UTC));
        assert_eq!(days(at, 36_500), LAST_EXPIRY);
        assert_eq!(days(at, i32::MAX), LAST_EXPIRY);
        assert_eq!(days(OffsetDateTime::UNIX_EPOCH, i32::MAX), LAST_EXPIRY);
    }

    fn raw(json: &str) -> kaeso_kpp::RawJson {
        kaeso_kpp::RawJson::from_string(json.to_owned()).unwrap()
    }

    fn logged_replies() -> Vec<(&'static str, Response)> {
        use kaeso_kpp::{
            Block, CacheWriteBreakdown, Citation, Opaque, OpaqueKind, RawItem, RawUsage,
            ReasoningBlock, RefusalBlock, StopReason, TextBlock, Usage,
        };
        let anthropic = DialectId::AnthropicMessages;
        let responses = DialectId::OpenAiResponses;

        let mut text = TextBlock::new("Paris is the capital of France.");
        text.citations.push(Citation::new(
            anthropic,
            raw(r#"{"type":"char_location","cited_text":"Paris","document_index":0}"#),
        ));
        let mut plain = Response::new("msg_01", "claude-sonnet-4-5", vec![Block::Text(text)]);
        plain.usage = Usage::new(12, 8);
        plain.usage.cache_read_tokens = 4;
        plain.usage.cache_write_tokens = 6;
        plain.usage.cache_write_breakdown = Some(CacheWriteBreakdown::new(6, 0));
        plain.usage.raw = Some(RawUsage::new(
            anthropic,
            serde_json::json!({"service_tier": "standard"}),
        ));

        let mut tools = Response::new(
            "resp_01",
            "gpt-5",
            vec![
                Block::text("Checking both cities."),
                Block::tool_call("call_1", "get_weather", raw(r#"{"city":"Paris"}"#)),
                Block::tool_call("call_2", "get_weather", raw(r#"{"city":"Berlin"}"#)),
            ],
        );
        tools.stop = StopReason::ToolUse;
        tools.usage = Usage::new(40, 22);

        let mut thinking = ReasoningBlock::new(anthropic);
        thinking.text.push("The user asks for a sum.".into());
        thinking.opaque.push(Opaque::new(
            anthropic,
            OpaqueKind::AnthropicThinkingSignature,
            "c2lnbmF0dXJl",
        ));
        let mut summary = ReasoningBlock::new(responses);
        summary.summary.push("Added the numbers.".into());
        summary.opaque.push(Opaque::new(
            responses,
            OpaqueKind::OpenAiEncryptedReasoning,
            "ZW5jcnlwdGVk",
        ));
        let mut redacted = ReasoningBlock::new(anthropic);
        redacted.opaque.push(Opaque::new(
            anthropic,
            OpaqueKind::AnthropicRedactedThinking,
            "cmVkYWN0ZWQ=",
        ));
        let mut reasoning = Response::new(
            "msg_02",
            "claude-opus-4-5",
            vec![
                Block::Reasoning(thinking),
                Block::Reasoning(summary),
                Block::Reasoning(redacted),
                Block::text("4"),
            ],
        );
        reasoning.stop = StopReason::MaxTokens;
        reasoning.usage = Usage::new(30, 120);
        reasoning.usage.reasoning_tokens = Some(100);

        let mut refusal = Response::new(
            "chatcmpl_01",
            "gpt-4o",
            vec![Block::Refusal(RefusalBlock::new("I can't help with that."))],
        );
        refusal.stop = StopReason::Refusal;
        refusal.usage = Usage::new(9, 7);

        let mut unknown = Response::new(
            "msg_03",
            "claude-sonnet-4-5",
            vec![
                Block::Unknown(RawItem::new(
                    anthropic,
                    raw(
                        r#"{"type":"server_tool_use","id":"srvtoolu_01","name":"web_search","input":{"query":"kaeso"}}"#,
                    ),
                )),
                Block::text("Found it."),
            ],
        );
        unknown.stop = StopReason::StopSequence("END".into());
        unknown.usage = Usage::new(20, 5);

        let mut other = Response::new("resp_02", "gpt-5", vec![]);
        other.stop = StopReason::Other("incomplete".into());
        other.usage = Usage::new(3, 0);

        vec![
            ("text", plain),
            ("tool_calls", tools),
            ("reasoning", reasoning),
            ("refusal", refusal),
            ("unknown_blocks", unknown),
            ("other_stop", other),
        ]
    }

    #[test]
    fn logged_replies_keep_the_v0_1_form() {
        for (name, reply) in logged_replies() {
            let stored = reply_json(&reply).unwrap();
            let value: serde_json::Value = serde_json::from_str(&stored).unwrap();
            assert!(value.get("format").is_none(), "{name}");
            insta::assert_snapshot!(
                format!("logged_reply_{name}"),
                serde_json::to_string_pretty(&value).unwrap()
            );
            let back: Response = serde_json::from_str(&stored).unwrap();
            assert_eq!(back, reply, "{name}");
        }
    }

    #[test]
    fn costs_beyond_the_log_are_saturated_instead_of_lost() {
        let id = RequestId::new();
        assert_eq!(logged_cost(Decimal::new(21, 3), id), 21_000_000_000);
        assert_eq!(
            logged_cost(Decimal::MAX, id),
            kaeso_core::cost::COST_RAW_MAX
        );
    }
}
