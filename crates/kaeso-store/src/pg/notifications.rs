use sqlx::PgExecutor;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub kind: String,
    pub delivery: String,
}

pub async fn settings(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Vec<Setting>, StoreError> {
    Ok(sqlx::query_as!(
        Setting,
        "SELECT kind, delivery FROM notification_settings WHERE user_id = $1 ORDER BY kind",
        user_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_delivery(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    kind: &str,
    delivery: &str,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO notification_settings (user_id, kind, delivery) VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, kind) DO UPDATE SET delivery = EXCLUDED.delivery, updated_at = now()",
        user_id,
        kind,
        delivery
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reset_delivery(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    kind: &str,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM notification_settings WHERE user_id = $1 AND kind = $2",
        user_id,
        kind
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn turned_off(
    db: impl PgExecutor<'_>,
    user_ids: &[Uuid],
    kind: &str,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id FROM notification_settings WHERE user_id = ANY($1) AND kind = $2 AND delivery = 'off'",
        user_ids,
        kind
    )
    .fetch_all(db)
    .await?)
}

pub async fn muted_orgs(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT org_id FROM notification_mutes WHERE user_id = $1 ORDER BY org_id",
        user_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn mutes_of(
    db: impl PgExecutor<'_>,
    user_ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT user_id, org_id FROM notification_mutes WHERE user_id = ANY($1)",
        user_ids
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.user_id, r.org_id)).collect())
}

pub async fn set_muted(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    org_id: Uuid,
    muted: bool,
) -> Result<(), StoreError> {
    if muted {
        sqlx::query!(
            "INSERT INTO notification_mutes (user_id, org_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            user_id,
            org_id
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "DELETE FROM notification_mutes WHERE user_id = $1 AND org_id = $2",
            user_id,
            org_id
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

pub async fn deliveries_of(
    db: impl PgExecutor<'_>,
    user_ids: &[Uuid],
    kind: &str,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT user_id, delivery FROM notification_settings WHERE user_id = ANY($1) AND kind = $2",
        user_ids,
        kind
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.user_id, r.delivery)).collect())
}

pub async fn claim_subject(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    kind: &str,
    subject: &str,
    on: time::Date,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO notifications_sent (user_id, kind, subject, sent_on) VALUES ($1, $2, $3, $4) \
         ON CONFLICT DO NOTHING",
        user_id,
        kind,
        subject,
        on
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn forget_sent_before(
    db: impl PgExecutor<'_>,
    on: time::Date,
) -> Result<u64, StoreError> {
    Ok(
        sqlx::query!("DELETE FROM notifications_sent WHERE sent_on < $1", on)
            .execute(db)
            .await?
            .rows_affected(),
    )
}

#[derive(Clone, Debug)]
pub struct DigestEntry<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: &'a str,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub at: time::OffsetDateTime,
}

pub async fn queue(db: impl PgExecutor<'_>, e: &DigestEntry<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO notification_digest (id, user_id, kind, org_id, project_id, payload, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        e.id,
        e.user_id,
        e.kind,
        e.org_id,
        e.project_id,
        e.payload,
        e.at
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Queued {
    pub id: Uuid,
    pub user_id: Uuid,
    pub payload: serde_json::Value,
}

pub async fn waiting_users(
    db: impl PgExecutor<'_>,
    before: time::OffsetDateTime,
    limit: i64,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT DISTINCT user_id FROM notification_digest WHERE sent_at IS NULL AND created_at < $1 \
         ORDER BY user_id LIMIT $2",
        before,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn claim_waiting(
    db: impl PgExecutor<'_>,
    user_ids: &[Uuid],
    before: time::OffsetDateTime,
    at: time::OffsetDateTime,
) -> Result<Vec<Queued>, StoreError> {
    Ok(sqlx::query_as!(
        Queued,
        "UPDATE notification_digest SET sent_at = $3 \
         WHERE user_id = ANY($1) AND sent_at IS NULL AND created_at < $2 \
         RETURNING id, user_id, payload",
        user_ids,
        before,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn forget_digest_before(
    db: impl PgExecutor<'_>,
    at: time::OffsetDateTime,
) -> Result<u64, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM notification_digest WHERE sent_at IS NOT NULL AND sent_at < $1",
        at
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn instance_admins(db: impl PgExecutor<'_>) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM users WHERE is_instance_admin AND state = 'active' AND email IS NOT NULL ORDER BY id"
    )
    .fetch_all(db)
    .await?)
}

pub async fn opted_in(db: impl PgExecutor<'_>, kind: &str) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT s.user_id FROM notification_settings s JOIN users u ON u.id = s.user_id \
         WHERE s.kind = $1 AND s.delivery <> 'off' AND u.state = 'active' ORDER BY s.user_id",
        kind
    )
    .fetch_all(db)
    .await?)
}
