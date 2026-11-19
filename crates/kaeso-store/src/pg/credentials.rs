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
    pub auth_method: String,
    pub account_label: Option<String>,
    pub auth_error: Option<String>,
    pub project: Option<String>,
    pub location: Option<String>,
    pub account_id: Option<String>,
    pub gateway_id: Option<String>,
    pub endpoint: Option<String>,
    pub spend_limited_until: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedRow {
    pub id: Uuid,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub revision: i64,
    pub auth_method: String,
    pub expires_at: Option<OffsetDateTime>,
    pub auth_error: Option<String>,
    pub location: Option<String>,
    pub account_id: Option<String>,
    pub gateway_id: Option<String>,
    pub endpoint: Option<String>,
    pub spend_limited_until: Option<OffsetDateTime>,
    /// For a link: whether the install's app asks each request to name the linked profile.
    pub billing_policy: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealed<'a> {
    pub ciphertext: &'a [u8],
    pub nonce: &'a [u8],
    pub wrapped_key: &'a [u8],
    pub key_id: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyInUse {
    pub id: Uuid,
    pub revision: i64,
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
    pub auth_method: &'a str,
    pub expires_at: Option<OffsetDateTime>,
    pub account_label: Option<&'a str>,
    pub project: Option<&'a str>,
    pub location: Option<&'a str>,
    pub account_id: Option<&'a str>,
    pub gateway_id: Option<&'a str>,
    pub endpoint: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignmentRow {
    pub credential_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub project_avatar: Option<String>,
    pub weight: i16,
}

/// One key of the pool a project draws from for a provider, with its weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolMember {
    pub credential_id: Uuid,
    pub weight: i16,
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

pub async fn lock_refresh(db: &mut PgConnection, id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended('provider_credential_refresh:' || $1::uuid::text, 0))",
        id
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
         last4, created_by_kind, created_by_id, auth_method, expires_at, account_label, project, location, \
         account_id, gateway_id, endpoint) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)",
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
        c.created_by_id,
        c.auth_method,
        c.expires_at,
        c.account_label,
        c.project,
        c.location,
        c.account_id,
        c.gateway_id,
        c.endpoint
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn replace(db: impl PgExecutor<'_>, c: &NewCredential<'_>) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET name = $3, ciphertext = $4, nonce = $5, wrapped_key = $6, key_id = $7, last4 = $8, \
         auth_method = $9, expires_at = $10, account_label = $11, project = $12, location = $13, account_id = $14, \
         gateway_id = $15, endpoint = $16, auth_error = NULL, spend_limited_until = NULL, revision = revision + 1, \
         updated_at = now() WHERE org_id = $1 AND id = $2",
        c.org_id,
        c.id,
        c.name,
        c.ciphertext,
        c.nonce,
        c.wrapped_key,
        c.key_id,
        c.last4,
        c.auth_method,
        c.expires_at,
        c.account_label,
        c.project,
        c.location,
        c.account_id,
        c.gateway_id,
        c.endpoint
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn refreshed(
    db: impl PgExecutor<'_>,
    id: Uuid,
    sealed: &Sealed<'_>,
    expires_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET ciphertext = $2, nonce = $3, wrapped_key = $4, key_id = $5, \
         expires_at = $6 WHERE id = $1 AND auth_method IN ('chatgpt', 'grok') AND auth_error IS NULL",
        id,
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

pub async fn sign_out(db: impl PgExecutor<'_>, id: Uuid, reason: &str) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE provider_credentials SET auth_error = $2 WHERE id = $1 \
         AND auth_method IN ('chatgpt', 'grok', 'azure_service_principal', 'linked_billing')",
        id,
        reason
    )
    .execute(db)
    .await?;
    Ok(())
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

pub async fn set_location(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    location: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET location = $3, revision = revision + 1, updated_at = now() \
         WHERE org_id = $1 AND id = $2 AND (auth_method = 'google_service_account' \
         OR (auth_method = 'api_key' AND location IS NOT NULL))",
        org_id,
        id,
        location
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

/// Moves a key of a provider whose API path names the account to another account or gateway.
pub async fn set_scope(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    account_id: &str,
    gateway_id: Option<&str>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET account_id = $3, gateway_id = $4, revision = revision + 1, \
         updated_at = now() WHERE org_id = $1 AND id = $2 AND account_id IS NOT NULL",
        org_id,
        id,
        account_id,
        gateway_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

/// Moves a key of a provider whose keys each belong to a resource to another endpoint.
pub async fn set_endpoint(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    endpoint: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET endpoint = $3, revision = revision + 1, \
         updated_at = now() WHERE org_id = $1 AND id = $2 AND endpoint IS NOT NULL",
        org_id,
        id,
        endpoint
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
         created_at, updated_at, auth_method, account_label, auth_error, project, location, account_id, gateway_id, endpoint, \
         spend_limited_until FROM provider_credentials WHERE org_id = $1 AND id = $2",
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
         created_at, updated_at, auth_method, account_label, auth_error, project, location, account_id, gateway_id, endpoint, \
         spend_limited_until FROM provider_credentials WHERE org_id = $1 \
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
        "SELECT a.credential_id, a.project_id, p.name AS project_name, p.avatar AS project_avatar, a.weight \
         FROM project_provider_keys a \
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

/// Sets the projects that draw from `credential` for its provider, each with its weight; the
/// project's other keys of the provider stay in its pool.
pub async fn set_assignments(
    db: &mut PgConnection,
    org_id: Uuid,
    provider: &str,
    credential: Uuid,
    projects: &[(Uuid, i16)],
) -> Result<(), StoreError> {
    let ids: Vec<Uuid> = projects.iter().map(|(p, _)| *p).collect();
    let weights: Vec<i16> = projects.iter().map(|(_, w)| *w).collect();
    sqlx::query!(
        "DELETE FROM project_provider_keys WHERE org_id = $1 AND credential_id = $2 AND NOT (project_id = ANY($3))",
        org_id,
        credential,
        &ids
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "INSERT INTO project_provider_keys (project_id, provider, org_id, credential_id, weight) \
         SELECT p, $2, $1, $3, w FROM unnest($4::uuid[], $5::smallint[]) AS t(p, w) \
         ON CONFLICT (project_id, provider, credential_id) DO UPDATE SET weight = EXCLUDED.weight \
         WHERE project_provider_keys.weight <> EXCLUDED.weight",
        org_id,
        provider,
        credential,
        &ids,
        &weights
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

/// The keys a project draws from for a provider: the ones assigned to it, or the provider's
/// default when none is.
pub async fn pool(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<Vec<PoolMember>, StoreError> {
    Ok(sqlx::query_as!(
        PoolMember,
        r#"SELECT a.credential_id AS "credential_id!", a.weight AS "weight!" FROM project_provider_keys a
         WHERE a.org_id = $1 AND a.project_id = $2 AND a.provider = $3
         UNION ALL
         SELECT c.id, 1::smallint FROM provider_credentials c
         WHERE c.org_id = $1 AND c.provider = $3 AND c.is_default AND NOT EXISTS (
             SELECT 1 FROM project_provider_keys a WHERE a.project_id = $2 AND a.provider = $3)
         ORDER BY 1"#,
        org_id,
        project_id,
        provider
    )
    .fetch_all(db)
    .await?)
}

/// One key of a project's pool for a provider, or `None` when it isn't in the pool.
pub async fn resolve_member(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
    credential: Uuid,
) -> Result<Option<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT c.id, c.ciphertext, c.nonce, c.wrapped_key, c.key_id, c.revision, c.auth_method, c.expires_at, \
         c.auth_error, c.location, c.account_id, c.gateway_id, c.endpoint, c.spend_limited_until, \
         (SELECT a.send_billing_policy FROM linked_billing_apps a WHERE a.provider = c.provider) AS billing_policy \
         FROM provider_credentials c \
         WHERE c.org_id = $1 AND c.provider = $3 AND c.id = $4 AND ( \
             EXISTS (SELECT 1 FROM project_provider_keys a WHERE a.credential_id = c.id AND a.project_id = $2) \
             OR (c.is_default AND NOT EXISTS ( \
                 SELECT 1 FROM project_provider_keys a WHERE a.project_id = $2 AND a.provider = $3)) \
             OR EXISTS (SELECT 1 FROM linked_billing_fallbacks f \
                 WHERE f.project_id = $2 AND f.provider = $3 AND f.credential_id = c.id))",
        org_id,
        project_id,
        provider,
        credential
    )
    .fetch_optional(db)
    .await?)
}

pub async fn resolve(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<Option<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT c.id, c.ciphertext, c.nonce, c.wrapped_key, c.key_id, c.revision, c.auth_method, c.expires_at, \
         c.auth_error, c.location, c.account_id, c.gateway_id, c.endpoint, c.spend_limited_until, \
         (SELECT a.send_billing_policy FROM linked_billing_apps a WHERE a.provider = c.provider) AS billing_policy \
         FROM provider_credentials c \
         LEFT JOIN project_provider_keys a ON a.credential_id = c.id AND a.project_id = $2 \
         WHERE c.org_id = $1 AND c.provider = $3 AND (a.project_id IS NOT NULL OR c.is_default) \
         ORDER BY a.project_id IS NULL, a.created_at, c.id LIMIT 1",
        org_id,
        project_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub async fn sealed(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT id, ciphertext, nonce, wrapped_key, key_id, revision, auth_method, expires_at, auth_error, location, \
         account_id, gateway_id, endpoint, spend_limited_until, \
         (SELECT a.send_billing_policy FROM linked_billing_apps a WHERE a.provider = provider_credentials.provider) \
         AS billing_policy \
         FROM provider_credentials WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn key_in_use(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<Option<KeyInUse>, StoreError> {
    Ok(sqlx::query_as!(
        KeyInUse,
        "SELECT c.id, c.revision FROM provider_credentials c \
         LEFT JOIN project_provider_keys a ON a.credential_id = c.id AND a.project_id = $2 \
         WHERE c.org_id = $1 AND c.provider = $3 AND (a.project_id IS NOT NULL OR c.is_default) \
         ORDER BY a.project_id IS NULL, a.created_at, c.id LIMIT 1",
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

/// A signed-in provider account whose access token runs out by a given time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Renewable {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub auth_method: String,
    pub expires_at: OffsetDateTime,
}

/// The signed-in accounts whose access token runs out by `by` and that were used since
/// `used_since`, soonest first.
pub async fn renewable(
    db: impl PgExecutor<'_>,
    by: OffsetDateTime,
    used_since: OffsetDateTime,
) -> Result<Vec<Renewable>, StoreError> {
    Ok(sqlx::query_as!(
        Renewable,
        r#"SELECT id, org_id, provider, auth_method, expires_at AS "expires_at!" FROM provider_credentials
         WHERE auth_method IN ('chatgpt', 'grok', 'linked_billing') AND auth_error IS NULL
         AND expires_at IS NOT NULL AND expires_at <= $1 AND last_used_at >= $2
         ORDER BY expires_at"#,
        by,
        used_since
    )
    .fetch_all(db)
    .await?)
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<SealedRow>, StoreError> {
    Ok(sqlx::query_as!(
        SealedRow,
        "SELECT id, ciphertext, nonce, wrapped_key, key_id, revision, auth_method, expires_at, auth_error, location, \
         account_id, gateway_id, endpoint, spend_limited_until, \
         (SELECT a.send_billing_policy FROM linked_billing_apps a WHERE a.provider = provider_credentials.provider) \
         AS billing_policy \
         FROM provider_credentials"
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
           UNION ALL SELECT key_id FROM org_oidc UNION ALL SELECT key_id FROM mcp_connections \
           UNION ALL SELECT oauth_client_secret_key_id FROM mcp_servers \
           WHERE oauth_client_secret_key_id IS NOT NULL \
           UNION ALL SELECT secret_key_id FROM guardrails WHERE secret_key_id IS NOT NULL \
           UNION ALL SELECT key_id FROM linked_billing_keys) k GROUP BY key_id ORDER BY key_id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.key_id, r.n)).collect())
}
