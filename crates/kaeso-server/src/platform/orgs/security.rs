use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::OrgId;
use kaeso_core::rights::OrgPreset;
use kaeso_crypto::{ExposeSecret, Sealed};
use kaeso_store::pg::{oidc, orgs};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::org::{audit_view, lock_changeable};
use crate::auth::oidc::Provider;
use crate::platform::access::Ctx;
use crate::platform::auth::oidc::secret_aad;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{RoleBook, RoleRef, check_org, org_to_store};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The SSO connection, without its secret.")]
pub struct SsoView {
    #[doc = "The issuer URL of the identity provider."]
    #[schema(example = "https://login.acme.example")]
    pub issuer: String,
    #[doc = "The client id Kaeso uses."]
    pub client_id: String,
    #[doc = "The email domains that sign in through it."]
    pub domains: Vec<String>,
    #[doc = "The master key id that wraps the client secret."]
    pub key_id: String,
    #[doc = "Where the provider must send users back (register it there)."]
    pub redirect_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The org's security settings.")]
pub struct SecurityView {
    #[doc = "Whether members need two-factor authentication."]
    pub require_2fa: bool,
    #[doc = "Whether members must sign in through the org's SSO."]
    pub require_sso: bool,
    #[doc = "The longest lifetime of new keys, in days; `null` for no limit."]
    pub max_key_lifetime_days: Option<i32>,
    #[doc = "Deprecated: the built-in org role of SSO users without an invite,\n\
        `custom` for a custom role; `null`: invite only."]
    #[schema(deprecated)]
    pub sso_default_preset: Option<String>,
    #[doc = "The org role of SSO users without an invite; `null`: invite only."]
    pub sso_default_role: Option<RoleRef>,
    #[doc = "The SSO connection, if one is set up."]
    pub sso: Option<SsoView>,
}

async fn view(state: &AppState, org: &orgs::Org) -> ApiResult<SecurityView> {
    let sso = oidc::get(&state.pg, org.id).await?.map(|c| SsoView {
        issuer: c.issuer,
        client_id: c.client_id,
        domains: c.domains,
        key_id: c.key_id,
        redirect_url: crate::auth::oidc::redirect_url(state),
    });
    let sso_default_role = match (org.sso_default_preset.as_deref(), org.sso_default_role_id) {
        (None, _) => None,
        (Some(_), Some(id)) => {
            let book = RoleBook::load(&state.pg, org.id).await?;
            Some(RoleRef::custom(
                Some(id),
                book.get(id).map(|r| r.name.as_str()),
            ))
        }
        (Some(p), None) => p.parse::<OrgPreset>().ok().map(RoleRef::org_builtin),
    };
    Ok(SecurityView {
        require_2fa: org.require_2fa,
        require_sso: org.require_sso,
        max_key_lifetime_days: org.max_key_lifetime_days,
        sso_default_preset: org.sso_default_preset.clone(),
        sso_default_role,
        sso,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/security",
    summary = "The org's security settings.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = SecurityView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<SecurityView>> {
    Ok(Json(view(&state, &ctx.org()?.org).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "An SSO connection.")]
pub struct SsoRequest {
    #[doc = "The issuer URL; Kaeso fetches `/.well-known/openid-configuration` from it."]
    #[schema(example = "https://login.acme.example")]
    pub issuer: String,
    #[doc = "The client id registered at the provider."]
    pub client_id: String,
    #[doc = "The client secret. Required when creating; leave out to keep the stored one. Never returned."]
    #[schema(value_type = Option<String>, write_only)]
    pub client_secret: Option<SecretString>,
    #[doc = "The email domains that sign in through this provider (at least one)."]
    #[schema(example = json!(["acme.example"]))]
    pub domains: Vec<String>,
    #[doc = "The org role of SSO users who arrive without an invite: a built-in\n\
        role other than `owner`, or a custom org role's id; `null` for invite\n\
        only. Only a role whose rights the caller holds can be chosen."]
    #[schema(example = "developer")]
    pub default_role: Option<String>,
    #[doc = "Deprecated: the built-in role; send `default_role` instead."]
    #[schema(deprecated)]
    pub default_preset: Option<String>,
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/sso",
    summary = "Sets up or replaces the SSO connection. The provider's discovery\n\
    document is fetched first, so a mistake shows now, not at sign-in.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = SsoRequest,
    responses((status = 200, body = SecurityView))
)]
pub async fn put_sso(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<SsoRequest>,
) -> ApiResult<Json<SecurityView>> {
    let scope = ctx.org()?;
    let org = scope.id();
    let issuer = url::Url::parse(req.issuer.trim())
        .map_err(|_| ApiError::invalid("issuer", "`issuer` must be a URL."))?;
    if !matches!(issuer.scheme(), "https" | "http") {
        return Err(ApiError::invalid(
            "issuer",
            "`issuer` must be an https URL.",
        ));
    }
    let issuer = issuer.as_str().trim_end_matches('/').to_owned();
    if req.client_id.trim().is_empty() {
        return Err(ApiError::invalid("client_id", "`client_id` is required."));
    }
    let mut domains = Vec::new();
    for (i, d) in req.domains.iter().enumerate() {
        let probe = kaeso_core::email::normalize(&format!("x@{}", d.trim())).map_err(|_| {
            ApiError::invalid(&format!("domains[{i}]"), format!("`{d}` is not a domain."))
        })?;
        domains.push(kaeso_core::email::domain(&probe).to_owned());
    }
    domains.sort();
    domains.dedup();
    if domains.is_empty() {
        return Err(ApiError::invalid(
            "domains",
            "Name at least one email domain whose users sign in through this provider.",
        ));
    }
    let default_role = match req
        .default_role
        .as_deref()
        .or(req.default_preset.as_deref())
    {
        None => None,
        Some(r) => {
            let book = RoleBook::load(&state.pg, scope.org.id).await?;
            let (grant, _) = book.org_role(r, "default_role")?;
            if grant.is_owner() {
                return Err(ApiError::invalid(
                    "default_role",
                    "SSO can't make owners; choose another role.",
                ));
            }
            check_org(scope, None, &grant)?;
            Some(org_to_store(&grant))
        }
    };
    let existing = oidc::get(&state.pg, scope.org.id).await?;
    let secret: String = match (&req.client_secret, &existing) {
        (Some(s), _) => s.expose_secret().to_owned(),
        (None, Some(c)) => {
            let sealed = Sealed {
                ciphertext: c.secret_ciphertext.clone(),
                nonce: c.secret_nonce.clone(),
                wrapped_key: c.wrapped_key.clone(),
                key_id: c.key_id.clone(),
            };
            let plain = kaeso_crypto::open(&state.keyring, &sealed, &secret_aad(org))
                .map_err(ApiError::internal)?;
            String::from_utf8(plain.expose_secret().clone()).map_err(ApiError::internal)?
        }
        (None, None) => {
            return Err(ApiError::invalid(
                "client_secret",
                "`client_secret` is required for a new connection.",
            ));
        }
    };
    Provider::discover(&state, &issuer, req.client_id.trim(), secret.clone())
        .await
        .map_err(|e| {
            tracing::info!(error = %e, "SSO connection unusable");
            ApiError::invalid("issuer", e.message())
        })?;
    let sealed = kaeso_crypto::seal(&state.keyring, secret.as_bytes(), &secret_aad(org))
        .map_err(ApiError::internal)?;
    let mut tx = state.pg.begin().await?;
    let mut row = lock_changeable(&mut tx, &ctx).await?;
    let stored = oidc::upsert(
        &mut tx,
        &oidc::Connection {
            org_id: scope.org.id,
            issuer: issuer.clone(),
            client_id: req.client_id.trim().to_owned(),
            secret_ciphertext: sealed.ciphertext,
            secret_nonce: sealed.nonce,
            wrapped_key: sealed.wrapped_key,
            key_id: sealed.key_id,
            domains: domains.clone(),
        },
    )
    .await?;
    if let Err(domain) = stored {
        return Err(ApiError::Conflict(format!(
            "The domain {domain} already signs in through another organization."
        )));
    }
    row.sso_default_preset = default_role.as_ref().map(|g| g.preset.clone());
    row.sso_default_role_id = default_role.and_then(|g| g.role_id);
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(org), "security.sso_update")
        .target("org", org)
        .after(serde_json::json!({ "issuer": issuer, "client_id": req.client_id.trim(), "domains": domains, "default_preset": row.sso_default_preset, "default_role": row.sso_default_role_id.map(|r| kaeso_core::id::RoleId::from_uuid(r).to_string()), "secret_replaced": req.client_secret.is_some() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(view(&state, &row).await?))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/security/sso",
    summary = "Removes the SSO connection. Refused while SSO is required.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 204, description = "Removed."))
)]
pub async fn delete_sso(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<StatusCode> {
    let scope = ctx.org()?;
    let mut tx = state.pg.begin().await?;
    let mut row = lock_changeable(&mut tx, &ctx).await?;
    if row.require_sso {
        return Err(ApiError::Conflict(
            "SSO is required in this organization; stop requiring it first.".into(),
        ));
    }
    if !oidc::delete(&mut *tx, row.id).await? {
        return Err(ApiError::not_found("SSO connection"));
    }
    row.sso_default_preset = None;
    row.sso_default_role_id = None;
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(scope.id()), "security.sso_delete")
        .target("org", scope.id())
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A switch.")]
pub struct Switch {
    #[doc = "On or off."]
    pub enabled: bool,
}

async fn set_flag(
    state: &AppState,
    ctx: &Ctx,
    action: &'static str,
    apply: impl FnOnce(&mut orgs::Org),
) -> ApiResult<Json<SecurityView>> {
    let scope = ctx.org()?;
    let mut tx = state.pg.begin().await?;
    let before = lock_changeable(&mut tx, ctx).await?;
    let mut row = before.clone();
    apply(&mut row);
    if row.require_sso && !before.require_sso && oidc::get(&mut *tx, row.id).await?.is_none() {
        return Err(ApiError::Conflict(
            "Set up an SSO connection before requiring SSO.".into(),
        ));
    }
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(scope.id()), action)
        .target("org", scope.id())
        .before(audit_view(&before))
        .after(audit_view(&row))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(view(state, &row).await?))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/require-sso",
    summary = "Requires (or stops requiring) members to sign in to this org through\n\
    its SSO. Needs an SSO connection.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = Switch,
    responses((status = 200, body = SecurityView))
)]
pub async fn require_sso(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<Switch>,
) -> ApiResult<Json<SecurityView>> {
    set_flag(&state, &ctx, "security.require_sso", |o| {
        o.require_sso = req.enabled
    })
    .await
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/require-2fa",
    summary = "Requires (or stops requiring) two-factor authentication for members who\n\
    sign in with a password.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = Switch,
    responses((status = 200, body = SecurityView))
)]
pub async fn require_2fa(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<Switch>,
) -> ApiResult<Json<SecurityView>> {
    set_flag(&state, &ctx, "security.require_2fa", |o| {
        o.require_2fa = req.enabled
    })
    .await
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A key lifetime policy.")]
pub struct KeyPolicy {
    #[doc = "The longest lifetime of new keys in days (1 to 3650); `null` for no\n\
        limit. Existing keys keep their expiry."]
    #[schema(example = 90)]
    pub max_key_lifetime_days: Option<i32>,
}

pub(crate) fn validate_lifetime(days: Option<i32>) -> ApiResult<Option<i32>> {
    match days {
        Some(d) if !(1..=3650).contains(&d) => Err(ApiError::invalid(
            "max_key_lifetime_days",
            "Use 1 to 3650 days, or null for no limit.",
        )),
        other => Ok(other),
    }
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/key-policy",
    summary = "Sets the org's maximum key lifetime.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = KeyPolicy,
    responses((status = 200, body = SecurityView))
)]
pub async fn key_policy(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<KeyPolicy>,
) -> ApiResult<Json<SecurityView>> {
    let days = validate_lifetime(req.max_key_lifetime_days)?;
    set_flag(&state, &ctx, "security.key_policy", |o| {
        o.max_key_lifetime_days = days
    })
    .await
}
