use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::credentials::Sealed;

/// The install's signing key, sealed like a provider key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigningKey {
    pub kid: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
}

pub async fn signing_key(db: impl PgExecutor<'_>) -> Result<Option<SigningKey>, StoreError> {
    Ok(sqlx::query_as!(
        SigningKey,
        "SELECT kid, ciphertext, nonce, wrapped_key, key_id FROM linked_billing_keys \
         ORDER BY created_at LIMIT 1"
    )
    .fetch_optional(db)
    .await?)
}

/// Stores the install's first signing key; `false` when another replica stored one first.
pub async fn insert_signing_key(
    db: impl PgExecutor<'_>,
    kid: &str,
    sealed: &Sealed<'_>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO linked_billing_keys (kid, ciphertext, nonce, wrapped_key, key_id) \
         SELECT $1, $2, $3, $4, $5 WHERE NOT EXISTS (SELECT 1 FROM linked_billing_keys)",
        kid,
        sealed.ciphertext,
        sealed.nonce,
        sealed.wrapped_key,
        sealed.key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn all_signing_keys(db: impl PgExecutor<'_>) -> Result<Vec<SigningKey>, StoreError> {
    Ok(sqlx::query_as!(
        SigningKey,
        "SELECT kid, ciphertext, nonce, wrapped_key, key_id FROM linked_billing_keys"
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_signing_key_wrapped(
    db: impl PgExecutor<'_>,
    kid: &str,
    old_wrapped: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE linked_billing_keys SET wrapped_key = $3, key_id = $4 WHERE kid = $1 AND wrapped_key = $2",
        kid,
        old_wrapped,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

/// The install's OAuth app at a provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRow {
    pub provider: String,
    pub client_id: String,
    pub audience: String,
    pub organization_id: String,
    pub federation_rule_id: String,
    pub workspace_id: Option<String>,
    pub service_account_id: Option<String>,
    pub send_billing_policy: bool,
    pub check_profile_id: Option<String>,
    pub checked_at: Option<OffsetDateTime>,
    pub check_problem: Option<String>,
    pub updated_by: Option<Uuid>,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewApp<'a> {
    pub provider: &'a str,
    pub client_id: &'a str,
    pub audience: &'a str,
    pub organization_id: &'a str,
    pub federation_rule_id: &'a str,
    pub workspace_id: Option<&'a str>,
    pub service_account_id: Option<&'a str>,
    pub send_billing_policy: bool,
    pub updated_by: Option<Uuid>,
}

pub async fn app(db: impl PgExecutor<'_>, provider: &str) -> Result<Option<AppRow>, StoreError> {
    Ok(sqlx::query_as!(
        AppRow,
        "SELECT provider, client_id, audience, organization_id, federation_rule_id, workspace_id, \
         service_account_id, send_billing_policy, check_profile_id, checked_at, check_problem, updated_by, \
         updated_at FROM linked_billing_apps WHERE provider = $1",
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub async fn apps(db: impl PgExecutor<'_>) -> Result<Vec<AppRow>, StoreError> {
    Ok(sqlx::query_as!(
        AppRow,
        "SELECT provider, client_id, audience, organization_id, federation_rule_id, workspace_id, \
         service_account_id, send_billing_policy, check_profile_id, checked_at, check_problem, updated_by, \
         updated_at FROM linked_billing_apps ORDER BY provider"
    )
    .fetch_all(db)
    .await?)
}

/// Sets the app; a new client id forgets the last check and its user profile, which belong to
/// the earlier app.
pub async fn put_app(db: impl PgExecutor<'_>, a: &NewApp<'_>) -> Result<AppRow, StoreError> {
    Ok(sqlx::query_as!(
        AppRow,
        "INSERT INTO linked_billing_apps AS l (provider, client_id, audience, organization_id, federation_rule_id, \
           workspace_id, service_account_id, send_billing_policy, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (provider) DO UPDATE SET client_id = EXCLUDED.client_id, audience = EXCLUDED.audience, \
           organization_id = EXCLUDED.organization_id, federation_rule_id = EXCLUDED.federation_rule_id, \
           workspace_id = EXCLUDED.workspace_id, service_account_id = EXCLUDED.service_account_id, \
           send_billing_policy = EXCLUDED.send_billing_policy, updated_by = EXCLUDED.updated_by, \
           updated_at = now(), \
           check_profile_id = CASE WHEN l.client_id = EXCLUDED.client_id \
             AND l.organization_id = EXCLUDED.organization_id THEN l.check_profile_id END, \
           checked_at = NULL, check_problem = NULL \
         RETURNING provider, client_id, audience, organization_id, federation_rule_id, workspace_id, \
           service_account_id, send_billing_policy, check_profile_id, checked_at, check_problem, updated_by, \
           updated_at",
        a.provider,
        a.client_id,
        a.audience,
        a.organization_id,
        a.federation_rule_id,
        a.workspace_id,
        a.service_account_id,
        a.send_billing_policy,
        a.updated_by
    )
    .fetch_one(db)
    .await?)
}

pub async fn delete_app(db: impl PgExecutor<'_>, provider: &str) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM linked_billing_apps WHERE provider = $1",
        provider
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

/// Records a check of the app: when it ran, what it found wrong (`None` when nothing), and the
/// user profile it keeps for checking.
pub async fn checked(
    db: impl PgExecutor<'_>,
    provider: &str,
    client_id: &str,
    at: OffsetDateTime,
    problem: Option<&str>,
    check_profile_id: Option<&str>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE linked_billing_apps SET checked_at = $3, check_problem = $4, \
         check_profile_id = coalesce($5, check_profile_id) WHERE provider = $1 AND client_id = $2",
        provider,
        client_id,
        at,
        problem,
        check_profile_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn profile(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
    client_id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_profile_id FROM linked_billing_profiles WHERE org_id = $1 AND provider = $2 AND client_id = $3",
        org_id,
        provider,
        client_id
    )
    .fetch_optional(db)
    .await?)
}

/// Keeps an organization's user profile; the one already kept wins, and is returned.
pub async fn keep_profile(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
    client_id: &str,
    user_profile_id: &str,
) -> Result<String, StoreError> {
    Ok(sqlx::query_scalar!(
        "WITH kept AS (INSERT INTO linked_billing_profiles (org_id, provider, client_id, user_profile_id) \
           VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING RETURNING user_profile_id) \
         SELECT user_profile_id AS \"user_profile_id!\" FROM kept UNION ALL \
         SELECT user_profile_id FROM linked_billing_profiles WHERE org_id = $1 AND provider = $2 AND client_id = $3 \
         LIMIT 1",
        org_id,
        provider,
        client_id,
        user_profile_id
    )
    .fetch_one(db)
    .await?)
}

/// The organizations whose links an app's user profiles hold: for a profile that is already
/// linked through another of them, the organization that holds the link.
pub async fn org_of_profile(
    db: impl PgExecutor<'_>,
    provider: &str,
    client_id: &str,
    user_profile_id: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT org_id FROM linked_billing_profiles WHERE provider = $1 AND client_id = $2 AND user_profile_id = $3",
        provider,
        client_id,
        user_profile_id
    )
    .fetch_optional(db)
    .await?)
}

/// A linked key's state that the console and requests read besides its tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedKey {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub auth_error: Option<String>,
}

/// The linked keys of a provider, across every organization.
pub async fn linked_keys(
    db: impl PgExecutor<'_>,
    provider: &str,
) -> Result<Vec<LinkedKey>, StoreError> {
    Ok(sqlx::query_as!(
        LinkedKey,
        "SELECT id, org_id, provider, auth_error FROM provider_credentials \
         WHERE auth_method = 'linked_billing' AND provider = $1 ORDER BY created_at",
        provider
    )
    .fetch_all(db)
    .await?)
}

/// Marks every working linked key of a provider as needing the install's app; returns how many
/// it marked.
pub async fn needs_setup(db: impl PgExecutor<'_>, provider: &str) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET auth_error = 'needs_setup' \
         WHERE auth_method = 'linked_billing' AND provider = $1 AND auth_error IS NULL",
        provider
    )
    .execute(db)
    .await?
    .rows_affected())
}

/// Lets the linked keys of a provider work again once its app is back; returns how many.
pub async fn setup_done(db: impl PgExecutor<'_>, provider: &str) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET auth_error = NULL \
         WHERE auth_method = 'linked_billing' AND provider = $1 AND auth_error = 'needs_setup'",
        provider
    )
    .execute(db)
    .await?
    .rows_affected())
}

/// Sets a linked key's state: `unlinked`, `needs_setup` or `on_hold`; `true` when it changed.
pub async fn mark(db: impl PgExecutor<'_>, id: Uuid, state: &str) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET auth_error = $2 \
         WHERE id = $1 AND auth_method = 'linked_billing' AND auth_error IS DISTINCT FROM $2",
        id,
        state
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

/// Takes a refresh's hold off again, when the refresh token never left Kaeso or the provider
/// refused it without using it.
pub async fn release(db: impl PgExecutor<'_>, id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE provider_credentials SET auth_error = NULL WHERE id = $1 AND auth_error = 'on_hold'",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Stores a linked key's renewed tokens and takes the refresh's hold off.
pub async fn renewed(
    db: impl PgExecutor<'_>,
    id: Uuid,
    sealed: &Sealed<'_>,
    expires_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET ciphertext = $2, nonce = $3, wrapped_key = $4, key_id = $5, \
         expires_at = $6, auth_error = NULL WHERE id = $1 AND auth_method = 'linked_billing' \
         AND auth_error = 'on_hold'",
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

/// Records a spend-limit refusal; `true` when the key wasn't limited already.
pub async fn spend_limited(
    db: impl PgExecutor<'_>,
    id: Uuid,
    until: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "WITH before AS (SELECT spend_limited_until FROM provider_credentials WHERE id = $1 FOR UPDATE) \
         UPDATE provider_credentials SET spend_limited_until = $2 \
         WHERE id = $1 AND auth_method = 'linked_billing' \
         RETURNING (SELECT spend_limited_until FROM before) IS NULL AS \"newly!\"",
        id,
        until
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(false))
}

/// Clears a spend limit once a request went through; `true` when one was set.
pub async fn spend_lifted(db: impl PgExecutor<'_>, id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE provider_credentials SET spend_limited_until = NULL \
         WHERE id = $1 AND spend_limited_until IS NOT NULL",
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallback {
    pub project_id: Uuid,
    pub provider: String,
    pub credential_id: Uuid,
}

pub async fn fallbacks(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Fallback>, StoreError> {
    Ok(sqlx::query_as!(
        Fallback,
        "SELECT project_id, provider, credential_id FROM linked_billing_fallbacks WHERE org_id = $1 \
         ORDER BY project_id, provider",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn fallback(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT credential_id FROM linked_billing_fallbacks WHERE org_id = $1 AND project_id = $2 AND provider = $3",
        org_id,
        project_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

/// Sets or, with `None`, removes a project's fallback for a provider.
pub async fn set_fallback(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    provider: &str,
    credential_id: Option<Uuid>,
) -> Result<(), StoreError> {
    match credential_id {
        Some(credential) => {
            sqlx::query!(
                "INSERT INTO linked_billing_fallbacks (project_id, provider, org_id, credential_id) \
                 VALUES ($1, $2, $3, $4) ON CONFLICT (project_id, provider) DO UPDATE SET \
                 credential_id = EXCLUDED.credential_id, created_at = now()",
                project_id,
                provider,
                org_id,
                credential
            )
            .execute(db)
            .await?;
        }
        None => {
            sqlx::query!(
                "DELETE FROM linked_billing_fallbacks WHERE org_id = $1 AND project_id = $2 AND provider = $3",
                org_id,
                project_id,
                provider
            )
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

/// The org's cost this month by provider key, for the keys that are links: what the linked
/// organizations paid through Kaeso.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedCost {
    pub credential_id: Uuid,
    pub cost: rust_decimal::Decimal,
    pub requests: i64,
}

pub async fn linked_costs(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    from: time::Date,
    to: time::Date,
) -> Result<Vec<LinkedCost>, StoreError> {
    Ok(sqlx::query_as!(
        LinkedCost,
        "SELECT u.credential_id AS \"credential_id!\", coalesce(sum(u.cost), 0) AS \"cost!\", \
           coalesce(sum(u.requests - u.errors), 0)::bigint AS \"requests!\" \
         FROM usage_daily u JOIN provider_credentials c ON c.id = u.credential_id AND c.org_id = u.org_id \
         WHERE u.org_id = $1 AND u.day >= $2 AND u.day <= $3 AND c.auth_method = 'linked_billing' \
         GROUP BY u.credential_id ORDER BY u.credential_id",
        org_id,
        from,
        to
    )
    .fetch_all(db)
    .await?)
}
