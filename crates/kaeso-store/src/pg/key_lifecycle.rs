use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

pub async fn archive_user_keys_in_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET archived_at = $3, updated_at = now() \
         WHERE org_id = $1 AND user_id = $2 AND archived_at IS NULL RETURNING id",
        org_id,
        user_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn archive_user_keys_in_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET archived_at = $3, updated_at = now() \
         WHERE project_id = $1 AND user_id = $2 AND archived_at IS NULL RETURNING id",
        project_id,
        user_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn archive_user_keys(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET archived_at = $2, updated_at = now() WHERE user_id = $1 AND archived_at IS NULL RETURNING id",
        user_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn archive_project_keys(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET archived_at = $2, updated_at = now() WHERE project_id = $1 AND archived_at IS NULL RETURNING id",
        project_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn archive_service_account_keys(
    db: impl PgExecutor<'_>,
    service_account_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET archived_at = $2, updated_at = now() WHERE service_account_id = $1 AND archived_at IS NULL RETURNING id",
        service_account_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn disable_for_deactivation(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET disabled_at = $2, disabled_reason = 'user_deactivated', updated_at = now() \
         WHERE user_id = $1 AND disabled_at IS NULL AND archived_at IS NULL RETURNING id",
        user_id,
        at
    )
    .fetch_all(db)
    .await?)
}

pub async fn enable_after_reactivation(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE api_keys SET disabled_at = NULL, disabled_reason = NULL, updated_at = now() \
         WHERE user_id = $1 AND disabled_reason = 'user_deactivated' AND archived_at IS NULL RETURNING id",
        user_id
    )
    .fetch_all(db)
    .await?)
}
