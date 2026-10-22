use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

pub async fn user_of(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    issuer: &str,
    subject: &str,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id FROM sso_identities WHERE org_id = $1 AND issuer = $2 AND subject = $3 FOR SHARE",
        org_id,
        issuer,
        subject
    )
    .fetch_optional(db)
    .await?)
}

pub async fn bind_pending(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    issuer: &str,
    subject: &str,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE sso_identities SET issuer = $3, subject = $4, linked_at = $5 \
         WHERE org_id = $1 AND user_id = $2 AND subject IS NULL",
        org_id,
        user_id,
        issuer,
        subject,
        now
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn link(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    issuer: &str,
    subject: &str,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO sso_identities (org_id, user_id, issuer, subject, linked_at) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (org_id, user_id) DO UPDATE SET issuer = $3, subject = $4, linked_at = $5",
        org_id,
        user_id,
        issuer,
        subject,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn unlink(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM sso_identities WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

#[derive(Clone, Debug)]
pub struct Linkable {
    pub org_id: Uuid,
    pub org_name: String,
    pub provider_name: Option<String>,
    pub linked_at: Option<OffsetDateTime>,
    pub sends_auth_time: Option<bool>,
}

pub async fn linkable(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Vec<Linkable>, StoreError> {
    Ok(sqlx::query_as!(
        Linkable,
        "SELECT o.id AS org_id, o.name AS org_name, c.display_name AS provider_name, i.linked_at, \
         c.sends_auth_time \
         FROM org_memberships m \
         JOIN organizations o ON o.id = m.org_id \
         JOIN org_oidc c ON c.org_id = m.org_id \
         LEFT JOIN sso_identities i ON i.org_id = m.org_id AND i.user_id = m.user_id AND i.issuer = c.issuer \
         WHERE m.user_id = $1 \
         ORDER BY o.name, o.id",
        user_id
    )
    .fetch_all(db)
    .await?)
}
