//! Requests and tool calls a source reported: Kaeso never made them, so it only logs and counts
//! them, as the gateway does its own, under the source that sent them. Nothing here can refuse a
//! call; a refused record only means Kaeso doesn't keep it.

use std::collections::HashMap;
use std::time::Duration;

use kaeso_core::catalogue::Fields;
use kaeso_core::id::RequestId;
use kaeso_core::kpmr::IdSetting;
use kaeso_core::rights::{ProjectRight, effective_project_rights};
use kaeso_core::sources::{Counted, SourceKind};
use kaeso_kpp::{
    Block, CacheWriteBreakdown, DialectId, GenerateRequest, Message, Request, Response, Role, Usage,
};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::tool_calls::ToolCallRow;
use kaeso_store::pg::sources::{Activity, Source};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::model_api::hooks::{Caller, Source as Origin};
use crate::state::AppState;

/// How long Kaeso remembers a record it logged, so an exporter that sends it again isn't counted
/// twice; older records are refused, as they could no longer be told apart from duplicates.
const REMEMBERED: Duration = Duration::from_secs(7 * 24 * 3600);
const VALKEY_DEADLINE: Duration = Duration::from_secs(2);
/// The longest error message a log entry keeps of what a source said went wrong.
const MAX_ERROR: usize = 1_000;
/// The most characters of a tool's or a model's name, as the console shows them.
const MAX_NAME: usize = 200;
/// The most of each token count one request may report; more is a broken or hostile record.
const MAX_TOKENS: u64 = 10_000_000;
/// The most one request may report it cost, in USD.
const MAX_COST: u64 = 100;
/// How far ahead of Kaeso's clock a record may be, for clients whose clocks run fast.
const CLOCK_SKEW: time::Duration = time::Duration::minutes(5);
/// Providers that make the models they list, preferred when several providers list a model.
const MAKERS: &[&str] = &[
    "openai",
    "anthropic",
    "google",
    "mistral",
    "xai",
    "deepseek",
    "cohere",
    "moonshotai",
    "zai",
    "minimax",
    "alibaba",
    "perplexity",
];

/// Why Kaeso didn't keep a record a source sent, as the source's `last_error` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The record names no model.
    NoModel,
    /// The record names no tool.
    NoTool,
    /// The record has none of the ids its format requires.
    NoId,
    /// A token count beyond what one request can use.
    TooManyTokens,
    /// A cost beyond what one request can cost.
    CostTooHigh,
    /// A status that isn't an HTTP status.
    BadStatus,
    /// Older than the 7 days Kaeso remembers records for.
    TooOld,
    /// Dated in the future.
    InFuture,
    /// The source counts for nobody: its member left and no email address matched.
    Nobody,
    /// The body isn't what the endpoint reads.
    Unreadable,
    /// The source is disabled or archived.
    Inactive,
    /// None of the batch's records is one a reader knows, such as the events of a client Kaeso
    /// has no reader for.
    Unrecognized,
}

impl Refusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Refusal::NoModel => "no_model",
            Refusal::NoTool => "no_tool",
            Refusal::NoId => "no_id",
            Refusal::TooManyTokens => "too_many_tokens",
            Refusal::CostTooHigh => "cost_too_high",
            Refusal::BadStatus => "bad_status",
            Refusal::TooOld => "too_old",
            Refusal::InFuture => "in_future",
            Refusal::Nobody => "nobody",
            Refusal::Unreadable => "unreadable",
            Refusal::Inactive => "inactive",
            Refusal::Unrecognized => "unrecognized",
        }
    }
}

/// A request a source reported, its token counts already the way Kaeso counts them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObservedRequest {
    /// What the source calls the record, unique within the source: the provider's request id,
    /// or the record's own id. A record sent again with the same id is logged once.
    pub external_id: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration: Duration,
    pub ttfb: Option<Duration>,
    /// The provider's id as KPMR names it, when the source says or Kaeso can tell.
    pub provider: Option<String>,
    /// `provider` only says how the client is set up, such as its sign-in, not which provider
    /// answered: a client pointed at a gateway reports its usual provider for any model, so a
    /// model that provider doesn't sell is its maker's.
    pub provider_guessed: bool,
    /// The reasoning effort the record says the request asked for, such as `high`.
    pub reasoning_effort: Option<String>,
    pub model: String,
    /// 200 for an answered request, else the provider's HTTP status or 502.
    pub status: u16,
    pub error_message: Option<String>,
    pub tokens: Counted,
    /// What the source said the request cost, in USD.
    pub reported_cost: Option<Decimal>,
    pub stream: bool,
    /// The client and its version, as a user agent, such as `claude-cli/2.1.322`.
    pub client: Option<String>,
    /// The person the source says made the request.
    pub email: Option<String>,
    pub tags: Vec<String>,
    /// The provider's id of the request, such as Anthropic's `req_…`, when the source names it.
    pub provider_request_id: Option<String>,
    /// The format the client speaks with its provider, for the log's request view.
    pub dialect: Option<DialectId>,
    /// What was asked and answered, when the source sends it; kept only where content is kept.
    pub prompt: Option<String>,
    pub reply: Option<String>,
    /// The whole exchange, when the source sends more than the last prompt and the reply's text:
    /// the system prompt, every message before the reply, and the reply's parts with its tool
    /// calls and reasoning. Each takes the place of `prompt` or `reply` when it isn't empty.
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub reply_parts: Vec<Block>,
    /// Other ids the same call goes by in the source's records, such as the span a GenAI log
    /// event was written in: a call reported under any of them is counted once.
    pub aliases: Vec<String>,
    /// What the request takes from an earlier batch of the same source.
    pub wants: Vec<Want>,
    /// The client names a provider Kaeso can't tell, such as a local model server, so neither
    /// the kind's usual provider nor a seller of the model is assumed.
    pub unguessed: bool,
}

/// What a record can leave to a later batch of its source: clients export in batches, so a
/// prompt, the moment a request began or a conversation's provider can arrive one export before
/// the request it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// The prompt a request answered; the first request that asks for it takes it.
    Prompt,
    /// When a request began and how long its response's headers took, as `<unix ms>:<ms>`; the
    /// first request that asks for it takes it.
    Began,
    /// The provider a conversation talks to.
    Provider,
    /// The `contents` (JSON) of the Gemini request a reply answered; the first request that asks
    /// for it takes it.
    GeminiContents,
}

impl Wanted {
    const fn as_str(self) -> &'static str {
        match self {
            Wanted::Prompt => "prompt",
            Wanted::Began => "began",
            Wanted::Provider => "provider",
            Wanted::GeminiContents => "gemini",
        }
    }

    // A prompt or a start belongs to a request that follows within moments; a conversation's
    // provider holds for as long as people keep one open.
    const fn kept(self) -> u64 {
        match self {
            Wanted::Prompt | Wanted::Began | Wanted::GeminiContents => 3_600,
            Wanted::Provider => 86_400,
        }
    }
}

/// What a reader carries for a conversation whose client names a provider Kaeso can't tell.
pub const UNNAMED_PROVIDER: &str = "-";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub what: Wanted,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    pub what: Wanted,
    pub key: String,
    pub value: String,
}

/// A tool call a source reported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedToolCall {
    pub external_id: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration: Duration,
    pub tool: String,
    /// The MCP server that has the tool; none for the client's built-in tools.
    pub server: Option<String>,
    pub ok: bool,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub client: Option<String>,
    pub email: Option<String>,
    pub arguments: Option<String>,
    pub result: Option<String>,
    /// Other ids the same call goes by, as for a request.
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    pub requests: Vec<ObservedRequest>,
    pub tool_calls: Vec<ObservedToolCall>,
    /// What the batch's records leave for the source's next batches.
    pub carried: Vec<Carried>,
    /// Records the reader couldn't use, and why the last one was refused.
    pub refused: u32,
    pub refusal: Option<Refusal>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorded {
    pub requests: usize,
    pub tool_calls: usize,
    pub duplicates: usize,
    pub refused: usize,
    /// Why the last refused record was refused.
    pub refusal: Option<Refusal>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("valkey: {0}")]
    Valkey(String),
}

/// The record's request id: the same for the same record of the same source, so even a
/// duplicate that got past the dedupe window writes the same id.
pub fn request_id(source: Uuid, external_id: &str) -> Uuid {
    let mut h = Sha256::new();
    h.update(source.as_bytes());
    h.update(external_id.as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(d.get(..16).unwrap_or(&[0u8; 16]));
    uuid::Builder::from_custom_bytes(b).into_uuid()
}

fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => s.get(..at).unwrap_or(s).to_owned(),
        None => s.to_owned(),
    }
}

// Marks every id of each record as seen and says which records were new: a record is new only
// when none of its ids was seen before. Valkey decides atomically across instances.
async fn first_time(
    state: &AppState,
    source: Uuid,
    records: &[Vec<Uuid>],
) -> Result<Vec<bool>, RecordError> {
    if records.iter().all(Vec::is_empty) {
        return Ok(vec![false; records.len()]);
    }
    let mut pipe = redis::pipe();
    for id in records.iter().flatten() {
        pipe.cmd("SET")
            .arg(state.valkey.key(&format!("obs:{source}:{id}")))
            .arg(1)
            .arg("NX")
            .arg("EX")
            .arg(REMEMBERED.as_secs());
    }
    let mut conn = state.valkey.conn();
    let set: Vec<Option<String>> =
        tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async(&mut conn))
            .await
            .map_err(|_| RecordError::Valkey("timed out".into()))?
            .map_err(|e| RecordError::Valkey(e.to_string()))?;
    let mut set = set.into_iter();
    Ok(records
        .iter()
        .map(|ids| {
            ids.iter()
                .fold(true, |new, _| set.next().flatten().is_some() && new)
        })
        .collect())
}

fn carry_key(state: &AppState, source: Uuid, what: Wanted, key: &str) -> String {
    state
        .valkey
        .key(&format!("obscarry:{source}:{}:{key}", what.as_str()))
}

/// Keeps what the batch leaves for later batches, and hands each request what it wants from
/// earlier ones. Returns the keys requests took, to forget once the batch is recorded: a batch
/// that fails is sent again and must find them still there.
async fn carry(
    state: &AppState,
    source: &Source,
    batch: &mut Batch,
) -> Result<Vec<String>, RecordError> {
    // Content waits in Valkey only for a source that keeps it.
    let kept = |what: Wanted| source.content || matches!(what, Wanted::Began | Wanted::Provider);
    batch.carried.retain(|c| kept(c.what));
    for r in &mut batch.requests {
        r.wants.retain(|w| kept(w.what));
    }
    let source = source.id;
    let mut conn = state.valkey.conn();
    if !batch.carried.is_empty() {
        let mut pipe = redis::pipe();
        for c in &batch.carried {
            pipe.cmd("SET")
                .arg(carry_key(state, source, c.what, &c.key))
                .arg(&c.value)
                .arg("EX")
                .arg(c.what.kept())
                .ignore();
        }
        tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async::<()>(&mut conn))
            .await
            .map_err(|_| RecordError::Valkey("timed out".into()))?
            .map_err(|e| RecordError::Valkey(e.to_string()))?;
    }
    let mut wanted: Vec<String> = batch
        .requests
        .iter()
        .flat_map(|r| &r.wants)
        .map(|w| carry_key(state, source, w.what, &w.key))
        .collect();
    wanted.sort();
    wanted.dedup();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let found: Vec<Option<String>> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::cmd("MGET").arg(&wanted).query_async(&mut conn),
    )
    .await
    .map_err(|_| RecordError::Valkey("timed out".into()))?
    .map_err(|e| RecordError::Valkey(e.to_string()))?;
    let mut found: HashMap<String, String> = wanted
        .into_iter()
        .zip(found)
        .filter_map(|(k, v)| Some((k, v?)))
        .collect();
    let mut taken = Vec::new();
    for r in &mut batch.requests {
        for w in std::mem::take(&mut r.wants) {
            let key = carry_key(state, source, w.what, &w.key);
            let value = match w.what {
                Wanted::Provider => found.get(&key).cloned(),
                Wanted::Prompt | Wanted::Began | Wanted::GeminiContents => {
                    found.remove(&key).inspect(|_| taken.push(key))
                }
            };
            if let Some(value) = value {
                fill(r, w.what, &value);
            }
        }
    }
    Ok(taken)
}

/// Gives a request what an earlier batch left for it.
fn fill(r: &mut ObservedRequest, what: Wanted, value: &str) {
    match what {
        Wanted::Prompt => {
            if r.prompt.is_none() && r.messages.is_empty() {
                r.prompt = Some(value.to_owned());
            }
        }
        Wanted::Provider => {
            if r.provider.is_none() && !r.unguessed {
                match value {
                    UNNAMED_PROVIDER => r.unguessed = true,
                    named => {
                        r.provider = Some(named.to_owned());
                        r.provider_guessed = true;
                    }
                }
            }
        }
        Wanted::GeminiContents => {
            if !r.messages.is_empty() {
                return;
            }
            let exchange = serde_json::from_str(value)
                .ok()
                .and_then(|v| crate::ingest::content::gemini_contents(&v));
            if let Some(e) = exchange {
                if r.prompt.is_none() {
                    r.prompt = e.last_user_text();
                }
                r.messages = e.messages;
            }
        }
        Wanted::Began => {
            let Some((at, headers)) = value.split_once(':') else {
                return;
            };
            let (Ok(at), Ok(headers)) = (at.parse::<i128>(), headers.parse::<u64>()) else {
                return;
            };
            let Ok(began) = OffsetDateTime::from_unix_timestamp_nanos(at * 1_000_000) else {
                return;
            };
            let Some(end) = r.started_at.map(|s| s + r.duration) else {
                return;
            };
            if began <= end {
                r.started_at = Some(began);
                r.duration = (end - began).try_into().unwrap_or_default();
                r.ttfb = Some(Duration::from_millis(headers));
            }
        }
    }
}

/// How a request made through the gateway is told apart from the copy its client reports to a
/// source: the gateway's id never reaches the client, so the copy is matched on what both name.
/// A copy that starts or ends within `SAME_TIME` of a gateway request of the same organization
/// and model, with the same tokens, or failed as it did, is that request.
const SAME_TIME: time::Duration = time::Duration::seconds(5);
/// How long the gateway's requests wait for their copies. Clients export every few seconds, and
/// each request the gateway answers holds a key this long, so it stays short.
const GATEWAY_KEPT: u64 = 120;
/// How long a fresh record that matches nothing waits for its gateway request to be noted, when
/// the organization's gateway served the same model lately. Requests are noted before their
/// reply ends, so this only covers one that ended without its app, noted after the fact.
const HOLD: Duration = Duration::from_millis(1_500);
/// The longest the end of a reply waits for its note.
const NOTE_DEADLINE: Duration = Duration::from_millis(250);

fn model_tag(model: &str) -> String {
    model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_ascii_lowercase()
}

// A request that failed is matched by its model and time alone: what a client reports of a
// failure, and what the gateway estimated of it, rarely agree on tokens.
fn fingerprint(state: &AppState, org: Uuid, model: &str, usage: Option<&Usage>) -> String {
    let mut h = Sha256::new();
    h.update(model_tag(model).as_bytes());
    match usage {
        Some(usage) => {
            let input = usage
                .input_tokens
                .saturating_add(usage.cache_read_tokens)
                .saturating_add(usage.cache_write_tokens);
            h.update(format!(":{input}:{}", usage.output_tokens).as_bytes());
        }
        None => h.update(b":failed"),
    }
    let d = h.finalize();
    let tag: String = d.iter().take(12).map(|b| format!("{b:02x}")).collect();
    state.valkey.key(&format!("obsgw:{org}:{tag}"))
}

// Set while the organization's gateway serves a model, so a fresh copy that matches nothing
// yet is worth a second look.
fn serving(state: &AppState, org: Uuid, model: &str) -> String {
    let mut h = Sha256::new();
    h.update(model_tag(model).as_bytes());
    let d = h.finalize();
    let tag: String = d.iter().take(12).map(|b| format!("{b:02x}")).collect();
    state.valkey.key(&format!("obsgw:{org}:m:{tag}"))
}

fn unix_ms(t: OffsetDateTime) -> i64 {
    i64::try_from(t.unix_timestamp_nanos() / 1_000_000).unwrap_or(0)
}

// Whether an organization has an active source, kept for a short while: most have none, and
// their requests then cost no Valkey write.
static WITH_SOURCES: std::sync::LazyLock<moka::future::Cache<Uuid, bool>> =
    std::sync::LazyLock::new(|| {
        moka::future::Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(30))
            .build()
    });

async fn has_sources(state: &AppState, org: Uuid) -> bool {
    if let Some(known) = WITH_SOURCES.get(&org).await {
        return known;
    }
    // While PostgreSQL is away, the answer is kept too, so the requests don't all ask again.
    let active = match kaeso_store::pg::sources::any_active(&state.pg, org).await {
        Ok(active) => active,
        Err(e) => {
            tracing::debug!(error = %e, "cannot tell whether an organization has sources");
            false
        }
    };
    WITH_SOURCES.insert(org, active).await;
    active
}

/// Forgets this instance's answer for the organization once a source is added or changed, so
/// its gateway requests are noted from then on.
pub async fn sources_changed(org: Uuid) {
    WITH_SOURCES.invalidate(&org).await;
}

/// What a source's copy of a gateway request is matched on.
#[derive(Clone, Debug)]
pub struct GatewayRequest {
    pub request_id: RequestId,
    pub org: Uuid,
    pub failed: bool,
    pub usage: Option<Usage>,
    /// The model the app named, the one called and the one that answered.
    pub models: Vec<String>,
    pub started_at: OffsetDateTime,
    pub duration: Duration,
}

impl GatewayRequest {
    /// The request as its copies would name it; `None` for one no source can report, such as
    /// an observed one or one refused before Kaeso knew its caller.
    pub fn of(o: &crate::model_api::hooks::Outcome) -> Option<Self> {
        let caller = o.caller.as_ref().filter(|_| o.source != Origin::Observed)?;
        Some(GatewayRequest {
            request_id: o.request_id,
            org: caller.org_id,
            failed: o.status >= 400,
            usage: o.usage.clone(),
            models: [&o.requested_model, &o.model_id, &o.response_model]
                .into_iter()
                .flatten()
                .cloned()
                .collect(),
            started_at: o.started_at,
            duration: o.duration,
        })
    }
}

/// Notes the request in the background, so the gateway's own work never waits on it; the reply's
/// end waits for the answer at most `NOTE_DEADLINE`.
pub fn note_soon(state: &AppState, r: GatewayRequest) -> Noting {
    let (noted, rx) = tokio::sync::oneshot::channel();
    let st = state.clone();
    state.in_flight.finish(async move {
        note(&st, r).await;
        let _ = noted.send(());
    });
    Noting(rx)
}

pub struct Noting(tokio::sync::oneshot::Receiver<()>);

impl Noting {
    pub async fn wait(self) {
        let _ = tokio::time::timeout(NOTE_DEADLINE, self.0).await;
    }
}

/// Notes a request the gateway answered once, so a source reporting the same request doesn't
/// count it again.
pub async fn note_gateway(state: &AppState, o: &mut crate::model_api::hooks::Outcome) {
    if o.sources_noted {
        return;
    }
    o.sources_noted = true;
    if let Some(r) = GatewayRequest::of(o) {
        note(state, r).await;
    }
}

/// Notes a gateway request for its copies to match, failed ones included; says whether it was
/// one to note. A note that can't be written only loses the match.
pub async fn note(state: &AppState, r: GatewayRequest) -> bool {
    let usage = match &r.usage {
        _ if r.failed => None,
        Some(u) if u.input_tokens > 0 || u.output_tokens > 0 => Some(u),
        _ => return false,
    };
    let mut models: Vec<String> = r
        .models
        .iter()
        .filter(|m| !m.is_empty())
        .map(|m| model_tag(m))
        .collect();
    models.sort();
    models.dedup();
    if models.is_empty() || !has_sources(state, r.org).await {
        return false;
    }
    let start = unix_ms(r.started_at);
    let end = unix_ms(r.started_at + r.duration);
    let member = format!("{}:{end}", r.request_id);
    let mut pipe = redis::pipe();
    for model in &models {
        let key = fingerprint(state, r.org, model, usage);
        pipe.cmd("ZADD").arg(&key).arg(start).arg(&member).ignore();
        pipe.cmd("EXPIRE").arg(&key).arg(GATEWAY_KEPT).ignore();
        pipe.cmd("SET")
            .arg(serving(state, r.org, model))
            .arg(1)
            .arg("EX")
            .arg(GATEWAY_KEPT)
            .ignore();
    }
    let mut conn = state.valkey.conn();
    match tokio::time::timeout(NOTE_DEADLINE, pipe.query_async::<()>(&mut conn)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "cannot note a gateway request for sources to match");
        }
        Err(_) => tracing::debug!("noting a gateway request for sources to match timed out"),
    }
    true
}

/// What the gateway's notes say of a request a source reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Match {
    /// It is a gateway request's copy, which only it matched.
    Yes,
    /// It matched nothing, but the organization's gateway serves its model: its request may
    /// not be noted yet.
    NotYet,
    No,
}

/// Which of the requests a source reports the gateway already logged; each match takes its
/// gateway request out of the matching, so each counts once. The whole batch is looked up in two
/// round trips.
async fn made_by_gateway(state: &AppState, org: Uuid, requests: &[&ObservedRequest]) -> Vec<Match> {
    let mut found = vec![Match::No; requests.len()];
    let keys: Vec<Option<(String, String, i64, i64)>> = requests
        .iter()
        .map(|r| {
            let start = r.started_at?;
            let usage = (r.status < 400).then(|| usage(r.tokens));
            let key = fingerprint(state, org, &r.model, usage.as_ref());
            let serving = serving(state, org, &r.model);
            Some((key, serving, unix_ms(start), unix_ms(start + r.duration)))
        })
        .collect();
    if keys.iter().all(Option::is_none) {
        return found;
    }
    let window = SAME_TIME.whole_milliseconds() as i64;
    let mut pipe = redis::pipe();
    for (key, serving, start, _) in keys.iter().flatten() {
        // A long request may have begun well before its copy says it did, when the client only
        // reports when it ended; its end then has to match.
        pipe.cmd("ZRANGEBYSCORE")
            .arg(key)
            .arg(start - 15 * 60_000)
            .arg(start + window)
            .arg("WITHSCORES");
        pipe.cmd("EXISTS").arg(serving);
    }
    let mut conn = state.valkey.conn();
    let Ok(Ok(listed)) = tokio::time::timeout(
        VALKEY_DEADLINE,
        pipe.query_async::<Vec<redis::Value>>(&mut conn),
    )
    .await
    else {
        return found;
    };
    let close = |a: i64, b: i64| (a - b).abs() <= window;
    let mut picked: Vec<(usize, &str, String)> = Vec::new();
    let mut listed = listed.into_iter();
    for (i, k) in keys.iter().enumerate() {
        let Some((key, _, start, end)) = k else {
            continue;
        };
        let (Some(candidates), Some(served)) = (listed.next(), listed.next()) else {
            break;
        };
        let candidates: Vec<(String, i64)> =
            redis::from_redis_value(candidates).unwrap_or_default();
        let served: bool = redis::from_redis_value(served).unwrap_or(false);
        let hit = candidates.into_iter().find(|(member, at)| {
            let ended = member
                .rsplit_once(':')
                .and_then(|(_, e)| e.parse::<i64>().ok());
            let taken = picked.iter().any(|(_, k, m)| k == key && m == member);
            !taken && (close(*at, *start) || ended.is_some_and(|e| close(e, *end)))
        });
        match hit {
            Some((member, _)) => picked.push((i, key.as_str(), member)),
            None if served => {
                if let Some(f) = found.get_mut(i) {
                    *f = Match::NotYet;
                }
            }
            None => {}
        }
    }
    if picked.is_empty() {
        return found;
    }
    let mut pipe = redis::pipe();
    for (_, key, member) in &picked {
        pipe.cmd("ZREM").arg(*key).arg(member);
    }
    // Of two copies racing for one gateway request, only the one that removes it matches.
    if let Ok(Ok(removed)) =
        tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async::<Vec<i64>>(&mut conn)).await
    {
        for ((i, _, _), n) in picked.iter().zip(removed) {
            if let Some(f) = found.get_mut(*i) {
                *f = if n == 1 { Match::Yes } else { Match::No };
            }
        }
    }
    found
}

/// How long to wait before looking again for the gateway request of a fresh copy that matched
/// nothing yet; `None` for a record that ended too long ago for its note to be missing still.
fn hold_for(now: OffsetDateTime, r: &ObservedRequest) -> Option<Duration> {
    let end = r.started_at? + r.duration;
    let left = end + time::Duration::try_from(HOLD).ok()? - now;
    let left = Duration::try_from(left).ok().filter(|d| !d.is_zero())?;
    Some(left.min(HOLD))
}

/// The copies among the requests: those that matched nothing while their model's gateway
/// requests may not be noted yet are looked up once more after a short wait.
async fn copies(
    state: &AppState,
    org: Uuid,
    requests: &[&ObservedRequest],
    now: OffsetDateTime,
) -> Vec<bool> {
    let mut found = made_by_gateway(state, org, requests).await;
    let mut wait = None;
    let mut waiting = Vec::new();
    for (i, c) in found.iter().enumerate() {
        if let (Match::NotYet, Some(r)) = (c, requests.get(i))
            && let Some(hold) = hold_for(now, r)
        {
            wait = wait.max(Some(hold));
            waiting.push(i);
        }
    }
    if let Some(wait) = wait {
        tokio::time::sleep(wait).await;
        let again: Vec<&ObservedRequest> = waiting
            .iter()
            .filter_map(|i| requests.get(*i).copied())
            .collect();
        for (i, c) in waiting
            .iter()
            .zip(made_by_gateway(state, org, &again).await)
        {
            if let Some(f) = found.get_mut(*i) {
                *f = c;
            }
        }
    }
    found.into_iter().map(|c| c == Match::Yes).collect()
}

/// Whether the person can see the project, so a source's requests may count for them.
pub async fn sees_project(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    user: Uuid,
) -> Result<bool, StoreError> {
    let Some(member) = kaeso_store::pg::members::get_org(&state.pg, org, user).await? else {
        return Ok(false);
    };
    let org_grant = crate::platform::grants::org_from_store(&member.grant()).ok();
    let own = kaeso_store::pg::members::get_project(&state.pg, project, user)
        .await?
        .and_then(|m| crate::platform::grants::project_from_store(&m.grant()).ok());
    Ok(effective_project_rights(org_grant.as_ref(), own.as_ref())
        .contains(ProjectRight::ProjectView))
}

// The person each email address names, when the source matches members and the address belongs
// to someone who can see the source's project.
async fn members(
    state: &AppState,
    source: &Source,
    emails: Vec<String>,
) -> Result<HashMap<String, Uuid>, StoreError> {
    let mut found = HashMap::new();
    if !source.match_members {
        return Ok(found);
    }
    let mut seen = std::collections::HashSet::new();
    for email in emails {
        let key = email.to_lowercase();
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(user) = kaeso_store::pg::users::get_by_email(&state.pg, &email).await? else {
            continue;
        };
        if sees_project(state, source.org_id, source.project_id, user.id).await? {
            found.insert(key, user.id);
        }
    }
    Ok(found)
}

/// Whether the source's own member still sees its project, or its service account is still an
/// active one of the project: who its records count for when no email address names a member.
async fn owner_counts(state: &AppState, source: &Source) -> Result<bool, StoreError> {
    if let Some(user) = source.user_id {
        return sees_project(state, source.org_id, source.project_id, user).await;
    }
    if let Some(account) = source.service_account_id {
        let found =
            kaeso_store::pg::service_accounts::get(&state.pg, source.project_id, account).await?;
        return Ok(found.is_some_and(|a| a.archived_at.is_none()));
    }
    Ok(false)
}

/// Whom a record of a source that still counts is for: the member its email address names,
/// else the source's member or service account.
fn caller(source: &Source, members: &HashMap<String, Uuid>, email: Option<&str>) -> Option<Caller> {
    let matched = email.and_then(|e| members.get(&e.to_lowercase()).copied());
    let (user, service_account) = match (matched, source.user_id, source.service_account_id) {
        (Some(u), _, _) | (None, Some(u), _) => (Some(u), None),
        (None, None, Some(s)) => (None, Some(s)),
        (None, None, None) => return None,
    };
    Some(Caller {
        org_id: source.org_id,
        project_id: source.project_id,
        key_id: None,
        user_id: user,
        service_account_id: service_account,
    })
}

/// The model's prices and catalogue entry: the project's own entry first, so its price overrides
/// count, else KPMR's; the provider is found from the model when the source named none.
pub struct Priced {
    pub provider: Option<String>,
    pub catalogue_model_id: Option<Uuid>,
    pub prices: Option<Fields>,
}

/// An effort level as the log keeps it: a short lowercase word, such as `high`; anything else a
/// record names isn't one.
fn effort_level(level: &str) -> Option<String> {
    let level = level.trim().to_ascii_lowercase();
    (!level.is_empty()
        && level.len() <= 16
        && level
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'))
    .then_some(level)
}

/// The prices of a request whose record names no provider: the provider its kind of source
/// usually talks to first, else whichever provider sells the model. The kind's provider names the
/// request even when neither has a price, as the client most likely spoke to it.
async fn price_guessed(
    state: &AppState,
    project: Uuid,
    guess: Option<&str>,
    model: &str,
) -> Priced {
    if let Some(g) = guess {
        let priced = price(state, project, Some(g), model).await;
        if priced.prices.is_some() {
            return priced;
        }
    }
    let mut priced = price(state, project, None, model).await;
    if priced.prices.is_none() && priced.provider.is_none() {
        priced.provider = guess.map(str::to_owned);
    }
    priced
}

pub async fn price(state: &AppState, project: Uuid, provider: Option<&str>, model: &str) -> Priced {
    let wanted = |p: &str, m: &str| provider.is_none_or(|want| want == p) && m == model;
    let models = crate::services::catalogue::project_models(state, project)
        .await
        .inspect_err(|e| {
            tracing::warn!(error = %e, "cannot read the project's models; an observed request is priced with KPMR's prices");
        })
        .ok();
    if let Some(models) = models
        && let Some(entry) = models
            .catalogue
            .entries()
            .iter()
            .find(|e| wanted(&e.provider, &e.model_id) || wanted(&e.provider, e.called()))
        && let Some(effective) =
            crate::services::catalogue::effective(state, entry, &models.providers)
    {
        return Priced {
            provider: Some(entry.provider.clone()),
            catalogue_model_id: Some(entry.id),
            prices: Some(effective.fields),
        };
    }
    let loaded = state.kpmr.current();
    let snapshot = &loaded.snapshot;
    let found = match provider {
        Some(p) => snapshot.model(p, model).map(|m| (p.to_owned(), m)),
        None => {
            let listing: Vec<(&String, &kaeso_core::kpmr::Model)> = snapshot
                .providers
                .iter()
                .filter_map(|(p, listed)| listed.models.get(model).map(|m| (p, m)))
                .collect();
            // A model several providers sell is priced by its maker, or not at all: a reseller's
            // price would be a guess.
            match listing.as_slice() {
                [(p, m)] => Some(((*p).clone(), *m)),
                many => MAKERS.iter().find_map(|maker| {
                    many.iter()
                        .find(|(p, _)| p.as_str() == *maker)
                        .map(|(p, m)| ((*p).clone(), *m))
                }),
            }
        }
    };
    match found {
        Some((p, m)) => Priced {
            provider: Some(p),
            catalogue_model_id: None,
            prices: Some(Fields::from_kpmr(m)),
        },
        None => Priced {
            provider: provider.map(str::to_owned),
            catalogue_model_id: None,
            prices: None,
        },
    }
}

fn static_provider(id: &str) -> Option<&'static str> {
    kaeso_core::provider::PROVIDERS
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.id)
}

fn content(r: &ObservedRequest, dialect: DialectId, model: &str) -> Option<(String, String)> {
    let prompt = r.prompt.as_deref().filter(|p| !p.is_empty());
    let reply = r.reply.as_deref().filter(|p| !p.is_empty());
    if prompt.is_none() && reply.is_none() && r.messages.is_empty() && r.reply_parts.is_empty() {
        return None;
    }
    let mut request = GenerateRequest::new(model);
    if let Some(system) = r.system.as_deref().filter(|s| !s.is_empty()) {
        request = request.with_system(system);
    }
    request.messages = if r.messages.is_empty() {
        vec![Message::user(prompt.unwrap_or_default())]
    } else {
        r.messages.clone()
    };
    let body = encoded(dialect, &request)
        // A dialect that can't carry reasoning given back in a request still shows the rest.
        .or_else(|| {
            let mut plain = request.clone();
            for m in &mut plain.messages {
                m.blocks.retain(|b| !matches!(b, Block::Reasoning(_)));
            }
            plain
                .messages
                .retain(|m| !m.blocks.is_empty() || m.role == Role::User);
            encoded(dialect, &plain)
        })
        // Parts the dialect can't carry, such as a tool call without an id, leave the prompt.
        .or_else(|| {
            let mut asked = GenerateRequest::new(model);
            asked.messages = vec![Message::user(prompt?)];
            encoded(dialect, &asked)
        })
        .unwrap_or_default();
    let parts = if r.reply_parts.is_empty() {
        reply.map(|t| vec![Block::text(t)]).unwrap_or_default()
    } else {
        r.reply_parts.clone()
    };
    let mut response = Response::new(r.external_id.clone(), model, parts);
    response.usage = usage(r.tokens);
    let reply = serde_json::to_string(&response).unwrap_or_default();
    Some((body, reply))
}

fn encoded(dialect: DialectId, request: &GenerateRequest) -> Option<String> {
    let request = Request::Generate(request.clone());
    let mut ctx = kaeso_codecs::EgressCtx::new(&request);
    kaeso_codecs::dispatch::encode_request(dialect, &request, &mut ctx)
        .ok()
        .and_then(|e| String::from_utf8(e.body).ok())
}

fn usage(t: Counted) -> Usage {
    let mut u = Usage::new(t.input, t.output);
    u.cache_read_tokens = t.cache_read;
    u.cache_write_tokens = t.cache_write;
    u.reasoning_tokens = (t.reasoning > 0).then_some(t.reasoning);
    // Writes kept for an hour cost more than those kept for five minutes.
    let one_hour = t.cache_write_1h.min(t.cache_write);
    if one_hour > 0 {
        u.cache_write_breakdown =
            Some(CacheWriteBreakdown::new(t.cache_write - one_hour, one_hour));
    }
    u
}

fn kind_of(source: &Source) -> Option<SourceKind> {
    source.kind.parse().ok()
}

/// Why Kaeso can't keep a request, checked before anything is counted.
fn invalid_request(r: &ObservedRequest, now: OffsetDateTime) -> Option<Refusal> {
    if r.external_id.trim().is_empty() {
        return Some(Refusal::NoId);
    }
    let t = r.tokens;
    if [t.input, t.output, t.cache_read, t.cache_write, t.reasoning]
        .iter()
        .any(|n| *n > MAX_TOKENS)
    {
        return Some(Refusal::TooManyTokens);
    }
    if r.reported_cost.is_some_and(|c| c > Decimal::from(MAX_COST)) {
        return Some(Refusal::CostTooHigh);
    }
    if r.model.trim().is_empty() {
        return Some(Refusal::NoModel);
    }
    if !(100..=599).contains(&r.status) {
        return Some(Refusal::BadStatus);
    }
    invalid_time(r.started_at, now)
}

fn invalid_call(c: &ObservedToolCall, now: OffsetDateTime) -> Option<Refusal> {
    if c.external_id.trim().is_empty() {
        return Some(Refusal::NoId);
    }
    if c.tool.trim().is_empty() {
        return Some(Refusal::NoTool);
    }
    invalid_time(c.started_at, now)
}

fn invalid_time(at: Option<OffsetDateTime>, now: OffsetDateTime) -> Option<Refusal> {
    let oldest = now - time::Duration::try_from(REMEMBERED).unwrap_or(time::Duration::ZERO);
    match at {
        Some(at) if at < oldest => Some(Refusal::TooOld),
        Some(at) if at > now + CLOCK_SKEW => Some(Refusal::InFuture),
        _ => None,
    }
}

/// Logs and counts a batch a source sent. Duplicates and refused records are counted as such;
/// nothing a source sends ever fails the batch, only Kaeso's own databases can.
pub async fn record(
    state: &AppState,
    source: &Source,
    mut batch: Batch,
) -> Result<Recorded, RecordError> {
    let now = state.clock.now();
    // Clients and collectors export more than model calls; a batch with nothing Kaeso reads only
    // says something is wrong while the source has never sent anything Kaeso could record.
    if batch.refusal == Some(Refusal::Unrecognized) && source.first_event_at.is_some() {
        batch.refused = 0;
        batch.refusal = None;
    }
    let mut out = Recorded {
        refused: batch.refused as usize,
        refusal: batch.refusal.take(),
        ..Recorded::default()
    };
    if source.disabled_at.is_some() || source.archived_at.is_some() {
        out.refused += batch.requests.len() + batch.tool_calls.len();
        out.refusal = Some(Refusal::Inactive);
        return Ok(out);
    }
    let mut refuse = |why: Option<Refusal>| match why {
        Some(why) => {
            out.refused += 1;
            out.refusal = Some(why);
            false
        }
        None => true,
    };
    batch.requests.retain(|r| refuse(invalid_request(r, now)));
    batch.tool_calls.retain(|c| refuse(invalid_call(c, now)));
    // A source whose member left the project, or whose service account was archived, counts
    // for nobody, whatever addresses its records name: its key may be in someone's hands who no
    // longer belongs.
    let records = batch.requests.len() + batch.tool_calls.len();
    if records > 0 && !owner_counts(state, source).await? {
        out.refused += records;
        out.refusal = Some(Refusal::Inactive);
        batch.requests.clear();
        batch.tool_calls.clear();
        batch.carried.clear();
    }
    let taken = carry(state, source, &mut batch).await?;
    let emails: Vec<String> = batch
        .requests
        .iter()
        .filter_map(|r| r.email.clone())
        .chain(batch.tool_calls.iter().filter_map(|c| c.email.clone()))
        .collect();
    // Everything that can fail runs before the records are marked as seen: an exporter whose
    // batch failed sends it again, and must not find its records taken for duplicates.
    let people = members(state, source, emails).await?;
    let request_ids: Vec<Uuid> = batch
        .requests
        .iter()
        .map(|r| request_id(source.id, &format!("r:{}", r.external_id)))
        .collect();
    let call_ids: Vec<Uuid> = batch
        .tool_calls
        .iter()
        .map(|c| request_id(source.id, &format!("t:{}", c.external_id)))
        .collect();
    let mut all: Vec<Vec<Uuid>> = batch
        .requests
        .iter()
        .zip(&request_ids)
        .map(|(r, id)| {
            std::iter::once(*id)
                .chain(
                    r.aliases
                        .iter()
                        .map(|a| request_id(source.id, &format!("r:{a}"))),
                )
                .collect()
        })
        .collect();
    all.extend(batch.tool_calls.iter().zip(&call_ids).map(|(c, id)| {
        std::iter::once(*id)
            .chain(
                c.aliases
                    .iter()
                    .map(|a| request_id(source.id, &format!("t:{a}"))),
            )
            .collect::<Vec<_>>()
    }));
    let new = first_time(state, source.id, &all).await?;
    let (new_requests, new_calls) = new.split_at(request_ids.len());
    let kind = kind_of(source);
    let candidates: Vec<&ObservedRequest> = batch
        .requests
        .iter()
        .zip(new_requests)
        .filter(|(_, fresh)| **fresh)
        .map(|(r, _)| r)
        .collect();
    let mut copies = copies(state, source.org_id, &candidates, now)
        .await
        .into_iter();
    for ((r, id), fresh) in batch
        .requests
        .into_iter()
        .zip(request_ids)
        .zip(new_requests)
    {
        if !*fresh {
            out.duplicates += 1;
            continue;
        }
        if copies.next().unwrap_or(false) {
            out.duplicates += 1;
            continue;
        }
        let Some(who) = caller(source, &people, r.email.as_deref()) else {
            out.refused += 1;
            out.refusal = Some(Refusal::Nobody);
            continue;
        };
        record_request(state, source, kind, who, r, id, now).await;
        out.requests += 1;
    }
    for ((c, id), fresh) in batch.tool_calls.into_iter().zip(call_ids).zip(new_calls) {
        if !*fresh {
            out.duplicates += 1;
            continue;
        }
        let Some(who) = caller(source, &people, c.email.as_deref()) else {
            out.refused += 1;
            out.refusal = Some(Refusal::Nobody);
            continue;
        };
        record_call(state, source, who, c, id, now).await;
        out.tool_calls += 1;
    }
    if !taken.is_empty() {
        forget(state, &taken).await;
    }
    if out.requests + out.tool_calls > 0 {
        kaeso_store::pg::sources::touch(&state.pg, source.id, now, None).await?;
    }
    if let Some(e) = out.refusal {
        kaeso_store::pg::sources::touch(&state.pg, source.id, now, Some(e.as_str())).await?;
    }
    crate::platform::projects::sources::add_activity(
        state,
        source.id,
        now,
        Activity {
            batches: 1,
            requests: out.requests as i64,
            tool_calls: out.tool_calls as i64,
            duplicates: out.duplicates as i64,
            refused: out.refused as i64,
        },
    )
    .await;
    Ok(out)
}

// What requests took from earlier batches goes with them; a key left behind only expires.
async fn forget(state: &AppState, keys: &[String]) {
    let mut conn = state.valkey.conn();
    let _: Result<Result<(), redis::RedisError>, _> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::cmd("DEL").arg(keys).query_async::<()>(&mut conn),
    )
    .await;
}

async fn record_request(
    state: &AppState,
    source: &Source,
    kind: Option<SourceKind>,
    who: Caller,
    r: ObservedRequest,
    id: Uuid,
    now: OffsetDateTime,
) {
    let model = cut(&r.model, MAX_NAME);
    let named = r
        .provider
        .clone()
        .filter(|p| p.len() <= 64 && kaeso_core::provider::ProviderId::is_valid(p));
    let priced_as = |model: String| {
        let named = named.clone();
        let unguessed = r.unguessed;
        let guessed = r.provider_guessed;
        async move {
            match named {
                Some(p) if guessed => {
                    price_guessed(state, source.project_id, Some(&p), &model).await
                }
                Some(p) => price(state, source.project_id, Some(&p), &model).await,
                None if unguessed => Priced {
                    provider: None,
                    catalogue_model_id: None,
                    prices: None,
                },
                None => {
                    let guess = kind.and_then(SourceKind::provider);
                    price_guessed(state, source.project_id, guess, &model).await
                }
            }
        }
    };
    let mut priced = priced_as(model.clone()).await;
    let mut effort = r.reasoning_effort.as_deref().and_then(effort_level);
    let mut model = model;
    // Antigravity names one model per setting (`gemini-3.8-flash-low`); logged as is, each level
    // would count as a model of its own, without a price.
    if priced.prices.is_none()
        && !r.unguessed
        && let Some((base, setting)) = IdSetting::split(&model)
        && !state.kpmr.current().snapshot.lists(&model)
    {
        let base = base.to_owned();
        let base_priced = priced_as(base.clone()).await;
        let takes = match &base_priced.prices {
            Some(f) => setting.fits(f.efforts.as_deref(), f.reasoning),
            // A model only resellers sell has no price here, but is still one model.
            None => state
                .kpmr
                .current()
                .snapshot
                .at_setting(base_priced.provider.as_deref().unwrap_or_default(), &model)
                .is_some(),
        };
        if takes {
            priced = base_priced;
            if let Some(level) = setting.effort() {
                effort.get_or_insert_with(|| level.to_owned());
            }
            model = base;
        }
    }
    let dialect = r.dialect.unwrap_or(DialectId::OpenAiChat);
    let mut o = crate::model_api::pipeline::blank_outcome(RequestId::from_uuid(id));
    o.caller = Some(who.clone());
    o.source = Origin::Observed;
    o.source_id = Some(source.id);
    o.ingress = dialect;
    o.egress = Some(dialect);
    if dialect == DialectId::Gemini {
        use kaeso_codecs::gemini::GeminiAction;
        o.gemini = Some(if r.stream {
            GeminiAction::StreamGenerateContent
        } else {
            GeminiAction::GenerateContent
        });
    }
    o.kind = kaeso_kpp::RequestKind::Generate;
    o.provider = priced.provider.as_deref().and_then(static_provider);
    o.custom_provider = match (&o.provider, &priced.provider) {
        (None, Some(p)) => Some(p.clone()),
        _ => None,
    };
    o.requested_model = Some(model.clone());
    o.model_id = Some(model.clone());
    o.reasoning_effort = effort;
    o.catalogue_model_id = priced.catalogue_model_id;
    o.prices = priced.prices;
    o.observed_cost = r.reported_cost.filter(|c| !c.is_sign_negative());
    o.stream = r.stream;
    o.status = r.status;
    if r.status >= 400 {
        o.error_kind = Some("provider_error".into());
        o.error_message = r.error_message.as_deref().map(|m| cut(m, MAX_ERROR));
    }
    o.usage = Some(usage(r.tokens));
    o.started_at = r.started_at.unwrap_or(now);
    o.duration = r.duration;
    o.ttfb = r.ttfb;
    o.user_agent = r.client.clone();
    o.tags = r.tags.clone();
    o.upstream_request_id = r.provider_request_id.as_deref().map(|id| cut(id, MAX_NAME));
    let kept = source
        .content
        .then(|| content(&r, dialect, &model))
        .flatten();
    o.content_kept = kept.is_some();
    if let Some((body, _)) = kept.as_ref().filter(|(body, _)| !body.is_empty()) {
        o.request_body = Some(bytes::Bytes::from(body.clone()));
    }
    o.reply = kept.and_then(|(_, reply)| serde_json::from_str::<Response>(&reply).ok());
    crate::services::limits::prepare_observed(
        state,
        &who,
        o.catalogue_model_id.unwrap_or_default(),
        o.started_at,
    )
    .await;
    crate::services::usage::account(state, &o).await;
    crate::services::logs::record(state, &o).await;
}

async fn record_call(
    state: &AppState,
    source: &Source,
    who: Caller,
    c: ObservedToolCall,
    id: Uuid,
    now: OffsetDateTime,
) {
    let row = ToolCallRow {
        id,
        org_id: source.org_id,
        project_id: source.project_id,
        server_id: Uuid::nil(),
        started_at: c.started_at.unwrap_or(now),
        tool: cut(&c.tool, MAX_NAME),
        upstream_tool: cut(&c.tool, MAX_NAME),
        caller_kind: if who.user_id.is_some() {
            "user"
        } else {
            "service_account"
        }
        .to_owned(),
        caller_id: who.user_id.or(who.service_account_id).unwrap_or_default(),
        key_id: None,
        grant_id: None,
        client_name: c.client.clone().unwrap_or_default(),
        protocol_version: String::new(),
        source: Origin::Observed.as_str().to_owned(),
        status: if c.ok { "ok" } else { "error" }.to_owned(),
        error_type: c.error_type.clone().unwrap_or_default(),
        error_message: c
            .error_message
            .as_deref()
            .map(|m| cut(m, MAX_ERROR))
            .unwrap_or_default(),
        duration_ms: u32::try_from(c.duration.as_millis()).unwrap_or(u32::MAX),
        upstream_ms: None,
        request_bytes: c.arguments.as_ref().map_or(0, |a| a.len() as u64),
        response_bytes: c.result.as_ref().map_or(0, |r| r.len() as u64),
        has_content: false,
        expires_at: now,
        source_id: source.id,
        server_name: c
            .server
            .as_deref()
            .map(|s| cut(s, MAX_NAME))
            .unwrap_or_default(),
    };
    // Counted where Analytics reads tool calls, as the MCP portal counts its own; reported calls
    // belong to no server of Kaeso's.
    state.tool_usage.add(
        row.started_at,
        source.org_id,
        source.project_id,
        Uuid::nil(),
        !c.ok,
    );
    let content = if source.content && (c.arguments.is_some() || c.result.is_some()) {
        Some((
            c.arguments.unwrap_or_default(),
            c.result.unwrap_or_default(),
        ))
    } else {
        None
    };
    crate::services::logs::record_tool_call(state, row, content).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn the_same_record_of_the_same_source_gets_the_same_id() {
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        assert_eq!(request_id(a, "r:req_1"), request_id(a, "r:req_1"));
        assert_ne!(request_id(a, "r:req_1"), request_id(a, "r:req_2"));
        assert_ne!(request_id(a, "r:req_1"), request_id(b, "r:req_1"));
    }

    #[test]
    fn long_text_is_cut_at_a_character() {
        assert_eq!(cut("äöü", 2), "äö");
        assert_eq!(cut("ab", 5), "ab");
    }

    fn request() -> ObservedRequest {
        ObservedRequest {
            external_id: "req_1".into(),
            model: "claude-haiku-4-5".into(),
            status: 200,
            ..ObservedRequest::default()
        }
    }

    #[test]
    fn records_beyond_what_one_request_can_be_are_refused() {
        let now = datetime!(2026-11-23 12:00 UTC);
        assert_eq!(invalid_request(&request(), now), None);
        let mut huge = request();
        huge.tokens.output = MAX_TOKENS + 1;
        assert!(invalid_request(&huge, now).is_some());
        let mut dear = request();
        dear.reported_cost = Some(Decimal::from(1_000_000));
        assert!(invalid_request(&dear, now).is_some());
        let mut odd = request();
        odd.status = 7;
        assert!(invalid_request(&odd, now).is_some());
        let mut nameless = request();
        nameless.model = " ".into();
        assert!(invalid_request(&nameless, now).is_some());
    }

    #[test]
    fn a_start_from_an_earlier_batch_times_the_request_from_then() {
        let mut r = request();
        r.started_at = Some(datetime!(2026-11-23 12:00:04 UTC));
        fill(&mut r, Wanted::Began, "1795435200000:300");
        assert_eq!(r.started_at, Some(datetime!(2026-11-23 12:00:00 UTC)));
        assert_eq!(r.duration, Duration::from_secs(4));
        assert_eq!(r.ttfb, Some(Duration::from_millis(300)));
        let mut late = request();
        late.started_at = Some(datetime!(2026-11-23 11:59:00 UTC));
        fill(&mut late, Wanted::Began, "1795435200000:300");
        assert_eq!(
            late.duration,
            Duration::ZERO,
            "a start after the end is someone else's"
        );
        let mut prompted = request();
        fill(&mut prompted, Wanted::Prompt, "Hi");
        fill(&mut prompted, Wanted::Provider, "openai");
        assert_eq!(
            (prompted.prompt.as_deref(), prompted.provider.as_deref()),
            (Some("Hi"), Some("openai"))
        );
        let mut local = request();
        fill(&mut local, Wanted::Provider, UNNAMED_PROVIDER);
        assert_eq!((local.provider, local.unguessed), (None, true));
    }

    #[test]
    fn only_a_copy_fresh_enough_to_miss_its_note_waits_for_it() {
        let ended = datetime!(2026-11-23 12:00:10 UTC);
        let mut r = request();
        r.started_at = Some(datetime!(2026-11-23 12:00:00 UTC));
        r.duration = Duration::from_secs(10);
        assert_eq!(hold_for(ended, &r), Some(HOLD));
        assert_eq!(
            hold_for(ended + time::Duration::seconds(1), &r),
            Some(Duration::from_millis(500))
        );
        assert_eq!(hold_for(ended + time::Duration::seconds(2), &r), None);
        assert_eq!(
            hold_for(ended - time::Duration::seconds(3), &r),
            Some(HOLD),
            "a client's clock ahead of Kaeso's waits no longer"
        );
        r.started_at = None;
        assert_eq!(hold_for(ended, &r), None);
    }

    #[test]
    fn a_copy_is_matched_on_its_model_alone_whatever_its_path_and_case() {
        assert_eq!(model_tag("anthropic/Claude-Haiku-5-5"), "claude-haiku-5-5");
        assert_eq!(model_tag("claude-haiku-5-5"), "claude-haiku-5-5");
    }

    #[test]
    fn an_effort_level_is_kept_only_as_a_short_word() {
        assert_eq!(effort_level("High").as_deref(), Some("high"));
        assert_eq!(effort_level(" xhigh ").as_deref(), Some("xhigh"));
        assert_eq!(effort_level(""), None);
        assert_eq!(effort_level("think really hard"), None);
        assert_eq!(effort_level(&"x".repeat(17)), None);
    }

    #[test]
    fn a_whole_exchange_is_logged_with_its_tool_calls_and_reasoning() {
        let mut r = request();
        r.system = Some("Be brief.".into());
        r.messages = vec![
            Message::user("Weather?"),
            Message::new(
                Role::Assistant,
                vec![Block::tool_call(
                    "c1",
                    "weather",
                    kaeso_kpp::RawJson::from_string("{}".into()).unwrap(),
                )],
            ),
            Message::new(
                Role::User,
                vec![Block::tool_result("c1", vec![Block::text("Sunny")], false)],
            ),
        ];
        let mut thought = kaeso_kpp::ReasoningBlock::default();
        thought.text = vec!["From the tool.".into()];
        r.reply_parts = vec![Block::Reasoning(thought), Block::text("Sunny.")];
        let (body, reply) = content(&r, DialectId::OpenAiChat, "gpt-5.4").unwrap();
        assert!(
            body.contains("Be brief.") && body.contains("weather") && body.contains("Sunny"),
            "{body}"
        );
        assert!(
            reply.contains("From the tool.") && reply.contains("Sunny."),
            "{reply}"
        );
    }

    #[test]
    fn a_record_must_be_from_the_last_seven_days_and_not_from_the_future() {
        let now = datetime!(2026-11-23 12:00 UTC);
        let at = |t| invalid_time(Some(t), now);
        assert_eq!(at(datetime!(2026-11-16 12:01 UTC)), None);
        assert_eq!(at(datetime!(2026-11-16 11:59 UTC)), Some(Refusal::TooOld));
        assert_eq!(at(datetime!(2026-11-23 12:04 UTC)), None);
        assert_eq!(at(datetime!(2026-11-23 12:06 UTC)), Some(Refusal::InFuture));
        assert_eq!(invalid_time(None, now), None);
    }
}
