use axum::Json;
use axum::extract::State;
use kaeso_store::pg::{instance, oidc, orgs};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::throttle_ip;
use crate::auth::throttle::METHODS_PER_IP;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::services::accounts;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The sign-in methods a sign-in page offers.")]
pub struct SignInMethodsView {
    #[doc = "Email and password."]
    pub password: bool,
    #[doc = "A sign-in link by email."]
    pub magic_link: bool,
    #[doc = "SSO. Without an address: on for the install. With one: the address's\n\
        domain belongs to an organization with an identity provider."]
    pub sso: bool,
    #[doc = "That organization requires SSO, so the page offers SSO alone."]
    pub sso_required: bool,
    #[doc = "The page offers to create an account (`auth/signup`). Without an\n\
        address: some address may. With one: this address may, by the\n\
        instance's account creation setting and its domain."]
    pub sign_up: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The address to sign in with.")]
pub struct MethodsRequest {
    #[doc = "The email address."]
    #[schema(example = "ada@example.com")]
    pub email: String,
}

fn install(state: &AppState, sign_up: bool) -> SignInMethodsView {
    let a = &state.config.auth;
    SignInMethodsView {
        password: a.password,
        magic_link: a.magic_link,
        sso: a.oidc,
        sso_required: false,
        sign_up,
    }
}

#[utoipa::path(
    get,
    path = "/auth/methods",
    summary = "The sign-in methods on this install, for the first step of a sign-in page.",
    tag = "auth",
    responses((status = 200, body = SignInMethodsView))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<SignInMethodsView>> {
    let policy = accounts::policy(&instance::get(&state.pg).await?);
    Ok(Json(install(&state, policy.sign_up_open())))
}

#[utoipa::path(
    post,
    path = "/auth/methods",
    summary = "The sign-in methods for an address: the install's, with SSO when the\n\
    address's domain belongs to an organization with an identity provider,\n\
    and whether the address may create an account.",
    tag = "auth",
    request_body = MethodsRequest,
    responses((status = 200, body = SignInMethodsView))
)]
pub async fn lookup(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<MethodsRequest>,
) -> ApiResult<Json<SignInMethodsView>> {
    throttle_ip(&state, &ctx, METHODS_PER_IP).await?;
    let email = kaeso_core::email::normalize(&req.email)
        .map_err(|e| ApiError::invalid("email", format!("{e}.")))?;
    let policy = accounts::policy(&instance::get(&state.pg).await?);
    let mut methods = install(&state, policy.may_sign_up(&email));
    let org = if methods.sso {
        oidc::org_for_domain(&state.pg, kaeso_core::email::domain(&email)).await?
    } else {
        None
    };
    let connected = match org {
        Some(org) => oidc::get(&state.pg, org).await?.is_some(),
        None => false,
    };
    methods.sso = connected;
    if connected && let Some(org) = org {
        methods.sso_required = orgs::get(&state.pg, org)
            .await?
            .is_some_and(|o| o.require_sso);
    }
    if methods.sso_required {
        methods.password = false;
        methods.magic_link = false;
        methods.sign_up = false;
    }
    Ok(Json(methods))
}
