use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub name: String,
    pub is_default: bool,
    pub key_id: String,
    pub last4: String,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub last_used_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedRow {
    pub id: Uuid,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
}

#[derive(Clone, Debug)]
pub struct NewCredential<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: &'a str,
    pub name: &'a str,
    pub ciphertext: &'a [u8],
    pub nonce: &'a [u8],
    pub wrapped_key: &'a [u8],
    pub key_id: &'a str,
    pub last4: &'a str,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignmentRow {
    pub credential_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub project_avatar: Option<String>,
}

pub async fn lock_provider(
    db: &mut PgConnection,
    org_id: Uuid,
    provider: &str,
) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('provider_credentials:' || $1::uuid::text || ':' || $2, 0))",
        org_id,
        provider
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn has_default(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM provider_credentials WHERE org_id = $1 AND provider = $2 AND is_default) \
         AS \"exists!\"",
        org_id,
        provider
    )
    .fetch_one(db)
    .await?)
}

pub async fn insert(db: impl PgExecutor<'_>, c: &NewCredential<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO provider_credentials (id, org_id, provider, name, ciphertext, nonce, wrapped_key, key_id, \
         last4, created_by_kind, created_by_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        c.id,
        c.org_id,
        c.provider,
        c.name,
        c.ciphertext,
        c.nonce,
        c.wrapped_key,
        c.key_id,
        c.last4,
        c.created_by_kind,
        c.created_by_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn replace(db: impl PgExecutor<'_>, c: &NewCredential<'_>) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET name = $3, ciphertext = $4, nonce = $5, wrapped_key = $6, key_id = $7, last4 = $8, \
         updated_at = now() WHERE org_id = $1 AND id = $2",
        c.org_id,
        c.id,
        c.name,
        c.ciphertext,
        c.nonce,
        c.wrapped_key,
        c.key_id,
        c.last4
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn rename(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    name: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET name = $3 WHERE org_id = $1 AND id = $2",
        org_id,
        id,
        name
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn set_default(
    db: &mut PgConnection,
    org_id: Uuid,
    provider: &str,
    id: Uuid,
) -> Result<bool, StoreError> {
    sqlx::query!(
        "UPDATE provider_credentials SET is_default = false \
         WHERE org_id = $1 AND provider = $2 AND is_default AND id <> $3",
        org_id,
        provider,
        id
    )
    .execute(&mut *db)
    .await?;
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET is_default = true WHERE org_id = $1 AND provider = $2 AND id = $3",
        org_id,
        provider,
        id
    )
    .execute(&mut *db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn promote_newest(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE provider_credentials SET is_default = true WHERE id = ( \
           SELECT id FROM provider_credentials WHERE org_id = $1 AND provider = $2 \
           AND NOT EXISTS (SELECT 1 FROM provider_credentials WHERE org_id = $1 AND provider = $2 AND is_default) \
           ORDER BY created_at DESC, id DESC LIMIT 1) \
         RETURNING id",
        org_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<CredentialRow>, StoreError> {
    Ok(sqlx::query_as!(
        CredentialRow,
        "SELECT id, org_id, provider, name, is_default, key_id, last4, created_by_kind, created_by_id, last_used_at, \
         created_at, updated_at FROM provider_credentials WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<CredentialRow>, StoreError> {
    Ok(sqlx::query_as!(
        CredentialRow,
        "SELECT id, org_id, provider, name, is_default, key_id, last4, created_by_kind, created_by_id, last_used_at, \
         created_at, updated_at FROM provider_credentials WHERE org_id = $1 \
         ORDER BY provider, is_default DESC, created_at DESC, id DESC",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM provider_credentials WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn assignments(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    credential: Option<Uuid>,
) -> Result<Vec<AssignmentRow>, StoreError> {
    Ok(sqlx::query_as!(
        AssignmentRow,
        "SELECT a.credential_id, a.project_id, p.name AS project_name, p.avatar AS project_avatar FROM project_provider_keys a \
         JOIN projects p ON p.id = a.project_id \
         WHERE a.org_id = $1 AND ($2::uuid IS NULL OR a.credential_id = $2) ORDER BY lower(p.name), p.id",
        org_id,
        credential
    )
    .fetch_all(db)
    .await?)
}

pub async fn active_projects(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM projects WHERE org_id = $1 AND id = ANY($2) AND archived_at IS NULL",
        org_id,
        ids
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_assignments(
    db: &mut PgConnection,
    org_id: Uuid,
    provider: &str,
    credential: Uuid,
    projects: &[Uuid],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_provider_keys WHERE org_id = $1 AND credential_id = $2 AND NOT (project_id = ANY($3))",
        org_id,
        credential,
        projects
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "INSERT INTO project_provider_keys (project_id, provider, org_id, credential_id) \
         SELECT unnest($4::uuid[]), $2, $1, $3 \
         ON CONFLICT (project_id, provider) DO UPDATE SET credential_id = EXCLUDED.credential_id, created_at = now() \
         WHERE project_provider_keys.credential_id <> EXCLUDED.credential_id",
        org_id,
        provider,
        credential,
        projects
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn resolve(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<Option<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT c.id, c.ciphertext, c.nonce, c.wrapped_key, c.key_id FROM provider_credentials c \
         LEFT JOIN project_provider_keys a ON a.credential_id = c.id AND a.project_id = $2 \
         WHERE c.org_id = $1 AND c.provider = $3 AND (a.project_id IS NOT NULL OR c.is_default) \
         ORDER BY a.project_id IS NULL LIMIT 1",
        org_id,
        project_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE provider_credentials SET last_used_at = $2 WHERE id = $1 \
         AND (last_used_at IS NULL OR last_used_at < $2::timestamptz - interval '1 minute')",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT id, ciphertext, nonce, wrapped_key, key_id FROM provider_credentials"
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_wrapped_key(
    db: impl PgExecutor<'_>,
    id: Uuid,
    old_wrapped_key: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET wrapped_key = $3, key_id = $4 WHERE id = $1 AND wrapped_key = $2",
        id,
        old_wrapped_key,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn key_id_usage(db: impl PgExecutor<'_>) -> Result<Vec<(String, i64)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT key_id AS \"key_id!\", count(*) AS \"n!\" FROM ( \
           SELECT key_id FROM provider_credentials UNION ALL SELECT key_id FROM user_totp \
           UNION ALL SELECT key_id FROM org_oidc) k GROUP BY key_id ORDER BY key_id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.key_id, r.n)).collect())
}
