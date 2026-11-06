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
