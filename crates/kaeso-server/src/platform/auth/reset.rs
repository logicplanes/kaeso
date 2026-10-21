use axum::Json;
use axum::extract::State;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::users;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::magic::LIFETIME;
use super::setup::hash_password;
use super::{EmailSent, throttle_ip};
use crate::auth::sessions;
use crate::auth::throttle::{self, EMAILS_PER_ADDRESS, EMAILS_PER_IP, TOKENS_PER_IP};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The address to send a reset link to.")]
pub struct ResetRequest {
    #[doc = "The account's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
}

fn passwords_on(state: &AppState) -> ApiResult<()> {
    if state.config.auth.password {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "Password sign-in is turned off on this install.".into(),
        ))
    }
}

#[utoipa::path(
    post,
    path = "/auth/password-reset",
    summary = "Sends a reset link if the address has an active account. Always answers\n\
    the same way.",
    tag = "auth",
    request_body = ResetRequest,
    responses((status = 200, description = "Accepted; an email was sent if the account exists.", body = EmailSent))
)]
pub async fn request(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ResetRequest>,
) -> ApiResult<Json<EmailSent>> {
    passwords_on(&state)?;
    throttle_ip(&state, &ctx, EMAILS_PER_IP).await?;
    let answer = Json(EmailSent::new("a password reset link"));
    let Ok(email) = kaeso_core::email::normalize(&req.email) else {
        return Ok(answer);
    };
    if throttle::hit(&state, EMAILS_PER_ADDRESS, &email)
        .await
        .is_err()
    {
        return Ok(answer);
    }
    let Some(user) = users::get_by_email(&state.pg, &email)
        .await?
        .filter(|u| u.state == "active")
    else {
        return Ok(answer);
    };
    let token = random_token();
    tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::PasswordReset,
            hash: &secret::hash(&token),
            user_id: Some(user.id),
            email: Some(&email),
            data: serde_json::json!({}),
            expires_at: ctx.now + LIFETIME,
        },
    )
    .await?;
    let url = format!(
        "{}auth/password-reset#token={token}",
        state.config.public_url
    );
    let lang =
        crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
            .await;
    state
        .mailer
        .send(kaeso_mail::templates::password_reset(lang, &email, &url));
    Ok(answer)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The token from a reset link.")]
pub struct ResetLookupRequest {
    #[doc = "The token from `…/auth/password-reset#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The account whose password a reset link sets.")]
pub struct ResetAccount {
    #[doc = "The account's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
    #[doc = "When the link expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: time::OffsetDateTime,
}

fn invalid_link() -> ApiError {
    ApiError::Authentication(
        "This reset link is invalid, used or expired; request a new one.".into(),
    )
}

#[utoipa::path(
    post,
    path = "/auth/password-reset/lookup",
    summary = "Names the account a reset link sets the password of, without using the\n\
    link, so the console can show the address and offer it to password managers.",
    tag = "auth",
    request_body = ResetLookupRequest,
    responses((status = 200, body = ResetAccount))
)]
pub async fn lookup(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ResetLookupRequest>,
) -> ApiResult<Json<ResetAccount>> {
    passwords_on(&state)?;
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let hash = secret::hash(req.token.expose_secret());
    let token = tokens::peek(&state.pg, TokenKind::PasswordReset, &hash, ctx.now)
        .await?
        .ok_or_else(invalid_link)?;
    let user = match token.user_id {
        Some(id) => users::get(&state.pg, id)
            .await?
            .filter(|u| u.state == "active"),
        None => None,
    };
    let email = user.and_then(|u| u.email).ok_or_else(invalid_link)?;
    Ok(Json(ResetAccount {
        email,
        expires_at: token.expires_at,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The token and the new password.")]
pub struct ResetVerifyRequest {
    #[doc = "The token from `…/auth/password-reset#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
    #[doc = "The new password, 12 to 128 characters."]
    #[schema(value_type = String, format = Password, write_only)]
    pub password: SecretString,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The password was changed.")]
pub struct PasswordChanged {
    #[doc = "Always `password_changed`."]
    #[schema(example = "password_changed")]
    pub status: &'static str,
}

#[utoipa::path(
    post,
    path = "/auth/password-reset/verify",
    summary = "Sets a new password with a reset link and ends every session of the\n\
    user. The link works once.",
    tag = "auth",
    request_body = ResetVerifyRequest,
    responses((status = 200, description = "The password is changed; sign in with it.", body = PasswordChanged))
)]
pub async fn verify(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ResetVerifyRequest>,
) -> ApiResult<Json<PasswordChanged>> {
    passwords_on(&state)?;
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let password_hash = hash_password("password", req.password).await?;
    let hash = secret::hash(req.token.expose_secret());
    let mut tx = state.pg.begin().await?;
    let token = tokens::consume(&mut *tx, TokenKind::PasswordReset, &hash, ctx.now)
        .await?
        .ok_or_else(invalid_link)?;
    let user_id = token
        .user_id
        .map(UserId::from_uuid)
        .ok_or_else(|| ApiError::internal("token without user"))?;
    let user = users::get(&mut *tx, user_id.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::PermissionDenied("This account is deactivated.".into()))?;
    users::set_password(&mut *tx, user.id, &password_hash).await?;
    users::mark_verified(&mut *tx, user.id, ctx.now).await?;
    for kind in [
        TokenKind::PasswordReset,
        TokenKind::MagicLink,
        TokenKind::TotpChallenge,
    ] {
        tokens::retire_all(&mut *tx, kind, Some(user.id), ctx.now).await?;
    }
    ctx.audit(None, "user.password_reset")
        .by(user_id)
        .target("user", user_id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    sessions::revoke_all(&state, user_id, None).await?;
    if let Some(email) = &user.email {
        let lang =
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await;
        state
            .mailer
            .send(kaeso_mail::templates::password_changed(lang, email));
    }
    Ok(Json(PasswordChanged {
        status: "password_changed",
    }))
}
