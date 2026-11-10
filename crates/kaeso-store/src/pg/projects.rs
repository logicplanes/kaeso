use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub content_logging: bool,
    pub keep_media: bool,
    pub max_key_lifetime_days: Option<i32>,
    pub archived_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub avatar: Option<String>,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    org_id: Uuid,
    name: &str,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO projects (id, org_id, name) VALUES ($1, $2, $3)",
        id,
        org_id,
        name
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Project>, StoreError> {
    Ok(sqlx::query_as!(
        Project,
        "SELECT id, org_id, name, content_logging, keep_media, max_key_lifetime_days, archived_at, created_at, avatar \
         FROM projects WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn hold_active(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT 1 AS \"one!\" FROM projects WHERE org_id = $1 AND id = $2 AND archived_at IS NULL FOR SHARE",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?
    .is_some())
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Project>, StoreError> {
    Ok(sqlx::query_as!(
        Project,
        "SELECT id, org_id, name, content_logging, keep_media, max_key_lifetime_days, archived_at, created_at, avatar \
         FROM projects WHERE org_id = $1 AND id = $2 FOR NO KEY UPDATE",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn ids_of_org(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM projects WHERE org_id = $1", org_id)
            .fetch_all(db)
            .await?,
    )
}

pub async fn set_keep_media(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    keep: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE projects SET keep_media = $3, updated_at = now() WHERE org_id = $1 AND id = $2",
        org_id,
        id,
        keep
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl PgExecutor<'_>, p: &Project) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE projects SET name = $3, content_logging = $4, max_key_lifetime_days = $5, archived_at = $6, \
         avatar = $7, updated_at = now() WHERE org_id = $1 AND id = $2",
        p.org_id,
        p.id,
        p.name,
        p.content_logging,
        p.max_key_lifetime_days,
        p.archived_at,
        p.avatar
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    member: Option<Uuid>,
    only: Option<&[Uuid]>,
    include_archived: bool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Project>, StoreError> {
    Ok(sqlx::query_as!(
        Project,
        "SELECT p.id, p.org_id, p.name, p.content_logging, p.keep_media, p.max_key_lifetime_days, p.archived_at, p.created_at, p.avatar \
         FROM projects p WHERE p.org_id = $1 \
         AND ($2::uuid IS NULL OR EXISTS (SELECT 1 FROM project_memberships m WHERE m.project_id = p.id AND m.user_id = $2)) \
         AND ($6::uuid[] IS NULL OR p.id = ANY($6)) \
         AND ($3 OR p.archived_at IS NULL) AND ($4::uuid IS NULL OR p.id > $4) \
         ORDER BY p.id LIMIT $5",
        org_id,
        member,
        include_archived,
        after,
        limit,
        only
    )
    .fetch_all(db)
    .await?)
}
