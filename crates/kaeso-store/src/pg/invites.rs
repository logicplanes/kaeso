use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct Invite {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub email: String,
    pub org_preset: Option<String>,
    pub org_role_id: Option<Uuid>,
    pub project_roles: Value,
    pub invited_by: Option<Uuid>,
    pub expires_at: OffsetDateTime,
    pub accepted_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct NewInvite<'a> {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub email: &'a str,
    pub org_preset: Option<&'a str>,
    pub org_role_id: Option<Uuid>,
    pub project_roles: Value,
    pub token_hash: &'a [u8],
    pub invited_by: Option<Uuid>,
    pub expires_at: OffsetDateTime,
}

pub async fn insert(db: impl PgExecutor<'_>, i: &NewInvite<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO invites (id, org_id, email, org_preset, project_roles, token_hash, invited_by, expires_at, org_role_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        i.id,
        i.org_id,
        i.email,
        i.org_preset,
        i.project_roles,
        i.token_hash,
        i.invited_by,
        i.expires_at,
        i.org_role_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    id: Uuid,
) -> Result<Option<Invite>, StoreError> {
    Ok(sqlx::query_as!(
        Invite,
        "SELECT id, org_id, email, org_preset, org_role_id, project_roles, invited_by, expires_at, accepted_at, revoked_at, created_at \
         FROM invites WHERE id = $2 AND org_id IS NOT DISTINCT FROM $1",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_pending_by_hash(
    db: impl PgExecutor<'_>,
    hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Invite>, StoreError> {
    Ok(sqlx::query_as!(
        Invite,
        "SELECT id, org_id, email, org_preset, org_role_id, project_roles, invited_by, expires_at, accepted_at, revoked_at, created_at \
         FROM invites WHERE token_hash = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > $2",
        hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn pending_for(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    email: &str,
    now: OffsetDateTime,
) -> Result<Option<Invite>, StoreError> {
    let found = sqlx::query_as!(
        Invite,
        "SELECT id, org_id, email, org_preset, org_role_id, project_roles, invited_by, expires_at, accepted_at, revoked_at, created_at \
         FROM invites WHERE org_id = $1 AND email_key(email) = email_key($2) AND accepted_at IS NULL AND revoked_at IS NULL \
         AND expires_at > $3 ORDER BY id DESC",
        org_id,
        email,
        now
    )
    .fetch_all(db)
    .await?;
    Ok(found
        .into_iter()
        .find(|i| kaeso_core::email::same_address(&i.email, email)))
}

pub async fn pending_of_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    now: OffsetDateTime,
) -> Result<Vec<Invite>, StoreError> {
    Ok(sqlx::query_as!(
        Invite,
        "SELECT id, org_id, email, org_preset, org_role_id, project_roles, invited_by, expires_at, accepted_at, revoked_at, created_at \
         FROM invites WHERE org_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > $2",
        org_id,
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    pending_only: bool,
    now: OffsetDateTime,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Invite>, StoreError> {
    Ok(sqlx::query_as!(
        Invite,
        "SELECT id, org_id, email, org_preset, org_role_id, project_roles, invited_by, expires_at, accepted_at, revoked_at, created_at \
         FROM invites WHERE org_id IS NOT DISTINCT FROM $1 \
         AND (NOT $2 OR (accepted_at IS NULL AND revoked_at IS NULL AND expires_at > $3)) \
         AND ($4::uuid IS NULL OR id > $4) ORDER BY id LIMIT $5",
        org_id,
        pending_only,
        now,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn accept(
    db: impl PgExecutor<'_>,
    id: Uuid,
    user_id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE invites SET accepted_at = $3, accepted_by = $2, updated_at = now() \
         WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > $3",
        id,
        user_id,
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn revoke(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE invites SET revoked_at = $3, updated_at = now() \
         WHERE id = $2 AND org_id IS NOT DISTINCT FROM $1 AND accepted_at IS NULL AND revoked_at IS NULL",
        org_id,
        id,
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn revoke_others_for(
    db: &mut sqlx::PgConnection,
    org_id: Uuid,
    email: &str,
    accepted: Uuid,
    now: OffsetDateTime,
) -> Result<u64, StoreError> {
    let others: Vec<Uuid> = sqlx::query!(
        "SELECT id, email FROM invites WHERE org_id = $1 AND email_key(email) = email_key($2) \
         AND id <> $3 AND accepted_at IS NULL AND revoked_at IS NULL FOR UPDATE",
        org_id,
        email,
        accepted
    )
    .fetch_all(&mut *db)
    .await?
    .into_iter()
    .filter(|i| kaeso_core::email::same_address(&i.email, email))
    .map(|i| i.id)
    .collect();
    Ok(sqlx::query!(
        "UPDATE invites SET revoked_at = $2, updated_at = now() WHERE id = ANY($1)",
        &others,
        now
    )
    .execute(&mut *db)
    .await?
    .rows_affected())
}

pub async fn renew(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    id: Uuid,
    hash: &[u8],
    expires_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE invites SET token_hash = $3, expires_at = $4, updated_at = now() \
         WHERE id = $2 AND org_id IS NOT DISTINCT FROM $1 AND accepted_at IS NULL AND revoked_at IS NULL",
        org_id,
        id,
        hash,
        expires_at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn delete_stale(db: impl PgExecutor<'_>, now: OffsetDateTime) -> Result<u64, StoreError> {
    let month_ago = now - time::Duration::days(30);
    Ok(sqlx::query!(
        "DELETE FROM invites WHERE expires_at < $1 OR accepted_at < $1 OR revoked_at < $1",
        month_ago
    )
    .execute(db)
    .await?
    .rows_affected())
}
