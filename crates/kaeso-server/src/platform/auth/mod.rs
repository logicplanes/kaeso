pub mod invites;
pub mod magic;
pub mod methods;
pub mod next;
pub mod oidc;
pub mod password;
pub mod reset;
pub mod session;
pub mod setup;
pub mod signup;

use axum::http::header;
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, UserId};
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
use crate::auth::throttle::{self, Refusal, Throttle};
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
        .routes(s.route(Access::Public, routes!(magic::lookup)))
        .routes(s.route(Access::Public, routes!(reset::request)))
        .routes(s.route(Access::Public, routes!(reset::verify)))
        .routes(s.route(Access::Public, routes!(oidc::start, oidc::start_link)))
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
    start_session(
        state,
        ctx,
        user,
        SignIn {
            user_id: UserId::from_uuid(user.id),
            method,
            second_factor,
            oidc_org,
            signed_in_at: ctx.now,
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await
}

pub(crate) async fn start_session(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    sign_in: SignIn,
) -> ApiResult<Response> {
    start_session_until(state, ctx, user, ctx.now + ABSOLUTE_LIFETIME, sign_in).await
}

pub(crate) async fn start_session_until(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    expires_at: time::OffsetDateTime,
    sign_in: SignIn,
) -> ApiResult<Response> {
    let mut conn = state.pg.acquire().await?;
    let created = sessions::create_until(&mut conn, ctx.now, &sign_in, expires_at).await?;
    signed_in_response(state, ctx, user, &sign_in, &created)
}

fn signed_in_response(
    state: &AppState,
    ctx: &Ctx,
    user: &users::User,
    sign_in: &SignIn,
    created: &sessions::Created,
) -> ApiResult<Response> {
    let (session, token) = (created.id, &created.token);
    let cookie = cookies::set(state, token).ok_or_else(|| ApiError::internal("session cookie"))?;
    let body = SignedIn {
        status: "signed_in",
        user: UserView::from_row(user),
        session: SessionView {
            id: session,
            method: sign_in.method.as_str().to_owned(),
            second_factor: sign_in.second_factor,
            sso_org: sign_in.oidc_org.map(OrgId::from_uuid),
            signed_in_at: sign_in.signed_in_at.min(ctx.now),
            last_used_at: ctx.now,
            expires_at: created.expires_at,
        },
    };
    Ok(([(header::SET_COOKIE, cookie)], axum::Json(body)).into_response())
}

pub(crate) fn refused(refusal: Refusal, limited: impl FnOnce(u64) -> String) -> ApiError {
    match refusal {
        Refusal::Limited { retry_after } => ApiError::RateLimited {
            message: limited(retry_after),
            retry_after: Some(retry_after),
        },
        Refusal::Unavailable => {
            ApiError::Unavailable("Signing in is unavailable right now; try again shortly.".into())
        }
    }
}

pub(crate) async fn throttle_ip(state: &AppState, ctx: &Ctx, t: Throttle) -> ApiResult<()> {
    let subject = crate::auth::key_failures::subject(ctx.ip);
    throttle::hit(state, t, &subject).await.map_err(|r| {
        refused(r, |retry_after| {
            format!(
                "Too many attempts from your address; try again in {} minutes.",
                retry_after.div_ceil(60)
            )
        })
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
