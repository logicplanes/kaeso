use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub content_logging: bool,
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
    is_default: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO projects (id, org_id, name, is_default) VALUES ($1, $2, $3, $4)",
        id,
        org_id,
        name,
        is_default
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
        "SELECT id, org_id, name, is_default, content_logging, max_key_lifetime_days, archived_at, created_at, avatar \
         FROM projects WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Project>, StoreError> {
    Ok(sqlx::query_as!(
        Project,
        "SELECT id, org_id, name, is_default, content_logging, max_key_lifetime_days, archived_at, created_at, avatar \
         FROM projects WHERE org_id = $1 AND id = $2 FOR NO KEY UPDATE",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn default_of(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Option<Project>, StoreError> {
    Ok(sqlx::query_as!(
        Project,
        "SELECT id, org_id, name, is_default, content_logging, max_key_lifetime_days, archived_at, created_at, avatar \
         FROM projects WHERE org_id = $1 AND is_default",
        org_id
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

pub async fn lock_default(db: &mut PgConnection, org_id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('projects_default:' || $1::uuid::text, 0))",
        org_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_default(
    db: &mut PgConnection,
    org_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    let active = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM projects WHERE org_id = $1 AND id = $2 AND archived_at IS NULL) AS \"exists!\"",
        org_id,
        id
    )
    .fetch_one(&mut *db)
    .await?;
    if !active {
        return Ok(false);
    }
    sqlx::query!(
        "UPDATE projects SET is_default = false, updated_at = now() WHERE org_id = $1 AND is_default AND id <> $2",
        org_id,
        id
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "UPDATE projects SET is_default = true, updated_at = now() WHERE org_id = $1 AND id = $2 AND NOT is_default",
        org_id,
        id
    )
    .execute(&mut *db)
    .await?;
    Ok(true)
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
        "SELECT p.id, p.org_id, p.name, p.is_default, p.content_logging, p.max_key_lifetime_days, p.archived_at, p.created_at, p.avatar \
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
