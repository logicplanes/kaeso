use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, SessionId, UserId};
use kaeso_store::pg::{members, sessions as store, totp, users};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::auth::sessions::{self, IDLE_LIFETIME};
use crate::platform::access::{Ctx, PolicyGap, from_sso, policy_gap};
use crate::platform::cookies;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{RoleRef, org_from_store};
use crate::platform::params::Path;
use crate::platform::views::{SessionView, UserView};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "One org the user belongs to.")]
pub struct Membership {
    #[doc = "The org."]
    pub org_id: OrgId,
    #[doc = "The org's name."]
    #[schema(example = "Acme")]
    pub org_name: String,
    #[doc = "`active` or `scheduled_for_deletion`; a scheduled org is read-only."]
    pub org_state: String,
    #[doc = "When a scheduled deletion becomes final; `null` while active."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub org_deletion_due_at: Option<OffsetDateTime>,
    #[doc = "The org's picture; `null` without one."]
    pub org_avatar_url: Option<String>,
    #[doc = "The user's org role."]
    pub role: RoleRef,
    #[doc = "Deprecated: the built-in role's name (`owner`, `admin`, `billing`,\n\
        `developer`, `auditor`, `member`); `null` for a custom role."]
    #[schema(example = "owner", deprecated)]
    pub preset: Option<String>,
    #[doc = "The user's effective org rights. In an org scheduled for deletion,\n\
        which is read-only, only the rights to see and export, and\n\
        `org.delete`, which restores it."]
    pub rights: Vec<&'static str>,
    #[doc = "The project rights the user's org role holds in every project of the\n\
        org (all of them for owners and admins), read-only ones only while\n\
        the org is scheduled for deletion. Their rights in one project:\n\
        `orgs/{org}/projects/{project}/my-rights`."]
    pub project_rights: Vec<&'static str>,
    #[doc = "Whether this session may use the org: `ok`, `needs_2fa` (set up TOTP\n\
        and sign in with it), `needs_sso` (sign in through the org's\n\
        identity provider) or `needs_other_sign_in` (this session came from\n\
        another org's SSO; sign in with a password or a sign-in link)."]
    #[schema(example = "ok")]
    pub access: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Who is signed in.")]
pub struct WhoAmI {
    #[doc = "The user."]
    pub user: UserView,
    #[doc = "Whether the user has confirmed TOTP."]
    pub totp_enabled: bool,
    #[doc = "This session."]
    pub session: SessionView,
    #[doc = "The user's orgs."]
    pub memberships: Vec<Membership>,
    #[doc = "Whether the user may create organizations (`POST me/orgs`): always\n\
        for instance admins; for everyone else as the instance setting\n\
        `org_creation` says, by their email domain when it is\n\
        `allowed_domains`. Never from a session that came from an org's SSO."]
    pub can_create_orgs: bool,
    #[doc = "Whether this session may use `instance/*`: the user is an instance\n\
        admin and didn't sign in through an org's SSO."]
    pub can_administer_instance: bool,
    #[doc = "Whether orgs can be deleted on this instance, and how long a deletion\n\
        waits."]
    pub org_deletion: OrgDeletionPolicy,
    #[doc = "The language the console is shown in, as in `me`:\n\
        the console reads it at start, before anything else is shown."]
    pub locale: crate::platform::me::profile::UserLocale,
    #[doc = "The install's IANA time zone: the console shows every time in it."]
    #[schema(example = "Europe/Berlin")]
    pub timezone: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The instance's org deletion settings, as members see them.")]
pub struct OrgDeletionPolicy {
    #[doc = "Whether an org can be deleted at all."]
    pub allowed: bool,
    #[doc = "Days a deleted org stays read-only and restorable."]
    #[schema(example = 7)]
    pub grace_days: i32,
}

#[utoipa::path(
    get,
    path = "/auth/session",
    summary = "The signed-in user, this session, and the user's orgs with what this\n\
    session may do in each.",
    tag = "auth",
    responses((status = 200, body = WhoAmI))
)]
pub async fn current(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<WhoAmI>> {
    let session = ctx.session()?;
    let user = users::get(&state.pg, session.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("user"))?;
    let orgs = members::orgs_of_user(&state.pg, session.user_id).await?;
    let memberships = orgs
        .into_iter()
        .map(|m| {
            let stored = m.grant();
            let grant = org_from_store(&stored)?;
            let access = match policy_gap(session, m.org_id, m.require_sso, m.require_2fa) {
                Some(PolicyGap::NeedsSso) => "needs_sso",
                Some(PolicyGap::NeedsOtherSignIn) => "needs_other_sign_in",
                Some(PolicyGap::Needs2fa) => "needs_2fa",
                None => "ok",
            };
            let mut rights = grant.effective().names();
            let mut project_rights = grant.every_project().names();
            if m.org_state == "scheduled_for_deletion" {
                rights.retain(|r| {
                    r.contains(".view") || r.ends_with(".export") || *r == "org.delete"
                });
                project_rights.retain(|r| r.contains(".view") || r.ends_with(".export"));
            }
            Ok(Membership {
                org_id: OrgId::from_uuid(m.org_id),
                org_name: m.org_name,
                org_avatar_url: crate::platform::views::avatar_url(m.org_avatar.as_deref()),
                org_state: m.org_state,
                org_deletion_due_at: m.org_deletion_due_at,
                role: RoleRef::of_org(&stored),
                preset: grant.preset_role().map(|p| p.as_str().to_owned()),
                rights,
                project_rights,
                access,
            })
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let instance = kaeso_store::pg::instance::get(&state.pg).await?;
    Ok(Json(WhoAmI {
        org_deletion: OrgDeletionPolicy {
            allowed: instance.org_deletion,
            grace_days: instance.org_deletion_grace_days,
        },
        user: UserView::from_row(&user),
        totp_enabled: totp::is_enabled(&state.pg, user.id).await?,
        session: SessionView {
            id: SessionId::from_uuid(session.id),
            method: session.method.clone(),
            second_factor: session.second_factor,
            sso_org: session.oidc_org_id.map(OrgId::from_uuid),
            signed_in_at: session.signed_in_at,
            last_used_at: session.last_used_at,
            expires_at: session.expires_at,
        },
        memberships,
        can_create_orgs: !from_sso(session)
            && crate::services::accounts::policy(&instance)
                .may_create_orgs(session.is_instance_admin, user.email.as_deref()),
        can_administer_instance: session.is_instance_admin && !from_sso(session),
        locale: crate::platform::me::profile::UserLocale::from_stored(
            users::get_locale(&state.pg, user.id).await?.as_deref(),
        ),
        timezone: instance.timezone,
    }))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    summary = "Ends this session and clears the cookie.",
    tag = "auth",
    responses((status = 204, description = "Signed out."))
)]
pub async fn logout(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Response> {
    let session = ctx.session()?;
    sessions::revoke(
        &state,
        UserId::from_uuid(session.user_id),
        SessionId::from_uuid(session.id),
    )
    .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookies::clear(&state))],
    )
        .into_response())
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A session in the user's list.")]
pub struct SessionListItem {
    #[doc = "The session."]
    #[serde(flatten)]
    pub session: SessionView,
    #[doc = "When it was created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "The address it signed in from."]
    #[schema(example = "203.0.113.7")]
    pub ip: Option<String>,
    #[doc = "The browser it signed in with."]
    pub user_agent: Option<String>,
    #[doc = "Whether it is the session making this request."]
    pub current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The user's active sessions.")]
pub struct SessionList {
    #[doc = "Newest first."]
    pub data: Vec<SessionListItem>,
}

#[utoipa::path(
    get,
    path = "/auth/sessions",
    summary = "The signed-in user's active sessions, newest first.",
    tag = "auth",
    responses((status = 200, body = SessionList))
)]
pub async fn list(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<SessionList>> {
    let me = ctx.session()?;
    let rows =
        store::list_for_user(&state.pg, me.user_id, ctx.now, ctx.now - IDLE_LIFETIME).await?;
    Ok(Json(SessionList {
        data: rows
            .into_iter()
            .map(|r| SessionListItem {
                current: r.id == me.id,
                session: SessionView {
                    id: SessionId::from_uuid(r.id),
                    method: r.method,
                    second_factor: r.second_factor,
                    sso_org: r.oidc_org_id.map(OrgId::from_uuid),
                    signed_in_at: r.signed_in_at,
                    last_used_at: r.last_used_at,
                    expires_at: r.expires_at,
                },
                created_at: r.created_at,
                ip: r.ip.map(|n| n.addr().to_string()),
                user_agent: r.user_agent,
            })
            .collect(),
    }))
}

#[utoipa::path(
    delete,
    path = "/auth/sessions/{session}",
    summary = "Ends one of the user's sessions.",
    tag = "auth",
    params(("session" = SessionId, Path, description = "The session to end.")),
    responses((status = 204, description = "Ended."))
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(session): Path<String>,
) -> ApiResult<StatusCode> {
    let user = ctx.user_id()?;
    let id: SessionId = session
        .parse()
        .map_err(|_| ApiError::not_found("session"))?;
    if sessions::revoke(&state, user, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("session"))
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "How many sessions ended.")]
pub struct Revoked {
    #[doc = "The number of sessions ended."]
    pub revoked: usize,
}

#[utoipa::path(
    post,
    path = "/auth/sessions/revoke-all",
    summary = "Ends every session of the user except this one.",
    tag = "auth",
    responses((status = 200, body = Revoked))
)]
pub async fn revoke_all(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<Revoked>> {
    let me = ctx.session()?;
    let revoked = sessions::revoke_all(
        &state,
        UserId::from_uuid(me.user_id),
        Some(SessionId::from_uuid(me.id)),
    )
    .await?;
    Ok(Json(Revoked { revoked }))
}
