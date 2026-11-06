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
