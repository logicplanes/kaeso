use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub id: Uuid,
    pub org_id: Uuid,
    pub scope: String,
    pub name: String,
    pub description: String,
    pub rights: Vec<String>,
    pub project_rights: Vec<String>,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub holders: i64,
}

pub async fn list(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Role>, StoreError> {
    Ok(sqlx::query_as!(
        Role,
        "SELECT r.id, r.org_id, r.scope, r.name, r.description, r.rights, r.project_rights, r.created_by, \
         r.created_at, r.updated_at, \
         ((SELECT count(*) FROM org_memberships m WHERE m.role_id = r.id) \
          + (SELECT count(*) FROM project_memberships m WHERE m.role_id = r.id) \
          + (SELECT count(*) FROM service_accounts s WHERE s.role_id = r.id AND s.archived_at IS NULL) \
          + (SELECT count(*) FROM invites i WHERE i.org_role_id = r.id AND i.accepted_at IS NULL \
             AND i.revoked_at IS NULL AND i.expires_at > now())) AS \"holders!\" \
         FROM org_roles r WHERE r.org_id = $1 ORDER BY r.scope, lower(r.name)",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Role>, StoreError> {
    Ok(sqlx::query_as!(
        Role,
        "SELECT r.id, r.org_id, r.scope, r.name, r.description, r.rights, r.project_rights, r.created_by, \
         r.created_at, r.updated_at, \
         ((SELECT count(*) FROM org_memberships m WHERE m.role_id = r.id) \
          + (SELECT count(*) FROM project_memberships m WHERE m.role_id = r.id) \
          + (SELECT count(*) FROM service_accounts s WHERE s.role_id = r.id AND s.archived_at IS NULL) \
          + (SELECT count(*) FROM invites i WHERE i.org_role_id = r.id AND i.accepted_at IS NULL \
             AND i.revoked_at IS NULL AND i.expires_at > now())) AS \"holders!\" \
         FROM org_roles r WHERE r.org_id = $1 AND r.id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct RoleWrite<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub scope: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub rights: &'a [String],
    pub project_rights: &'a [String],
    pub created_by: Option<Uuid>,
}

pub fn is_name_taken(e: &StoreError) -> bool {
    matches!(e, StoreError::Postgres(sqlx::Error::Database(d)) if d.constraint() == Some("org_roles_name"))
}

pub async fn insert(db: impl PgExecutor<'_>, r: &RoleWrite<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO org_roles (id, org_id, scope, name, description, rights, project_rights, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        r.id,
        r.org_id,
        r.scope,
        r.name,
        r.description,
        r.rights,
        r.project_rights,
        r.created_by
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl PgExecutor<'_>, r: &RoleWrite<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE org_roles SET name = $3, description = $4, rights = $5, project_rights = $6, updated_at = now() \
         WHERE org_id = $1 AND id = $2",
        r.org_id,
        r.id,
        r.name,
        r.description,
        r.rights,
        r.project_rights
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn lock(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT 1 AS \"one!\" FROM org_roles WHERE org_id = $1 AND id = $2 FOR UPDATE",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?
    .is_some())
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM org_roles WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn count(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM org_roles WHERE org_id = $1",
        org_id
    )
    .fetch_one(db)
    .await?)
}

pub async fn pending_project_invites(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    role: &str,
) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM invites WHERE org_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL \
         AND expires_at > now() AND project_roles @> jsonb_build_array(jsonb_build_object('role_id', $2::text))",
        org_id,
        role
    )
    .fetch_one(db)
    .await?)
}

pub async fn builtin_holders(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(String, String, i64)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT 'org' AS \"scope!\", preset AS \"preset!\", count(*) AS \"n!\" FROM org_memberships \
         WHERE org_id = $1 AND preset <> 'custom' GROUP BY preset \
         UNION ALL \
         SELECT 'project', x.preset, count(*) FROM ( \
           SELECT m.preset FROM project_memberships m JOIN projects p ON p.id = m.project_id \
           WHERE p.org_id = $1 AND m.preset <> 'custom' \
           UNION ALL \
           SELECT s.preset FROM service_accounts s JOIN projects p ON p.id = s.project_id \
           WHERE p.org_id = $1 AND s.archived_at IS NULL AND s.preset <> 'custom') x GROUP BY x.preset",
        org_id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.scope, r.preset, r.n))
    .collect())
}
