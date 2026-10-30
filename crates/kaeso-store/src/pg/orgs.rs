use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Org {
    pub id: Uuid,
    pub name: String,
    pub state: String,
    pub deletion_due_at: Option<OffsetDateTime>,
    pub content_logging: bool,
    pub metadata_retention_days: i32,
    pub content_retention_days: i32,
    pub require_2fa: bool,
    pub require_sso: bool,
    pub max_key_lifetime_days: Option<i32>,
    pub sso_default_preset: Option<String>,
    pub sso_default_role_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub avatar: Option<String>,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    name: &str,
    metadata_retention_days: i32,
    content_retention_days: i32,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO organizations (id, name, metadata_retention_days, content_retention_days) VALUES ($1, $2, $3, $4)",
        id,
        name,
        metadata_retention_days,
        content_retention_days
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<Org>, StoreError> {
    Ok(sqlx::query_as!(
        Org,
        "SELECT id, name, state, deletion_due_at, content_logging, metadata_retention_days, \
         content_retention_days, require_2fa, require_sso, max_key_lifetime_days, sso_default_preset, sso_default_role_id, created_at, avatar \
         FROM organizations WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<Org>, StoreError> {
    Ok(sqlx::query_as!(
        Org,
        "SELECT id, name, state, deletion_due_at, content_logging, metadata_retention_days, \
         content_retention_days, require_2fa, require_sso, max_key_lifetime_days, sso_default_preset, sso_default_role_id, created_at, avatar \
         FROM organizations WHERE id = $1 FOR NO KEY UPDATE",
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn update(db: impl PgExecutor<'_>, org: &Org) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE organizations SET name = $2, content_logging = $3, metadata_retention_days = $4, \
         content_retention_days = $5, require_2fa = $6, require_sso = $7, max_key_lifetime_days = $8, \
         sso_default_preset = $9, state = $10, deletion_due_at = $11, avatar = $12, sso_default_role_id = $13, \
         updated_at = now() WHERE id = $1",
        org.id,
        org.name,
        org.content_logging,
        org.metadata_retention_days,
        org.content_retention_days,
        org.require_2fa,
        org.require_sso,
        org.max_key_lifetime_days,
        org.sso_default_preset,
        org.state,
        org.deletion_due_at,
        org.avatar,
        org.sso_default_role_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrgOrder {
    #[default]
    Created,
    Deletion,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
    search: Option<&str>,
    order: OrgOrder,
    sso: Option<bool>,
) -> Result<Vec<Org>, StoreError> {
    // PostgreSQL text can't hold NUL, so no row matches one, and the driver refuses it.
    if search.is_some_and(|s| s.contains('\0')) {
        return Ok(Vec::new());
    }
    let pattern = search.map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    Ok(match order {
        OrgOrder::Created => {
            sqlx::query_as!(
                Org,
                "SELECT id, name, state, deletion_due_at, content_logging, metadata_retention_days, \
                 content_retention_days, require_2fa, require_sso, max_key_lifetime_days, sso_default_preset, sso_default_role_id, created_at, avatar \
                 FROM organizations o WHERE ($1::uuid IS NULL OR id > $1) AND ($3::text IS NULL OR name ILIKE $3) \
                 AND ($4::bool IS NULL OR EXISTS (SELECT 1 FROM org_oidc x WHERE x.org_id = o.id) = $4) \
                 ORDER BY id LIMIT $2",
                after,
                limit,
                pattern,
                sso
            )
            .fetch_all(db)
            .await?
        }
        OrgOrder::Deletion => {
            sqlx::query_as!(
                Org,
                "SELECT id, name, state, deletion_due_at, content_logging, metadata_retention_days, \
                 content_retention_days, require_2fa, require_sso, max_key_lifetime_days, sso_default_preset, sso_default_role_id, created_at, avatar \
                 FROM organizations o WHERE ($1::uuid IS NULL OR (COALESCE(o.deletion_due_at, 'infinity'), o.id) > \
                 (SELECT COALESCE(c.deletion_due_at, 'infinity'), c.id FROM organizations c WHERE c.id = $1)) \
                 AND ($3::text IS NULL OR name ILIKE $3) \
                 AND ($4::bool IS NULL OR EXISTS (SELECT 1 FROM org_oidc x WHERE x.org_id = o.id) = $4) \
                 ORDER BY COALESCE(o.deletion_due_at, 'infinity'), o.id LIMIT $2",
                after,
                limit,
                pattern,
                sso
            )
            .fetch_all(db)
            .await?
        }
    })
}

pub async fn due_for_deletion(
    db: impl PgExecutor<'_>,
    now: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM organizations WHERE state = 'scheduled_for_deletion' AND deletion_due_at <= $1",
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn delete_final(tx: &mut sqlx::PgConnection, org_id: Uuid) -> Result<(), StoreError> {
    let pictures = sqlx::query_scalar!(
        "SELECT avatar AS \"avatar!\" FROM organizations WHERE id = $1 AND avatar IS NOT NULL \
         UNION SELECT avatar FROM projects WHERE org_id = $1 AND avatar IS NOT NULL",
        org_id
    )
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query("SET LOCAL kaeso.audit_purge = 'on'")
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM audit_events WHERE org_id = $1", org_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM sessions WHERE oidc_org_id = $1", org_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM invites WHERE org_id = $1", org_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM organizations WHERE id = $1", org_id)
        .execute(&mut *tx)
        .await?;
    for hash in &pictures {
        super::avatars::delete_if_unused(&mut *tx, hash).await?;
    }
    sqlx::query("SET LOCAL kaeso.audit_purge = 'off'")
        .execute(&mut *tx)
        .await?;
    Ok(())
}
