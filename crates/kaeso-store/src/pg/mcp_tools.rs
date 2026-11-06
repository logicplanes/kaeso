use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolRow {
    pub id: Uuid,
    pub server_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub annotations: Option<Value>,
    pub meta: Option<Value>,
    pub definition_hash: Vec<u8>,
    pub approved_hash: Option<Vec<u8>>,
    pub approved: Option<Value>,
    pub state: String,
    pub first_seen_at: OffsetDateTime,
    pub changed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

impl ToolRow {
    pub fn is_approved(&self) -> bool {
        self.state == "active"
            && self.approved_hash.as_deref() == Some(self.definition_hash.as_slice())
    }
}

#[derive(Clone, Debug)]
pub struct Definition<'a> {
    pub name: &'a str,
    pub title: Option<&'a str>,
    pub description: &'a str,
    pub input_schema: &'a Value,
    pub output_schema: Option<&'a Value>,
    pub annotations: Option<&'a Value>,
    pub meta: Option<&'a Value>,
    pub hash: &'a [u8],
}

// What a stored definition was approved as: the review shows it next to the new one.
#[derive(Clone, Debug)]
pub struct Approval<'a> {
    pub hash: &'a [u8],
    pub definition: &'a Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectToolRow {
    pub tool: ToolRow,
    pub server_handle: String,
    pub server_name: String,
    pub enabled: bool,
    pub confirm: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerTools {
    pub total: i64,
    pub changed: i64,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    server_id: Uuid,
    org_id: Uuid,
    d: &Definition<'_>,
    approval: Option<&Approval<'_>>,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    let state = if approval.is_some() {
        "active"
    } else {
        "changed"
    };
    Ok(sqlx::query!(
        "INSERT INTO mcp_tools (id, server_id, org_id, name, title, description, input_schema, output_schema, \
         annotations, meta, definition_hash, approved_hash, approved, state, first_seen_at, changed_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
         CASE WHEN $14 = 'changed' THEN $15::timestamptz END, $15) \
         ON CONFLICT (server_id, name) DO NOTHING",
        id,
        server_id,
        org_id,
        d.name,
        d.title,
        d.description,
        d.input_schema,
        d.output_schema,
        d.annotations,
        d.meta,
        d.hash,
        approval.map(|a| a.hash),
        approval.map(|a| a.definition),
        state,
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn update_definition(
    db: impl PgExecutor<'_>,
    id: Uuid,
    d: &Definition<'_>,
    approval: Option<&Approval<'_>>,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_tools SET title = $3, description = $4, input_schema = $5, output_schema = $6, \
         annotations = $7, meta = $8, definition_hash = $9, \
         approved_hash = COALESCE($10, approved_hash), approved = COALESCE($11, approved), \
         state = CASE WHEN $9 = COALESCE($10, approved_hash) THEN 'active' ELSE 'changed' END, \
         changed_at = CASE WHEN $9 = COALESCE($10, approved_hash) THEN changed_at ELSE $12 END, \
         updated_at = $12 WHERE id = $1 AND name = $2",
        id,
        d.name,
        d.title,
        d.description,
        d.input_schema,
        d.output_schema,
        d.annotations,
        d.meta,
        d.hash,
        approval.map(|a| a.hash),
        approval.map(|a| a.definition),
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn approve(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    expected_hash: &[u8],
    approval: &Approval<'_>,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_tools SET approved_hash = $4, approved = $5, state = 'active', updated_at = $6 \
         WHERE org_id = $1 AND id = $2 AND definition_hash = $3 AND definition_hash = $4 AND state = 'changed'",
        org_id,
        id,
        expected_hash,
        approval.hash,
        approval.definition,
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn mark_removed(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
    present: &[String],
    now: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE mcp_tools SET state = 'removed', updated_at = $3 \
         WHERE server_id = $1 AND name <> ALL($2::text[]) AND state <> 'removed' RETURNING id",
        server_id,
        present,
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn of_server(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
) -> Result<Vec<ToolRow>, StoreError> {
    Ok(sqlx::query_as!(
        ToolRow,
        "SELECT id, server_id, org_id, name, title, description, input_schema, output_schema, annotations, meta, \
         definition_hash, approved_hash, approved, state, first_seen_at, changed_at, updated_at \
         FROM mcp_tools WHERE server_id = $1 ORDER BY name",
        server_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<ToolRow>, StoreError> {
    Ok(sqlx::query_as!(
        ToolRow,
        "SELECT id, server_id, org_id, name, title, description, input_schema, output_schema, annotations, meta, \
         definition_hash, approved_hash, approved, state, first_seen_at, changed_at, updated_at \
         FROM mcp_tools WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn counts(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(Uuid, ServerTools)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT server_id, count(*) FILTER (WHERE state <> 'removed') AS \"total!\", \
         count(*) FILTER (WHERE state = 'changed') AS \"changed!\" \
         FROM mcp_tools WHERE org_id = $1 GROUP BY server_id ORDER BY server_id",
        org_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.server_id,
                ServerTools {
                    total: r.total,
                    changed: r.changed,
                },
            )
        })
        .collect())
}

pub async fn of_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<ProjectToolRow>, StoreError> {
    let rows = sqlx::query!(
        "SELECT t.id, t.server_id, t.org_id, t.name, t.title, t.description, t.input_schema, t.output_schema, \
         t.annotations, t.meta, t.definition_hash, t.approved_hash, t.approved, t.state, t.first_seen_at, \
         t.changed_at, t.updated_at, s.handle AS server_handle, s.name AS server_name, \
         COALESCE(pt.enabled, false) AS \"enabled!\", COALESCE(pt.confirm, false) AS \"confirm!\" \
         FROM project_mcp_servers ps JOIN mcp_servers s ON s.id = ps.server_id \
         JOIN mcp_tools t ON t.server_id = ps.server_id \
         LEFT JOIN project_tools pt ON pt.project_id = ps.project_id AND pt.tool_id = t.id \
         WHERE ps.project_id = $1 AND t.state <> 'removed' ORDER BY s.handle, t.name",
        project_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ProjectToolRow {
            tool: ToolRow {
                id: r.id,
                server_id: r.server_id,
                org_id: r.org_id,
                name: r.name,
                title: r.title,
                description: r.description,
                input_schema: r.input_schema,
                output_schema: r.output_schema,
                annotations: r.annotations,
                meta: r.meta,
                definition_hash: r.definition_hash,
                approved_hash: r.approved_hash,
                approved: r.approved,
                state: r.state,
                first_seen_at: r.first_seen_at,
                changed_at: r.changed_at,
                updated_at: r.updated_at,
            },
            server_handle: r.server_handle,
            server_name: r.server_name,
            enabled: r.enabled,
            confirm: r.confirm,
        })
        .collect())
}

pub async fn set_enabled(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    tools: &[Uuid],
    enabled: bool,
) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO project_tools (project_id, server_id, tool_id, enabled) \
         SELECT ps.project_id, t.server_id, t.id, $3 FROM mcp_tools t \
         JOIN project_mcp_servers ps ON ps.server_id = t.server_id AND ps.project_id = $1 \
         WHERE t.id = ANY($2::uuid[]) \
         ON CONFLICT (project_id, tool_id) DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now()",
        project_id,
        tools,
        enabled
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn disable_everywhere(db: impl PgExecutor<'_>, tool_id: Uuid) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "UPDATE project_tools SET enabled = false, updated_at = now() WHERE tool_id = $1 AND enabled",
        tool_id
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn projects_using(
    db: impl PgExecutor<'_>,
    tool_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT project_id FROM project_tools WHERE tool_id = $1 AND enabled ORDER BY project_id",
        tool_id
    )
    .fetch_all(db)
    .await?)
}
