use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub org_id: Uuid,
    pub last4: String,
    pub previous_until: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub last_error_at: Option<OffsetDateTime>,
}

pub async fn token(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Option<Token>, StoreError> {
    Ok(sqlx::query_as!(
        Token,
        "SELECT org_id, last4, previous_until, created_at, last_used_at, last_error, last_error_at \
         FROM scim_tokens WHERE org_id = $1",
        org_id
    )
    .fetch_optional(db)
    .await?)
}

/// Sets the organization's token; a token it replaces stays valid until `previous_until`.
pub async fn set_token(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    hash: &[u8],
    last4: &str,
    previous_until: OffsetDateTime,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO scim_tokens (org_id, hash, last4, created_at) VALUES ($1, $2, $3, $5) \
         ON CONFLICT (org_id) DO UPDATE SET previous_hash = scim_tokens.hash, \
         previous_until = $4, hash = EXCLUDED.hash, last4 = EXCLUDED.last4, \
         created_at = EXCLUDED.created_at",
        org_id,
        hash,
        last4,
        previous_until,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_token(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<bool, StoreError> {
    Ok(
        sqlx::query!("DELETE FROM scim_tokens WHERE org_id = $1", org_id)
            .execute(db)
            .await?
            .rows_affected()
            == 1,
    )
}

/// The organization a token belongs to: the current one, or the one it replaced while its grace
/// period lasts.
pub async fn org_of(
    db: impl PgExecutor<'_>,
    hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT org_id FROM scim_tokens WHERE hash = $1 OR (previous_hash = $1 AND previous_until > $2)",
        hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn used(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    now: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE scim_tokens SET last_used_at = $2, \
         last_error = COALESCE($3, last_error), \
         last_error_at = CASE WHEN $3::text IS NULL THEN last_error_at ELSE $2 END \
         WHERE org_id = $1",
        org_id,
        now,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScimUser {
    pub user_id: Uuid,
    pub external_id: Option<String>,
    pub active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub async fn user(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ScimUser>, StoreError> {
    Ok(sqlx::query_as!(
        ScimUser,
        "SELECT user_id, external_id, active, created_at, updated_at FROM scim_users \
         WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn user_by_external(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    external_id: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id FROM scim_users WHERE org_id = $1 AND external_id = $2",
        org_id,
        external_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn put_user(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    external_id: Option<&str>,
    active: bool,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO scim_users (org_id, user_id, external_id, active, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $5) ON CONFLICT (org_id, user_id) DO UPDATE SET \
         external_id = COALESCE(EXCLUDED.external_id, scim_users.external_id), \
         active = EXCLUDED.active, updated_at = EXCLUDED.updated_at",
        org_id,
        user_id,
        external_id,
        active,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_user(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM scim_users WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The members SCIM provisioned and still active.
pub async fn provisioned(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT s.user_id FROM scim_users s JOIN org_memberships m \
         ON m.org_id = s.org_id AND m.user_id = s.user_id WHERE s.org_id = $1 AND s.active",
        org_id
    )
    .fetch_all(db)
    .await?)
}

/// A user SCIM may see: a member of the organization, or one it deprovisioned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub user_id: Uuid,
    pub email: Option<String>,
    pub name: String,
    pub member: bool,
    pub external_id: Option<String>,
    pub created_at: OffsetDateTime,
}

pub async fn users(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Listed>, StoreError> {
    Ok(sqlx::query_as!(
        Listed,
        r#"SELECT u.id AS "user_id!", u.email, u.name AS "name!", (m.user_id IS NOT NULL) AS "member!",
           s.external_id, u.created_at AS "created_at!"
           FROM users u
           LEFT JOIN org_memberships m ON m.org_id = $1 AND m.user_id = u.id
           LEFT JOIN scim_users s ON s.org_id = $1 AND s.user_id = u.id
           WHERE m.user_id IS NOT NULL OR s.user_id IS NOT NULL
           ORDER BY u.created_at, u.id"#,
        org_id
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: Uuid,
    pub org_id: Uuid,
    pub display_name: String,
    pub external_id: Option<String>,
    pub members: Vec<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub async fn groups(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Group>, StoreError> {
    Ok(sqlx::query_as!(
        Group,
        "SELECT id, org_id, display_name, external_id, members, created_at, updated_at \
         FROM scim_groups WHERE org_id = $1 ORDER BY created_at, id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn group(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Group>, StoreError> {
    Ok(sqlx::query_as!(
        Group,
        "SELECT id, org_id, display_name, external_id, members, created_at, updated_at \
         FROM scim_groups WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// Writes a group; `false` when another group of the organization has its name.
pub async fn put_group(db: impl PgExecutor<'_>, g: &Group) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO scim_groups (id, org_id, display_name, external_id, members, created_at, \
         updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (id) DO UPDATE SET \
         display_name = EXCLUDED.display_name, external_id = EXCLUDED.external_id, \
         members = EXCLUDED.members, updated_at = EXCLUDED.updated_at \
         WHERE NOT EXISTS (SELECT 1 FROM scim_groups o WHERE o.org_id = EXCLUDED.org_id \
             AND o.display_name = EXCLUDED.display_name AND o.id <> EXCLUDED.id)",
        g.id,
        g.org_id,
        g.display_name,
        g.external_id,
        &g.members,
        g.created_at,
        g.updated_at
    )
    .execute(db)
    .await
    .map(|r| r.rows_affected() == 1)
    .or_else(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => Ok(false),
        e => Err(e),
    })?)
}

pub async fn delete_group(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM scim_groups WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}
