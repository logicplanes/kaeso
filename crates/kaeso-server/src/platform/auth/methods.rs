use axum::Json;
use axum::extract::State;
use kaeso_core::id::OrgId;
use kaeso_store::pg::{instance, oidc, users};
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
    #[doc = "The organization that manages the address, at its verified domain,\n\
        requires SSO, so the page offers its provider alone: the account can't\n\
        sign in with a password or a sign-in link anywhere on the install."]
    pub sso_required: bool,
    #[doc = "The page offers to create an account (`auth/signup`). Without an\n\
        address: some address may. With one: this address may, by the\n\
        instance's account creation setting and its domain."]
    pub sign_up: bool,
    #[doc = "The identity provider the page offers, by name: the one of the org whose\n\
        verified domain the address is at, `null` when no org's is. Always\n\
        `null` without an address, since SSO is offered only after it."]
    pub provider: Option<SsoProviderView>,
    #[doc = "The install is set up: its first user exists, so a setup link no longer\n\
        works and people sign in instead."]
    pub set_up: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An identity provider a sign-in page offers.")]
pub struct SsoProviderView {
    #[doc = "The org it signs in to."]
    pub org: OrgId,
    #[doc = "The org's name, so the page says where the provider signs in."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "The name on the sign-in button, such as `Acme ID`; `null` to say SSO."]
    #[schema(example = "Acme ID")]
    pub name: Option<String>,
    #[doc = "The connection has **Start without a click** on: when the page offers\n\
        this provider alone, it goes on to the provider right after the address,\n\
        without showing the button."]
    pub auto_redirect: bool,
}

impl SsoProviderView {
    fn of(c: oidc::Connection, org_name: &str) -> SsoProviderView {
        SsoProviderView {
            org: OrgId::from_uuid(c.org_id),
            org_name: org_name.to_owned(),
            name: c.display_name,
            auto_redirect: c.auto_redirect,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The address to sign in with.")]
pub struct MethodsRequest {
    #[doc = "The email address."]
    #[schema(example = "ada@example.com")]
    pub email: String,
}

fn install(state: &AppState, sign_up: bool, set_up: bool) -> SignInMethodsView {
    let a = &state.config.auth;
    SignInMethodsView {
        password: a.password,
        magic_link: a.magic_link,
        sso_required: false,
        sign_up,
        provider: None,
        set_up,
    }
}

#[utoipa::path(
    get,
    path = "/auth/methods",
    summary = "The sign-in methods on this install, for the first step of a sign-in page.",
    description = "It never names an identity provider: single sign-on is offered only after \
    the address, by `POST auth/methods`.",
    tag = "auth",
    responses((status = 200, body = SignInMethodsView))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<SignInMethodsView>> {
    let policy = accounts::policy(&instance::get(&state.pg).await?);
    let set_up = users::any_exist(&state.pg).await?;
    Ok(Json(install(&state, policy.sign_up_open(), set_up)))
}

#[utoipa::path(
    post,
    path = "/auth/methods",
    summary = "The sign-in methods for an address: the install's, with `provider` when\n\
    the address's domain is a verified domain of an organization with an\n\
    identity provider, and whether the address may create an account.",
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
    let set_up = users::any_exist(&state.pg).await?;
    let mut methods = install(&state, policy.may_sign_up(&email), set_up);
    let manager = crate::services::managed::of_address(&state.pg, &email).await?;
    let connection = match &manager {
        Some(m) => oidc::get(&state.pg, m.org_id).await?,
        None => None,
    };
    if let (Some(manager), Some(connection)) = (manager, connection) {
        methods.sso_required = manager.require_sso;
        methods.provider = Some(SsoProviderView::of(connection, &manager.org_name));
    }
    if methods.sso_required {
        methods.password = false;
        methods.magic_link = false;
        methods.sign_up = false;
    }
    Ok(Json(methods))
}
