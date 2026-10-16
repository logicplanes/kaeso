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
}

pub async fn get(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Option<Connection>, StoreError> {
    Ok(sqlx::query_as!(
        Connection,
        "SELECT c.org_id, c.issuer, c.client_id, c.secret_ciphertext, c.secret_nonce, c.wrapped_key, c.key_id, \
         COALESCE(array_agg(d.domain ORDER BY d.domain) FILTER (WHERE d.domain IS NOT NULL), '{}') AS \"domains!\" \
         FROM org_oidc c LEFT JOIN org_oidc_domains d ON d.org_id = c.org_id \
         WHERE c.org_id = $1 GROUP BY c.org_id",
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
        "SELECT org_id FROM org_oidc_domains WHERE domain = lower($1)",
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

pub async fn upsert(db: &mut sqlx::PgConnection, c: &Connection) -> Result<Upserted, StoreError> {
    sqlx::query!(
        "INSERT INTO org_oidc (org_id, issuer, client_id, secret_ciphertext, secret_nonce, wrapped_key, key_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (org_id) DO UPDATE SET issuer = $2, client_id = $3, \
         secret_ciphertext = $4, secret_nonce = $5, wrapped_key = $6, key_id = $7, updated_at = now()",
        c.org_id,
        c.issuer,
        c.client_id,
        c.secret_ciphertext,
        c.secret_nonce,
        c.wrapped_key,
        c.key_id
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM org_oidc_domains WHERE org_id = $1", c.org_id)
        .execute(&mut *db)
        .await?;
    for domain in &c.domains {
        let taken = sqlx::query!(
            "INSERT INTO org_oidc_domains (domain, org_id) VALUES (lower($1), $2) ON CONFLICT (domain) DO NOTHING",
            domain,
            c.org_id
        )
        .execute(&mut *db)
        .await?
        .rows_affected()
            == 0;
        if taken {
            return Ok(Upserted::DomainTaken(domain.clone()));
        }
    }
    Ok(Upserted::Saved)
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
         '{}'::text[] AS \"domains!\" FROM org_oidc"
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
