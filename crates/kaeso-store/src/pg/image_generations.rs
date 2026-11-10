use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

/// An image generation an app started through Kaeso: whose it is, where it runs, and what its
/// provider last said about it.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageGenerationRow {
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
    pub content_url: Option<String>,
    pub content_mime_type: Option<String>,
    pub failure_reason: Option<String>,
    pub error_message: Option<String>,
    pub units: Option<Value>,
    pub cost: Option<Decimal>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub accounted_at: Option<OffsetDateTime>,
}

/// What the provider last said about a generation.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageGenerationState {
    pub status: String,
    pub content_url: Option<String>,
    pub content_mime_type: Option<String>,
    pub failure_reason: Option<String>,
    pub error_message: Option<String>,
    pub completed_at: Option<OffsetDateTime>,
}

/// Which of the project's generations a listing shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageGenerationFilter {
    /// Only generations older than this one, which must be the project's.
    pub before: Option<Uuid>,
    pub status: Option<String>,
    pub model: Option<String>,
}

pub async fn insert(db: impl PgExecutor<'_>, g: &ImageGenerationRow) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO image_generations (id, org_id, project_id, api_key_id, user_id, \
         service_account_id, provider, credential_id, catalogue_model_id, model, upstream_model, \
         dialect, upstream_id, request_id, params, prices, status, content_url, \
         content_mime_type, failure_reason, error_message, units, cost, created_at, \
         completed_at, accounted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, \
         $19, $20, $21, $22, $23, $24, $25, $26)",
        g.id,
        g.org_id,
        g.project_id,
        g.api_key_id,
        g.user_id,
        g.service_account_id,
        g.provider,
        g.credential_id,
        g.catalogue_model_id,
        g.model,
        g.upstream_model,
        g.dialect,
        g.upstream_id,
        g.request_id,
        g.params,
        g.prices,
        g.status,
        g.content_url,
        g.content_mime_type,
        g.failure_reason,
        g.error_message,
        g.units,
        g.cost,
        g.created_at,
        g.completed_at,
        g.accounted_at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The project's generation `id`, or `None` when the project has none by that id.
pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<ImageGenerationRow>, StoreError> {
    Ok(sqlx::query_as!(
        ImageGenerationRow,
        "SELECT id, org_id, project_id, api_key_id, user_id, service_account_id, provider, \
         credential_id, catalogue_model_id, model, upstream_model, dialect, upstream_id, \
         request_id, params, prices, status, content_url, content_mime_type, failure_reason, \
         error_message, units, cost, created_at, completed_at, accounted_at \
         FROM image_generations WHERE org_id = $1 AND project_id = $2 AND id = $3",
        org_id,
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// A page of the project's generations, newest first.
pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    filter: &ImageGenerationFilter,
    limit: i64,
) -> Result<Vec<ImageGenerationRow>, StoreError> {
    Ok(sqlx::query_as!(
        ImageGenerationRow,
        "SELECT g.id, g.org_id, g.project_id, g.api_key_id, g.user_id, g.service_account_id, \
         g.provider, g.credential_id, g.catalogue_model_id, g.model, g.upstream_model, \
         g.dialect, g.upstream_id, g.request_id, g.params, g.prices, g.status, g.content_url, \
         g.content_mime_type, g.failure_reason, g.error_message, g.units, g.cost, g.created_at, \
         g.completed_at, g.accounted_at \
         FROM image_generations g \
         LEFT JOIN image_generations b ON b.org_id = $1 AND b.project_id = $2 AND b.id = $3 \
         WHERE g.org_id = $1 AND g.project_id = $2 \
           AND ($3::uuid IS NULL OR (g.created_at, g.id) < (b.created_at, b.id)) \
           AND ($4::text IS NULL OR g.status = $4) \
           AND ($5::text IS NULL OR g.model = $5) \
         ORDER BY g.created_at DESC, g.id DESC \
         LIMIT $6",
        org_id,
        project_id,
        filter.before,
        filter.status,
        filter.model,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// Writes what the provider last said. Once a generation was accounted for, only news of the
/// same status is written, such as a fresh link to a completed image.
pub async fn update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    s: &ImageGenerationState,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE image_generations SET status = $3, content_url = $4, content_mime_type = $5, \
         failure_reason = $6, error_message = $7, completed_at = COALESCE(completed_at, $8) \
         WHERE org_id = $1 AND id = $2 AND (accounted_at IS NULL OR status = $3)",
        org_id,
        id,
        s.status,
        s.content_url,
        s.content_mime_type,
        s.failure_reason,
        s.error_message,
        s.completed_at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Marks a finished generation accounted for with its units and cost; returns `true` only to
/// the one caller that did, so a generation is billed once however many requests see it end.
pub async fn account(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    units: Option<&Value>,
    cost: Option<Decimal>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE image_generations SET units = $3, cost = $4, accounted_at = $5 \
         WHERE org_id = $1 AND id = $2 AND accounted_at IS NULL \
           AND status IN ('completed', 'failed') RETURNING id",
        org_id,
        id,
        units,
        cost,
        at
    )
    .fetch_optional(db)
    .await?
    .is_some())
}
