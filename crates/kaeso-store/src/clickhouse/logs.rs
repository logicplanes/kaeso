use clickhouse::Row;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{ClickHouse, parts};
use crate::StoreError;
use crate::pg::usage::{Filter as UsageFilter, Member};

#[derive(Clone, Debug, PartialEq, Row, Serialize, Deserialize)]
pub struct LogRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub request_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub org_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub key_id: Uuid,
    pub principal_kind: String,
    #[serde(with = "clickhouse::serde::uuid")]
    pub principal_id: Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub started_at: OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub finished_at: OffsetDateTime,
    pub duration_ms: u32,
    pub ttfb_ms: Option<u32>,
    pub endpoint: String,
    pub ingress_dialect: String,
    pub egress_dialect: String,
    pub provider: String,
    pub requested_model: String,
    pub resolved_model: String,
    pub stream: bool,
    pub status: u16,
    pub error_kind: String,
    pub error_message: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub cost: i128,
    pub user_agent: String,
    pub client_ip: String,
    pub upstream_request_id: String,
    pub has_content: bool,
    pub logical_bytes: u64,
    #[serde(with = "clickhouse::serde::time::datetime")]
    pub expires_at: OffsetDateTime,
    pub stream_events: u32,
    pub stream_bytes: u64,
    pub stream_first_byte_ms: Option<u32>,
    pub stream_keepalives: u32,
    pub usage_estimated: bool,
    pub source: String,
    pub overhead_us: Option<u32>,
    pub stream_processing_us: Option<u32>,
    pub cache_breakpoints: Option<u16>,
    pub request_upload_us: Option<u32>,
    pub response_cache: String,
    pub alias: String,
    pub line: String,
    pub fallbacks: String,
    pub media_unit: String,
    pub media_input: u64,
    pub media_output: u64,
    pub media_detail: String,
    pub cost_known: bool,
    pub cost_basis: String,
    /// The app's own tags from `kaeso-tags`, in the order it sent them.
    pub tags: Vec<String>,
    /// The guardrails that checked the request and its reply, as JSON; empty without any.
    pub guardrails: String,
    /// The actions of those guardrails, so the log can filter by them.
    pub guardrail_actions: Vec<String>,
    /// The provider key that paid; the nil id when no provider was sent the request.
    #[serde(with = "clickhouse::serde::uuid")]
    pub credential_id: Uuid,
    /// The source that reported a request Kaeso observed; the nil id for the gateway's own.
    #[serde(with = "clickhouse::serde::uuid")]
    pub source_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Row, Serialize, Deserialize)]
pub struct ContentRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub request_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub org_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub started_at: OffsetDateTime,
    pub request_body: String,
    pub reply: String,
    #[serde(with = "clickhouse::serde::time::datetime")]
    pub expires_at: OffsetDateTime,
    /// How many placeholders for parts `request_body` holds; 0 for a body stored whole.
    pub request_parts: u32,
}

pub async fn insert_logs<'a>(
    ch: &ClickHouse,
    rows: impl IntoIterator<Item = &'a LogRow>,
) -> Result<(), StoreError> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut insert = ch.client().insert::<LogRow>("request_log").await?;
    for r in rows {
        insert.write(r).await?;
    }
    insert.end().await?;
    Ok(())
}

pub async fn insert_contents<'a>(
    ch: &ClickHouse,
    rows: impl IntoIterator<Item = &'a ContentRow>,
) -> Result<(), StoreError> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut insert = ch.client().insert::<ContentRow>("request_content").await?;
    for r in rows {
        insert.write(r).await?;
    }
    insert.end().await?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
    pub key: Option<Uuid>,
    pub principal: Option<Uuid>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub status: Option<u16>,
    pub failed: Option<bool>,
    pub error_kind: Option<String>,
    pub stream: Option<bool>,
    pub source: Option<String>,
    pub ingress_dialect: Option<String>,
    pub egress_dialect: Option<String>,
    pub response_cache: Option<String>,
    pub min_duration_ms: Option<u32>,
    /// The alias the app named, exactly.
    pub alias: Option<String>,
    /// Whether an earlier route step failed and a later one answered.
    pub fell_back: Option<bool>,
    pub endpoint: Option<String>,
    /// A decimal amount in USD, compared as a decimal, never as a float.
    pub min_cost: Option<String>,
    /// The model or alias name exactly as the app sent it.
    pub requested_model: Option<String>,
    /// One of the request's tags.
    pub tag: Option<String>,
    /// Text the error message contains, in any case.
    pub error_message: Option<String>,
    /// The client app, as [`client_of`] names it from the user agent.
    pub client: Option<String>,
    /// An action a guardrail took on the request: `allow`, `block` or `replace`.
    pub guardrail_action: Option<String>,
    /// The source that reported the requests; the nil id for the gateway's own.
    pub source_id: Option<Uuid>,
}

/// User-agent prefixes and the client app each names, compared without case. An entry's
/// version follows its prefix after a `/` or a space, such as `codex_cli_rs/0.59.0` or
/// `OpenAI/Python 2.9.0`. Matched in SQL as in Rust, so entries logged before a client was
/// added get it too.
pub const CLIENTS: &[(&str, &str)] = &[
    ("codex_cli_rs/", "codex"),
    ("claude-cli/", "claude-code"),
    ("openai/python", "openai-python"),
    ("openai-python/", "openai-python"),
    ("openai/js", "openai-js"),
    ("anthropic/python", "anthropic-python"),
    ("anthropic/js", "anthropic-js"),
    ("ai-sdk/", "ai-sdk"),
    ("curl/", "curl"),
    ("geminicli", "gemini-cli"),
    ("antigravity/", "antigravity"),
    ("litellm", "litellm"),
];

/// The client a request without a known user agent prefix counts as.
pub const OTHER_CLIENT: &str = "other";

/// Every client name the filter takes, in the order of [`CLIENTS`], then `other`.
pub fn client_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    for (_, name) in CLIENTS {
        if !names.contains(name) {
            names.push(name);
        }
    }
    names.push(OTHER_CLIENT);
    names
}

fn client_prefix(ua: &str) -> Option<(&'static str, &'static str)> {
    let lower = ua.to_ascii_lowercase();
    CLIENTS
        .iter()
        .find(|(prefix, _)| lower.starts_with(prefix))
        .copied()
}

/// The client app a user agent names: empty without one, `other` for one not in [`CLIENTS`].
pub fn client_of(ua: &str) -> &'static str {
    if ua.is_empty() {
        return "";
    }
    client_prefix(ua).map_or(OTHER_CLIENT, |(_, name)| name)
}

/// The version after a known client's prefix, such as `0.59.0`.
pub fn client_version(ua: &str) -> Option<&str> {
    let (prefix, _) = client_prefix(ua)?;
    let rest = ua.get(prefix.len()..)?.trim_start_matches(['/', ' ']);
    let version = rest.split([' ', ';', '(']).next().unwrap_or_default();
    (!version.is_empty()).then_some(version)
}

// The prefixes are this file's own text, so they are written into the SQL; a `'` could never
// come from a request.
static CLIENT_SQL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let arms: Vec<String> = CLIENTS
        .iter()
        .map(|(prefix, name)| format!("startsWith(lower(user_agent), '{prefix}'), '{name}'"))
        .collect();
    format!(
        "multiIf(user_agent = '', '', {}, '{OTHER_CLIENT}')",
        arms.join(", ")
    )
});

pub async fn search(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    f: &Filter,
    after: Option<(OffsetDateTime, Uuid)>,
    limit: i64,
) -> Result<Vec<LogRow>, StoreError> {
    let mut sql =
        String::from("SELECT ?fields FROM request_log WHERE org_id = ? AND project_id = ?");
    let mut q_parts: Vec<Bind> = vec![Bind::Uuid(org), Bind::Uuid(project)];
    filter_sql(f, &mut sql, &mut q_parts);
    if let Some((at, id)) = after {
        sql.push_str(" AND (started_at, request_id) < (fromUnixTimestamp64Milli(?), ?)");
        q_parts.push(Bind::Millis(at));
        q_parts.push(Bind::Uuid(id));
    }
    sql.push_str(" ORDER BY started_at DESC, request_id DESC LIMIT ?");
    q_parts.push(Bind::I64(limit));
    let mut q = ch.client().query(&sql);
    for b in q_parts {
        q = b.bind(q);
    }
    Ok(q.fetch_all::<LogRow>().await?)
}

enum Bind {
    Uuid(Uuid),
    Millis(OffsetDateTime),
    Str(String),
    U16(u16),
    I64(i64),
    Bool(bool),
}

impl Bind {
    fn bind(self, q: clickhouse::query::Query) -> clickhouse::query::Query {
        match self {
            Bind::Uuid(u) => q.bind(u),
            Bind::Millis(t) => {
                q.bind(i64::try_from(t.unix_timestamp_nanos() / 1_000_000).unwrap_or(0))
            }
            Bind::Str(s) => q.bind(s),
            Bind::U16(v) => q.bind(v),
            Bind::I64(v) => q.bind(v),
            Bind::Bool(v) => q.bind(v),
        }
    }
}

fn filter_sql(f: &Filter, sql: &mut String, binds: &mut Vec<Bind>) {
    if let Some(t) = f.from {
        sql.push_str(" AND started_at >= fromUnixTimestamp64Milli(?)");
        binds.push(Bind::Millis(t));
    }
    if let Some(t) = f.to {
        sql.push_str(" AND started_at < fromUnixTimestamp64Milli(?)");
        binds.push(Bind::Millis(t));
    }
    if let Some(k) = f.key {
        sql.push_str(" AND key_id = ?");
        binds.push(Bind::Uuid(k));
    }
    if let Some(p) = f.principal {
        sql.push_str(" AND principal_id = ?");
        binds.push(Bind::Uuid(p));
    }
    if let Some(m) = &f.model {
        sql.push_str(" AND resolved_model = ?");
        binds.push(Bind::Str(m.clone()));
    }
    if let Some(p) = &f.provider {
        sql.push_str(" AND provider = ?");
        binds.push(Bind::Str(p.clone()));
    }
    if let Some(s) = f.status {
        sql.push_str(" AND status = ?");
        binds.push(Bind::U16(s));
    }
    if let Some(failed) = f.failed {
        sql.push_str(if failed {
            " AND status >= 400"
        } else {
            " AND status < 400"
        });
    }
    if let Some(k) = &f.error_kind {
        sql.push_str(" AND error_kind = ?");
        binds.push(Bind::Str(k.clone()));
    }
    if let Some(s) = f.stream {
        sql.push_str(" AND stream = ?");
        binds.push(Bind::Bool(s));
    }
    if let Some(s) = &f.source {
        sql.push_str(" AND source = ?");
        binds.push(Bind::Str(s.clone()));
    }
    if let Some(s) = f.source_id {
        sql.push_str(" AND source_id = ?");
        binds.push(Bind::Uuid(s));
    }
    if let Some(d) = &f.ingress_dialect {
        sql.push_str(" AND ingress_dialect = ?");
        binds.push(Bind::Str(d.clone()));
    }
    if let Some(c) = &f.response_cache {
        sql.push_str(" AND response_cache = ?");
        binds.push(Bind::Str(c.clone()));
    }
    if let Some(d) = &f.egress_dialect {
        sql.push_str(" AND egress_dialect = ?");
        binds.push(Bind::Str(d.clone()));
    }
    if let Some(ms) = f.min_duration_ms {
        sql.push_str(" AND duration_ms >= ?");
        binds.push(Bind::I64(i64::from(ms)));
    }
    if let Some(a) = &f.alias {
        sql.push_str(" AND alias = ?");
        binds.push(Bind::Str(a.clone()));
    }
    // A try the step made again lists `"retried":true`; a fallback doesn't.
    if let Some(fell) = f.fell_back {
        sql.push_str(if fell {
            " AND arrayExists(x -> NOT JSONExtractBool(x, 'retried'), JSONExtractArrayRaw(fallbacks))"
        } else {
            " AND NOT arrayExists(x -> NOT JSONExtractBool(x, 'retried'), JSONExtractArrayRaw(fallbacks))"
        });
    }
    if let Some(e) = &f.endpoint {
        sql.push_str(" AND endpoint = ?");
        binds.push(Bind::Str(e.clone()));
    }
    if let Some(c) = &f.min_cost {
        sql.push_str(" AND cost >= toDecimal128(?, 12)");
        binds.push(Bind::Str(c.clone()));
    }
    if let Some(m) = &f.requested_model {
        sql.push_str(" AND requested_model = ?");
        binds.push(Bind::Str(m.clone()));
    }
    if let Some(t) = &f.tag {
        sql.push_str(" AND has(tags, ?)");
        binds.push(Bind::Str(t.clone()));
    }
    if let Some(a) = &f.guardrail_action {
        sql.push_str(" AND has(guardrail_actions, ?)");
        binds.push(Bind::Str(a.clone()));
    }
    if let Some(m) = &f.error_message {
        sql.push_str(" AND positionCaseInsensitiveUTF8(error_message, ?) > 0");
        binds.push(Bind::Str(m.clone()));
    }
    if let Some(c) = &f.client {
        sql.push_str(&format!(" AND {} = ?", CLIENT_SQL.as_str()));
        binds.push(Bind::Str(c.clone()));
    }
}

/// One kind of failure: requests with the same error kind, status, provider and message once
/// its numbers and ids are left out, so "retry after 12 s" and "retry after 30 s" count as one.
#[derive(Clone, Debug, PartialEq, Row, Deserialize)]
pub struct ErrorGroup {
    pub error_kind: String,
    pub status: u16,
    pub provider: String,
    pub count: u64,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub first_at: OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub last_at: OffsetDateTime,
    /// The newest message of the group, as it was logged.
    pub example: String,
    #[serde(with = "clickhouse::serde::uuid")]
    pub request_id: Uuid,
}

// What a message's pattern leaves out, in order: request ids, UUIDs, then any other number.
const MESSAGE_VARIES: [(&str, &str); 3] = [
    ("req_[0-9A-Za-z]+", "req_"),
    (
        "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        "uuid",
    ),
    ("[0-9]+", "0"),
];

/// The most frequent failures among the requests the filter keeps, most first.
pub async fn common_errors(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    f: &Filter,
    limit: u32,
) -> Result<Vec<ErrorGroup>, StoreError> {
    // The columns come in the order of `ErrorGroup`; the message's pattern only groups.
    let mut sql = String::from(
        "SELECT error_kind, status, provider, count() AS count, min(started_at) AS first_at, \
         max(started_at) AS last_at, argMax(error_message, started_at) AS example, \
         argMax(request_id, started_at) AS request_id \
         FROM request_log WHERE org_id = ? AND project_id = ? AND status >= 400",
    );
    let mut binds: Vec<Bind> = vec![Bind::Uuid(org), Bind::Uuid(project)];
    filter_sql(f, &mut sql, &mut binds);
    let mut pattern = String::from("error_message");
    for (regex, with) in MESSAGE_VARIES {
        pattern = format!("replaceRegexpAll({pattern}, ?, ?)");
        binds.push(Bind::Str(regex.to_owned()));
        binds.push(Bind::Str(with.to_owned()));
    }
    sql.push_str(&format!(
        " GROUP BY error_kind, status, provider, {pattern} ORDER BY count DESC, last_at DESC LIMIT ?"
    ));
    binds.push(Bind::I64(i64::from(limit)));
    let mut q = ch.client().query(&sql);
    for b in binds {
        q = b.bind(q);
    }
    Ok(q.fetch_all::<ErrorGroup>().await?)
}

pub async fn get(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    id: Uuid,
) -> Result<Option<LogRow>, StoreError> {
    Ok(ch
        .client()
        .query("SELECT ?fields FROM request_log WHERE org_id = ? AND project_id = ? AND request_id = ? LIMIT 1")
        .bind(org)
        .bind(project)
        .bind(id)
        .fetch_optional::<LogRow>()
        .await?)
}

pub async fn contents(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    ids: &[Uuid],
) -> Result<Vec<ContentRow>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows = ch
        .client()
        .query("SELECT ?fields FROM request_content WHERE org_id = ? AND project_id = ? AND request_id IN ?")
        .bind(org)
        .bind(project)
        .bind(ids)
        .fetch_all::<ContentRow>()
        .await?;
    let wanted: Vec<parts::Hash> = rows
        .iter()
        .filter(|r| r.request_parts > 0)
        .flat_map(|r| parts::hashes(&r.request_body))
        .collect();
    if wanted.is_empty() {
        return Ok(rows);
    }
    let found = parts::bodies(ch, org, project, &wanted).await?;
    for row in rows.iter_mut().filter(|r| r.request_parts > 0) {
        let missing = parts::hashes(&row.request_body)
            .iter()
            .filter(|h| !found.contains_key(*h))
            .count();
        if missing > 0 {
            tracing::warn!(request_id = %row.request_id, missing, "parts of a logged request body are gone; it reads with their placeholders");
        }
        row.request_body = parts::splice(&row.request_body, &found);
        row.request_parts = 0;
    }
    Ok(rows)
}

#[derive(Clone, Debug, PartialEq, Eq, Row, Deserialize)]
pub struct ProjectStorage {
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    pub entries: u64,
    pub with_content: u64,
    pub logical_bytes: u64,
}

pub async fn storage(ch: &ClickHouse, org: Uuid) -> Result<Vec<ProjectStorage>, StoreError> {
    Ok(ch
        .client()
        .query(
            "SELECT project_id, count() AS entries, countIf(has_content) AS with_content, sum(logical_bytes) AS logical_bytes \
             FROM request_log WHERE org_id = ? GROUP BY project_id ORDER BY project_id",
        )
        .bind(org)
        .fetch_all::<ProjectStorage>()
        .await?)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Row, Deserialize)]
pub struct TableSize {
    pub rows: u64,
    pub bytes_on_disk: u64,
    pub compressed: u64,
    pub uncompressed: u64,
}

pub async fn table_size(ch: &ClickHouse, table: &str) -> Result<TableSize, StoreError> {
    Ok(ch
        .client()
        .query(
            "SELECT toUInt64(sum(rows)) AS rows, toUInt64(sum(bytes_on_disk)) AS bytes_on_disk, \
             toUInt64(sum(data_compressed_bytes)) AS compressed, toUInt64(sum(data_uncompressed_bytes)) AS uncompressed \
             FROM system.parts WHERE active AND database = ? AND table = ?",
        )
        .bind(ch.database())
        .bind(table)
        .fetch_optional::<TableSize>()
        .await?
        .unwrap_or_default())
}

/// All content as received the log holds, over every org: what the content tables' size on
/// disk is compared with.
pub async fn logical_total(ch: &ClickHouse) -> Result<u64, StoreError> {
    #[derive(Row, Deserialize)]
    struct Total {
        bytes: u64,
    }
    Ok(ch
        .client()
        .query("SELECT toUInt64(sum(logical_bytes)) AS bytes FROM request_log")
        .fetch_one::<Total>()
        .await?
        .bytes)
}

/// Brings the expiry of an organization's entries down to `days` after they started, where it is
/// later, so no entry outlives a retention that was shortened after it was written. ClickHouse
/// recalculates the TTL of the parts the mutation rewrites, so they expire by the new value.
pub async fn shorten_expiry(
    ch: &ClickHouse,
    org: Uuid,
    days: u32,
    content_only: bool,
) -> Result<(), StoreError> {
    let tables: &[&str] = if content_only {
        &["request_content", "tool_call_content"]
    } else {
        &[
            "request_log",
            "request_content",
            "tool_calls",
            "tool_call_content",
        ]
    };
    for table in tables {
        ch.client()
            .query(
                "ALTER TABLE ? UPDATE expires_at = toDateTime(started_at) + toIntervalDay(?) \
                 WHERE org_id = ? AND expires_at > toDateTime(started_at) + toIntervalDay(?)",
            )
            .with_setting("mutations_sync", "0")
            .bind(clickhouse::sql::Identifier(table))
            .bind(days)
            .bind(org)
            .bind(days)
            .execute()
            .await?;
    }
    Ok(())
}

pub async fn delete_range(
    ch: &ClickHouse,
    org: Uuid,
    project: Option<Uuid>,
    from: OffsetDateTime,
    to: OffsetDateTime,
    content_only: bool,
) -> Result<(), StoreError> {
    let tables: &[&str] = if content_only {
        &["request_content", "tool_call_content"]
    } else {
        &[
            "request_log",
            "request_content",
            "tool_calls",
            "tool_call_content",
        ]
    };
    for table in tables {
        let mut sql = String::from(
            "DELETE FROM ? WHERE org_id = ? AND started_at >= fromUnixTimestamp64Milli(?) AND started_at < fromUnixTimestamp64Milli(?)",
        );
        if project.is_some() {
            sql.push_str(" AND project_id = ?");
        }
        let millis =
            |t: OffsetDateTime| i64::try_from(t.unix_timestamp_nanos() / 1_000_000).unwrap_or(0);
        let mut q = ch
            .client()
            .query(&sql)
            .with_setting("lightweight_deletes_sync", "0")
            .bind(clickhouse::sql::Identifier(table))
            .bind(org)
            .bind(millis(from))
            .bind(millis(to));
        if let Some(p) = project {
            q = q.bind(p);
        }
        q.execute().await?;
    }
    delete_parts(ch, org, project, from, to).await
}

// A part goes with a range only when every request that may rely on it does: one that started
// between the part's `first_at` and its `last_at` plus the rewrite interval.
async fn delete_parts(
    ch: &ClickHouse,
    org: Uuid,
    project: Option<Uuid>,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> Result<(), StoreError> {
    let mut sql = String::from(
        "DELETE FROM request_content_part WHERE org_id = ? AND first_at >= fromUnixTimestamp64Milli(?) AND last_at < fromUnixTimestamp64Milli(?)",
    );
    if project.is_some() {
        sql.push_str(" AND project_id = ?");
    }
    let rewrite = time::Duration::try_from(parts::REWRITE_EVERY).unwrap_or(time::Duration::HOUR);
    let mut q = ch
        .client()
        .query(&sql)
        .with_setting("lightweight_deletes_sync", "0")
        .bind(org)
        .bind(millis(from))
        .bind(millis(to - rewrite));
    if let Some(p) = project {
        q = q.bind(p);
    }
    q.execute().await?;
    Ok(())
}

fn millis(t: OffsetDateTime) -> i64 {
    i64::try_from(t.unix_timestamp_nanos() / 1_000_000).unwrap_or(0)
}

const ORG_TABLES: &[&str] = &[
    "request_log",
    "request_content",
    "request_content_part",
    "tool_calls",
    "tool_call_content",
];

pub async fn delete_org(ch: &ClickHouse, org: Uuid) -> Result<(), StoreError> {
    for table in ORG_TABLES {
        ch.client()
            .query("DELETE FROM ? WHERE org_id = ?")
            .bind(clickhouse::sql::Identifier(table))
            .bind(org)
            .execute()
            .await?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Row, Deserialize)]
pub struct Disk {
    pub bytes: u64,
    pub rows: u64,
}

pub async fn disk(ch: &ClickHouse) -> Result<Disk, StoreError> {
    Ok(ch
        .client()
        .query(
            "SELECT sum(bytes_on_disk) AS bytes, sumIf(rows, table = 'request_log') AS rows \
             FROM system.parts WHERE active AND database = currentDatabase()",
        )
        .fetch_one::<Disk>()
        .await?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineGroup {
    Project,
    Model,
    Provider,
    Key,
    Member,
    Route,
    ErrorKind,
    /// The client app the user agent names; empty without one.
    Client,
    /// Empty for requests that named a model.
    Alias,
    Endpoint,
    Status,
    /// `hit`, `miss` or `bypass`; empty where the response cache was off.
    ResponseCache,
    /// The source that reported observed requests, the nil id for the gateway's own.
    Source,
}

impl TimelineGroup {
    fn key(self) -> &'static str {
        match self {
            TimelineGroup::Client => CLIENT_SQL.as_str(),
            TimelineGroup::Project => "toString(project_id)",
            TimelineGroup::Model => "concat(provider, '/', resolved_model)",
            TimelineGroup::Provider => "provider",
            TimelineGroup::Key => {
                "if(source_id = toUUID('00000000-0000-0000-0000-000000000000'), toString(key_id), \
                 concat('source:', toString(source_id)))"
            }
            TimelineGroup::Member => "concat(principal_kind, ':', toString(principal_id))",
            TimelineGroup::Route => "concat(ingress_dialect, ':', egress_dialect, ':', provider)",
            TimelineGroup::ErrorKind => "error_kind",
            TimelineGroup::Alias => "alias",
            TimelineGroup::Endpoint => "endpoint",
            TimelineGroup::Status => "toString(status)",
            TimelineGroup::ResponseCache => "response_cache",
            TimelineGroup::Source => "toString(source_id)",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub project: Option<Uuid>,
    pub filter: UsageFilter,
    pub failed: Option<bool>,
    pub group: Option<TimelineGroup>,
    pub top: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Row, Deserialize)]
pub struct TimelineRow {
    pub start_unix: i64,
    pub key: String,
    pub requests: u64,
    pub errors: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub cost: i128,
    pub duration: Vec<f64>,
    pub duration_max: u32,
    pub ttfb: Vec<f64>,
    pub with_ttfb: u64,
}

fn timeline_where(
    org: Uuid,
    from: OffsetDateTime,
    to: OffsetDateTime,
    t: &Timeline,
    sql: &mut String,
    binds: &mut Vec<Bind>,
) {
    sql.push_str(" FROM request_log WHERE org_id = ?");
    binds.push(Bind::Uuid(org));
    if let Some(p) = t.project {
        sql.push_str(" AND project_id = ?");
        binds.push(Bind::Uuid(p));
    }
    sql.push_str(" AND started_at >= fromUnixTimestamp64Milli(?) AND started_at < fromUnixTimestamp64Milli(?)");
    binds.push(Bind::Millis(from));
    binds.push(Bind::Millis(to));
    let f = &t.filter;
    if let Some(p) = &f.provider {
        sql.push_str(" AND provider = ?");
        binds.push(Bind::Str(p.clone()));
    }
    if let Some(m) = &f.model {
        sql.push_str(" AND resolved_model = ?");
        binds.push(Bind::Str(m.id.clone()));
        if let Some(p) = &m.provider {
            sql.push_str(" AND provider = ?");
            binds.push(Bind::Str(p.clone()));
        }
    }
    if let Some(k) = f.key {
        sql.push_str(
            " AND key_id = ? AND source_id = toUUID('00000000-0000-0000-0000-000000000000')",
        );
        binds.push(Bind::Uuid(k));
    }
    if let Some(s) = f.source {
        sql.push_str(" AND source_id = ?");
        binds.push(Bind::Uuid(s));
    }
    if let Some(m) = f.member {
        let (kind, id) = match m {
            Member::User(u) => ("user", u),
            Member::ServiceAccount(s) => ("service_account", s),
        };
        sql.push_str(" AND principal_kind = ? AND principal_id = ?");
        binds.push(Bind::Str(kind.to_owned()));
        binds.push(Bind::Uuid(id));
    }
    if let Some(r) = &f.route {
        sql.push_str(" AND ingress_dialect = ? AND egress_dialect = ? AND provider = ?");
        binds.push(Bind::Str(r.ingress_dialect.clone()));
        binds.push(Bind::Str(r.egress_dialect.clone()));
        binds.push(Bind::Str(r.provider.clone()));
    }
    if let Some(failed) = t.failed {
        sql.push_str(if failed {
            " AND status >= 400"
        } else {
            " AND status < 400"
        });
    }
}

// The row written when a video job or an image generation finishes carries the job's cost and
// tokens, but the job is the request that started it, and the row has no duration of its own.
const STARTED: &str = "endpoint NOT IN ('videos/completed', 'flows/image/completed')";

pub async fn timeline(
    ch: &ClickHouse,
    org: Uuid,
    from: OffsetDateTime,
    to: OffsetDateTime,
    step_seconds: u32,
    t: &Timeline,
) -> Result<Vec<TimelineRow>, StoreError> {
    // The grouping expression comes from the enum above, never from input; the values are bound.
    let key = t.group.map_or("''", TimelineGroup::key);
    let mut sql = format!(
        "SELECT toInt64(toUnixTimestamp(toStartOfInterval(started_at, toIntervalSecond(?)))) AS start_unix, \
         {key} AS key, countIf({STARTED}) AS requests, countIf(status >= 400 AND {STARTED}) AS errors, \
         sum(input_tokens) AS input_tokens, sum(output_tokens) AS output_tokens, \
         sum(cache_read_tokens) AS cache_read_tokens, sum(cache_write_tokens) AS cache_write_tokens, \
         sum(reasoning_tokens) AS reasoning_tokens, sum(cost) AS cost, \
         quantilesIf(0.5, 0.95)(duration_ms, {STARTED}) AS duration, maxIf(duration_ms, {STARTED}) AS duration_max, \
         quantilesIf(0.5, 0.95)(assumeNotNull(ttfb_ms), isNotNull(ttfb_ms) AND {STARTED}) AS ttfb, \
         countIf(isNotNull(ttfb_ms) AND {STARTED}) AS with_ttfb"
    );
    let mut binds: Vec<Bind> = vec![Bind::I64(i64::from(step_seconds))];
    timeline_where(org, from, to, t, &mut sql, &mut binds);
    if let (Some(group), Some(top)) = (t.group, t.top) {
        let key = group.key();
        sql.push_str(&format!(" AND {key} IN (SELECT {key} AS top_key"));
        timeline_where(org, from, to, t, &mut sql, &mut binds);
        sql.push_str(" GROUP BY top_key ORDER BY count() DESC, top_key LIMIT ?)");
        binds.push(Bind::I64(i64::from(top)));
    }
    sql.push_str(" GROUP BY start_unix, key ORDER BY start_unix, key");
    let mut q = ch.client().query(&sql);
    for b in binds {
        q = b.bind(q);
    }
    Ok(q.fetch_all::<TimelineRow>().await?)
}

#[cfg(test)]
mod client_tests {
    use super::*;

    #[test]
    fn a_user_agent_names_its_client_and_version() {
        let cases = [
            (
                "codex_cli_rs/0.59.0 (Mac OS 26.1.0; arm64) Ghostty/1.2.3",
                "codex",
                Some("0.59.0"),
            ),
            (
                "claude-cli/2.1.31 (external, cli)",
                "claude-code",
                Some("2.1.31"),
            ),
            ("OpenAI/Python 2.9.0", "openai-python", Some("2.9.0")),
            ("openai-python/1.40.0", "openai-python", Some("1.40.0")),
            ("OpenAI/JS 5.1.0", "openai-js", Some("5.1.0")),
            (
                "Anthropic/Python 0.70.0",
                "anthropic-python",
                Some("0.70.0"),
            ),
            ("Anthropic/JS 0.60.0", "anthropic-js", Some("0.60.0")),
            ("ai-sdk/5.0.1 runtime/node", "ai-sdk", Some("5.0.1")),
            ("curl/8.7.1", "curl", Some("8.7.1")),
            ("curl/", "curl", None),
            ("Mozilla/5.0 (Macintosh)", "other", None),
            ("", "", None),
        ];
        for (ua, client, version) in cases {
            assert_eq!(
                (client_of(ua), client_version(ua)),
                (client, version),
                "{ua}"
            );
        }
        assert_eq!(
            client_names(),
            [
                "codex",
                "claude-code",
                "openai-python",
                "openai-js",
                "anthropic-python",
                "anthropic-js",
                "ai-sdk",
                "curl",
                "gemini-cli",
                "antigravity",
                "litellm",
                "other"
            ]
        );
    }

    #[test]
    fn the_sql_names_clients_from_the_same_table() {
        let sql = CLIENT_SQL.as_str();
        for (prefix, name) in CLIENTS {
            assert!(sql.contains(&format!(
                "startsWith(lower(user_agent), '{prefix}'), '{name}'"
            )));
            assert_eq!(prefix.to_ascii_lowercase(), *prefix, "compared lowercased");
            assert!(!prefix.contains('\'') && !name.contains('\''));
        }
        assert!(sql.starts_with("multiIf(user_agent = '', '', ") && sql.ends_with(", 'other')"));
    }
}
