use axum::Json;
use axum::extract::State;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{totp, users};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::Duration;
use utoipa::ToSchema;

use super::password::challenge;
use super::{EmailSent, SignedIn, sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::{self, EMAILS_PER_ADDRESS, EMAILS_PER_IP, TOKENS_PER_IP};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::state::AppState;

pub const LIFETIME: Duration = Duration::minutes(15);

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The address to send a link to.")]
pub struct MagicLinkRequest {
    #[doc = "The account's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
}

fn links_on(state: &AppState) -> ApiResult<()> {
    if state.config.auth.magic_link {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "Sign-in links are turned off on this install.".into(),
        ))
    }
}

#[utoipa::path(
    post,
    path = "/auth/magic-link",
    summary = "Sends a sign-in link if the address has an active account. Always\n\
    answers the same way.",
    tag = "auth",
    request_body = MagicLinkRequest,
    responses((status = 200, description = "Accepted; an email was sent if the account exists.", body = EmailSent))
)]
pub async fn request(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<MagicLinkRequest>,
) -> ApiResult<Json<EmailSent>> {
    links_on(&state)?;
    throttle_ip(&state, &ctx, EMAILS_PER_IP).await?;
    let answer = Json(EmailSent::new("a sign-in link"));
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
            kind: TokenKind::MagicLink,
            hash: &secret::hash(&token),
            user_id: Some(user.id),
            email: Some(&email),
            data: serde_json::json!({}),
            expires_at: ctx.now + LIFETIME,
        },
    )
    .await?;
    let url = format!("{}auth/magic-link#token={token}", state.config.public_url);
    let lang =
        crate::services::mail_lang::of_user(&state.pg, user.id, ctx.accept_language.as_deref())
            .await;
    state
        .mailer
        .send(kaeso_mail::templates::magic_link(lang, &email, &url));
    Ok(answer)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The token from the link.")]
pub struct VerifyRequest {
    #[doc = "The token from `…/auth/magic-link#token=…`."]
    #[schema(value_type = String, write_only)]
    pub token: SecretString,
}

#[utoipa::path(
    post,
    path = "/auth/magic-link/verify",
    summary = "Signs in with a sign-in link. The link works once. With TOTP set up,\n\
    answers with a challenge for `auth/totp` instead of a session.",
    description = "A link from `kaeso user sign-in-link` works even while sign-in links by \
    email are turned off.",
    tag = "auth",
    request_body = VerifyRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set.", body = SignedIn),
        (status = 202, description = "A TOTP or recovery code is needed: call `auth/totp`.", body = super::password::TotpRequired),
    )
)]
pub async fn verify(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<VerifyRequest>,
) -> ApiResult<Response> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let hash = secret::hash(req.token.expose_secret());
    let token = tokens::consume(&state.pg, TokenKind::MagicLink, &hash, ctx.now)
        .await?
        .ok_or_else(|| {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "magic_link", "reason" => "invalid").increment(1);
            ApiError::Authentication("This sign-in link is invalid, used or expired; request a new one.".into())
        })?;
    let by_operator = token.data.get("issued_by").and_then(|v| v.as_str())
        == Some(crate::services::users::OPERATOR_LINK);
    if !by_operator {
        links_on(&state)?;
    }
    let user_id = token
        .user_id
        .map(UserId::from_uuid)
        .ok_or_else(|| ApiError::internal("token without user"))?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::PermissionDenied("This account is deactivated.".into()))?;
    users::mark_verified(&state.pg, user.id, ctx.now).await?;
    if totp::is_enabled(&state.pg, user.id).await? {
        return challenge(&state, &ctx, &user, Method::MagicLink).await;
    }
    sign_in(&state, &ctx, &user, Method::MagicLink, false, None).await
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Whom a sign-in link signs in.")]
pub struct LinkAccount {
    #[doc = "The account's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
    #[doc = "When the link expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: time::OffsetDateTime,
}

#[utoipa::path(
    post,
    path = "/auth/magic-link/lookup",
    summary = "Names the account a sign-in link signs in, without using the link, so\n\
    the console can ask before signing in.",
    tag = "auth",
    request_body = VerifyRequest,
    responses((status = 200, body = LinkAccount))
)]
pub async fn lookup(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<VerifyRequest>,
) -> ApiResult<Json<LinkAccount>> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let hash = secret::hash(req.token.expose_secret());
    let invalid = || {
        ApiError::Authentication(
            "This sign-in link is invalid, used or expired; request a new one.".into(),
        )
    };
    let token = tokens::peek(&state.pg, TokenKind::MagicLink, &hash, ctx.now)
        .await?
        .ok_or_else(invalid)?;
    let user = match token.user_id {
        Some(id) => users::get(&state.pg, id)
            .await?
            .filter(|u| u.state == "active"),
        None => None,
    };
    let email = user.and_then(|u| u.email).ok_or_else(invalid)?;
    Ok(Json(LinkAccount {
        email,
        expires_at: token.expires_at,
    }))
}
