use axum::Json;
use axum::extract::State;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{instance, users};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::Duration;
use utoipa::ToSchema;

use super::setup::{hash_password, validate_email, validate_name};
use super::{EmailSent, SignedIn, sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::{self, EMAILS_PER_ADDRESS, EMAILS_PER_IP, TOKENS_PER_IP};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::services::accounts;
use crate::state::AppState;

const LIFETIME: Duration = Duration::hours(24);

async fn account_policy(state: &AppState) -> ApiResult<kaeso_core::accounts::AccountPolicy> {
    Ok(accounts::policy(&instance::get(&state.pg).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The address to sign up with.")]
pub struct SignupRequest {
    #[doc = "The email address."]
    #[schema(example = "ada@example.com")]
    pub email: String,
}

#[utoipa::path(
    post,
    path = "/auth/signup",
    summary = "Sends a link to create an account (or, for an existing account, a\n\
    sign-in link). Refused when the instance's account creation setting\n\
    doesn't allow the address.",
    tag = "auth",
    request_body = SignupRequest,
    responses((status = 200, body = EmailSent))
)]
pub async fn request(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<SignupRequest>,
) -> ApiResult<Json<EmailSent>> {
    throttle_ip(&state, &ctx, EMAILS_PER_IP).await?;
    let email = validate_email("email", &req.email)?;
    if let Some(refused) = accounts::refuse_sign_up(&account_policy(&state).await?, &email) {
        return Err(refused);
    }
    let answer = Json(EmailSent {
        status: "email_sent",
        message: format!("Open the link we sent to {email} to continue."),
    });
    if throttle::hit(&state, EMAILS_PER_ADDRESS, &email)
        .await
        .is_err()
    {
        return Ok(answer);
    }
    let token = random_token();
    let existing = users::get_by_email(&state.pg, &email).await?;
    let (kind, url) = match &existing {
        Some(_) => (
            TokenKind::MagicLink,
            format!("{}auth/magic-link#token={token}", state.config.public_url),
        ),
        None => (
            TokenKind::Signup,
            format!("{}auth/signup#token={token}", state.config.public_url),
        ),
    };
    if existing.as_ref().is_some_and(|u| u.state != "active") {
        return Ok(answer);
    }
    let lifetime = if kind == TokenKind::MagicLink {
        super::magic::LIFETIME
    } else {
        LIFETIME
    };
    tokens::insert(
        &state.pg,
        NewToken {
            kind,
            hash: &secret::hash(&token),
            user_id: existing.as_ref().map(|u| u.id),
            email: Some(&email),
            data: serde_json::json!({}),
            expires_at: ctx.now + lifetime,
        },
    )
    .await?;
    let lang = match &existing {
        Some(user) => {
            crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
                .await
        }
        None => kaeso_mail::Locale::choose(None, ctx.accept_language.as_deref()),
    };
    state.mailer.send(match kind {
        TokenKind::MagicLink => kaeso_mail::templates::magic_link(lang, &email, &url),
        _ => kaeso_mail::templates::signup(lang, &email, &url),
    });
    Ok(answer)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The new account.")]
pub struct SignupVerify {
    #[doc = "The token from `…/auth/signup#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
    #[doc = "The display name."]
    #[schema(example = "Ada Lovelace")]
    pub name: String,
    #[doc = "A password, 12 to 128 characters; leave out to sign in by magic link only."]
    #[schema(value_type = Option<String>, format = Password, write_only)]
    pub password: Option<SecretString>,
}

#[utoipa::path(
    post,
    path = "/auth/signup/verify",
    summary = "Creates the account with the link and signs in. The account creation\n\
    setting is checked again: a link sent before it changed no longer works.",
    tag = "auth",
    request_body = SignupVerify,
    responses((status = 200, body = SignedIn))
)]
pub async fn verify(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<SignupVerify>,
) -> ApiResult<Response> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let name = validate_name("name", &req.name)?;
    let password_hash = match req.password {
        Some(p) => Some(hash_password("password", p).await?),
        None => None,
    };
    let mut tx = state.pg.begin().await?;
    let token = tokens::consume(
        &mut *tx,
        TokenKind::Signup,
        &secret::hash(req.token.expose_secret()),
        ctx.now,
    )
    .await?
    .ok_or_else(|| {
        ApiError::Authentication(
            "This signup link is invalid, used or expired; request a new one.".into(),
        )
    })?;
    let email = token
        .email
        .ok_or_else(|| ApiError::internal("signup token without address"))?;
    if let Some(refused) = accounts::refuse_sign_up(&account_policy(&state).await?, &email) {
        return Err(refused);
    }
    let id = UserId::new();
    let created = users::insert(
        &mut *tx,
        &users::NewUser {
            id: id.uuid(),
            email: &email,
            name: &name,
            password_hash: password_hash.as_deref(),
            email_verified_at: Some(ctx.now),
            is_instance_admin: false,
        },
    )
    .await?;
    if !created {
        return Err(ApiError::Conflict(
            "An account with this address exists; sign in instead.".into(),
        ));
    }
    ctx.audit(None, "user.signup")
        .by(id)
        .target("user", id)
        .after(serde_json::json!({ "email": email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let user = users::get(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new user vanished"))?;
    sign_in(&state, &ctx, &user, Method::Signup, false, None).await
}
