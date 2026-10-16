use kaeso_core::id::UserId;
use kaeso_store::StoreError;
use kaeso_store::pg::{totp, users};

use kaeso_mail::Actor;

use crate::auth::sessions;
use crate::services::mail_lang;
use crate::state::AppState;

pub async fn reset_2fa<'a>(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    user: UserId,
    by: impl Into<Actor<'a>>,
) -> Result<bool, StoreError> {
    reset_2fa_requested(state, tx, user, by, None).await
}

pub async fn reset_2fa_requested<'a>(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    user: UserId,
    by: impl Into<Actor<'a>>,
    accept_language: Option<&str>,
) -> Result<bool, StoreError> {
    let had = totp::remove(&mut *tx, user.uuid()).await?;
    sessions::revoke_all(state, user, None).await?;
    if let Some(email) = users::get(&mut *tx, user.uuid())
        .await?
        .and_then(|u| u.email)
    {
        let lang = mail_lang::of_user(&mut *tx, user.uuid(), accept_language).await;
        state
            .mailer
            .send(kaeso_mail::templates::two_factor_reset(lang, &email, by));
    }
    Ok(had)
}

#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("{0}")]
    LastOwner(String),
    #[error("{0}")]
    Store(#[from] StoreError),
}

impl From<LifecycleError> for crate::platform::error::ApiError {
    fn from(e: LifecycleError) -> Self {
        match e {
            LifecycleError::LastOwner(m) => crate::platform::error::ApiError::Conflict(m),
            LifecycleError::Store(e) => e.into(),
        }
    }
}

async fn ensure_not_last_owner(
    tx: &mut sqlx::PgConnection,
    user: UserId,
    doing: &str,
) -> Result<(), LifecycleError> {
    let orgs = kaeso_store::pg::members::orgs_where_last_owner(&mut *tx, user.uuid()).await?;
    if orgs.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = orgs.into_iter().map(|(_, n)| n).collect();
    Err(LifecycleError::LastOwner(format!(
        "Transfer ownership of {} before you {doing}.",
        names.join(", ")
    )))
}

pub async fn deactivate(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    user: UserId,
    at: time::OffsetDateTime,
    accept_language: Option<&str>,
) -> Result<Vec<uuid::Uuid>, LifecycleError> {
    users::set_state(&mut *tx, user.uuid(), "deactivated").await?;
    let keys =
        kaeso_store::pg::key_lifecycle::disable_for_deactivation(&mut *tx, user.uuid(), at).await?;
    sessions::revoke_all(state, user, None).await?;
    if let Some(email) = users::get(&mut *tx, user.uuid())
        .await?
        .and_then(|u| u.email)
    {
        let lang = mail_lang::of_user(&mut *tx, user.uuid(), accept_language).await;
        state
            .mailer
            .send(kaeso_mail::templates::account_deactivated(lang, &email));
    }
    Ok(keys)
}

pub async fn reactivate(
    tx: &mut sqlx::PgConnection,
    user: UserId,
) -> Result<Vec<uuid::Uuid>, LifecycleError> {
    users::set_state(&mut *tx, user.uuid(), "active").await?;
    Ok(kaeso_store::pg::key_lifecycle::enable_after_reactivation(&mut *tx, user.uuid()).await?)
}

pub async fn delete(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    user: UserId,
    at: time::OffsetDateTime,
) -> Result<Vec<uuid::Uuid>, LifecycleError> {
    ensure_not_last_owner(&mut *tx, user, "delete the account").await?;
    let keys = kaeso_store::pg::key_lifecycle::archive_user_keys(&mut *tx, user.uuid(), at).await?;
    kaeso_store::pg::members::delete_all_of_user(&mut *tx, user.uuid()).await?;
    totp::remove(&mut *tx, user.uuid()).await?;
    sqlx::query!("DELETE FROM sign_in_tokens WHERE user_id = $1", user.uuid())
        .execute(&mut *tx)
        .await
        .map_err(StoreError::from)?;
    users::delete_personal_data(&mut *tx, user.uuid(), at).await?;
    sessions::revoke_all(state, user, None).await?;
    Ok(keys)
}

pub async fn ensure_can_leave(
    tx: &mut sqlx::PgConnection,
    org: uuid::Uuid,
    user: UserId,
) -> Result<(), LifecycleError> {
    let orgs = kaeso_store::pg::members::orgs_where_last_owner(&mut *tx, user.uuid()).await?;
    if orgs.iter().any(|(id, _)| *id == org) {
        return Err(LifecycleError::LastOwner(
            "You are this organization's last owner; transfer ownership before leaving.".into(),
        ));
    }
    Ok(())
}
