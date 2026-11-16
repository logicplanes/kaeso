use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq)]
pub struct FileRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub credential_id: Uuid,
    pub provider: String,
    pub upstream_id: String,
    pub purpose: String,
    pub filename: String,
    pub bytes: i64,
    pub lines: i32,
    pub endpoint: Option<String>,
    pub catalogue_model_id: Option<Uuid>,
    pub model: Option<String>,
    pub upstream_model: Option<String>,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BatchRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub credential_id: Uuid,
    pub provider: String,
    pub upstream_id: String,
    pub input_file_id: Uuid,
    pub output_file_id: Option<Uuid>,
    pub error_file_id: Option<Uuid>,
    pub endpoint: String,
    pub catalogue_model_id: Uuid,
    pub model: String,
    pub upstream_model: String,
    pub prices: Option<Value>,
    pub metadata: Option<Value>,
    pub status: String,
    pub view: Value,
    pub created_at: OffsetDateTime,
    pub accounted_at: Option<OffsetDateTime>,
}

pub async fn insert_file(db: impl PgExecutor<'_>, f: &FileRow) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO batch_files (id, org_id, project_id, api_key_id, user_id, service_account_id, \
         credential_id, provider, upstream_id, purpose, filename, bytes, lines, endpoint, \
         catalogue_model_id, model, upstream_model, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19) \
         ON CONFLICT (provider, upstream_id) DO NOTHING",
        f.id,
        f.org_id,
        f.project_id,
        f.api_key_id,
        f.user_id,
        f.service_account_id,
        f.credential_id,
        f.provider,
        f.upstream_id,
        f.purpose,
        f.filename,
        f.bytes,
        f.lines,
        f.endpoint,
        f.catalogue_model_id,
        f.model,
        f.upstream_model,
        f.created_at,
        f.expires_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn file(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<FileRow>, StoreError> {
    Ok(sqlx::query_as!(
        FileRow,
        "SELECT id, org_id, project_id, api_key_id, user_id, service_account_id, credential_id, \
         provider, upstream_id, purpose, filename, bytes, lines, endpoint, catalogue_model_id, \
         model, upstream_model, created_at, expires_at FROM batch_files \
         WHERE org_id = $1 AND project_id = $2 AND id = $3",
        org_id,
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// The Kaeso id of a provider's file the project already knows, such as a batch's output.
pub async fn file_by_upstream(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    provider: &str,
    upstream_id: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM batch_files WHERE project_id = $1 AND provider = $2 AND upstream_id = $3",
        project_id,
        provider,
        upstream_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn delete_file(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM batch_files WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn insert(db: impl PgExecutor<'_>, b: &BatchRow) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO batches (id, org_id, project_id, api_key_id, user_id, service_account_id, \
         credential_id, provider, upstream_id, input_file_id, output_file_id, error_file_id, \
         endpoint, catalogue_model_id, model, upstream_model, prices, metadata, status, view, \
         created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, \
         $19, $20, $21)",
        b.id,
        b.org_id,
        b.project_id,
        b.api_key_id,
        b.user_id,
        b.service_account_id,
        b.credential_id,
        b.provider,
        b.upstream_id,
        b.input_file_id,
        b.output_file_id,
        b.error_file_id,
        b.endpoint,
        b.catalogue_model_id,
        b.model,
        b.upstream_model,
        b.prices,
        b.metadata,
        b.status,
        b.view,
        b.created_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<BatchRow>, StoreError> {
    Ok(sqlx::query_as!(
        BatchRow,
        "SELECT id, org_id, project_id, api_key_id, user_id, service_account_id, credential_id, \
         provider, upstream_id, input_file_id, output_file_id, error_file_id, endpoint, \
         catalogue_model_id, model, upstream_model, prices, metadata, status, view, created_at, \
         accounted_at FROM batches WHERE org_id = $1 AND project_id = $2 AND id = $3",
        org_id,
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// The project's batches, newest first, after `after` when given.
pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<BatchRow>, StoreError> {
    Ok(sqlx::query_as!(
        BatchRow,
        "SELECT b.id, b.org_id, b.project_id, b.api_key_id, b.user_id, b.service_account_id, \
         b.credential_id, b.provider, b.upstream_id, b.input_file_id, b.output_file_id, \
         b.error_file_id, b.endpoint, b.catalogue_model_id, b.model, b.upstream_model, b.prices, \
         b.metadata, b.status, b.view, b.created_at, b.accounted_at FROM batches b \
         WHERE b.org_id = $1 AND b.project_id = $2 AND ($3::uuid IS NULL OR (b.created_at, b.id) < ( \
             SELECT a.created_at, a.id FROM batches a WHERE a.project_id = $2 AND a.id = $3)) \
         ORDER BY b.created_at DESC, b.id DESC LIMIT $4",
        org_id,
        project_id,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// Batches whose usage isn't recorded yet, oldest first.
pub async fn open(db: impl PgExecutor<'_>, limit: i64) -> Result<Vec<BatchRow>, StoreError> {
    Ok(sqlx::query_as!(
        BatchRow,
        "SELECT id, org_id, project_id, api_key_id, user_id, service_account_id, credential_id, \
         provider, upstream_id, input_file_id, output_file_id, error_file_id, endpoint, \
         catalogue_model_id, model, upstream_model, prices, metadata, status, view, created_at, \
         accounted_at FROM batches WHERE accounted_at IS NULL ORDER BY created_at LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: Uuid,
    status: &str,
    view: &Value,
    output_file_id: Option<Uuid>,
    error_file_id: Option<Uuid>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE batches SET status = $2, view = $3, output_file_id = $4, error_file_id = $5 \
         WHERE id = $1",
        id,
        status,
        view,
        output_file_id,
        error_file_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Marks a batch's usage recorded; `false` when another gateway already did.
pub async fn mark_accounted(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE batches SET accounted_at = $2 WHERE id = $1 AND accounted_at IS NULL",
        id,
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}
