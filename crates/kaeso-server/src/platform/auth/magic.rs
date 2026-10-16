use axum::Json;
use axum::extract::State;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::users;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::Duration;
use utoipa::ToSchema;

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
    summary = "Signs in with a sign-in link. The link works once.",
    tag = "auth",
    request_body = VerifyRequest,
    responses((status = 200, description = "Signed in; the session cookie is set.", body = SignedIn))
)]
pub async fn verify(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<VerifyRequest>,
) -> ApiResult<Response> {
    links_on(&state)?;
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let hash = secret::hash(req.token.expose_secret());
    let token = tokens::consume(&state.pg, TokenKind::MagicLink, &hash, ctx.now)
        .await?
        .ok_or_else(|| {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "magic_link", "reason" => "invalid").increment(1);
            ApiError::Authentication("This sign-in link is invalid, used or expired; request a new one.".into())
        })?;
    let user_id = token
        .user_id
        .map(UserId::from_uuid)
        .ok_or_else(|| ApiError::internal("token without user"))?;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::PermissionDenied("This account is deactivated.".into()))?;
    users::mark_verified(&state.pg, user.id, ctx.now).await?;
    sign_in(&state, &ctx, &user, Method::MagicLink, false, None).await
}
