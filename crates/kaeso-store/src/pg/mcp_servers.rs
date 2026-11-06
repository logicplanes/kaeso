use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::credentials::Sealed;

#[derive(Clone, Debug, PartialEq)]
pub struct ServerRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub handle: String,
    pub name: String,
    pub url: String,
    pub auth_method: String,
    pub credential_mode: String,
    pub oauth_registration: Option<String>,
    pub oauth_issuer: Option<String>,
    pub oauth_client_id: Option<String>,
    pub has_client_secret: bool,
    pub scopes: Vec<String>,
    pub change_policy: String,
    pub protocol_version: Option<String>,
    pub server_info: Option<Value>,
    pub status: String,
    pub last_checked_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub revision: i64,
}

#[derive(Clone, Debug)]
pub struct NewServer<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub handle: &'a str,
    pub name: &'a str,
    pub url: &'a str,
    pub auth_method: &'a str,
    pub credential_mode: &'a str,
    pub oauth_registration: Option<&'a str>,
    pub scopes: &'a [String],
    pub change_policy: &'a str,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
}

#[derive(Clone, Debug, Default)]
pub struct Changes<'a> {
    pub name: Option<&'a str>,
    pub handle: Option<&'a str>,
    pub change_policy: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Updated {
    Done(Box<ServerRow>),
    NotFound,
    HandleTaken,
}

#[derive(Clone, Debug)]
pub struct OAuthClient<'a> {
    pub registration: &'a str,
    pub issuer: &'a str,
    pub client_id: &'a str,
    pub secret: Option<Sealed<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedSecret {
    pub server_id: Uuid,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
}

#[derive(Clone, Debug)]
pub struct Check<'a> {
    pub status: &'a str,
    pub protocol_version: Option<&'a str>,
    pub server_info: Option<&'a Value>,
    pub last_error: Option<&'a str>,
    pub at: OffsetDateTime,
}

const UNIQUE_VIOLATION: &str = "23505";

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some(UNIQUE_VIOLATION))
}

pub async fn insert(db: impl PgExecutor<'_>, s: &NewServer<'_>) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO mcp_servers (id, org_id, handle, name, url, auth_method, credential_mode, \
         oauth_registration, scopes, change_policy, created_by_kind, created_by_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         ON CONFLICT (org_id, handle) DO NOTHING",
        s.id,
        s.org_id,
        s.handle,
        s.name,
        s.url,
        s.auth_method,
        s.credential_mode,
        s.oauth_registration,
        s.scopes,
        s.change_policy,
        s.created_by_kind,
        s.created_by_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn list(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<ServerRow>, StoreError> {
    Ok(sqlx::query_as!(
        ServerRow,
        "SELECT id, org_id, handle, name, url, auth_method, credential_mode, oauth_registration, \
         oauth_issuer, oauth_client_id, oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         scopes, change_policy, protocol_version, server_info, status, last_checked_at, last_error, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM mcp_servers WHERE org_id = $1 ORDER BY name, handle",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<ServerRow>, StoreError> {
    Ok(sqlx::query_as!(
        ServerRow,
        "SELECT id, org_id, handle, name, url, auth_method, credential_mode, oauth_registration, \
         oauth_issuer, oauth_client_id, oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         scopes, change_policy, protocol_version, server_info, status, last_checked_at, last_error, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM mcp_servers WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_by_id(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<ServerRow>, StoreError> {
    Ok(sqlx::query_as!(
        ServerRow,
        "SELECT id, org_id, handle, name, url, auth_method, credential_mode, oauth_registration, \
         oauth_issuer, oauth_client_id, oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         scopes, change_policy, protocol_version, server_info, status, last_checked_at, last_error, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM mcp_servers WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    c: &Changes<'_>,
) -> Result<Updated, StoreError> {
    let row = sqlx::query_as!(
        ServerRow,
        "UPDATE mcp_servers SET name = COALESCE($3, name), handle = COALESCE($4, handle), \
         change_policy = COALESCE($5, change_policy), revision = revision + 1, updated_at = now() \
         WHERE org_id = $1 AND id = $2 \
         RETURNING id, org_id, handle, name, url, auth_method, credential_mode, oauth_registration, \
         oauth_issuer, oauth_client_id, oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         scopes, change_policy, protocol_version, server_info, status, last_checked_at, last_error, \
         created_by_kind, created_by_id, created_at, updated_at, revision",
        org_id,
        id,
        c.name,
        c.handle,
        c.change_policy
    )
    .fetch_optional(db)
    .await;
    match row {
        Ok(Some(row)) => Ok(Updated::Done(Box::new(row))),
        Ok(None) => Ok(Updated::NotFound),
        Err(e) if is_unique_violation(&e) => Ok(Updated::HandleTaken),
        Err(e) => Err(e.into()),
    }
}

pub async fn set_oauth_client(
    db: impl PgExecutor<'_>,
    id: Uuid,
    c: &OAuthClient<'_>,
) -> Result<bool, StoreError> {
    let secret = c.secret.as_ref();
    Ok(sqlx::query!(
        "UPDATE mcp_servers SET oauth_registration = $2, oauth_issuer = $3, oauth_client_id = $4, \
         oauth_client_secret_ciphertext = $5, oauth_client_secret_nonce = $6, \
         oauth_client_secret_wrapped_key = $7, oauth_client_secret_key_id = $8, \
         revision = revision + 1, updated_at = now() WHERE id = $1 AND auth_method = 'oauth'",
        id,
        c.registration,
        c.issuer,
        c.client_id,
        secret.map(|s| s.ciphertext),
        secret.map(|s| s.nonce),
        secret.map(|s| s.wrapped_key),
        secret.map(|s| s.key_id)
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

// A client the admin registered by hand: its issuer is learned on the first sign-in.
pub async fn set_manual_client(
    db: impl PgExecutor<'_>,
    id: Uuid,
    client_id: &str,
    secret: Option<&Sealed<'_>>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_servers SET oauth_registration = 'manual', oauth_issuer = NULL, oauth_client_id = $2, \
         oauth_client_secret_ciphertext = $3, oauth_client_secret_nonce = $4, \
         oauth_client_secret_wrapped_key = $5, oauth_client_secret_key_id = $6, \
         revision = revision + 1, updated_at = now() WHERE id = $1 AND auth_method = 'oauth'",
        id,
        client_id,
        secret.map(|s| s.ciphertext),
        secret.map(|s| s.nonce),
        secret.map(|s| s.wrapped_key),
        secret.map(|s| s.key_id)
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn client_secret(
    db: impl PgExecutor<'_>,
    id: Uuid,
) -> Result<Option<SealedSecret>, StoreError> {
    let row = sqlx::query!(
        "SELECT id, oauth_client_secret_ciphertext AS \"ciphertext!\", oauth_client_secret_nonce AS \"nonce!\", \
         oauth_client_secret_wrapped_key AS \"wrapped_key!\", oauth_client_secret_key_id AS \"key_id!\" \
         FROM mcp_servers WHERE id = $1 AND oauth_client_secret_ciphertext IS NOT NULL",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| SealedSecret {
        server_id: r.id,
        ciphertext: r.ciphertext,
        nonce: r.nonce,
        wrapped_key: r.wrapped_key,
        key_id: r.key_id,
    }))
}

pub async fn record_check(
    db: impl PgExecutor<'_>,
    id: Uuid,
    c: &Check<'_>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE mcp_servers SET status = $2, protocol_version = COALESCE($3, protocol_version), \
         server_info = COALESCE($4, server_info), last_error = $5, last_checked_at = $6 WHERE id = $1",
        id,
        c.status,
        c.protocol_version,
        c.server_info,
        c.last_error,
        c.at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn due(
    db: impl PgExecutor<'_>,
    checked_before: OffsetDateTime,
    limit: i64,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM mcp_servers WHERE last_checked_at IS NULL OR last_checked_at < $1 \
         ORDER BY last_checked_at NULLS FIRST, id LIMIT $2",
        checked_before,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM mcp_servers WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn of_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<ServerRow>, StoreError> {
    Ok(sqlx::query_as!(
        ServerRow,
        "SELECT s.id, s.org_id, s.handle, s.name, s.url, s.auth_method, s.credential_mode, s.oauth_registration, \
         s.oauth_issuer, s.oauth_client_id, s.oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         s.scopes, s.change_policy, s.protocol_version, s.server_info, s.status, s.last_checked_at, s.last_error, \
         s.created_by_kind, s.created_by_id, s.created_at, s.updated_at, s.revision \
         FROM project_mcp_servers ps JOIN mcp_servers s ON s.id = ps.server_id \
         WHERE ps.project_id = $1 ORDER BY s.name, s.handle",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn projects(db: impl PgExecutor<'_>, server_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT project_id FROM project_mcp_servers WHERE server_id = $1 ORDER BY created_at, project_id",
        server_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn project_links(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT server_id, project_id FROM project_mcp_servers WHERE org_id = $1 \
         ORDER BY server_id, created_at, project_id",
        org_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.server_id, r.project_id))
        .collect())
}

pub async fn set_projects(
    db: &mut PgConnection,
    org_id: Uuid,
    server_id: Uuid,
    projects: &[Uuid],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_mcp_servers WHERE org_id = $1 AND server_id = $2 AND project_id <> ALL($3::uuid[])",
        org_id,
        server_id,
        projects
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "INSERT INTO project_mcp_servers (project_id, org_id, server_id) \
         SELECT p.id, p.org_id, $2 FROM projects p WHERE p.org_id = $1 AND p.id = ANY($3::uuid[]) \
         ON CONFLICT DO NOTHING",
        org_id,
        server_id,
        projects
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn set_project_servers(
    db: &mut PgConnection,
    org_id: Uuid,
    project_id: Uuid,
    servers: &[Uuid],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM project_mcp_servers WHERE org_id = $1 AND project_id = $2 AND server_id <> ALL($3::uuid[])",
        org_id,
        project_id,
        servers
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "INSERT INTO project_mcp_servers (project_id, org_id, server_id) \
         SELECT $2, s.org_id, s.id FROM mcp_servers s WHERE s.org_id = $1 AND s.id = ANY($3::uuid[]) \
         ON CONFLICT DO NOTHING",
        org_id,
        project_id,
        servers
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<SealedSecret>, StoreError> {
    let rows = sqlx::query!(
        "SELECT id, oauth_client_secret_ciphertext AS \"ciphertext!\", oauth_client_secret_nonce AS \"nonce!\", \
         oauth_client_secret_wrapped_key AS \"wrapped_key!\", oauth_client_secret_key_id AS \"key_id!\" \
         FROM mcp_servers WHERE oauth_client_secret_ciphertext IS NOT NULL"
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| SealedSecret {
            server_id: r.id,
            ciphertext: r.ciphertext,
            nonce: r.nonce,
            wrapped_key: r.wrapped_key,
            key_id: r.key_id,
        })
        .collect())
}

pub async fn set_wrapped_key(
    db: impl PgExecutor<'_>,
    id: Uuid,
    old_wrapped_key: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE mcp_servers SET oauth_client_secret_wrapped_key = $3, oauth_client_secret_key_id = $4 \
         WHERE id = $1 AND oauth_client_secret_wrapped_key = $2",
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

// The servers where each person signs in with their own account, in every organization the
// person belongs to.
pub async fn per_person_of_member(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<ServerRow>, StoreError> {
    Ok(sqlx::query_as!(
        ServerRow,
        "SELECT s.id, s.org_id, s.handle, s.name, s.url, s.auth_method, s.credential_mode, s.oauth_registration, \
         s.oauth_issuer, s.oauth_client_id, s.oauth_client_secret_ciphertext IS NOT NULL AS \"has_client_secret!\", \
         s.scopes, s.change_policy, s.protocol_version, s.server_info, s.status, s.last_checked_at, s.last_error, \
         s.created_by_kind, s.created_by_id, s.created_at, s.updated_at, s.revision \
         FROM mcp_servers s JOIN org_memberships m ON m.org_id = s.org_id AND m.user_id = $1 \
         WHERE s.credential_mode = 'per_user' AND s.auth_method <> 'none' ORDER BY s.name, s.handle",
        user_id
    )
    .fetch_all(db)
    .await?)
}
