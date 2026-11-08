use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use futures::future::BoxFuture;
use kaeso_codecs::gemini::GeminiAction;
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, RequestKind, Response};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::logs::{self as ch, ContentRow, LogRow};
use kaeso_store::clickhouse::tool_calls::{self, ToolCallContentRow, ToolCallRow};
use kaeso_store::pg::log_jobs::{self, LogSettings};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::model_api::hooks::{Outcome, Sink};
use crate::state::AppState;

const WRITE_RETRY_WAIT: Duration = Duration::from_secs(2);
// ClickHouse's client sets no timeouts of its own, so a stalled server would hold the writer, and
// the batch it took, until the gateway restarts.
const WRITE_DEADLINE: Duration = Duration::from_secs(60);

struct Entry {
    seq: u64,
    log: Option<LogRow>,
    content: Option<ContentRow>,
    call: Option<ToolCallRow>,
    call_content: Option<ToolCallContentRow>,
    size: u64,
}

impl Entry {
    fn holds_any(&self) -> bool {
        self.log.is_some()
            || self.content.is_some()
            || self.call.is_some()
            || self.call_content.is_some()
    }
}

// Every finished request lands here, so the queue is split per worker thread; rows carry a
// sequence number so taking and trimming still go oldest first across all shards.
const SHARDS: usize = 16;

#[derive(Default)]
pub struct Buffer {
    shards: [Mutex<VecDeque<Entry>>; SHARDS],
    next: AtomicU64,
    rows: AtomicUsize,
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

type Shard<'a> = MutexGuard<'a, VecDeque<Entry>>;

fn oldest(shards: &mut [Shard<'_>]) -> Option<Entry> {
    let mut queues: Vec<&mut VecDeque<Entry>> = shards.iter_mut().map(|q| &mut **q).collect();
    oldest_of(&mut queues)
}

fn oldest_of<Q: std::borrow::BorrowMut<VecDeque<Entry>>>(queues: &mut [Q]) -> Option<Entry> {
    let mut first: Option<(u64, usize)> = None;
    for (i, q) in queues.iter().enumerate() {
        if let Some(e) = q.borrow().front()
            && first.is_none_or(|(seq, _)| e.seq < seq)
        {
            first = Some((e.seq, i));
        }
    }
    first
        .and_then(|(_, i)| queues.get_mut(i))
        .and_then(|q| q.borrow_mut().pop_front())
}

impl Buffer {
    fn lock_all(&self) -> Vec<Shard<'_>> {
        self.shards
            .iter()
            .map(|q| q.lock().unwrap_or_else(PoisonError::into_inner))
            .collect()
    }

    // The counters grow before a row is queued and shrink after it leaves, so they never
    // fall below what the queues hold.
    fn counted(&self, e: &Entry) {
        self.rows.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(e.size, Ordering::Relaxed);
    }

    fn uncounted(&self, e: &Entry) {
        self.rows.fetch_sub(1, Ordering::Relaxed);
        self.bytes.fetch_sub(e.size, Ordering::Relaxed);
    }

    fn push(&self, mut entry: Entry, cap: u64, flush_rows: usize) {
        entry.seq = self.next.fetch_add(1, Ordering::Relaxed);
        self.counted(&entry);
        let shard = crate::shard::of_this_thread(SHARDS);
        if let Some(q) = self.shards.get(shard) {
            q.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_back(entry);
        }
        if self.bytes.load(Ordering::Relaxed) > cap {
            self.trim(&mut self.lock_all(), cap);
        }
        let rows = self.len();
        self.gauges(rows);
        if rows >= flush_rows {
            self.wake.notify_one();
        }
    }

    fn trim(&self, shards: &mut [Shard<'_>], cap: u64) {
        while self.bytes.load(Ordering::Relaxed) > cap {
            let Some(old) = oldest(shards) else { break };
            self.uncounted(&old);
            self.dropped.fetch_add(1, Ordering::Relaxed);
            metrics::counter!(crate::metrics::LOG_ROWS_DROPPED).increment(1);
        }
    }

    fn gauges(&self, rows: usize) {
        metrics::gauge!(crate::metrics::LOG_BUFFER_ROWS).set(rows as f64);
        metrics::gauge!(crate::metrics::LOG_BUFFER_BYTES)
            .set(self.bytes.load(Ordering::Relaxed) as f64);
    }

    // Each shard's queue is swapped out under its lock, and the oldest rows across them are merged
    // with no lock held: a request that finishes meanwhile never waits for the merge.
    fn take(&self, max: usize) -> Vec<Entry> {
        let mut queues: Vec<VecDeque<Entry>> = self
            .shards
            .iter()
            .map(|q| std::mem::take(&mut *q.lock().unwrap_or_else(PoisonError::into_inner)))
            .collect();
        let held: usize = queues.iter().map(VecDeque::len).sum();
        let mut batch = Vec::with_capacity(max.min(held));
        while batch.len() < max {
            let Some(e) = oldest_of(&mut queues) else {
                break;
            };
            batch.push(e);
        }
        for (q, mut rest) in self.shards.iter().zip(queues) {
            if rest.is_empty() {
                continue;
            }
            let mut q = q.lock().unwrap_or_else(PoisonError::into_inner);
            rest.append(&mut q);
            *q = rest;
        }
        for e in &batch {
            self.uncounted(e);
        }
        self.gauges(self.len());
        batch
    }

    fn put_back(&self, batch: Vec<Entry>, cap: u64) {
        let mut shards = self.lock_all();
        for e in batch.into_iter().rev() {
            self.counted(&e);
            let shard = usize::try_from(e.seq % SHARDS as u64).unwrap_or(0);
            if let Some(q) = shards.get_mut(shard) {
                q.push_front(e);
            }
        }
        self.trim(&mut shards, cap);
        drop(shards);
        self.gauges(self.len());
    }

    pub fn len(&self) -> usize {
        self.rows.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub async fn flush_once(state: &AppState) -> Result<usize, StoreError> {
    flush_within(state, WRITE_DEADLINE).await
}

pub async fn flush_within(state: &AppState, wait: Duration) -> Result<usize, StoreError> {
    match flush_by(state, Some(tokio::time::Instant::now() + wait)).await {
        Flushed::Wrote(n) => Ok(n),
        Flushed::Failed(e) => Err(e),
        Flushed::OutOfTime => {
            metrics::counter!(crate::metrics::CLICKHOUSE_WRITE_FAILURES).increment(1);
            tracing::warn!(waited = ?wait, rows = state.logs.len(), "ClickHouse did not take request logs in time; keeping them in memory");
            Ok(0)
        }
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
    let Some(clickhouse) = state.clickhouse.as_ref() else {
        return Flushed::Wrote(0);
    };
    let batch = state.logs.take(state.config.logs.flush_rows as usize);
    if batch.is_empty() {
        return Flushed::Wrote(0);
    }
    let taken = batch.len();
    // One table at a time, each row dropped from the batch once written: a later table that
    // fails puts back only what it still holds, so nothing is written twice.
    let mut batch = batch;
    for stage in [
        Stage::Logs,
        Stage::Calls,
        Stage::Contents,
        Stage::CallContents,
    ] {
        let written = match stage {
            Stage::Logs => {
                let rows: Vec<&LogRow> = batch.iter().filter_map(|e| e.log.as_ref()).collect();
                within(deadline, ch::insert_logs(clickhouse, rows)).await
            }
            Stage::Calls => {
                let rows: Vec<&ToolCallRow> =
                    batch.iter().filter_map(|e| e.call.as_ref()).collect();
                within(deadline, tool_calls::insert_calls(clickhouse, rows)).await
            }
            Stage::Contents => {
                let rows: Vec<&ContentRow> =
                    batch.iter().filter_map(|e| e.content.as_ref()).collect();
                within(deadline, ch::insert_contents(clickhouse, rows)).await
            }
            Stage::CallContents => {
                let rows: Vec<&ToolCallContentRow> = batch
                    .iter()
                    .filter_map(|e| e.call_content.as_ref())
                    .collect();
                within(deadline, tool_calls::insert_contents(clickhouse, rows)).await
            }
        };
        match written {
            Some(Ok(())) => {}
            Some(Err(e)) => return Flushed::Failed(keep_unwritten(state, batch, e)),
            None => {
                put_back(state, batch);
                return Flushed::OutOfTime;
            }
        }
        batch = batch
            .into_iter()
            .map(|mut e| {
                match stage {
                    Stage::Logs => e.log = None,
                    Stage::Calls => e.call = None,
                    Stage::Contents => e.content = None,
                    Stage::CallContents => e.call_content = None,
                }
                e
            })
            .filter(Entry::holds_any)
            .collect();
    }
    Flushed::Wrote(taken)
}

#[derive(Clone, Copy)]
enum Stage {
    Logs,
    Calls,
    Contents,
    CallContents,
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

pub fn spawn_writer(state: AppState, draining: CancellationToken, stop: CancellationToken) {
    if !state.request_logs() {
        return;
    }
    tokio::spawn(async move {
        loop {
            let every = if draining.is_cancelled() {
                crate::serve::SHUTDOWN_FLUSH_INTERVAL.min(state.config.logs.flush_interval)
            } else {
                state.config.logs.flush_interval
            };
            tokio::select! {
                () = stop.cancelled() => return,
                () = draining.cancelled(), if !draining.is_cancelled() => {}
                () = tokio::time::sleep(every) => {}
                () = state.logs.wake.notified() => {}
            }
            loop {
                match flush_once(&state).await {
                    Ok(n) if n >= state.config.logs.flush_rows as usize => continue,
                    Ok(_) => break,
                    Err(e) => {
                        tracing::warn!(error = %e, rows = state.logs.len(), "cannot write request logs; keeping them in memory");
                        tokio::select! {
                            () = stop.cancelled() => return,
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
        .get_or_refresh(&project.to_string(), move || async move {
            log_jobs::settings(&pg, project)
                .await
                .map(|s| s.map(Cached::from))
        })
        .await?;
    Ok(found.map(|c| (*c).into()))
}

pub async fn keeps_content(state: &AppState, project: Uuid) -> bool {
    if !state.request_logs() {
        return false;
    }
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

pub fn endpoint(
    ingress: DialectId,
    kind: RequestKind,
    gemini: Option<GeminiAction>,
) -> &'static str {
    if let Some(action) = gemini {
        return match action {
            GeminiAction::StreamGenerateContent => "models:streamGenerateContent",
            GeminiAction::CountTokens => "models:countTokens",
            GeminiAction::EmbedContent => "models:embedContent",
            GeminiAction::BatchEmbedContents => "models:batchEmbedContents",
            _ => "models:generateContent",
        };
    }
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
    if !state.request_logs() {
        return;
    }
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
        endpoint: endpoint(o.ingress, o.kind, o.gemini).into(),
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
        request_upload_ms: o.upload.map(ms),
        response_cache: o
            .response_cache
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default(),
        alias: o.alias.clone().unwrap_or_default(),
        fallbacks: if o.fallbacks.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&o.fallbacks).unwrap_or_default()
        },
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
            seq: 0,
            log: Some(log),
            content,
            call: None,
            call_content: None,
            size,
        },
        cfg.buffer_size.0,
        cfg.flush_rows as usize,
    );
}

// A tool call joins the request log's queue, written in the same batches; its content is kept
// only where the organization and the project keep request content.
pub async fn record_tool_call(
    state: &AppState,
    mut row: ToolCallRow,
    content: Option<(String, String)>,
) {
    if !state.request_logs() {
        return;
    }
    let cfg = match settings(state, row.project_id).await {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return,
        Err(e) => {
            metrics::counter!(crate::metrics::LOG_ROWS_DROPPED).increment(1);
            tracing::error!(error = %e, call = %row.id, "cannot load the project's log settings; the tool call's log is lost");
            return;
        }
    };
    row.expires_at = days(row.started_at, cfg.metadata_days);
    let content = content.filter(|_| cfg.org_content && cfg.project_content);
    row.has_content = content.is_some();
    let call_content = content.map(|(arguments, result)| ToolCallContentRow {
        id: row.id,
        org_id: row.org_id,
        project_id: row.project_id,
        started_at: row.started_at,
        arguments,
        result,
        expires_at: days(row.started_at, cfg.content_days.min(cfg.metadata_days)),
    });
    let size = 512
        + (row.tool.len()
            + row.upstream_tool.len()
            + row.error_message.len()
            + row.client_name.len()) as u64
        + call_content
            .as_ref()
            .map_or(0, |c| (c.arguments.len() + c.result.len()) as u64);
    let limits = &state.config.logs;
    state.logs.push(
        Entry {
            seq: 0,
            log: None,
            content: None,
            call: Some(row),
            call_content,
            size,
        },
        limits.buffer_size.0,
        limits.flush_rows as usize,
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

    fn finish_charged<'a>(
        &'a self,
        state: &'a AppState,
        outcome: Outcome,
        charged: tokio::sync::oneshot::Sender<()>,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            crate::services::usage::account(state, &outcome).await;
            let _ = charged.send(());
            record(state, &outcome).await;
        })
    }
}

pub async fn run_jobs(state: &AppState) -> Result<String, StoreError> {
    let Some(clickhouse) = state.clickhouse.as_ref() else {
        return Ok("request logs are off; log jobs wait".to_owned());
    };
    let jobs = log_jobs::pending(&state.pg, 20).await?;
    let mut done = 0;
    for job in jobs {
        let result = ch::delete_range(
            clickhouse,
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

    fn row(size: u64) -> Entry {
        Entry {
            seq: 0,
            log: None,
            content: None,
            call: None,
            call_content: None,
            size,
        }
    }

    fn seqs(entries: &[Entry]) -> Vec<u64> {
        entries.iter().map(|e| e.seq).collect()
    }

    #[test]
    fn rows_from_every_thread_are_taken_put_back_and_dropped_oldest_first() {
        let buffer = std::sync::Arc::new(Buffer::default());
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let buffer = buffer.clone();
                std::thread::spawn(move || {
                    for _ in 0..25 {
                        buffer.push(row(10), u64::MAX, usize::MAX);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(buffer.len(), 100);

        let first = buffer.take(30);
        assert_eq!(seqs(&first), (0..30).collect::<Vec<_>>());
        buffer.put_back(first, u64::MAX);
        assert_eq!(buffer.len(), 100);

        buffer.push(row(10), 500, usize::MAX);
        assert_eq!(buffer.len(), 50);
        assert_eq!(buffer.dropped(), 51);
        assert_eq!(
            seqs(&buffer.take(usize::MAX)),
            (51..=100).collect::<Vec<_>>()
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn a_partial_take_leaves_the_newer_rows_in_order_for_the_next() {
        let buffer = std::sync::Arc::new(Buffer::default());
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let buffer = buffer.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        buffer.push(row(10), u64::MAX, usize::MAX);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let mut taken = Vec::new();
        loop {
            let batch = buffer.take(37);
            if batch.is_empty() {
                break;
            }
            assert!(batch.len() <= 37);
            taken.extend(seqs(&batch));
        }
        assert_eq!(taken, (0..400).collect::<Vec<_>>());
        assert!(buffer.is_empty());
    }

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
