use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{SessionId, UserId};
use kaeso_store::pg::tokens::{self, TokenKind};
use kaeso_store::pg::{members, totp, users};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::sessions::{self, Method, SignIn};
use crate::auth::throttle::{self, SIGN_IN_PER_ACCOUNT};
use crate::auth::totp::{SecondFactorError, aad, check_code, new_recovery_codes};
use crate::platform::access::Ctx;
use crate::platform::auth::password::account_attempt;
use crate::platform::auth::setup::hash_password;
use crate::platform::auth::start_session;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::state::AppState;

fn method_of(name: &str) -> Method {
    match name {
        "magic_link" => Method::MagicLink,
        "oidc" => Method::Oidc,
        "setup" => Method::Setup,
        "signup" => Method::Signup,
        _ => Method::Password,
    }
}

async fn rotate(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    second_factor: bool,
    end_all: bool,
) -> ApiResult<Response> {
    let session = ctx.session()?.clone();
    let user_id = UserId::from_uuid(user.id);
    if end_all {
        sessions::revoke_all(state, user_id, None).await?;
    } else {
        sessions::revoke(state, user_id, SessionId::from_uuid(session.id)).await?;
    }
    start_session(
        state,
        ctx,
        user,
        SignIn {
            user_id,
            method: method_of(&session.method),
            second_factor,
            oidc_org: session.oidc_org_id,
            signed_in_at: session.signed_in_at,
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new password.")]
pub struct PasswordChange {
    #[doc = "The current password, required when one is set."]
    #[schema(value_type = Option<String>, format = Password, write_only)]
    pub current_password: Option<SecretString>,
    #[doc = "The new password, 12 to 128 characters."]
    #[schema(value_type = String, format = Password, write_only)]
    pub new_password: SecretString,
}

#[utoipa::path(
    post,
    path = "/me/password",
    summary = "Sets or changes the password. Ends every session of the user; the\n\
    caller gets a new session (new cookie). Without a current password (a\n\
    user who signs in by sign-in link or SSO), a recent sign-in is needed.",
    tag = "me",
    request_body = PasswordChange,
    responses((status = 200, description = "Changed; a new session cookie is set.", body = crate::platform::auth::SignedIn))
)]
pub async fn change_password(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<PasswordChange>,
) -> ApiResult<Response> {
    let user_id = ctx.user_id()?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    match (&user.password_hash, req.current_password) {
        (Some(stored), Some(current)) => {
            let attempts = user_id.to_string();
            account_attempt(&state, &attempts).await?;
            let stored = stored.clone();
            let ok = tokio::task::spawn_blocking(move || {
                kaeso_crypto::password::verify(&current, &stored)
            })
            .await
            .map_err(ApiError::internal)?;
            if !ok {
                return Err(ApiError::invalid(
                    "current_password",
                    "The current password is wrong.",
                ));
            }
            throttle::clear(&state, SIGN_IN_PER_ACCOUNT, &attempts).await;
        }
        (Some(_), None) => {
            return Err(ApiError::invalid(
                "current_password",
                "Enter your current password.",
            ));
        }
        (None, _) => ctx.require_recent_sign_in()?,
    }
    let hash = hash_password("new_password", req.new_password).await?;
    let mut tx = state.pg.begin().await?;
    users::set_password(&mut *tx, user.id, &hash).await?;
    for kind in [TokenKind::PasswordReset, TokenKind::TotpChallenge] {
        tokens::retire_all(&mut *tx, kind, Some(user.id), ctx.now).await?;
    }
    ctx.audit(None, "user.password_change")
        .target("user", user_id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    if let Some(email) = &user.email {
        let lang =
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await;
        state
            .mailer
            .send(kaeso_mail::templates::password_changed(lang, email));
    }
    let second_factor = ctx.session()?.second_factor;
    rotate(&state, &ctx, &user, second_factor, true).await
}

#[derive(Serialize, ToSchema)]
#[schema(description = "A TOTP secret waiting for its first code.")]
pub struct TotpSetup {
    #[doc = "The secret in base32, to type into an authenticator app."]
    #[schema(example = "JBSWY3DPEHPK3PXP")]
    pub secret: String,
    #[doc = "The `otpauth://` URI, for a QR code."]
    pub otpauth_uri: String,
}

#[utoipa::path(
    post,
    path = "/me/totp",
    summary = "Starts TOTP setup: a new secret, confirmed with `me/totp/confirm`.\n\
    Refused while TOTP is on.",
    tag = "me",
    responses((status = 200, body = TotpSetup))
)]
pub async fn totp_begin(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<TotpSetup>> {
    let user_id = ctx.user_id()?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    let secret = kaeso_crypto::totp::new_secret();
    let sealed = kaeso_crypto::seal(
        &state.keyring,
        secrecy::ExposeSecret::expose_secret(&secret),
        &aad(user_id),
    )
    .map_err(ApiError::internal)?;
    let stored = totp::set_pending(
        &state.pg,
        user_id.uuid(),
        &sealed.ciphertext,
        &sealed.nonce,
        &sealed.wrapped_key,
        &sealed.key_id,
    )
    .await?;
    if !stored {
        return Err(ApiError::Conflict(
            "Two-factor authentication is already on; turn it off to set it up again.".into(),
        ));
    }
    let account = user.email.unwrap_or_else(|| user_id.to_string());
    Ok(Json(TotpSetup {
        secret: kaeso_crypto::totp::base32(&secret),
        otpauth_uri: kaeso_crypto::totp::uri(&secret, &account).map_err(ApiError::internal)?,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The first code from the authenticator app.")]
pub struct TotpConfirm {
    #[doc = "A 6-digit code."]
    #[schema(example = "123456")]
    pub code: String,
}

#[derive(Serialize, ToSchema)]
#[schema(description = "TOTP is on.")]
pub struct TotpEnabled {
    #[doc = "Ten one-time recovery codes. Shown once; store them safely."]
    pub recovery_codes: Vec<String>,
}

impl std::fmt::Debug for TotpSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TotpSetup").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for TotpEnabled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TotpEnabled")
            .field("recovery_codes", &self.recovery_codes.len())
            .finish()
    }
}

#[utoipa::path(
    post,
    path = "/me/totp/confirm",
    summary = "Confirms TOTP setup with a code and returns ten recovery codes. The\n\
    session gets a new id and counts as signed in with a second factor.",
    tag = "me",
    request_body = TotpConfirm,
    responses((status = 200, description = "On; a new session cookie is set.", body = TotpEnabled))
)]
pub async fn totp_confirm(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<TotpConfirm>,
) -> ApiResult<Response> {
    let user_id = ctx.user_id()?;
    match check_code(&state, user_id, &req.code, true).await {
        Ok(()) => {}
        Err(SecondFactorError::Invalid) => {
            return Err(ApiError::invalid(
                "code",
                "That code isn't valid; check your device's clock and try the next one.",
            ));
        }
        Err(e) => return Err(ApiError::internal(e)),
    }
    let (codes, hashes) = new_recovery_codes();
    let mut tx = state.pg.begin().await?;
    totp::replace_recovery_codes(&mut tx, user_id.uuid(), &hashes).await?;
    ctx.audit(None, "user.totp_enable")
        .target("user", user_id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    if let Some(email) = &user.email {
        let lang =
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await;
        state
            .mailer
            .send(kaeso_mail::templates::two_factor_enabled(lang, email));
    }
    let signed_in = rotate(&state, &ctx, &user, true, false).await?;
    let (parts, _) = signed_in.into_parts();
    Ok((
        parts.headers,
        Json(TotpEnabled {
            recovery_codes: codes,
        }),
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/me/totp",
    summary = "Turns TOTP off. Refused while an org of the user requires 2FA. Needs a\n\
    recent sign-in.",
    tag = "me",
    responses((status = 204, description = "Off."))
)]
pub async fn totp_disable(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Response> {
    ctx.require_recent_sign_in()?;
    let user_id = ctx.user_id()?;
    let orgs = members::orgs_of_user(&state.pg, user_id.uuid()).await?;
    if let Some(org) = orgs.iter().find(|o| o.require_2fa) {
        return Err(ApiError::Conflict(format!(
            "The organization {} requires two-factor authentication, so it cannot be turned off.",
            org.org_name
        )));
    }
    let mut tx = state.pg.begin().await?;
    if !totp::remove(&mut tx, user_id.uuid()).await? {
        return Err(ApiError::not_found("TOTP setup"));
    }
    ctx.audit(None, "user.totp_disable")
        .target("user", user_id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    if let Some(email) = &user.email {
        let lang =
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await;
        state
            .mailer
            .send(kaeso_mail::templates::two_factor_disabled(lang, email));
    }
    let signed_in = rotate(&state, &ctx, &user, false, false).await?;
    let (parts, _) = signed_in.into_parts();
    Ok((StatusCode::NO_CONTENT, parts.headers).into_response())
}

#[utoipa::path(
    post,
    path = "/me/recovery-codes",
    summary = "Replaces the recovery codes with ten new ones. Needs TOTP on and a\n\
    recent sign-in.",
    tag = "me",
    responses((status = 200, body = TotpEnabled))
)]
pub async fn recovery_codes(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<TotpEnabled>> {
    ctx.require_recent_sign_in()?;
    let user_id = ctx.user_id()?;
    if !totp::is_enabled(&state.pg, user_id.uuid()).await? {
        return Err(ApiError::Conflict(
            "Turn on two-factor authentication first.".into(),
        ));
    }
    let (codes, hashes) = new_recovery_codes();
    let mut tx = state.pg.begin().await?;
    totp::replace_recovery_codes(&mut tx, user_id.uuid(), &hashes).await?;
    ctx.audit(None, "user.recovery_codes_regenerate")
        .target("user", user_id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(TotpEnabled {
        recovery_codes: codes,
    }))
}

#[cfg(test)]
mod tests {
    use super::{TotpEnabled, TotpSetup};

    #[test]
    fn totp_secrets_and_recovery_codes_never_print() {
        let setup = format!(
            "{:?}",
            TotpSetup {
                secret: "JBSWY3DPEHPK3PXP".to_owned(),
                otpauth_uri: "otpauth://totp/Kaeso:a?secret=JBSWY3DPEHPK3PXP".to_owned(),
            }
        );
        assert!(!setup.contains("JBSWY3DPEHPK3PXP"), "{setup}");
        let enabled = format!(
            "{:?}",
            TotpEnabled {
                recovery_codes: vec!["abcd-efgh-jkmn-pqrs".to_owned()],
            }
        );
        assert!(!enabled.contains("abcd-efgh"), "{enabled}");
    }
}
