use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, SessionId, UserId};
use kaeso_store::pg::sessions as sessions_store;
use kaeso_store::pg::{members, oidc, sso_identities, users};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::auth::sessions;
use crate::platform::access::Ctx;
use crate::platform::auth::next;
use crate::platform::auth::oidc::{StartResponse, begin_confirm, begin_link};
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
    back to `next` with an error code in `#sso_error=`.",
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
    in through it, and the account's sessions it signed in end.",
    description = "Needs a sign-in within the last 15 minutes. When the session that asks \
    came from that org's SSO, it ends too.",
    tag = "me",
    params(("org" = OrgId, Path)),
    responses((status = 204, description = "Unlinked."))
)]
pub async fn unlink(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(org): Path<String>,
) -> ApiResult<StatusCode> {
    ctx.require_recent_sign_in()?;
    let me = ctx.user_id()?;
    let org: OrgId = org.parse().map_err(|_| ApiError::not_found("link"))?;
    let mut tx = state.pg.begin().await?;
    if !sso_identities::unlink(&mut *tx, org.uuid(), me.uuid()).await? {
        return Err(ApiError::not_found("link"));
    }
    let ended = sessions_store::revoke_sso(&mut *tx, org.uuid(), Some(me.uuid()), ctx.now).await?;
    ctx.audit(Some(org), "user.sso_unlink")
        .target("user", me)
        .after(serde_json::json!({ "sessions_ended": ended.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    sessions::forget(&state, &ended).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org whose identity provider can confirm the sign-in.")]
pub struct ConfirmSsoView {
    #[doc = "The org."]
    pub org_id: OrgId,
    #[doc = "The org's name."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "The name on its sign-in button; `null`: the button says SSO."]
    #[schema(example = "Acme ID")]
    pub provider_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "How the signed-in user can confirm it's them before an action that needs a recent sign-in."
)]
pub struct ConfirmWays {
    #[doc = "With the account's password, and its second factor if it has one."]
    pub password: bool,
    #[doc = "With a sign-in link sent to the account's verified email address."]
    pub magic_link: bool,
    #[doc = "Through these orgs' identity providers, which the account is linked to;\n\
        listed only while `password`, `magic_link` and `set_password` are all\n\
        `false`. A confirmation there makes the session one from that org's SSO.\n\
        A provider that couldn't confirm one isn't listed (`confirms_sign_in` in\n\
        `GET orgs/{org}/security`). The org that manages the account comes first."]
    pub sso: Vec<ConfirmSsoView>,
    #[doc = "The account has no password yet but can set one through a link sent to its\n\
        email address (`POST auth/password-reset`), then confirm with it."]
    pub set_password: bool,
}

#[utoipa::path(
    get,
    path = "/me/sign-in-again",
    summary = "The ways the signed-in user can confirm it's them when an action needs a\n\
    sign-in within the last 15 minutes.",
    description = "Single sign-on is listed only when the account has no other way to confirm, \
    since an identity provider may ignore the request for a fresh sign-in. When the org that \
    manages the account requires SSO, only its identity provider confirms, since the account \
    can't sign in another way.",
    tag = "me",
    responses((status = 200, body = ConfirmWays))
)]
pub async fn confirm_ways(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<ConfirmWays>> {
    let me = ctx.user_id()?;
    let user = users::get(&state.pg, me.uuid())
        .await?
        .ok_or_else(|| ApiError::Authentication("Sign in first.".into()))?;
    let auth = &state.config.auth;
    let sso = sso_identities::linkable(&state.pg, me.uuid())
        .await?
        .into_iter()
        .filter(|r| r.linked_at.is_some() && r.confirms_sign_in != Some(false))
        .map(|r| ConfirmSsoView {
            org_id: OrgId::from_uuid(r.org_id),
            org_name: r.org_name,
            provider_name: r.provider_name,
        })
        .collect();
    let mut ways = ConfirmWays {
        password: auth.password && user.password_hash.is_some(),
        magic_link: auth.magic_link && user.email.is_some() && user.email_verified_at.is_some(),
        sso,
        set_password: auth.password && user.password_hash.is_none() && user.email.is_some(),
    };
    let manager = match user.email.as_deref() {
        Some(email) => crate::services::managed::of_address(&state.pg, email).await?,
        None => None,
    };
    if let Some(manager) = &manager {
        let managing = OrgId::from_uuid(manager.org_id);
        ways.sso.sort_by_key(|s| s.org_id != managing);
        if manager.require_sso {
            ways.password = false;
            ways.magic_link = false;
            ways.set_password = false;
            ways.sso.retain(|s| s.org_id == managing);
        }
    }
    // A provider that ignores max_age=0 fails every confirmation and can't be told apart before
    // one, so SSO is offered only when nothing else would let the person confirm.
    if ways.password || ways.magic_link || ways.set_password {
        ways.sso.clear();
    }
    Ok(Json(ways))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The org whose identity provider confirms the sign-in.")]
pub struct ConfirmRequest {
    #[doc = "The org; one of `sso` in `GET me/sign-in-again`."]
    pub org: OrgId,
    #[doc = "The console path to come back to, as in `POST auth/oidc/start`."]
    #[schema(example = "/orgs/org_01hx/providers")]
    pub next: Option<String>,
}

#[utoipa::path(
    post,
    path = "/me/sign-in-again/sso",
    summary = "Starts confirming the sign-in at an org's identity provider: answers with\n\
    the provider's URL, which asks the person to sign in again.",
    description = "Only the same account, from a fresh sign-in at the provider, confirms: the \
    session is replaced by a new one from that org's SSO, signed in now, and the browser comes \
    back to `next`. On failure it comes back to `next` with an error code in `#sso_error=`, \
    such as `other_account` or `link_required`. `cannot_confirm` means the provider's answer \
    didn't show a fresh sign-in: it left out `auth_time`, or carried an older sign-in the provider \
    reused. The org's connection is then marked so, and `GET me/sign-in-again` stops offering it.",
    tag = "me",
    request_body = ConfirmRequest,
    responses((status = 200, body = StartResponse))
)]
pub async fn confirm(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<ConfirmRequest>,
) -> ApiResult<Response> {
    let me = ctx.user_id()?;
    let session = SessionId::from_uuid(ctx.session()?.id);
    let next = next::validate(&state.config.public_url, req.next.as_deref())?;
    member_with_sso(&state, req.org, me).await?;
    let started = begin_confirm(&state, &ctx, req.org, me, session, next).await?;
    Ok((
        [(header::SET_COOKIE, started.cookie)],
        Json(StartResponse {
            authorization_url: started.authorization_url,
        }),
    )
        .into_response())
}
