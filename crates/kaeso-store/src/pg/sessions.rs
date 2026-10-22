use std::net::IpAddr;

use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub method: String,
    pub second_factor: bool,
    pub oidc_org_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub signed_in_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_used_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub user_state: String,
    pub is_instance_admin: bool,
}

#[derive(Clone, Debug)]
pub struct SessionListing {
    pub id: Uuid,
    pub method: String,
    pub second_factor: bool,
    pub oidc_org_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub signed_in_at: OffsetDateTime,
    pub last_used_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub ip: Option<ipnet::IpNet>,
    pub user_agent: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewSession<'a> {
    pub id: Uuid,
    pub token_hash: &'a [u8],
    pub user_id: Uuid,
    pub method: &'a str,
    pub second_factor: bool,
    pub oidc_org_id: Option<Uuid>,
    pub signed_in_at: OffsetDateTime,
    pub now: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub ip: Option<IpAddr>,
    pub user_agent: Option<&'a str>,
}

pub async fn insert(db: impl PgExecutor<'_>, s: &NewSession<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO sessions (id, token_hash, user_id, method, second_factor, oidc_org_id, \
         signed_in_at, last_used_at, expires_at, ip, user_agent) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        s.id,
        s.token_hash,
        s.user_id,
        s.method,
        s.second_factor,
        s.oidc_org_id,
        s.signed_in_at,
        s.now,
        s.expires_at,
        s.ip.map(ipnet::IpNet::from),
        s.user_agent,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_by_hash(
    db: impl PgExecutor<'_>,
    hash: &[u8],
) -> Result<Option<SessionRow>, StoreError> {
    Ok(sqlx::query_as!(
        SessionRow,
        "SELECT s.id, s.user_id, s.method, s.second_factor, s.oidc_org_id, s.signed_in_at, \
         s.last_used_at, s.expires_at, u.state AS user_state, u.is_instance_admin \
         FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL",
        hash
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    Touched,
    Recent,
    Ended,
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<Touch, StoreError> {
    let row = sqlx::query!(
        "WITH s AS (SELECT id, revoked_at FROM sessions WHERE id = $1), \
         t AS (UPDATE sessions SET last_used_at = $2 \
               WHERE id = $1 AND last_used_at < $2 AND revoked_at IS NULL RETURNING id) \
         SELECT (SELECT count(*) FROM t) AS \"touched!\", \
                (SELECT count(*) FROM s WHERE revoked_at IS NULL) AS \"live!\"",
        id,
        at
    )
    .fetch_one(db)
    .await?;
    Ok(if row.touched > 0 {
        Touch::Touched
    } else if row.live > 0 {
        Touch::Recent
    } else {
        Touch::Ended
    })
}

pub async fn is_live(
    db: impl PgExecutor<'_>,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM sessions WHERE id = $1 AND revoked_at IS NULL AND expires_at > $2) AS "live!""#,
        id,
        now
    )
    .fetch_one(db)
    .await?)
}

pub async fn revoke(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<Option<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE sessions SET revoked_at = $3 WHERE id = $2 AND user_id = $1 AND revoked_at IS NULL \
         RETURNING token_hash",
        user_id,
        id,
        at
    )
    .fetch_optional(db)
    .await?)
}

pub async fn replace(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<Option<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE sessions SET revoked_at = $3, replaced = true \
         WHERE id = $2 AND user_id = $1 AND revoked_at IS NULL RETURNING token_hash",
        user_id,
        id,
        at
    )
    .fetch_optional(db)
    .await?)
}

pub async fn is_replaced(db: impl PgExecutor<'_>, hash: &[u8]) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM sessions WHERE token_hash = $1 AND replaced) AS "replaced!""#,
        hash
    )
    .fetch_one(db)
    .await?)
}

pub async fn revoke_all(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    keep: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE sessions SET revoked_at = $3 WHERE user_id = $1 AND revoked_at IS NULL \
         AND ($2::uuid IS NULL OR id <> $2) RETURNING token_hash",
        user_id,
        keep,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn revoke_sso(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE sessions SET revoked_at = $3 WHERE method = 'oidc' AND oidc_org_id = $1 \
         AND ($2::uuid IS NULL OR user_id = $2) AND revoked_at IS NULL RETURNING token_hash",
        org_id,
        user_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn revoke_other_sign_ins_at(
    db: impl PgExecutor<'_>,
    domains: &[String],
    keep: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE sessions s SET revoked_at = $3 FROM users u WHERE u.id = s.user_id \
         AND s.method <> 'oidc' AND s.revoked_at IS NULL AND ($2::uuid IS NULL OR s.id <> $2) \
         AND split_part(email_key(u.email), '@', 2) = ANY($1) RETURNING s.token_hash",
        domains,
        keep,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn list_for_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    now: OffsetDateTime,
    idle_since: OffsetDateTime,
) -> Result<Vec<SessionListing>, StoreError> {
    Ok(sqlx::query_as!(
        SessionListing,
        "SELECT id, method, second_factor, oidc_org_id, created_at, signed_in_at, last_used_at, expires_at, ip, user_agent \
         FROM sessions WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > $2 AND last_used_at > $3 \
         ORDER BY id DESC",
        user_id,
        now,
        idle_since
    )
    .fetch_all(db)
    .await?)
}

pub async fn hashes_for_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT token_hash FROM sessions WHERE user_id = $1 AND revoked_at IS NULL",
        user_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn delete_stale(
    db: impl PgExecutor<'_>,
    now: OffsetDateTime,
    idle_since: OffsetDateTime,
) -> Result<u64, StoreError> {
    let day_ago = now - time::Duration::days(1);
    Ok(sqlx::query!(
        "DELETE FROM sessions WHERE expires_at < $1 OR last_used_at < $2 OR revoked_at < $3",
        now,
        idle_since,
        day_ago
    )
    .execute(db)
    .await?
    .rows_affected())
}
