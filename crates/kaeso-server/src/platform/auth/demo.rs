use axum::extract::State;
use axum::response::Response;
use kaeso_store::pg::users;

use super::{SignedIn, sign_in, throttle_ip};
use crate::auth::sessions::Method;
use crate::auth::throttle::SIGN_IN_PER_IP;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const READ_ONLY: &str = "This demo is read-only; install Kaeso to change it.";

#[utoipa::path(
    post,
    path = "/auth/demo",
    summary = "Signs in to a read-only demo as its demo user.",
    description = "Only on an install that names a demo user (`auth.demo_user`); elsewhere it \
    answers 404. The session sees what the demo user sees, and the install refuses every change.",
    tag = "auth",
    responses((status = 200, description = "Signed in (the session cookie is set).", body = SignedIn))
)]
pub async fn demo(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Response> {
    let Some(address) = state.config.auth.demo_user() else {
        return Err(ApiError::NotFound("This install has no demo.".into()));
    };
    throttle_ip(&state, &ctx, SIGN_IN_PER_IP).await?;
    let email = kaeso_core::email::normalize(address)
        .map_err(|e| ApiError::internal(format!("the demo user's address is invalid: {e}")))?;
    let user = users::get_by_email(&state.pg, &email)
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::internal("the demo user doesn't exist or is deactivated"))?;
    sign_in(&state, &ctx, &user, Method::Password, false, None).await
}
