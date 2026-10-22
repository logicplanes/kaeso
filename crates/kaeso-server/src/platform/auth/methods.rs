use axum::Json;
use axum::extract::State;
use kaeso_core::id::OrgId;
use kaeso_store::pg::{instance, oidc, orgs, users};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::throttle_ip;
use crate::auth::throttle::METHODS_PER_IP;
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
    #[doc = "The organization of the address's verified domain requires SSO, so the\n\
        page offers its provider alone."]
    pub sso_required: bool,
    #[doc = "The page offers to create an account (`auth/signup`). Without an\n\
        address: some address may. With one: this address may, by the\n\
        instance's account creation setting and its domain."]
    pub sign_up: bool,
    #[doc = "The identity provider the page offers, by name. With an address: the one\n\
        of the org whose verified domain the address is at, `null` when no org's\n\
        is. Without one: the one of the org in `org`, only while its connection\n\
        has **Start without a click** on; otherwise `null`."]
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
    #[doc = "The org's name, so the page says where the provider signs in."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "The name on the sign-in button, such as `Acme ID`; `null` to say SSO."]
    #[schema(example = "Acme ID")]
    pub name: Option<String>,
    #[doc = "The connection has **Start without a click** on: the org's start link\n\
        goes on to the provider, and after the address a page that offers SSO\n\
        alone goes on without asking, unless the browser is signed in already,\n\
        the person just signed out or the last attempt failed."]
    pub auto_redirect: bool,
}

impl SsoProviderView {
    fn of(c: oidc::Connection, org: &orgs::Org) -> SsoProviderView {
        SsoProviderView {
            org: OrgId::from_uuid(c.org_id),
            org_name: org.name.clone(),
            name: c.display_name,
            auto_redirect: c.auto_redirect,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MethodsQuery {
    #[doc = "The org of a start link (`/sign-in?org=…&start=sso`), whose identity\n\
        provider the page offers while its connection has **Start without a\n\
        click** on."]
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
        sso_required: false,
        sign_up,
        provider: None,
        set_up,
    }
}

async fn offered(state: &AppState, org: Option<OrgId>) -> ApiResult<Option<SsoProviderView>> {
    let Some(org) = org else {
        return Ok(None);
    };
    let (Some(connection), Some(row)) = (
        oidc::get(&state.pg, org.uuid()).await?,
        orgs::get(&state.pg, org.uuid()).await?,
    ) else {
        return Ok(None);
    };
    Ok(connection
        .auto_redirect
        .then(|| SsoProviderView::of(connection, &row)))
}

#[utoipa::path(
    get,
    path = "/auth/methods",
    summary = "The sign-in methods on this install, for the first step of a sign-in page.",
    description = "It names an identity provider only for `org` of an organization whose \
    connection has Start without a click on, for the hand-off of that organization's start link. \
    Otherwise single sign-on is offered only after the address, by `POST auth/methods`.",
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
    let org = oidc::org_for_domain(&state.pg, kaeso_core::email::domain(&email)).await?;
    let connection = match org {
        Some(org) => oidc::get(&state.pg, org).await?,
        None => None,
    };
    let org = match &connection {
        Some(c) => orgs::get(&state.pg, c.org_id).await?,
        None => None,
    };
    if let (Some(connection), Some(org)) = (connection, org) {
        methods.sso_required = org.require_sso;
        methods.provider = Some(SsoProviderView::of(connection, &org));
    }
    if methods.sso_required {
        methods.password = false;
        methods.magic_link = false;
        methods.sign_up = false;
    }
    Ok(Json(methods))
}
