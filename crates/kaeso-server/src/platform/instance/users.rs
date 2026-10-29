use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{InviteId, OrgId, UserId};
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_mail::Actor;
use kaeso_store::pg::invites::{self, NewInvite};
use kaeso_store::pg::{members, totp, users};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::auth::sessions;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::RoleRef;
use crate::platform::json::Body;
use crate::platform::orgs::invites::{INVITE_LIFETIME, InviteView};
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::validate::validate_email;
use crate::platform::views::{ManagedBy, UserView};
use crate::services::managed::Domains;
use crate::services::users as lifecycle;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A user as instance admins see them.")]
pub struct InstanceUserView {
    #[doc = "The user."]
    #[serde(flatten)]
    pub user: UserView,
    #[doc = "Whether TOTP is on."]
    pub totp_enabled: bool,
    #[doc = "The org that manages the account: its address is at a verified email\n\
        domain of an org with an SSO connection; `null` when no org does."]
    pub managed_by: Option<ManagedBy>,
}

fn managed_by(domains: &Domains, u: &users::User) -> Option<ManagedBy> {
    domains
        .manager(u.email.as_deref())
        .map(|(org, name)| ManagedBy::new(org, name))
}

async fn view(state: &AppState, u: &users::User) -> ApiResult<InstanceUserView> {
    let domains = Domains::load(&state.pg, None).await?;
    Ok(InstanceUserView {
        user: UserView::from_row(u),
        totp_enabled: totp::is_enabled(&state.pg, u.id).await?,
        managed_by: managed_by(&domains, u),
    })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UserFilter {
    #[doc = "Part of the email address or name."]
    pub search: Option<String>,
    #[doc = "`active`, `deactivated` or `deleted`."]
    pub state: Option<String>,
}

#[utoipa::path(
    get,
    path = "/instance/users",
    summary = "Every user of the install.",
    tag = "instance",
    params(PageParams, UserFilter),
    responses((status = 200, body = List<InstanceUserView>))
)]
pub async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
    Query(f): Query<UserFilter>,
) -> ApiResult<Json<List<InstanceUserView>>> {
    let page = page.page()?;
    let rows = users::list(
        &state.pg,
        page.after,
        page.fetch(),
        f.search.as_deref(),
        f.state.as_deref(),
    )
    .await?;
    let list = List::from_rows(&page, rows, |u| u.id, |u| u);
    let ids: Vec<uuid::Uuid> = list.data.iter().map(|u| u.id).collect();
    let with_totp = totp::enabled_among(&state.pg, &ids).await?;
    let domains = Domains::load(&state.pg, None).await?;
    let data = list
        .data
        .iter()
        .map(|u| InstanceUserView {
            user: UserView::from_row(u),
            totp_enabled: with_totp.contains(&u.id),
            managed_by: managed_by(&domains, u),
        })
        .collect();
    Ok(Json(List {
        data,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

async fn user(state: &AppState, id: &str) -> ApiResult<users::User> {
    let id: UserId = id.parse().map_err(|_| ApiError::not_found("user"))?;
    users::get(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("user"))
}

#[utoipa::path(
    get,
    path = "/instance/users/{user}",
    summary = "One user.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 200, body = InstanceUserView))
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceUserView>> {
    Ok(Json(view(&state, &user(&state, &id).await?).await?))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org a user belongs to, as instance admins see it.")]
pub struct InstanceUserOrg {
    #[doc = "The org."]
    pub id: OrgId,
    #[doc = "Its name."]
    #[schema(example = "Acme")]
    pub name: String,
    #[doc = "`active` or `scheduled_for_deletion`."]
    #[schema(example = "active")]
    pub state: String,
    #[doc = "The user's role in it."]
    pub role: RoleRef,
}

#[utoipa::path(
    get,
    path = "/instance/users/{user}/orgs",
    summary = "The orgs a user belongs to, by name, with their role in each.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 200, body = List<InstanceUserOrg>))
)]
pub async fn orgs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<List<InstanceUserOrg>>> {
    let u = user(&state, &id).await?;
    let mut orgs: Vec<InstanceUserOrg> = members::orgs_of_user(&state.pg, u.id)
        .await?
        .into_iter()
        .map(|o| InstanceUserOrg {
            role: RoleRef::of_org(&o.grant()),
            id: OrgId::from_uuid(o.org_id),
            name: o.org_name,
            state: o.org_state,
        })
        .collect();
    orgs.sort_by_key(|o| o.name.to_lowercase());
    Ok(Json(List::complete(orgs)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "An invitation to the install.")]
pub struct InstanceInvite {
    #[doc = "The address to invite."]
    #[schema(example = "ada@example.com")]
    pub email: String,
}

#[utoipa::path(
    post,
    path = "/instance/users/invites",
    summary = "Invites someone to create an account on this install (without an org).",
    tag = "instance",
    request_body = InstanceInvite,
    responses((status = 201, body = InviteView))
)]
pub async fn invite(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<InstanceInvite>,
) -> ApiResult<(StatusCode, Json<InviteView>)> {
    let email = validate_email("email", &req.email)?;
    if users::email_taken(&state.pg, &email, None).await? {
        return Err(ApiError::Conflict(format!(
            "{email} already has an account."
        )));
    }
    crate::platform::orgs::invites::throttle_invites(&state, &email).await?;
    let token = random_token();
    let id = InviteId::new();
    let mut tx = state.pg.begin().await?;
    invites::insert(
        &mut *tx,
        &NewInvite {
            id: id.uuid(),
            org_id: None,
            email: &email,
            org_preset: None,
            org_role_id: None,
            project_roles: serde_json::json!([]),
            token_hash: &secret::hash(&token),
            invited_by: ctx.acting_user().map(|u| u.uuid()),
            expires_at: ctx.now + INVITE_LIFETIME,
        },
    )
    .await?;
    ctx.audit(None, Action::InstanceInvite)
        .target("invite", id)
        .after(serde_json::json!({ "email": email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let (name, inviter_lang) = match ctx.acting_user() {
        Some(u) => (
            users::get(&state.pg, u.uuid()).await?.and_then(|u| u.name),
            crate::services::mail_lang::chosen(&state.pg, u.uuid()).await,
        ),
        None => (None, None),
    };
    let inviter = name.as_ref().map_or(Actor::Administrator, Actor::from);
    let lang = crate::services::mail_lang::of_address(
        &state.pg,
        &email,
        inviter_lang.as_deref(),
        ctx.accept_language.as_deref(),
    )
    .await;
    state
        .mailer
        .send(kaeso_mail::templates::instance_invitation(
            lang,
            &email,
            inviter,
            &format!("{}invite#token={token}", state.config.public_url),
        ));
    let row = invites::get(&state.pg, None, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("invite"))?;
    Ok((
        StatusCode::CREATED,
        Json(InviteView::from_row(
            &row,
            ctx.now,
            &crate::platform::grants::RoleBook::default(),
        )),
    ))
}

fn not_self(ctx: &Ctx, target: &users::User, what: &str) -> ApiResult<()> {
    if ctx.acting_user().map(|u| u.uuid()) == Some(target.id) {
        return Err(ApiError::invalid(
            "user",
            format!("You cannot {what} your own account here."),
        ));
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/instance/users/{user}/deactivate",
    summary = "Deactivates a user: no sign-in, every session ends, their keys are\n\
    disabled until reactivation.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 200, body = InstanceUserView))
)]
pub async fn deactivate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceUserView>> {
    let target = user(&state, &id).await?;
    not_self(&ctx, &target, "deactivate")?;
    if target.state != "active" {
        return Err(ApiError::Conflict(
            "Only active users can be deactivated.".into(),
        ));
    }
    let uid = UserId::from_uuid(target.id);
    let mut tx = state.pg.begin().await?;
    let keys = lifecycle::deactivate(
        &state,
        &mut tx,
        uid,
        ctx.now,
        ctx.accept_language.as_deref(),
    )
    .await?;
    ctx.audit(None, Action::UserDeactivate)
        .target("user", uid)
        .after(serde_json::json!({ "disabled_keys": keys.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    Ok(Json(view(&state, &user(&state, &id).await?).await?))
}

#[utoipa::path(
    post,
    path = "/instance/users/{user}/reactivate",
    summary = "Reactivates a user and re-enables the keys deactivation disabled.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 200, body = InstanceUserView))
)]
pub async fn reactivate(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceUserView>> {
    let target = user(&state, &id).await?;
    if target.state != "deactivated" {
        return Err(ApiError::Conflict(
            "Only deactivated users can be reactivated.".into(),
        ));
    }
    let uid = UserId::from_uuid(target.id);
    let mut tx = state.pg.begin().await?;
    let keys = lifecycle::reactivate(&mut tx, uid).await?;
    ctx.audit(None, Action::UserReactivate)
        .target("user", uid)
        .after(serde_json::json!({ "enabled_keys": keys.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    sessions::refresh_user(&state, uid).await?;
    Ok(Json(view(&state, &user(&state, &id).await?).await?))
}

#[utoipa::path(
    delete,
    path = "/instance/users/{user}",
    summary = "Deletes a user: name and email are removed, keys archived; usage, logs\n\
    and audit logs keep the id as \"Deleted user\". Refused for the last owner\n\
    of an org. Needs a recent sign-in.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 204, description = "Deleted."))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    ctx.require_recent_sign_in()?;
    let target = user(&state, &id).await?;
    not_self(&ctx, &target, "delete")?;
    if target.state == "deleted" {
        return Err(ApiError::not_found("user"));
    }
    let uid = UserId::from_uuid(target.id);
    let mut tx = state.pg.begin().await?;
    let keys = lifecycle::delete(&state, &mut tx, uid, ctx.now).await?;
    ctx.audit(None, Action::UserDelete)
        .target("user", uid)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/instance/users/{user}/sessions/revoke",
    summary = "Ends every session and agent of a user and tells them.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 204, description = "Ended."))
)]
pub async fn end_sessions(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let target = user(&state, &id).await?;
    let uid = UserId::from_uuid(target.id);
    let ended = sessions::revoke_all(&state, uid, None).await?;
    let mut tx = state.pg.begin().await?;
    ctx.audit(None, Action::UserSessionsRevoke)
        .target("user", uid)
        .after(serde_json::json!({ "ended": ended }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    if let Some(email) = &target.email {
        let lang = crate::services::mail_lang::of_user(
            &state.pg,
            target.id,
            ctx.accept_language.as_deref(),
        )
        .await;
        state.mailer.send(kaeso_mail::templates::sessions_ended(
            lang,
            email,
            Actor::InstallAdministrator,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/instance/users/{user}/reset-2fa",
    summary = "Resets a user's two-factor authentication and ends their sessions and agents.\n\
    Needs a recent sign-in.",
    tag = "instance",
    params(("user" = UserId, Path)),
    responses((status = 204, description = "Reset."))
)]
pub async fn reset_2fa(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    ctx.require_recent_sign_in()?;
    let target = user(&state, &id).await?;
    let uid = UserId::from_uuid(target.id);
    let mut tx = state.pg.begin().await?;
    lifecycle::reset_2fa_requested(
        &state,
        &mut tx,
        uid,
        Actor::InstallAdministrator,
        ctx.accept_language.as_deref(),
    )
    .await?;
    ctx.audit(None, Action::User2faReset)
        .target("user", uid)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "The instance admin flag.")]
pub struct AdminFlag {
    #[doc = "Whether the user administers the install."]
    pub is_instance_admin: bool,
}

#[utoipa::path(
    put,
    path = "/instance/users/{user}/instance-admin",
    summary = "Grants or removes instance administration. The last instance admin\n\
    cannot remove the flag from themselves. Needs a recent sign-in.",
    tag = "instance",
    params(("user" = UserId, Path)),
    request_body = AdminFlag,
    responses((status = 200, body = InstanceUserView))
)]
pub async fn set_admin(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Body(req): Body<AdminFlag>,
) -> ApiResult<Json<InstanceUserView>> {
    ctx.require_recent_sign_in()?;
    let target = user(&state, &id).await?;
    if target.state != "active" {
        return Err(ApiError::Conflict(
            "Only active users can be instance admins.".into(),
        ));
    }
    let uid = UserId::from_uuid(target.id);
    let mut tx = state.pg.begin().await?;
    if !req.is_instance_admin {
        crate::services::users::ensure_not_last_instance_admin(&mut tx, uid).await?;
    }
    users::set_instance_admin(&mut *tx, uid.uuid(), req.is_instance_admin).await?;
    ctx.audit(None, Action::UserInstanceAdmin)
        .target("user", uid)
        .before(serde_json::json!({ "is_instance_admin": target.is_instance_admin }))
        .after(serde_json::json!({ "is_instance_admin": req.is_instance_admin }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    sessions::refresh_user(&state, uid).await?;
    Ok(Json(view(&state, &user(&state, &id).await?).await?))
}
