use clickhouse::Row;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::ClickHouse;
use crate::StoreError;

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
    pub overhead_ms: Option<u32>,
    pub stream_processing_ms: Option<u32>,
    pub cache_breakpoints: Option<u16>,
    pub request_upload_ms: Option<u32>,
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
}

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
    if let Some(d) = &f.ingress_dialect {
        sql.push_str(" AND ingress_dialect = ?");
        binds.push(Bind::Str(d.clone()));
    }
    if let Some(d) = &f.egress_dialect {
        sql.push_str(" AND egress_dialect = ?");
        binds.push(Bind::Str(d.clone()));
    }
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
    Ok(ch
        .client()
        .query("SELECT ?fields FROM request_content WHERE org_id = ? AND project_id = ? AND request_id IN ?")
        .bind(org)
        .bind(project)
        .bind(ids)
        .fetch_all::<ContentRow>()
        .await?)
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

pub async fn delete_range(
    ch: &ClickHouse,
    org: Uuid,
    project: Option<Uuid>,
    from: OffsetDateTime,
    to: OffsetDateTime,
    content_only: bool,
) -> Result<(), StoreError> {
    let tables: &[&str] = if content_only {
        &["request_content"]
    } else {
        &["request_log", "request_content"]
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
    Ok(())
}

const ORG_TABLES: &[&str] = &["request_log", "request_content"];

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
