use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct Totp {
    pub secret_ciphertext: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
    pub enabled_at: Option<OffsetDateTime>,
    pub last_used_step: Option<i64>,
}

pub async fn get(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Option<Totp>, StoreError> {
    Ok(sqlx::query_as!(
        Totp,
        "SELECT secret_ciphertext, secret_nonce, wrapped_key, key_id, enabled_at, last_used_step \
         FROM user_totp WHERE user_id = $1",
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn is_enabled(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM user_totp WHERE user_id = $1 AND enabled_at IS NOT NULL) AS \"e!\"",
        user_id
    )
    .fetch_one(db)
    .await?)
}

pub async fn enabled_among(
    db: impl PgExecutor<'_>,
    user_ids: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT user_id FROM user_totp WHERE user_id = ANY($1) AND enabled_at IS NOT NULL",
        user_ids
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_pending(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    ciphertext: &[u8],
    nonce: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "INSERT INTO user_totp (user_id, secret_ciphertext, secret_nonce, wrapped_key, key_id) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id) DO UPDATE SET secret_ciphertext = $2, secret_nonce = $3, wrapped_key = $4, \
         key_id = $5, last_used_step = NULL, updated_at = now() WHERE user_totp.enabled_at IS NULL",
        user_id,
        ciphertext,
        nonce,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn use_step(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    step: i64,
    confirm: Option<OffsetDateTime>,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "UPDATE user_totp SET last_used_step = $2, enabled_at = COALESCE(enabled_at, $3), updated_at = now() \
         WHERE user_id = $1 AND (last_used_step IS NULL OR last_used_step < $2) \
         AND (enabled_at IS NULL) = ($3::timestamptz IS NOT NULL)",
        user_id,
        step,
        confirm
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn failed_codes(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<i32, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT failed_codes FROM user_totp WHERE user_id = $1",
        user_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(0))
}

pub async fn count_failed_code(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<i32, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE user_totp SET failed_codes = failed_codes + 1, updated_at = now() WHERE user_id = $1 \
         RETURNING failed_codes",
        user_id
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(0))
}

pub async fn clear_failed_codes(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE user_totp SET failed_codes = 0, updated_at = now() WHERE user_id = $1 AND failed_codes > 0",
        user_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove(db: &mut sqlx::PgConnection, user_id: Uuid) -> Result<bool, StoreError> {
    sqlx::query!(
        "DELETE FROM user_recovery_codes WHERE user_id = $1",
        user_id
    )
    .execute(&mut *db)
    .await?;
    let done = sqlx::query!("DELETE FROM user_totp WHERE user_id = $1", user_id)
        .execute(&mut *db)
        .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn replace_recovery_codes(
    db: &mut sqlx::PgConnection,
    user_id: Uuid,
    hashes: &[[u8; 32]],
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM user_recovery_codes WHERE user_id = $1",
        user_id
    )
    .execute(&mut *db)
    .await?;
    for h in hashes {
        sqlx::query!(
            "INSERT INTO user_recovery_codes (id, user_id, code_hash) VALUES ($1, $2, $3)",
            Uuid::now_v7(),
            user_id,
            h.as_slice()
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn use_recovery_code(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    hash: &[u8],
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "UPDATE user_recovery_codes SET used_at = $3 WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL",
        user_id,
        hash,
        at
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn remaining_recovery_codes(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM user_recovery_codes WHERE user_id = $1 AND used_at IS NULL",
        user_id
    )
    .fetch_one(db)
    .await?)
}

pub async fn all_sealed(db: impl PgExecutor<'_>) -> Result<Vec<(Uuid, Totp)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT user_id, secret_ciphertext, secret_nonce, wrapped_key, key_id, enabled_at, last_used_step FROM user_totp"
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.user_id,
                Totp {
                    secret_ciphertext: r.secret_ciphertext,
                    secret_nonce: r.secret_nonce,
                    wrapped_key: r.wrapped_key,
                    key_id: r.key_id,
                    enabled_at: r.enabled_at,
                    last_used_step: r.last_used_step,
                },
            )
        })
        .collect())
}

pub async fn set_wrapped_key(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    old_wrapped_key: &[u8],
    wrapped_key: &[u8],
    key_id: &str,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE user_totp SET wrapped_key = $3, key_id = $4, updated_at = now() \
         WHERE user_id = $1 AND wrapped_key = $2",
        user_id,
        old_wrapped_key,
        wrapped_key,
        key_id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}
