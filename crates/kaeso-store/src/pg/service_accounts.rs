use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use super::members::{Grant, impl_grant};
use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceAccount {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub preset: String,
    pub role_id: Option<Uuid>,
    pub rights_added: Vec<String>,
    pub rights_removed: Vec<String>,
    pub role_name: Option<String>,
    pub role_rights: Option<Vec<String>>,
    pub role_project_rights: Option<Vec<String>>,
    pub created_by: Option<Uuid>,
    pub archived_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

impl_grant!(ServiceAccount);

impl ServiceAccount {
    pub fn set_grant(&mut self, g: Grant) {
        self.preset = g.preset;
        self.role_id = g.role_id;
        self.rights_added = g.rights_added;
        self.rights_removed = g.rights_removed;
        self.role_name = g.role_name;
        self.role_rights = g.role_rights;
        self.role_project_rights = g.role_project_rights;
    }
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    project_id: Uuid,
    name: &str,
    g: &Grant,
    created_by: Option<Uuid>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO service_accounts (id, project_id, name, preset, role_id, rights_added, rights_removed, created_by) \
         VALUES ($1, $2, $3, $4, $8, $5, $6, $7)",
        id,
        project_id,
        name,
        g.preset,
        &g.rights_added,
        &g.rights_removed,
        created_by,
        g.role_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<ServiceAccount>, StoreError> {
    Ok(sqlx::query_as!(
        ServiceAccount,
        "SELECT s.id, s.project_id, s.name, s.preset, s.role_id, s.rights_added, s.rights_removed, r.name AS \"role_name?\", \
         r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", s.created_by, s.archived_at, s.created_at \
         FROM service_accounts s LEFT JOIN org_roles r ON r.id = s.role_id WHERE s.project_id = $1 AND s.id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<ServiceAccount>, StoreError> {
    Ok(sqlx::query_as!(
        ServiceAccount,
        "SELECT s.id, s.project_id, s.name, s.preset, s.role_id, s.rights_added, s.rights_removed, r.name AS \"role_name?\", \
         r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", s.created_by, s.archived_at, s.created_at \
         FROM service_accounts s LEFT JOIN org_roles r ON r.id = s.role_id WHERE s.project_id = $1 AND s.id = $2 \
         FOR NO KEY UPDATE OF s",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    include_archived: bool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ServiceAccount>, StoreError> {
    Ok(sqlx::query_as!(
        ServiceAccount,
        "SELECT s.id, s.project_id, s.name, s.preset, s.role_id, s.rights_added, s.rights_removed, r.name AS \"role_name?\", \
         r.rights AS \"role_rights?\", r.project_rights AS \"role_project_rights?\", s.created_by, s.archived_at, s.created_at \
         FROM service_accounts s LEFT JOIN org_roles r ON r.id = s.role_id \
         WHERE s.project_id = $1 AND ($2 OR s.archived_at IS NULL) AND ($3::uuid IS NULL OR s.id > $3) \
         ORDER BY s.id LIMIT $4",
        project_id,
        include_archived,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn update(db: impl PgExecutor<'_>, s: &ServiceAccount) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE service_accounts SET name = $3, preset = $4, rights_added = $5, rights_removed = $6, \
         archived_at = $7, role_id = $8, updated_at = now() WHERE project_id = $1 AND id = $2",
        s.project_id,
        s.id,
        s.name,
        s.preset,
        &s.rights_added,
        &s.rights_removed,
        s.archived_at,
        s.role_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn archive_all_in_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    at: OffsetDateTime,
) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "UPDATE service_accounts SET archived_at = $2, updated_at = now() WHERE project_id = $1 AND archived_at IS NULL",
        project_id,
        at
    )
    .execute(db)
    .await?
    .rows_affected())
}
