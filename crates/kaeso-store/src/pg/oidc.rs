use sqlx::PgExecutor;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct Connection {
    pub org_id: Uuid,
    pub issuer: String,
    pub client_id: String,
    pub secret_ciphertext: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub domains: Vec<String>,
    pub verified_domains: Vec<String>,
    pub verification_token: String,
    pub display_name: Option<String>,
    pub auto_redirect: bool,
    pub require_email_verified: bool,
    pub sends_auth_time: Option<bool>,
}

pub async fn get(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Option<Connection>, StoreError> {
    Ok(sqlx::query_as!(
        Connection,
        "SELECT c.org_id, c.issuer, c.client_id, c.secret_ciphertext, c.secret_nonce, c.wrapped_key, c.key_id, \
         COALESCE(array_agg(d.domain ORDER BY d.domain) FILTER (WHERE d.domain IS NOT NULL), '{}') AS \"domains!\", \
         COALESCE(array_agg(d.domain ORDER BY d.domain) FILTER (WHERE d.verified_at IS NOT NULL), '{}') AS \"verified_domains!\", \
         c.verification_token, c.display_name, c.auto_redirect, c.require_email_verified, c.sends_auth_time \
         FROM org_oidc c LEFT JOIN org_oidc_domains d ON d.org_id = c.org_id \
         WHERE c.org_id = $1 GROUP BY c.org_id",
        org_id
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    pub issuer: String,
    pub client_id: String,
}

pub async fn lock_pointer(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Option<Pointer>, StoreError> {
    Ok(sqlx::query_as!(
        Pointer,
        "SELECT issuer, client_id FROM org_oidc WHERE org_id = $1 FOR SHARE",
        org_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn org_for_domain(
    db: impl PgExecutor<'_>,
    domain: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT org_id FROM org_oidc_domains WHERE domain = lower($1) AND verified_at IS NOT NULL",
        domain
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Upserted {
    Saved,
    DomainTaken(String),
}

// Whether ID tokens carry auth_time depends on the provider and its app, so a connection to
// another issuer or client id starts unknown again; `sends_auth_time` is never taken from `c`.
pub async fn upsert(db: &mut sqlx::PgConnection, c: &Connection) -> Result<Upserted, StoreError> {
    sqlx::query!(
        "INSERT INTO org_oidc (org_id, issuer, client_id, secret_ciphertext, secret_nonce, wrapped_key, key_id, \
         display_name, auto_redirect, verification_token, require_email_verified) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (org_id) DO UPDATE SET issuer = $2, client_id = $3, \
         secret_ciphertext = $4, secret_nonce = $5, wrapped_key = $6, key_id = $7, display_name = $8, \
         auto_redirect = $9, require_email_verified = $11, updated_at = now(), \
         sends_auth_time = CASE WHEN org_oidc.issuer = $2 AND org_oidc.client_id = $3 \
         THEN org_oidc.sends_auth_time END",
        c.org_id,
        c.issuer,
        c.client_id,
        c.secret_ciphertext,
        c.secret_nonce,
        c.wrapped_key,
        c.key_id,
        c.display_name,
        c.auto_redirect,
        c.verification_token,
        c.require_email_verified
    )
    .execute(&mut *db)
    .await?;
    let domains: Vec<String> = c.domains.iter().map(|d| d.to_lowercase()).collect();
    sqlx::query!(
        "DELETE FROM org_oidc_domains WHERE org_id = $1 AND domain <> ALL($2)",
        c.org_id,
        &domains
    )
    .execute(&mut *db)
    .await?;
    for domain in &domains {
        if verified_elsewhere(&mut *db, c.org_id, domain).await? {
            return Ok(Upserted::DomainTaken(domain.clone()));
        }
        sqlx::query!(
            "INSERT INTO org_oidc_domains (domain, org_id) VALUES ($1, $2) ON CONFLICT (org_id, domain) DO NOTHING",
            domain,
            c.org_id
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(Upserted::Saved)
}

pub async fn verified_elsewhere(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    domain: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM org_oidc_domains WHERE domain = lower($2) AND org_id <> $1 \
         AND verified_at IS NOT NULL) AS \"taken!\"",
        org_id,
        domain
    )
    .fetch_one(db)
    .await?)
}

pub async fn mark_verified(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    domain: &str,
    at: time::OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE org_oidc_domains SET verified_at = $3 \
         WHERE org_id = $1 AND domain = lower($2) AND verified_at IS NULL",
        org_id,
        domain,
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn verified_at(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<(String, time::OffsetDateTime)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT domain, verified_at AS \"verified_at!\" FROM org_oidc_domains \
         WHERE org_id = $1 AND verified_at IS NOT NULL",
        org_id
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.domain, r.verified_at))
    .collect())
}

pub async fn domain_owners(db: impl PgExecutor<'_>, domain: &str) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT org_id FROM org_oidc_domains WHERE domain = lower($1) ORDER BY org_id",
        domain
    )
    .fetch_all(db)
    .await?)
}

pub async fn record_auth_time(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    issuer: &str,
    sends: bool,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE org_oidc SET sends_auth_time = $3 \
         WHERE org_id = $1 AND issuer = $2 AND sends_auth_time IS DISTINCT FROM $3",
        org_id,
        issuer,
        sends
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<bool, StoreError> {
    Ok(
        sqlx::query!("DELETE FROM org_oidc WHERE org_id = $1", org_id)
            .execute(db)
            .await?
            .rows_affected()
            == 1,
    )
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<Connection>, StoreError> {
    Ok(sqlx::query_as!(
        Connection,
        "SELECT org_id, issuer, client_id, secret_ciphertext, secret_nonce, wrapped_key, key_id, \
         '{}'::text[] AS \"domains!\", '{}'::text[] AS \"verified_domains!\", \
         verification_token, display_name, auto_redirect, require_email_verified, sends_auth_time FROM org_oidc"
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_wrapped_key(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    old_wrapped_key: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE org_oidc SET wrapped_key = $3, key_id = $4, updated_at = now() \
         WHERE org_id = $1 AND wrapped_key = $2",
        org_id,
        old_wrapped_key,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}
