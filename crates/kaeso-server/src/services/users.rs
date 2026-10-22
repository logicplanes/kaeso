use kaeso_core::id::UserId;
use kaeso_store::StoreError;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorReset {
    UnknownEmail,
    Removed,
    NothingToRemove,
}

pub async fn operator_reset_2fa(
    state: &AppState,
    email: &str,
) -> Result<OperatorReset, crate::platform::error::ApiError> {
    let Some(user) = users::get_by_email(&state.pg, email).await? else {
        return Ok(OperatorReset::UnknownEmail);
    };
    let id = UserId::from_uuid(user.id);
    let mut tx = state.pg.begin().await?;
    let had = reset_2fa(state, &mut tx, id, Actor::Operator).await?;
    crate::platform::audit::system(
        &mut tx,
        None,
        "user.2fa_reset",
        Some(("user", id.to_string())),
        None,
        state.clock.now(),
    )
    .await?;
    tx.commit().await?;
    Ok(if had {
        OperatorReset::Removed
    } else {
        OperatorReset::NothingToRemove
    })
}

pub const OPERATOR_LINK: &str = "operator";

pub enum OperatorSignIn {
    UnknownEmail,
    Deactivated,
    Link(secrecy::SecretString),
}

impl std::fmt::Debug for OperatorSignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperatorSignIn::UnknownEmail => f.write_str("UnknownEmail"),
            OperatorSignIn::Deactivated => f.write_str("Deactivated"),
            OperatorSignIn::Link(_) => f.write_str("Link(…)"),
        }
    }
}

pub async fn operator_sign_in_link(
    state: &AppState,
    email: &str,
) -> Result<OperatorSignIn, crate::platform::error::ApiError> {
    let Ok(email) = kaeso_core::email::normalize(email) else {
        return Ok(OperatorSignIn::UnknownEmail);
    };
    let Some(user) = users::get_by_email(&state.pg, &email).await? else {
        return Ok(OperatorSignIn::UnknownEmail);
    };
    if user.state != "active" {
        return Ok(OperatorSignIn::Deactivated);
    }
    let id = UserId::from_uuid(user.id);
    let now = state.clock.now();
    let token = kaeso_crypto::random_token();
    let mut tx = state.pg.begin().await?;
    tokens::insert(
        &mut *tx,
        NewToken {
            kind: TokenKind::MagicLink,
            hash: &kaeso_core::secret::hash(&token),
            user_id: Some(user.id),
            email: user.email.as_deref(),
            data: serde_json::json!({ "issued_by": OPERATOR_LINK }),
            expires_at: now + crate::platform::auth::magic::LIFETIME,
        },
    )
    .await?;
    crate::platform::audit::system(
        &mut tx,
        None,
        "user.sign_in_link",
        Some(("user", id.to_string())),
        None,
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(OperatorSignIn::Link(secrecy::SecretString::from(format!(
        "{}auth/magic-link#token={token}",
        state.config.public_url
    ))))
}

#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("{0}")]
    LastHolder(String),
    #[error("{0}")]
    Store(#[from] StoreError),
}

impl From<LifecycleError> for crate::platform::error::ApiError {
    fn from(e: LifecycleError) -> Self {
        match e {
            LifecycleError::LastHolder(m) => crate::platform::error::ApiError::Conflict(m),
            LifecycleError::Store(e) => e.into(),
        }
    }
}

pub const LAST_INSTANCE_ADMIN: &str =
    "This is the last instance admin; make someone else an instance admin first.";

pub async fn ensure_not_last_instance_admin(
    tx: &mut sqlx::PgConnection,
    user: UserId,
) -> Result<(), LifecycleError> {
    users::lock_instance_admins(&mut *tx).await?;
    let admin = users::get(&mut *tx, user.uuid())
        .await?
        .is_some_and(|u| u.is_instance_admin && u.state == "active");
    if admin && users::count_instance_admins(&mut *tx).await? <= 1 {
        return Err(LifecycleError::LastHolder(LAST_INSTANCE_ADMIN.to_owned()));
    }
    Ok(())
}

async fn ensure_not_last_owner(
    tx: &mut sqlx::PgConnection,
    user: UserId,
    doing: &str,
) -> Result<(), LifecycleError> {
    kaeso_store::pg::members::lock_owners_of_user(&mut *tx, user.uuid()).await?;
    let orgs = kaeso_store::pg::members::orgs_where_last_owner(&mut *tx, user.uuid()).await?;
    if orgs.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = orgs.into_iter().map(|(_, n)| n).collect();
    Err(LifecycleError::LastHolder(format!(
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
    ensure_not_last_instance_admin(&mut *tx, user).await?;
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
    ensure_not_last_instance_admin(&mut *tx, user).await?;
    let keys = kaeso_store::pg::key_lifecycle::archive_user_keys(&mut *tx, user.uuid(), at).await?;
    kaeso_store::pg::members::delete_all_of_user(&mut *tx, user.uuid()).await?;
    totp::remove(&mut *tx, user.uuid()).await?;
    kaeso_store::pg::tokens::delete_of_user(&mut *tx, user.uuid()).await?;
    if let Some(avatar) = users::delete_personal_data(&mut *tx, user.uuid(), at).await? {
        kaeso_store::pg::avatars::delete_if_unused(&mut *tx, &avatar).await?;
    }
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
        return Err(LifecycleError::LastHolder(
            "You are this organization's last owner; transfer ownership before leaving.".into(),
        ));
    }
    Ok(())
}
