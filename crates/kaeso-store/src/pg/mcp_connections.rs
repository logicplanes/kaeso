use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::credentials::Sealed;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRow {
    pub id: Uuid,
    pub server_id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub issuer: Option<String>,
    pub header_name: Option<String>,
    pub last4: String,
    pub expires_at: Option<OffsetDateTime>,
    pub account_label: Option<String>,
    pub auth_error: Option<String>,
    pub last_used_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedConnection {
    pub id: Uuid,
    pub server_id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub issuer: Option<String>,
    pub header_name: Option<String>,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub expires_at: Option<OffsetDateTime>,
    pub auth_error: Option<String>,
    pub revision: i64,
}

#[derive(Clone, Debug)]
pub struct NewConnection<'a> {
    pub id: Uuid,
    pub server_id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: &'a str,
    pub issuer: Option<&'a str>,
    pub header_name: Option<&'a str>,
    pub sealed: Sealed<'a>,
    pub last4: &'a str,
    pub expires_at: Option<OffsetDateTime>,
    pub account_label: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonConnection {
    pub connection: ConnectionRow,
    pub server_name: String,
    pub server_handle: String,
}

// Each server has at most one shared connection and one per person; putting a new one replaces
// the old in place, so a refresh in flight for the old secret can't overwrite the new one.
pub async fn put(db: &mut PgConnection, c: &NewConnection<'_>) -> Result<Uuid, StoreError> {
    let replaced = sqlx::query_scalar!(
        "UPDATE mcp_connections SET kind = $3, issuer = $4, header_name = $5, ciphertext = $6, nonce = $7, \
         wrapped_key = $8, key_id = $9, last4 = $10, expires_at = $11, account_label = $12, auth_error = NULL, \
         revision = revision + 1, updated_at = now() \
         WHERE server_id = $1 AND user_id IS NOT DISTINCT FROM $2 RETURNING id",
        c.server_id,
        c.user_id,
        c.kind,
        c.issuer,
        c.header_name,
        c.sealed.ciphertext,
        c.sealed.nonce,
        c.sealed.wrapped_key,
        c.sealed.key_id,
        c.last4,
        c.expires_at,
        c.account_label
    )
    .fetch_optional(&mut *db)
    .await?;
    if let Some(id) = replaced {
        return Ok(id);
    }
    sqlx::query!(
        "INSERT INTO mcp_connections (id, server_id, org_id, user_id, kind, issuer, header_name, ciphertext, \
         nonce, wrapped_key, key_id, last4, expires_at, account_label) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        c.id,
        c.server_id,
        c.org_id,
        c.user_id,
        c.kind,
        c.issuer,
        c.header_name,
        c.sealed.ciphertext,
        c.sealed.nonce,
        c.sealed.wrapped_key,
        c.sealed.key_id,
        c.last4,
        c.expires_at,
        c.account_label
    )
    .execute(&mut *db)
    .await?;
    Ok(c.id)
}

pub async fn lock_refresh(db: &mut PgConnection, id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('mcp_connection_refresh:' || $1::uuid::text, 0))",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn sealed(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<Option<SealedConnection>, StoreError> {
    Ok(sqlx::query_as!(
        SealedConnection,
        "SELECT id, server_id, user_id, kind, issuer, header_name, ciphertext, nonce, wrapped_key, key_id, \
         expires_at, auth_error, revision FROM mcp_connections \
         WHERE server_id = $1 AND user_id IS NOT DISTINCT FROM $2",
        server_id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn of_server(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
) -> Result<Vec<ConnectionRow>, StoreError> {
    Ok(sqlx::query_as!(
        ConnectionRow,
        "SELECT id, server_id, org_id, user_id, kind, issuer, header_name, last4, expires_at, account_label, \
         auth_error, last_used_at, created_at, updated_at, revision FROM mcp_connections \
         WHERE server_id = $1 ORDER BY user_id NULLS FIRST, created_at",
        server_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn shared_of_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<ConnectionRow>, StoreError> {
    Ok(sqlx::query_as!(
        ConnectionRow,
        "SELECT id, server_id, org_id, user_id, kind, issuer, header_name, last4, expires_at, account_label, \
         auth_error, last_used_at, created_at, updated_at, revision FROM mcp_connections \
         WHERE org_id = $1 AND user_id IS NULL ORDER BY server_id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<PersonConnection>, StoreError> {
    let rows = sqlx::query!(
        "SELECT c.id, c.server_id, c.org_id, c.user_id, c.kind, c.issuer, c.header_name, c.last4, c.expires_at, \
         c.account_label, c.auth_error, c.last_used_at, c.created_at, c.updated_at, c.revision, \
         s.name AS server_name, s.handle AS server_handle \
         FROM mcp_connections c JOIN mcp_servers s ON s.id = c.server_id \
         WHERE c.user_id = $1 ORDER BY s.name, s.handle",
        user_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| PersonConnection {
            connection: ConnectionRow {
                id: r.id,
                server_id: r.server_id,
                org_id: r.org_id,
                user_id: r.user_id,
                kind: r.kind,
                issuer: r.issuer,
                header_name: r.header_name,
                last4: r.last4,
                expires_at: r.expires_at,
                account_label: r.account_label,
                auth_error: r.auth_error,
                last_used_at: r.last_used_at,
                created_at: r.created_at,
                updated_at: r.updated_at,
                revision: r.revision,
            },
            server_name: r.server_name,
            server_handle: r.server_handle,
        })
        .collect())
}

pub async fn refreshed(
    db: impl PgExecutor<'_>,
    id: Uuid,
    revision: i64,
    sealed: &Sealed<'_>,
    expires_at: Option<OffsetDateTime>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_connections SET ciphertext = $3, nonce = $4, wrapped_key = $5, key_id = $6, expires_at = $7, \
         revision = revision + 1, updated_at = now() \
         WHERE id = $1 AND revision = $2 AND kind = 'oauth' AND auth_error IS NULL",
        id,
        revision,
        sealed.ciphertext,
        sealed.nonce,
        sealed.wrapped_key,
        sealed.key_id,
        expires_at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

// True only for the call that turns a working connection into a signed-out one, so the people
// responsible hear about it once rather than once per failed request.
pub async fn sign_out(db: impl PgExecutor<'_>, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_connections SET auth_error = 'signed_out', updated_at = now() \
         WHERE id = $1 AND auth_error IS NULL",
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE mcp_connections SET last_used_at = $2 \
         WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $2::timestamptz - interval '1 minute')",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM mcp_connections WHERE server_id = $1 AND user_id IS NOT DISTINCT FROM $2",
        server_id,
        user_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<SealedConnection>, StoreError> {
    Ok(sqlx::query_as!(
        SealedConnection,
        "SELECT id, server_id, user_id, kind, issuer, header_name, ciphertext, nonce, wrapped_key, key_id, \
         expires_at, auth_error, revision FROM mcp_connections"
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
        "UPDATE mcp_connections SET wrapped_key = $3, key_id = $4 WHERE id = $1 AND wrapped_key = $2",
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

// A per-person server lists the same tools to everyone, so its sync signs in as whoever used it
// last with a working connection.
pub async fn person_for_sync(
    db: impl PgExecutor<'_>,
    server_id: Uuid,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id AS \"user_id!\" FROM mcp_connections \
         WHERE server_id = $1 AND user_id IS NOT NULL AND auth_error IS NULL \
         ORDER BY last_used_at DESC NULLS LAST, updated_at DESC LIMIT 1",
        server_id
    )
    .fetch_optional(db)
    .await?)
}
