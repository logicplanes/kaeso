use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{totp, users};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;

use super::{SignedIn, refused, sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::{
    self, FAILURES_PER_ACCOUNT, FAILURES_PER_ACCOUNT_AND_IP, SIGN_IN_PER_IP, Throttle,
};
use crate::auth::totp::{Factor, SecondFactorError};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::state::AppState;

const CHALLENGE_LIFETIME: Duration = Duration::minutes(5);

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Email and password.")]
pub struct PasswordRequest {
    #[doc = "The account's email address."]
    #[schema(example = "owner@example.com")]
    pub email: String,
    #[doc = "The password."]
    #[schema(value_type = String, format = Password, write_only)]
    pub password: SecretString,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The second step is needed.")]
pub struct TotpRequired {
    #[doc = "Always `totp_required`."]
    #[schema(example = "totp_required")]
    pub status: &'static str,
    #[doc = "Pass to `auth/totp` with a code. Valid for 5 minutes."]
    pub challenge: String,
    #[doc = "When the challenge expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

fn wrong() -> ApiError {
    metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "password", "reason" => "wrong")
        .increment(1);
    ApiError::Authentication("The email or password is wrong.".into())
}

fn locked(retry_after: u64) -> String {
    format!(
        "Too many failed sign-ins; try again in {} minutes or use a sign-in link.",
        retry_after.div_ceil(60)
    )
}

pub(crate) struct Attempts {
    account: String,
    pair: String,
}

impl Attempts {
    pub(crate) fn new(ctx: &Ctx, account: &str) -> Attempts {
        let account = account.to_lowercase();
        let pair = format!("{account}|{}", crate::auth::key_failures::subject(ctx.ip));
        Attempts { account, pair }
    }

    fn counters(&self) -> [(Throttle, &str); 2] {
        [
            (FAILURES_PER_ACCOUNT_AND_IP, self.pair.as_str()),
            (FAILURES_PER_ACCOUNT, self.account.as_str()),
        ]
    }

    pub(crate) async fn begin(&self, state: &AppState) -> ApiResult<()> {
        for (t, subject) in self.counters() {
            throttle::hit(state, t, subject)
                .await
                .map_err(|r| refused(r, locked))?;
        }
        Ok(())
    }

    pub(crate) async fn succeeded(&self, state: &AppState) {
        throttle::clear(state, FAILURES_PER_ACCOUNT_AND_IP, &self.pair).await;
        throttle::release(state, FAILURES_PER_ACCOUNT, &self.account).await;
    }
}

pub(crate) async fn challenge(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    method: Method,
) -> ApiResult<Response> {
    let challenge = random_token();
    let expires_at = ctx.now + CHALLENGE_LIFETIME;
    tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::TotpChallenge,
            hash: &secret::hash(&challenge),
            user_id: Some(user.id),
            email: None,
            data: serde_json::json!({ "method": method.as_str() }),
            expires_at,
        },
    )
    .await?;
    let body = TotpRequired {
        status: "totp_required",
        challenge,
        expires_at,
    };
    Ok((axum::http::StatusCode::ACCEPTED, Json(body)).into_response())
}

#[utoipa::path(
    post,
    path = "/auth/password",
    summary = "Signs in with email and password. With TOTP set up, answers with a\n\
    challenge for `auth/totp` instead of a session.",
    tag = "auth",
    request_body = PasswordRequest,
    responses(
        (status = 200, description = "Signed in (the session cookie is set), or a TOTP code is needed.", body = SignedIn),
        (status = 202, description = "A TOTP or recovery code is needed: call `auth/totp`.", body = TotpRequired),
    )
)]
pub async fn password(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<PasswordRequest>,
) -> ApiResult<Response> {
    if !state.config.auth.password {
        return Err(ApiError::PermissionDenied(
            "Password sign-in is turned off on this install.".into(),
        ));
    }
    throttle_ip(&state, &ctx, SIGN_IN_PER_IP).await?;
    let email = kaeso_core::email::normalize(&req.email).map_err(|_| wrong())?;
    let attempts = Attempts::new(&ctx, &email);
    attempts.begin(&state).await?;

    let user = users::get_by_email(&state.pg, &email).await?;
    let stored = user
        .as_ref()
        .and_then(|u| u.password_hash.clone())
        .unwrap_or_else(|| kaeso_crypto::password::dummy_hash().to_owned());
    let pw = req.password;
    let matches = tokio::task::spawn_blocking(move || kaeso_crypto::password::verify(&pw, &stored))
        .await
        .map_err(ApiError::internal)?;
    let user = match user {
        Some(u) if matches && u.password_hash.is_some() => u,
        _ => return Err(wrong()),
    };
    if user.state != "active" {
        return Err(ApiError::PermissionDenied(
            "This account is deactivated; contact your Kaeso administrator.".into(),
        ));
    }
    attempts.succeeded(&state).await;
    if totp::is_enabled(&state.pg, user.id).await? {
        return challenge(&state, &ctx, &user, Method::Password).await;
    }
    sign_in(&state, &ctx, &user, Method::Password, false, None).await
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The second step of a password or sign-in link sign-in.")]
pub struct TotpRequest {
    #[doc = "The challenge from `auth/password` or `auth/magic-link/verify`."]
    #[schema(value_type = String, write_only)]
    pub challenge: SecretString,
    #[doc = "A 6-digit code from the authenticator app."]
    #[schema(example = "123456")]
    pub code: Option<String>,
    #[doc = "Or one of the recovery codes (each works once)."]
    #[schema(example = "abcd-efgh-jkmn-pqrs")]
    pub recovery_code: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/totp",
    summary = "Completes a password or sign-in link sign-in with a TOTP or recovery\n\
    code. A code of the same 30-second step is accepted only once.",
    tag = "auth",
    request_body = TotpRequest,
    responses((status = 200, description = "Signed in; the session cookie is set.", body = SignedIn))
)]
pub async fn totp(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<TotpRequest>,
) -> ApiResult<Response> {
    throttle_ip(&state, &ctx, SIGN_IN_PER_IP).await?;
    let hash = secret::hash(req.challenge.expose_secret());
    let challenge = tokens::peek(&state.pg, TokenKind::TotpChallenge, &hash, ctx.now)
        .await?
        .ok_or_else(|| {
            ApiError::Authentication(
                "The sign-in has expired; enter your email and password again.".into(),
            )
        })?;
    let user_id = challenge
        .user_id
        .map(UserId::from_uuid)
        .ok_or_else(|| ApiError::internal("challenge without user"))?;
    let attempts = Attempts::new(&ctx, &user_id.to_string());
    attempts.begin(&state).await?;
    let factor = match (&req.code, &req.recovery_code) {
        (Some(code), None) => Factor::Code(code),
        (None, Some(code)) => Factor::Recovery(code),
        _ => {
            return Err(ApiError::bad_request(
                "Send either `code` or `recovery_code`.",
            ));
        }
    };
    match crate::auth::totp::check(&state, user_id, factor).await {
        Ok(()) => {}
        Err(SecondFactorError::Invalid) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "totp", "reason" => "wrong").increment(1);
            return Err(ApiError::Authentication(
                "That code isn't valid or was already used.".into(),
            ));
        }
        Err(e) => return Err(ApiError::internal(e)),
    }
    if tokens::consume(&state.pg, TokenKind::TotpChallenge, &hash, ctx.now)
        .await?
        .is_none()
    {
        return Err(ApiError::Authentication(
            "The sign-in has expired; enter your email and password again.".into(),
        ));
    }
    attempts.succeeded(&state).await;
    let user = users::get(&state.pg, user_id.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::PermissionDenied("This account is deactivated.".into()))?;
    let method = match challenge.data.get("method").and_then(|m| m.as_str()) {
        Some("magic_link") => Method::MagicLink,
        _ => Method::Password,
    };
    sign_in(&state, &ctx, &user, method, true, None).await
}
