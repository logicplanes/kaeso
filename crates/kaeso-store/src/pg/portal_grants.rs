use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;
use crate::pg::agents::TokenKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub redirect_uri: String,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct NewGrant<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub client_id: Uuid,
    pub redirect_uri: &'a str,
    pub now: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct NewToken<'a> {
    pub id: Uuid,
    pub grant_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: &'a [u8],
    pub family: Uuid,
    pub resource: &'a str,
    pub redirect_uri: Option<&'a str>,
    pub code_challenge: Option<&'a str>,
    pub now: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsedToken {
    pub id: Uuid,
    pub grant_id: Uuid,
    pub family: Uuid,
    pub resource: String,
    pub redirect_uri: Option<String>,
    pub code_challenge: Option<String>,
    pub client_id: Uuid,
    pub fresh: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortalAuth {
    pub grant_id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub resource: String,
    pub token_expires_at: Option<OffsetDateTime>,
    pub user_state: String,
}

pub async fn insert(db: impl PgExecutor<'_>, g: &NewGrant<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO portal_grants (id, user_id, org_id, project_id, client_id, redirect_uri, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        g.id,
        g.user_id,
        g.org_id,
        g.project_id,
        g.client_id,
        g.redirect_uri,
        g.now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn of_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<GrantRow>, StoreError> {
    Ok(sqlx::query_as!(
        GrantRow,
        "SELECT g.id, g.user_id, g.org_id, g.project_id, g.client_id, c.name AS client_name, g.redirect_uri, \
         g.created_at, g.last_used_at, g.revoked_at \
         FROM portal_grants g JOIN agent_clients c ON c.id = g.client_id \
         WHERE g.project_id = $1 AND g.revoked_at IS NULL ORDER BY g.created_at DESC, g.id",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn of_user(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Vec<GrantRow>, StoreError> {
    Ok(sqlx::query_as!(
        GrantRow,
        "SELECT g.id, g.user_id, g.org_id, g.project_id, g.client_id, c.name AS client_name, g.redirect_uri, \
         g.created_at, g.last_used_at, g.revoked_at \
         FROM portal_grants g JOIN agent_clients c ON c.id = g.client_id \
         WHERE g.user_id = $1 AND g.revoked_at IS NULL ORDER BY g.created_at DESC, g.id",
        user_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<GrantRow>, StoreError> {
    Ok(sqlx::query_as!(
        GrantRow,
        "SELECT g.id, g.user_id, g.org_id, g.project_id, g.client_id, c.name AS client_name, g.redirect_uri, \
         g.created_at, g.last_used_at, g.revoked_at \
         FROM portal_grants g JOIN agent_clients c ON c.id = g.client_id \
         WHERE g.project_id = $1 AND g.id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn revoke(
    db: &mut PgConnection,
    project_id: Uuid,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    let revoked = sqlx::query!(
        "UPDATE portal_grants SET revoked_at = $3 WHERE project_id = $1 AND id = $2 AND revoked_at IS NULL",
        project_id,
        id,
        now
    )
    .execute(&mut *db)
    .await?
    .rows_affected()
        == 1;
    if revoked {
        sqlx::query!(
            "UPDATE portal_tokens SET revoked_at = $2 WHERE grant_id = $1 AND revoked_at IS NULL",
            id,
            now
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(revoked)
}

pub async fn revoke_of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    now: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE portal_grants SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL RETURNING id",
        user_id,
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE portal_grants SET last_used_at = $2 \
         WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $2::timestamptz - interval '1 minute')",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn insert_token(db: impl PgExecutor<'_>, t: &NewToken<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO portal_tokens (id, grant_id, kind, token_hash, family, resource, redirect_uri, code_challenge, \
         created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        t.id,
        t.grant_id,
        t.kind.as_str(),
        t.token_hash,
        t.family,
        t.resource,
        t.redirect_uri,
        t.code_challenge,
        t.now,
        t.expires_at
    )
    .execute(db)
    .await?;
    Ok(())
}

// Spent by the UPDATE that marks it, as agent tokens are: of two requests racing with one code
// or refresh token, the loser sees `fresh = false` and its family is revoked.
pub async fn spend(
    db: &mut PgConnection,
    kind: TokenKind,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<UsedToken>, StoreError> {
    let found = sqlx::query!(
        "SELECT t.id, t.grant_id, t.family, t.resource, t.redirect_uri, t.code_challenge, g.client_id \
         FROM portal_tokens t JOIN portal_grants g ON g.id = t.grant_id \
         WHERE t.token_hash = $1 AND t.kind = $2 AND t.revoked_at IS NULL AND g.revoked_at IS NULL \
         AND (t.expires_at IS NULL OR t.expires_at > $3)",
        token_hash,
        kind.as_str(),
        now
    )
    .fetch_optional(&mut *db)
    .await?;
    let Some(found) = found else {
        return Ok(None);
    };
    let spent = sqlx::query!(
        "UPDATE portal_tokens SET used_at = $2 WHERE id = $1 AND used_at IS NULL",
        found.id,
        now
    )
    .execute(&mut *db)
    .await?;
    Ok(Some(UsedToken {
        id: found.id,
        grant_id: found.grant_id,
        family: found.family,
        resource: found.resource,
        redirect_uri: found.redirect_uri,
        code_challenge: found.code_challenge,
        client_id: found.client_id,
        fresh: spent.rows_affected() == 1,
    }))
}

pub async fn revoke_family(
    db: impl PgExecutor<'_>,
    family: Uuid,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE portal_tokens SET revoked_at = $2 WHERE family = $1 AND revoked_at IS NULL",
        family,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke_token(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE portal_tokens SET revoked_at = $2 WHERE token_hash = $1 AND kind <> 'code' \
         AND revoked_at IS NULL RETURNING family",
        token_hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn authenticate(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<PortalAuth>, StoreError> {
    Ok(sqlx::query_as!(
        PortalAuth,
        "SELECT g.id AS grant_id, g.user_id, g.org_id, g.project_id, g.client_id, c.name AS client_name, \
         t.resource, t.expires_at AS token_expires_at, u.state AS user_state \
         FROM portal_tokens t JOIN portal_grants g ON g.id = t.grant_id \
         JOIN agent_clients c ON c.id = g.client_id JOIN users u ON u.id = g.user_id \
         WHERE t.token_hash = $1 AND t.kind = 'access' AND t.revoked_at IS NULL AND g.revoked_at IS NULL \
         AND (t.expires_at IS NULL OR t.expires_at > $2)",
        token_hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn access_hashes(
    db: impl PgExecutor<'_>,
    grant: Uuid,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT token_hash FROM portal_tokens WHERE grant_id = $1 AND kind = 'access'",
        grant
    )
    .fetch_all(db)
    .await?)
}

pub async fn sweep(db: impl PgExecutor<'_>, before: OffsetDateTime) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM portal_tokens WHERE (expires_at IS NOT NULL AND expires_at < $1) \
         OR (revoked_at IS NOT NULL AND revoked_at < $1)",
        before
    )
    .execute(db)
    .await?
    .rows_affected())
}
