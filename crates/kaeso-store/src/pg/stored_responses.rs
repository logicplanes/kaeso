use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

/// Whether a project may keep responses at its providers: the organization allows it and the
/// project turned it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub org_allows: bool,
    pub enabled: bool,
}

impl Settings {
    pub fn is_on(&self) -> bool {
        self.org_allows && self.enabled
    }
}

/// A response a provider keeps for one of the organization's projects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredResponse {
    pub id_hash: Vec<u8>,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub credential_id: Uuid,
    pub provider: String,
    pub model: String,
    pub catalogue_model_id: Uuid,
    pub request_id: Uuid,
    pub total_tokens: i64,
    pub expires_at: OffsetDateTime,
}

pub async fn settings(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Option<Settings>, StoreError> {
    Ok(sqlx::query_as!(
        Settings,
        "SELECT o.stored_responses AS org_allows, p.stored_responses AS enabled \
         FROM projects p JOIN organizations o ON o.id = p.org_id WHERE p.id = $1",
        project_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn org_allows(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT stored_responses FROM organizations WHERE id = $1",
        org_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

pub async fn set_org(db: impl PgExecutor<'_>, org_id: Uuid, on: bool) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE organizations SET stored_responses = $2 WHERE id = $1",
        org_id,
        on
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn project_for_update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
) -> Result<Option<bool>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT stored_responses FROM projects WHERE org_id = $1 AND id = $2 FOR UPDATE",
        org_id,
        project_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn set_project(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    on: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE projects SET stored_responses = $3 WHERE org_id = $1 AND id = $2",
        org_id,
        project_id,
        on
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Records a stored response, and forgets the project's responses that expired.
pub async fn insert(db: &sqlx::PgPool, r: &StoredResponse) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO stored_responses (id_hash, org_id, project_id, credential_id, provider, model, \
         catalogue_model_id, request_id, total_tokens, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id_hash) DO NOTHING",
        r.id_hash,
        r.org_id,
        r.project_id,
        r.credential_id,
        r.provider,
        r.model,
        r.catalogue_model_id,
        r.request_id,
        r.total_tokens,
        r.expires_at
    )
    .execute(db)
    .await?;
    sqlx::query!(
        "DELETE FROM stored_responses WHERE project_id = $1 AND expires_at < now()",
        r.project_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The project's stored response with this id, while the provider keeps it.
pub async fn find(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    id_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<StoredResponse>, StoreError> {
    Ok(sqlx::query_as!(
        StoredResponse,
        "SELECT id_hash, org_id, project_id, credential_id, provider, model, catalogue_model_id, \
         request_id, total_tokens, expires_at FROM stored_responses \
         WHERE org_id = $1 AND project_id = $2 AND id_hash = $3 AND expires_at > $4",
        org_id,
        project_id,
        id_hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn delete(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id_hash: &[u8],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM stored_responses WHERE project_id = $1 AND id_hash = $2",
        project_id,
        id_hash
    )
    .execute(db)
    .await?;
    Ok(())
}
