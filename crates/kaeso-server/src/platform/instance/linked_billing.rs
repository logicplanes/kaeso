use axum::Json;
use axum::extract::State;
use kaeso_core::linked_billing as core;
use kaeso_store::pg::linked_billing::{self as store, AppRow, NewApp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::services::linked_billing::{self as link, LinkError};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "How the install pays a provider through its customers' own organizations.")]
pub struct LinkedBillingView {
    #[doc = "The provider, such as `anthropic`."]
    #[schema(value_type = kaeso_core::provider::ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's name."]
    #[schema(example = "Anthropic")]
    pub provider_name: String,
    #[doc = "What an organization links, such as `Claude organization`."]
    #[schema(example = "Claude organization")]
    pub organization: String,
    #[doc = "Where the provider's admins register the install's OAuth app."]
    #[schema(example = "https://platform.claude.com/settings/oauth-apps")]
    pub apps_url: String,
    #[doc = "The provider's guide to setting up the app."]
    pub docs_url: String,
    #[doc = "The redirect URI to register on the app, built from the install's public URL."]
    #[schema(example = "https://kaeso.example.com/linked-billing/callback")]
    pub redirect_uri: String,
    #[doc = "The issuer to register as the app's identity provider: the install signs its\n\
        client assertions itself."]
    #[schema(example = "https://kaeso.example.com/platform/v1/linked-billing/oidc")]
    pub issuer: String,
    #[doc = "Where the issuer's public keys are, for an identity provider registered by\n\
        its JWKS URL."]
    #[schema(example = "https://kaeso.example.com/platform/v1/linked-billing/oidc/jwks.json")]
    pub jwks_url: String,
    #[doc = "The issuer's public keys, to paste for an identity provider registered with\n\
        its key set, as an install the provider can't reach needs."]
    #[schema(value_type = Object)]
    pub jwks: Value,
    #[doc = "The subject of every assertion, for the app's subject pattern."]
    #[schema(example = "kaeso:linked-billing:anthropic")]
    pub subject: String,
    #[doc = "The app, once an instance admin set it up; `null` before."]
    pub app: Option<LinkedBillingApp>,
    #[doc = "How many provider keys of organizations are links through the app."]
    pub linked_keys: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The install's OAuth app at the provider.")]
pub struct LinkedBillingApp {
    #[doc = "The app's client id."]
    #[schema(example = "oaapp_01EXAMPLE")]
    pub client_id: String,
    #[doc = "The audience the app's client authentication expects."]
    pub audience: String,
    #[doc = "The UUID of the install's organization at the provider."]
    pub organization_id: String,
    #[doc = "The federation rule that maps the install's assertions to the app's operator\n\
        service account."]
    #[schema(example = "fdrl_01EXAMPLE")]
    pub federation_rule_id: String,
    #[doc = "The workspace the rule's tokens run in, when the rule covers several."]
    pub workspace_id: Option<String>,
    #[doc = "The operator service account, when set."]
    pub service_account_id: Option<String>,
    #[doc = "Whether each request also names the linked user profile and sets\n\
        `billing_policy`, which the provider's beta may require of some installs."]
    pub send_billing_policy: bool,
    #[doc = "When **Check** last ran; `null` since the app changed."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<OffsetDateTime>,
    #[doc = "What the last check found wrong: `federation` (the service account token was\n\
        refused), `client` (the client id or assertion was refused), `request` (the\n\
        link request was refused, such as a redirect URI that isn't registered),\n\
        `scope`, `unauthorized` (no operator service account or workspace) or\n\
        `unreachable`; `null` when it passed."]
    pub check_problem: Option<String>,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The install's OAuth app at the provider.")]
pub struct LinkedBillingAppSet {
    #[doc = "The app's client id, from the app's page."]
    pub client_id: String,
    #[doc = "The audience the app's client authentication tab shows."]
    pub audience: String,
    #[doc = "The UUID of the install's organization at the provider."]
    pub organization_id: String,
    #[doc = "The federation rule for the operator service account, `fdrl_…`."]
    pub federation_rule_id: String,
    #[doc = "The workspace for the rule's tokens, `wrkspc_…`; needed when the rule covers\n\
        more than one."]
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[doc = "The operator service account, `svac_…`."]
    #[serde(default)]
    pub service_account_id: Option<String>,
    #[doc = "Send the linked user profile and `billing_policy` with each request; turn it\n\
        on only when the provider says the install needs them."]
    #[serde(default)]
    pub send_billing_policy: bool,
}

impl std::fmt::Debug for LinkedBillingAppSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkedBillingAppSet")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

fn offered(provider: &str) -> ApiResult<&'static core::LinkedBilling> {
    core::of(provider).ok_or_else(|| ApiError::not_found("provider"))
}

fn app_view(a: AppRow) -> LinkedBillingApp {
    LinkedBillingApp {
        client_id: a.client_id,
        audience: a.audience,
        organization_id: a.organization_id,
        federation_rule_id: a.federation_rule_id,
        workspace_id: a.workspace_id,
        service_account_id: a.service_account_id,
        send_billing_policy: a.send_billing_policy,
        checked_at: a.checked_at,
        check_problem: a.check_problem,
        updated_at: a.updated_at,
    }
}

fn link_failed(e: LinkError) -> ApiError {
    match e {
        LinkError::Store(e) => e.into(),
        other => ApiError::internal(other),
    }
}

async fn view(state: &AppState, offer: &core::LinkedBilling) -> ApiResult<LinkedBillingView> {
    let issuer = link::issuer(state);
    let app = store::app(&state.pg, offer.provider).await?;
    let linked_keys = i64::try_from(store::linked_keys(&state.pg, offer.provider).await?.len())
        .unwrap_or(i64::MAX);
    Ok(LinkedBillingView {
        provider: offer.provider.to_owned(),
        provider_name: kaeso_core::provider::provider(offer.provider)
            .map_or(offer.provider, |p| p.name)
            .to_owned(),
        organization: offer.organization.to_owned(),
        apps_url: offer.apps_url.to_owned(),
        docs_url: offer.docs_url.to_owned(),
        redirect_uri: link::redirect_uri(state),
        jwks_url: format!("{issuer}/jwks.json"),
        issuer,
        jwks: link::jwks(state).await.map_err(link_failed)?,
        subject: core::subject(offer.provider),
        app: app.map(app_view),
        linked_keys,
    })
}

#[utoipa::path(
    get,
    path = "/instance/linked-billing",
    summary = "The providers whose organizations can pay through linked billing, and the\n\
    install's app at each.",
    tag = "instance",
    responses((status = 200, body = Vec<LinkedBillingView>))
)]
pub async fn list(
    State(state): State<AppState>,
    _ctx: Ctx,
) -> ApiResult<Json<Vec<LinkedBillingView>>> {
    let mut out = Vec::new();
    for offer in core::ALL {
        out.push(view(&state, offer).await?);
    }
    Ok(Json(out))
}

const MAX_ID: usize = 100;

fn field(name: &str, value: &str, max: usize) -> ApiResult<String> {
    let v = value.trim();
    if v.is_empty()
        || v.chars().count() > max
        || v.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(ApiError::invalid(
            name,
            format!("`{name}` must be 1 to {max} characters without spaces."),
        ));
    }
    Ok(v.to_owned())
}

fn optional(name: &str, value: Option<&str>) -> ApiResult<Option<String>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => field(name, v, MAX_ID).map(Some),
    }
}

#[utoipa::path(
    put,
    path = "/instance/linked-billing/{provider}",
    summary = "Sets up or replaces the install's OAuth app for linked billing.",
    description = "Linked keys keep working while the client id stays the same. A new\n\
    client id is a new app, whose links the earlier keys don't have: they show\n\
    `needs_setup` until they are linked again or the earlier app is set back.",
    tag = "instance",
    params(("provider" = kaeso_core::provider::ProviderId, Path)),
    request_body = LinkedBillingAppSet,
    responses((status = 200, body = LinkedBillingView))
)]
pub async fn set(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(provider): Path<String>,
    Body(req): Body<LinkedBillingAppSet>,
) -> ApiResult<Json<LinkedBillingView>> {
    let offer = offered(&provider)?;
    let client_id = field("client_id", &req.client_id, 200)?;
    let audience = field("audience", &req.audience, 500)?;
    let organization_id = field("organization_id", &req.organization_id, MAX_ID)?;
    let federation_rule_id = field("federation_rule_id", &req.federation_rule_id, MAX_ID)?;
    let workspace_id = optional("workspace_id", req.workspace_id.as_deref())?;
    let service_account_id = optional("service_account_id", req.service_account_id.as_deref())?;
    let user = ctx.user_id().ok().map(|u| u.uuid());
    let mut tx = state.pg.begin().await?;
    let before = store::app(&mut *tx, offer.provider).await?;
    let after = store::put_app(
        &mut *tx,
        &NewApp {
            provider: offer.provider,
            client_id: &client_id,
            audience: &audience,
            organization_id: &organization_id,
            federation_rule_id: &federation_rule_id,
            workspace_id: workspace_id.as_deref(),
            service_account_id: service_account_id.as_deref(),
            send_billing_policy: req.send_billing_policy,
            updated_by: user,
        },
    )
    .await?;
    ctx.audit(None, Action::InstanceLinkedBillingUpdate)
        .target("linked_billing", offer.provider)
        .before(before.as_ref().map_or(Value::Null, audited))
        .after(audited(&after))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    match &before {
        Some(b) if b.client_id != after.client_id => {
            crate::services::linked_billing_keys::needs_setup(&state, offer.provider).await;
        }
        _ => {
            crate::services::linked_billing_keys::setup_done(&state, offer.provider).await?;
        }
    }
    Ok(Json(view(&state, offer).await?))
}

fn audited(a: &AppRow) -> Value {
    json!({
        "client_id": a.client_id,
        "audience": a.audience,
        "organization_id": a.organization_id,
        "federation_rule_id": a.federation_rule_id,
        "workspace_id": a.workspace_id,
        "service_account_id": a.service_account_id,
        "send_billing_policy": a.send_billing_policy,
    })
}

#[utoipa::path(
    delete,
    path = "/instance/linked-billing/{provider}",
    summary = "Removes the install's OAuth app for linked billing.",
    description = "Every linked key of the provider shows `needs_setup` and stops paying\n\
    until an app is set up again; the links themselves stay at the provider.",
    tag = "instance",
    params(("provider" = kaeso_core::provider::ProviderId, Path)),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(provider): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    let offer = offered(&provider)?;
    let mut tx = state.pg.begin().await?;
    let before = store::app(&mut *tx, offer.provider)
        .await?
        .ok_or_else(|| ApiError::not_found("app"))?;
    store::delete_app(&mut *tx, offer.provider).await?;
    ctx.audit(None, Action::InstanceLinkedBillingDelete)
        .target("linked_billing", offer.provider)
        .before(audited(&before))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::linked_billing_keys::needs_setup(&state, offer.provider).await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// What a check found wrong, by the step that failed.
fn problem(step: Step, e: &LinkError) -> &'static str {
    match (step, e.code()) {
        (_, None) if matches!(e, LinkError::Unreachable(_) | LinkError::TimedOut) => "unreachable",
        (Step::Federation, _) => "federation",
        (Step::Profile, _) => "federation",
        (Step::Push, Some("invalid_client")) => "client",
        (Step::Push, Some("invalid_scope")) => "scope",
        (Step::Push, Some("unauthorized_client")) => "unauthorized",
        (Step::Push, _) => "request",
    }
}

#[derive(Clone, Copy)]
enum Step {
    Federation,
    Profile,
    Push,
}

#[utoipa::path(
    post,
    path = "/instance/linked-billing/{provider}/check",
    summary = "Checks the install's OAuth app against the provider before anyone links.",
    description = "Gets a token for the app's operator service account, keeps one user\n\
    profile of its own for checks, and makes a link request with the install's\n\
    redirect URI, which nobody approves: a wrong client id, audience or issuer, a\n\
    redirect URI the app doesn't list, or a missing scope shows as `check_problem`.",
    tag = "instance",
    params(("provider" = kaeso_core::provider::ProviderId, Path)),
    responses((status = 200, body = LinkedBillingView))
)]
pub async fn check(
    State(state): State<AppState>,
    _ctx: Ctx,
    Path(provider): Path<String>,
) -> ApiResult<Json<LinkedBillingView>> {
    let offer = offered(&provider)?;
    let app = store::app(&state.pg, offer.provider)
        .await?
        .ok_or_else(|| ApiError::not_found("app"))?;
    let (found, profile) = checked(&state, &app).await;
    if let Some(e) = &found {
        tracing::info!(error = %e.1, "the check of the install's linked billing app failed");
    }
    store::checked(
        &state.pg,
        offer.provider,
        &app.client_id,
        state.clock.now(),
        found.as_ref().map(|(p, _)| *p),
        profile.as_deref(),
    )
    .await?;
    if found.is_none() {
        crate::services::linked_billing_keys::setup_done(&state, offer.provider).await?;
    }
    Ok(Json(view(&state, offer).await?))
}

async fn checked(
    state: &AppState,
    app: &AppRow,
) -> (Option<(&'static str, LinkError)>, Option<String>) {
    let token = match link::operator_token(state, app).await {
        Ok(t) => t,
        Err(e) => return (Some((problem(Step::Federation, &e), e)), None),
    };
    let profile = match &app.check_profile_id {
        Some(p) => p.clone(),
        None => {
            match link::create_profile(state, app, &token, "Kaeso check", "kaeso-check").await {
                Ok(p) => p,
                Err(e) => return (Some((problem(Step::Profile, &e), e)), None),
            }
        }
    };
    let (_, challenge) = link::pkce();
    let probe = kaeso_crypto::random_token();
    match link::push(state, app, &profile, &probe, &challenge).await {
        Ok(_) => (None, Some(profile)),
        Err(e) => (Some((problem(Step::Push, &e), e)), Some(profile)),
    }
}

// --- The install's OpenID issuer (public) -------------------------------------------------

#[utoipa::path(
    get,
    path = "/linked-billing/oidc/.well-known/openid-configuration",
    summary = "The OpenID discovery document of the issuer that signs the install's client\n\
    assertions for linked billing.",
    tag = "instance",
    responses((status = 200, body = Object))
)]
pub async fn discovery(State(state): State<AppState>) -> Json<Value> {
    Json(link::discovery(&state))
}

#[utoipa::path(
    get,
    path = "/linked-billing/oidc/jwks.json",
    summary = "The public keys of the issuer that signs the install's client assertions for\n\
    linked billing.",
    tag = "instance",
    responses((status = 200, body = Object))
)]
pub async fn jwks(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(link::jwks(&state).await.map_err(link_failed)?))
}
