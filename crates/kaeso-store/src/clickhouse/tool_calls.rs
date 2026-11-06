use clickhouse::Row;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::ClickHouse;
use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Row, Serialize, Deserialize)]
pub struct ToolCallRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub org_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub server_id: Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub started_at: OffsetDateTime,
    pub tool: String,
    pub upstream_tool: String,
    pub caller_kind: String,
    #[serde(with = "clickhouse::serde::uuid")]
    pub caller_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid::option")]
    pub key_id: Option<Uuid>,
    #[serde(with = "clickhouse::serde::uuid::option")]
    pub grant_id: Option<Uuid>,
    pub client_name: String,
    pub protocol_version: String,
    pub source: String,
    pub status: String,
    pub error_type: String,
    pub error_message: String,
    pub duration_ms: u32,
    pub upstream_ms: Option<u32>,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub has_content: bool,
    #[serde(with = "clickhouse::serde::time::datetime")]
    pub expires_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Row, Serialize, Deserialize)]
pub struct ToolCallContentRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub org_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub started_at: OffsetDateTime,
    pub arguments: String,
    pub result: String,
    #[serde(with = "clickhouse::serde::time::datetime")]
    pub expires_at: OffsetDateTime,
}

pub async fn insert_calls<'a>(
    ch: &ClickHouse,
    rows: impl IntoIterator<Item = &'a ToolCallRow>,
) -> Result<(), StoreError> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut insert = ch.client().insert::<ToolCallRow>("tool_calls").await?;
    for r in rows {
        insert.write(r).await?;
    }
    insert.end().await?;
    Ok(())
}

pub async fn insert_contents<'a>(
    ch: &ClickHouse,
    rows: impl IntoIterator<Item = &'a ToolCallContentRow>,
) -> Result<(), StoreError> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut insert = ch
        .client()
        .insert::<ToolCallContentRow>("tool_call_content")
        .await?;
    for r in rows {
        insert.write(r).await?;
    }
    insert.end().await?;
    Ok(())
}

pub async fn get(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    id: Uuid,
) -> Result<Option<ToolCallRow>, StoreError> {
    Ok(ch
        .client()
        .query(
            "SELECT ?fields FROM tool_calls WHERE org_id = ? AND project_id = ? AND id = ? LIMIT 1",
        )
        .bind(org)
        .bind(project)
        .bind(id)
        .fetch_optional::<ToolCallRow>()
        .await?)
}

pub async fn content(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    id: Uuid,
) -> Result<Option<ToolCallContentRow>, StoreError> {
    Ok(ch
        .client()
        .query("SELECT ?fields FROM tool_call_content WHERE org_id = ? AND project_id = ? AND id = ? LIMIT 1")
        .bind(org)
        .bind(project)
        .bind(id)
        .fetch_optional::<ToolCallContentRow>()
        .await?)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
    pub key: Option<Uuid>,
    pub caller: Option<Uuid>,
    pub server: Option<Uuid>,
    pub tool: Option<String>,
    pub status: Option<String>,
    pub error_type: Option<String>,
}

enum Bind {
    Uuid(Uuid),
    Millis(OffsetDateTime),
    Str(String),
    I64(i64),
}

fn millis(t: OffsetDateTime) -> i64 {
    i64::try_from(t.unix_timestamp_nanos() / 1_000_000).unwrap_or(0)
}

// Newest first, by `(started_at, id)`, so a page continues where the last one ended.
pub async fn search(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    f: &Filter,
    after: Option<(OffsetDateTime, Uuid)>,
    limit: i64,
) -> Result<Vec<ToolCallRow>, StoreError> {
    let mut sql =
        String::from("SELECT ?fields FROM tool_calls WHERE org_id = ? AND project_id = ?");
    let mut binds = vec![Bind::Uuid(org), Bind::Uuid(project)];
    let mut push = |clause: &str, bind: Bind| {
        sql.push_str(clause);
        binds.push(bind);
    };
    if let Some(t) = f.from {
        push(
            " AND started_at >= fromUnixTimestamp64Milli(?)",
            Bind::Millis(t),
        );
    }
    if let Some(t) = f.to {
        push(
            " AND started_at < fromUnixTimestamp64Milli(?)",
            Bind::Millis(t),
        );
    }
    if let Some(k) = f.key {
        push(" AND key_id = ?", Bind::Uuid(k));
    }
    if let Some(c) = f.caller {
        push(" AND caller_id = ?", Bind::Uuid(c));
    }
    if let Some(s) = f.server {
        push(" AND server_id = ?", Bind::Uuid(s));
    }
    if let Some(t) = &f.tool {
        push(" AND tool = ?", Bind::Str(t.clone()));
    }
    if let Some(s) = &f.status {
        push(" AND status = ?", Bind::Str(s.clone()));
    }
    if let Some(e) = &f.error_type {
        push(" AND error_type = ?", Bind::Str(e.clone()));
    }
    if let Some((at, id)) = after {
        push(
            " AND (started_at, id) < (fromUnixTimestamp64Milli(?), ?)",
            Bind::Millis(at),
        );
        binds.push(Bind::Uuid(id));
    }
    sql.push_str(" ORDER BY started_at DESC, id DESC LIMIT ?");
    binds.push(Bind::I64(limit));
    let mut q = ch.client().query(&sql);
    for b in binds {
        q = match b {
            Bind::Uuid(u) => q.bind(u),
            Bind::Millis(t) => q.bind(millis(t)),
            Bind::Str(s) => q.bind(s),
            Bind::I64(v) => q.bind(v),
        };
    }
    Ok(q.fetch_all::<ToolCallRow>().await?)
}

#[derive(Clone, Debug, PartialEq, Eq, Row, Deserialize)]
pub struct ToolCallCount {
    #[serde(with = "clickhouse::serde::uuid")]
    pub server_id: Uuid,
    pub upstream_tool: String,
    pub calls: u64,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub last: OffsetDateTime,
}

// One row per tool called since `since`, for the project's tool lists.
pub async fn counts(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    since: OffsetDateTime,
) -> Result<Vec<ToolCallCount>, StoreError> {
    Ok(ch
        .client()
        .query(
            "SELECT server_id, upstream_tool, count() AS calls, max(started_at) AS last FROM tool_calls \
             WHERE org_id = ? AND project_id = ? AND started_at >= fromUnixTimestamp64Milli(?) \
             GROUP BY server_id, upstream_tool",
        )
        .bind(org)
        .bind(project)
        .bind(millis(since))
        .fetch_all::<ToolCallCount>()
        .await?)
}
