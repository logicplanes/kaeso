use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::SessionId;

use crate::platform::access::Ctx;
use crate::platform::auth::session::{self, Revoked, SessionList};
use crate::platform::error::ApiResult;
use crate::platform::params::Path;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/me/sessions",
    summary = "The user's sessions, the current one first.",
    tag = "me",
    responses((status = 200, body = SessionList))
)]
pub async fn list(state: State<AppState>, ctx: Ctx) -> ApiResult<Json<SessionList>> {
    session::list(state, ctx).await
}

#[utoipa::path(
    delete,
    path = "/me/sessions/{session}",
    summary = "Ends one of the user's sessions.",
    tag = "me",
    params(("session" = SessionId, Path)),
    responses((status = 204, description = "Ended."))
)]
pub async fn revoke(state: State<AppState>, ctx: Ctx, id: Path<String>) -> ApiResult<StatusCode> {
    session::revoke(state, ctx, id).await
}

#[utoipa::path(
    post,
    path = "/me/sessions/revoke-all",
    summary = "Ends every session of the user except the current one.",
    tag = "me",
    responses((status = 200, body = Revoked))
)]
pub async fn revoke_all(state: State<AppState>, ctx: Ctx) -> ApiResult<Json<Revoked>> {
    session::revoke_all(state, ctx).await
}
