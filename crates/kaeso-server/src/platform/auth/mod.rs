pub mod invites;
pub mod magic;
pub mod methods;
pub mod oidc;
pub mod password;
pub mod reset;
pub mod session;
pub mod setup;
pub mod signup;

use axum::http::header;
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{SessionId, UserId};
use kaeso_store::pg::users;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::access::{Access, Ctx, Secure};
use super::cookies;
use super::error::{ApiError, ApiResult};
use super::views::{SessionView, UserView};
use crate::auth::sessions::{self, ABSOLUTE_LIFETIME, Method, SignIn};
use crate::auth::throttle::{self, Throttle};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(Access::Public, routes!(setup::setup)))
        .routes(s.route(Access::Public, routes!(methods::get, methods::lookup)))
        .routes(s.route(Access::Public, routes!(password::password)))
        .routes(s.route(Access::Public, routes!(password::totp)))
        .routes(s.route(Access::Public, routes!(magic::request)))
        .routes(s.route(Access::Public, routes!(magic::verify)))
        .routes(s.route(Access::Public, routes!(reset::request)))
        .routes(s.route(Access::Public, routes!(reset::verify)))
        .routes(s.route(Access::Public, routes!(oidc::start)))
        .routes(s.route(Access::Public, routes!(oidc::callback)))
        .routes(s.route(Access::Public, routes!(invites::lookup)))
        .routes(s.route(Access::Public, routes!(invites::accept_invite)))
        .routes(s.route(Access::Public, routes!(signup::request)))
        .routes(s.route(Access::Public, routes!(signup::verify)))
        .routes(s.route(Access::Session, routes!(session::current)))
        .routes(s.route(Access::Session, routes!(session::logout)))
        .routes(s.route(Access::Session, routes!(session::list)))
        .routes(s.route(Access::Session, routes!(session::revoke)))
        .routes(s.route(Access::Session, routes!(session::revoke_all)))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A successful sign-in.")]
pub struct SignedIn {
    #[doc = "Always `signed_in`."]
    #[schema(example = "signed_in")]
    pub status: &'static str,
    #[doc = "The user."]
    pub user: UserView,
    #[doc = "The new session. Its secret id is only in the `Set-Cookie` header."]
    pub session: SessionView,
}

pub(crate) async fn sign_in(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    method: Method,
    second_factor: bool,
    oidc_org: Option<uuid::Uuid>,
) -> ApiResult<Response> {
    let mut conn = state.pg.acquire().await?;
    let created = sessions::create(
        &mut conn,
        ctx.now,
        &SignIn {
            user_id: UserId::from_uuid(user.id),
            method,
            second_factor,
            oidc_org,
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await?;
    signed_in_response(
        state,
        ctx,
        user,
        method,
        second_factor,
        created.id,
        &created.token,
    )
}

fn signed_in_response(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    method: Method,
    second_factor: bool,
    session: SessionId,
    token: &secrecy::SecretString,
) -> ApiResult<Response> {
    let cookie = cookies::set(state, token).ok_or_else(|| ApiError::internal("session cookie"))?;
    let body = SignedIn {
        status: "signed_in",
        user: UserView::from_row(user),
        session: SessionView {
            id: session,
            method: method.as_str().to_owned(),
            second_factor,
            signed_in_at: ctx.now,
            last_used_at: ctx.now,
            expires_at: ctx.now + ABSOLUTE_LIFETIME,
        },
    };
    Ok(([(header::SET_COOKIE, cookie)], axum::Json(body)).into_response())
}

pub(crate) async fn throttle_ip(state: &AppState, ctx: &Ctx, t: Throttle) -> ApiResult<()> {
    let subject = ctx
        .ip
        .map_or_else(|| "unknown".to_owned(), |ip| ip.to_string());
    throttle::hit(state, t, &subject)
        .await
        .map_err(|retry_after| ApiError::RateLimited {
            message: format!(
                "Too many attempts from your address; try again in {} minutes.",
                retry_after.div_ceil(60)
            ),
            retry_after: Some(retry_after),
        })
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The answer to requests that must not reveal whether an account exists.")]
pub struct EmailSent {
    #[doc = "Always `email_sent`."]
    #[schema(example = "email_sent")]
    pub status: &'static str,
    #[doc = "What happens next."]
    pub message: String,
}

impl EmailSent {
    pub(crate) fn new(what: &str) -> EmailSent {
        EmailSent {
            status: "email_sent",
            message: format!("If an account exists for this address, we sent it {what}."),
        }
    }
}
