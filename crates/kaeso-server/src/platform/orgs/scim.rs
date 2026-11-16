use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::OrgId;
use kaeso_core::keys::{KeyKind, PROJECT_PREFIX};
use kaeso_store::pg::{oidc, scim as store};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::params::Path;
use crate::state::AppState;

/// How long a replaced SCIM token keeps working, so the identity provider can take the new one.
pub const GRACE: Duration = Duration::hours(24);

/// What every SCIM token starts with.
pub const TOKEN_PREFIX: &str = "sk-kso-scim-";

const NO_SSO: &str = "Connect single sign-on first; SCIM manages the accounts it signs in.";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "The organization's SCIM provisioning.")]
pub struct ScimStatus {
    #[doc = "Whether the organization has a SCIM token, so its identity provider can provision."]
    pub enabled: bool,
    #[doc = "The SCIM base URL to give the identity provider."]
    pub base_url: String,
    #[doc = "The last four characters of the token."]
    pub last4: Option<String>,
    #[doc = "When the token was created."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[doc = "Until when the token it replaced still works."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub previous_valid_until: Option<OffsetDateTime>,
    #[doc = "When the identity provider last called."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "The last request Kaeso refused or couldn't finish, as it answered it."]
    pub last_error: Option<String>,
    #[doc = "When that request came."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_error_at: Option<OffsetDateTime>,
    #[doc = "How many members the identity provider provisioned and keeps active."]
    pub provisioned: usize,
}

async fn status(state: &AppState, org: OrgId) -> ApiResult<ScimStatus> {
    let token = store::token(&state.pg, org.uuid()).await?;
    let provisioned = store::provisioned(&state.pg, org.uuid()).await?.len();
    let now = state.clock.now();
    Ok(ScimStatus {
        enabled: token.is_some(),
        base_url: crate::scim::base_url(state, org),
        last4: token.as_ref().map(|t| t.last4.clone()),
        created_at: token.as_ref().map(|t| t.created_at),
        previous_valid_until: token
            .as_ref()
            .and_then(|t| t.previous_until)
            .filter(|u| *u > now),
        last_used_at: token.as_ref().and_then(|t| t.last_used_at),
        last_error: token.as_ref().and_then(|t| t.last_error.clone()),
        last_error_at: token.as_ref().and_then(|t| t.last_error_at),
        provisioned,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/scim",
    summary = "The organization's SCIM provisioning: its base URL, token and last use.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = ScimStatus))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<ScimStatus>> {
    Ok(Json(status(&state, ctx.org()?.id()).await?))
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[schema(description = "A new SCIM token, shown once.")]
pub struct ScimTokenCreated {
    #[doc = "The bearer token for the identity provider. Kaeso keeps only its hash."]
    pub token: String,
    #[doc = "The provisioning with the new token."]
    pub scim: ScimStatus,
}

impl std::fmt::Debug for ScimTokenCreated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScimTokenCreated")
            .field("token", &"[redacted]")
            .field("scim", &self.scim)
            .finish()
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/scim/token",
    summary = "Creates the organization's SCIM token, or replaces it; a replaced token\n\
    keeps working for 24 hours. The token is shown once.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 201, body = ScimTokenCreated))
)]
pub async fn rotate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<(StatusCode, Json<ScimTokenCreated>)> {
    let org = ctx.org()?.id();
    if oidc::get(&state.pg, org.uuid()).await?.is_none() {
        return Err(ApiError::Conflict(NO_SSO.to_owned()));
    }
    let made = crate::platform::keys::new_secret(KeyKind::Project);
    let random = made
        .secret
        .expose_secret()
        .strip_prefix(PROJECT_PREFIX)
        .unwrap_or_default()
        .to_owned();
    let token = format!("{TOKEN_PREFIX}{random}");
    let hash = kaeso_core::secret::hash(&token);
    let mut tx = state.pg.begin().await?;
    let replaced = store::token(&mut *tx, org.uuid()).await?.is_some();
    store::set_token(
        &mut *tx,
        org.uuid(),
        &hash,
        &made.last4,
        ctx.now + GRACE,
        ctx.now,
    )
    .await?;
    ctx.audit(Some(org), Action::SecurityScimRotate)
        .target("org", org)
        .after(serde_json::json!({ "last4": made.last4, "replaced": replaced }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(ScimTokenCreated {
            token,
            scim: status(&state, org).await?,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/scim/token",
    summary = "Deletes the organization's SCIM token at once, its replaced token too. Members\n\
    stay; the identity provider can no longer change them.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 204, description = "Deleted."))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<StatusCode> {
    let org = ctx.org()?.id();
    let mut tx = state.pg.begin().await?;
    if !store::delete_token(&mut *tx, org.uuid()).await? {
        return Err(ApiError::not_found("SCIM token"));
    }
    ctx.audit(Some(org), Action::SecurityScimDelete)
        .target("org", org)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
