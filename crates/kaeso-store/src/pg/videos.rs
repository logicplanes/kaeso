use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

/// A video job an app started through Kaeso: whose it is, where it runs, and how far it is.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub provider: String,
    pub credential_id: Uuid,
    pub catalogue_model_id: Option<Uuid>,
    pub model: String,
    pub upstream_model: String,
    pub dialect: String,
    pub upstream_id: String,
    pub request_id: Uuid,
    pub params: Value,
    pub prices: Option<Value>,
    pub status: String,
    pub progress: i16,
    pub error: Option<Value>,
    pub remixed_from: Option<Uuid>,
    pub units: Option<Value>,
    pub cost: Option<Decimal>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
    pub accounted_at: Option<OffsetDateTime>,
}

/// What the provider last said about a job.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoProgress {
    pub status: String,
    pub progress: i16,
    pub error: Option<Value>,
    pub completed_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}

pub async fn insert(db: impl PgExecutor<'_>, v: &VideoRow) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO videos (id, org_id, project_id, api_key_id, user_id, service_account_id, provider, credential_id, \
         catalogue_model_id, model, upstream_model, dialect, upstream_id, request_id, params, \
         prices, status, progress, error, remixed_from, units, cost, created_at, completed_at, \
         expires_at, accounted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, \
         $19, $20, $21, $22, $23, $24, $25, $26)",
        v.id,
        v.org_id,
        v.project_id,
        v.api_key_id,
        v.user_id,
        v.service_account_id,
        v.provider,
        v.credential_id,
        v.catalogue_model_id,
        v.model,
        v.upstream_model,
        v.dialect,
        v.upstream_id,
        v.request_id,
        v.params,
        v.prices,
        v.status,
        v.progress,
        v.error,
        v.remixed_from,
        v.units,
        v.cost,
        v.created_at,
        v.completed_at,
        v.expires_at,
        v.accounted_at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The project's video `id`, or `None` when the project has none by that id.
pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<VideoRow>, StoreError> {
    Ok(sqlx::query_as!(
        VideoRow,
        "SELECT id, org_id, project_id, api_key_id, user_id, service_account_id, provider, credential_id, catalogue_model_id, \
         model, upstream_model, dialect, upstream_id, request_id, params, prices, status, progress, \
         error, remixed_from, units, cost, created_at, completed_at, expires_at, accounted_at \
         FROM videos WHERE org_id = $1 AND project_id = $2 AND id = $3",
        org_id,
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// A page of the project's videos, newest first unless `ascending`, after the video `after`
/// when it names one of the project's.
pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    after: Option<Uuid>,
    ascending: bool,
    limit: i64,
) -> Result<Vec<VideoRow>, StoreError> {
    Ok(sqlx::query_as!(
        VideoRow,
        "SELECT v.id, v.org_id, v.project_id, v.api_key_id, v.user_id, v.service_account_id, v.provider, v.credential_id, \
         v.catalogue_model_id, v.model, v.upstream_model, v.dialect, v.upstream_id, v.request_id, \
         v.params, v.prices, v.status, v.progress, v.error, v.remixed_from, v.units, v.cost, \
         v.created_at, v.completed_at, v.expires_at, v.accounted_at \
         FROM videos v \
         LEFT JOIN videos a ON a.org_id = $1 AND a.project_id = $2 AND a.id = $3 \
         WHERE v.org_id = $1 AND v.project_id = $2 \
           AND ($3::uuid IS NULL OR ( \
             CASE WHEN $4 THEN (v.created_at, v.id) > (a.created_at, a.id) \
                  ELSE (v.created_at, v.id) < (a.created_at, a.id) END)) \
         ORDER BY \
           CASE WHEN $4 THEN v.created_at END ASC, CASE WHEN $4 THEN v.id END ASC, \
           CASE WHEN NOT $4 THEN v.created_at END DESC, CASE WHEN NOT $4 THEN v.id END DESC \
         LIMIT $5",
        org_id,
        project_id,
        after,
        ascending,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// Whether the project has the video `id`, for a listing's `after`.
pub async fn exists(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM videos WHERE org_id = $1 AND project_id = $2 AND id = $3)",
        org_id,
        project_id,
        id
    )
    .fetch_one(db)
    .await?
    .unwrap_or(false))
}

/// Writes what the provider last said about a job, until the job was accounted for.
pub async fn update_progress(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    p: &VideoProgress,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE videos SET status = $3, progress = $4, error = $5, completed_at = $6, \
         expires_at = COALESCE($7, expires_at) \
         WHERE org_id = $1 AND id = $2 AND accounted_at IS NULL",
        org_id,
        id,
        p.status,
        p.progress,
        p.error,
        p.completed_at,
        p.expires_at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Marks a finished job accounted for with its units and cost; returns `true` only to the one
/// caller that did, so a job is billed once however many requests see it finish.
pub async fn account(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    p: &VideoProgress,
    units: Option<&Value>,
    cost: Option<Decimal>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE videos SET status = $3, progress = $4, error = $5, completed_at = $6, \
         expires_at = COALESCE($7, expires_at), units = $8, cost = $9, accounted_at = $10 \
         WHERE org_id = $1 AND id = $2 AND accounted_at IS NULL RETURNING id",
        org_id,
        id,
        p.status,
        p.progress,
        p.error,
        p.completed_at,
        p.expires_at,
        units,
        cost,
        at
    )
    .fetch_optional(db)
    .await?
    .is_some())
}

/// Removes the project's video `id`; returns whether it was there.
pub async fn delete(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM videos WHERE org_id = $1 AND project_id = $2 AND id = $3",
        org_id,
        project_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0)
}
