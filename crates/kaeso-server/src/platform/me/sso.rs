use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, UserId};
use kaeso_store::pg::{members, oidc, sso_identities};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::auth::next;
use crate::platform::auth::oidc::{StartResponse, begin_link};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org of the user's that signs in through SSO.")]
pub struct SsoLinkView {
    #[doc = "The org."]
    pub org_id: OrgId,
    #[doc = "The org's name."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "The name on its sign-in button; `null`: the button says SSO."]
    #[schema(example = "Acme ID")]
    pub provider_name: Option<String>,
    #[doc = "Whether the account is linked to an identity at the org's identity\n\
        provider. Only a linked account signs in through it."]
    pub linked: bool,
    #[doc = "When it was linked; `null` while it isn't."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub linked_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The user's orgs that sign in through SSO.")]
pub struct SsoLinks {
    #[doc = "By org name."]
    pub data: Vec<SsoLinkView>,
}

#[utoipa::path(
    get,
    path = "/me/sso-links",
    summary = "The user's orgs that sign in through SSO, and whether the account is\n\
    linked to each org's identity provider.",
    tag = "me",
    responses((status = 200, body = SsoLinks))
)]
pub async fn list(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<SsoLinks>> {
    let me = ctx.user_id()?;
    let rows = sso_identities::linkable(&state.pg, me.uuid()).await?;
    Ok(Json(SsoLinks {
        data: rows
            .into_iter()
            .map(|r| SsoLinkView {
                org_id: OrgId::from_uuid(r.org_id),
                org_name: r.org_name,
                provider_name: r.provider_name,
                linked: r.linked_at.is_some(),
                linked_at: r.linked_at,
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The org whose identity provider to link the account to.")]
pub struct LinkRequest {
    #[doc = "The org; the user must be a member and the org must have an SSO\n\
        connection."]
    pub org: OrgId,
    #[doc = "The console path to come back to, as in `POST auth/oidc/start`.\n\
        Without it, the account's security page."]
    #[schema(example = "/settings/account/security")]
    pub next: Option<String>,
}

#[utoipa::path(
    post,
    path = "/me/sso-links",
    summary = "Starts linking the account to an org's identity provider: answers with\n\
    the provider's URL. Whoever signs in there is linked to this account, and\n\
    the browser comes back to `next` signed in as before.",
    description = "Needs a sign-in within the last 15 minutes. On failure the browser comes \
    back to `next` with the reason in `#sso_error=`.",
    tag = "me",
    request_body = LinkRequest,
    responses((status = 200, body = StartResponse))
)]
pub async fn link(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<LinkRequest>,
) -> ApiResult<Response> {
    ctx.require_recent_sign_in()?;
    let me = ctx.user_id()?;
    let next = next::validate(&state.config.public_url, req.next.as_deref())?;
    member_with_sso(&state, req.org, me).await?;
    let linking = begin_link(&state, &ctx, req.org, me, next).await?;
    Ok((
        [(header::SET_COOKIE, linking.cookie)],
        Json(StartResponse {
            authorization_url: linking.authorization_url,
        }),
    )
        .into_response())
}

async fn member_with_sso(state: &AppState, org: OrgId, me: UserId) -> ApiResult<()> {
    let missing = || ApiError::not_found("SSO connection");
    members::get_org(&state.pg, org.uuid(), me.uuid())
        .await?
        .ok_or_else(missing)?;
    oidc::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(missing)?;
    Ok(())
}

#[utoipa::path(
    delete,
    path = "/me/sso-links/{org}",
    summary = "Unlinks the account from an org's identity provider: it no longer signs\n\
    in through it.",
    tag = "me",
    params(("org" = OrgId, Path)),
    responses((status = 204, description = "Unlinked."))
)]
pub async fn unlink(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(org): Path<String>,
) -> ApiResult<StatusCode> {
    let me = ctx.user_id()?;
    let org: OrgId = org.parse().map_err(|_| ApiError::not_found("link"))?;
    let mut tx = state.pg.begin().await?;
    if !sso_identities::unlink(&mut *tx, org.uuid(), me.uuid()).await? {
        return Err(ApiError::not_found("link"));
    }
    ctx.audit(Some(org), "user.sso_unlink")
        .target("user", me)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
