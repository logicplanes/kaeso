use axum::Json;
use axum::extract::State;
use kaeso_core::id::OrgId;
use kaeso_store::pg::{instance, oidc, orgs, users};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::throttle_ip;
use crate::auth::throttle::METHODS_PER_IP;
use crate::config::AuthConfig;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Query;
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
    #[doc = "The identity provider the page offers by name. Without an address: the\n\
        one of the org in `org`, or the install's only one when SSO is its only\n\
        sign-in method, so the page needs no address; otherwise `null`. With an\n\
        address: the one of its domain."]
    pub provider: Option<SsoProviderView>,
    #[doc = "The install is set up: its first user exists, so a setup link no longer\n\
        works and people sign in instead."]
    pub set_up: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An identity provider a sign-in page offers.")]
pub struct SsoProviderView {
    #[doc = "The org it signs in to; pass it as `org` to `auth/oidc/start`."]
    pub org: OrgId,
    #[doc = "The name on the sign-in button, such as `Acme ID`; `null` to say SSO."]
    #[schema(example = "Acme ID")]
    pub name: Option<String>,
    #[doc = "The page starts the sign-in at the provider without a click, unless the\n\
        person just signed out or the last attempt failed."]
    pub auto_redirect: bool,
}

impl SsoProviderView {
    fn of(c: oidc::Connection) -> SsoProviderView {
        SsoProviderView {
            org: OrgId::from_uuid(c.org_id),
            name: c.display_name,
            auto_redirect: c.auto_redirect,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MethodsQuery {
    #[doc = "The org of a sign-in link (`/sign-in?org=…`), whose identity provider\n\
        the page offers."]
    pub org: Option<OrgId>,
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
        sso: a.oidc,
        sso_required: false,
        sign_up,
        provider: None,
        set_up,
    }
}

pub fn sso_only(auth: &AuthConfig) -> bool {
    auth.oidc && !auth.password && !auth.magic_link
}

async fn offered(state: &AppState, org: Option<OrgId>) -> ApiResult<Option<SsoProviderView>> {
    if !state.config.auth.oidc {
        return Ok(None);
    }
    let org = match org {
        Some(org) => Some(org.uuid()),
        None if sso_only(&state.config.auth) => match oidc::connected(&state.pg).await? {
            oidc::Connected::Only(org) => Some(org),
            oidc::Connected::Nowhere | oidc::Connected::Several => None,
        },
        None => None,
    };
    Ok(match org {
        Some(org) => oidc::get(&state.pg, org).await?.map(SsoProviderView::of),
        None => None,
    })
}

#[utoipa::path(
    get,
    path = "/auth/methods",
    summary = "The sign-in methods on this install, for the first step of a sign-in page.",
    tag = "auth",
    params(MethodsQuery),
    responses((status = 200, body = SignInMethodsView))
)]
pub async fn get(
    State(state): State<AppState>,
    Query(q): Query<MethodsQuery>,
) -> ApiResult<Json<SignInMethodsView>> {
    let policy = accounts::policy(&instance::get(&state.pg).await?);
    let set_up = users::any_exist(&state.pg).await?;
    let mut methods = install(&state, policy.sign_up_open(), set_up);
    methods.provider = offered(&state, q.org).await?;
    Ok(Json(methods))
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
    let set_up = users::any_exist(&state.pg).await?;
    let mut methods = install(&state, policy.may_sign_up(&email), set_up);
    let org = if methods.sso {
        oidc::org_for_domain(&state.pg, kaeso_core::email::domain(&email)).await?
    } else {
        None
    };
    let connection = match org {
        Some(org) => oidc::get(&state.pg, org).await?,
        None => None,
    };
    methods.sso = connection.is_some();
    if let Some(connection) = connection {
        methods.sso_required = orgs::get(&state.pg, connection.org_id)
            .await?
            .is_some_and(|o| o.require_sso);
        methods.provider = Some(SsoProviderView::of(connection));
    }
    if methods.sso_required {
        methods.password = false;
        methods.magic_link = false;
        methods.sign_up = false;
    }
    Ok(Json(methods))
}
