use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogJob {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
    pub kind: String,
    pub from_at: OffsetDateTime,
    pub to_at: OffsetDateTime,
    pub requested_by_kind: String,
    pub requested_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub done_at: Option<OffsetDateTime>,
}

pub struct NewLogJob<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
    pub kind: &'a str,
    pub from_at: OffsetDateTime,
    pub to_at: OffsetDateTime,
    pub requested_by_kind: &'a str,
    pub requested_by_id: Option<Uuid>,
}

pub async fn enqueue(db: impl PgExecutor<'_>, job: &NewLogJob<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO log_jobs (id, org_id, project_id, kind, from_at, to_at, requested_by_kind, requested_by_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        job.id,
        job.org_id,
        job.project_id,
        job.kind,
        job.from_at,
        job.to_at,
        job.requested_by_kind,
        job.requested_by_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn pending(db: impl PgExecutor<'_>, limit: i64) -> Result<Vec<LogJob>, StoreError> {
    Ok(sqlx::query_as!(
        LogJob,
        "SELECT id, org_id, project_id, kind, from_at, to_at, requested_by_kind, requested_by_id, created_at, attempts, \
         last_error, done_at FROM log_jobs WHERE done_at IS NULL ORDER BY created_at LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    limit: i64,
) -> Result<Vec<LogJob>, StoreError> {
    Ok(sqlx::query_as!(
        LogJob,
        "SELECT id, org_id, project_id, kind, from_at, to_at, requested_by_kind, requested_by_id, created_at, attempts, \
         last_error, done_at FROM log_jobs WHERE org_id = $1 ORDER BY created_at DESC LIMIT $2",
        org_id,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn finish(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE log_jobs SET attempts = attempts + 1, last_error = $3, \
         done_at = CASE WHEN $3::text IS NULL THEN $2::timestamptz ELSE NULL END WHERE id = $1",
        id,
        at,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogSettings {
    pub org_content: bool,
    pub project_content: bool,
    pub metadata_days: i32,
    pub content_days: i32,
}

pub async fn settings(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Option<LogSettings>, StoreError> {
    Ok(sqlx::query_as!(
        LogSettings,
        "SELECT o.content_logging AS org_content, p.content_logging AS project_content, \
         o.metadata_retention_days AS metadata_days, o.content_retention_days AS content_days \
         FROM projects p JOIN organizations o ON o.id = p.org_id WHERE p.id = $1",
        project_id
    )
    .fetch_optional(db)
    .await?)
}
