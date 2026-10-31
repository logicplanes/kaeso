use sqlx::PgExecutor;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheSettings {
    pub org_allows: bool,
    pub enabled: bool,
    pub ttl_seconds: i32,
    pub generation: i64,
}

impl CacheSettings {
    pub fn is_on(&self) -> bool {
        self.org_allows && self.enabled
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectCache {
    pub enabled: bool,
    pub ttl_seconds: i32,
    pub generation: i64,
}

pub async fn settings(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Option<CacheSettings>, StoreError> {
    Ok(sqlx::query_as!(
        CacheSettings,
        "SELECT o.response_cache AS org_allows, p.response_cache AS enabled, \
         p.response_cache_ttl_seconds AS ttl_seconds, p.response_cache_generation AS generation \
         FROM projects p JOIN organizations o ON o.id = p.org_id WHERE p.id = $1",
        project_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn project_for_update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
) -> Result<Option<ProjectCache>, StoreError> {
    Ok(sqlx::query_as!(
        ProjectCache,
        "SELECT response_cache AS enabled, response_cache_ttl_seconds AS ttl_seconds, \
         response_cache_generation AS generation \
         FROM projects WHERE org_id = $1 AND id = $2 FOR UPDATE",
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
    next: &ProjectCache,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE projects SET response_cache = $3, response_cache_ttl_seconds = $4, \
         response_cache_generation = $5 WHERE org_id = $1 AND id = $2",
        org_id,
        project_id,
        next.enabled,
        next.ttl_seconds,
        next.generation
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn org_allows(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT response_cache FROM organizations WHERE id = $1",
        org_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

pub async fn set_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    allows: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE organizations SET response_cache = $2 WHERE id = $1",
        org_id,
        allows
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn clear_org(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE projects SET response_cache_generation = response_cache_generation + 1 \
         WHERE org_id = $1 RETURNING id",
        org_id
    )
    .fetch_all(db)
    .await?)
}
