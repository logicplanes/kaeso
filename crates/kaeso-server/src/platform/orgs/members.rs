use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{MembershipId, OrgId, UserId};
use kaeso_core::rights::OrgGrant;
use kaeso_store::pg::key_lifecycle;
use kaeso_store::pg::members::{self, OrgMember};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::auth::sessions;
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{
    GrantRequest, GrantView, RoleBook, RoleRef, check_org, check_org_peer, lock_roles,
    org_from_store, org_to_store, org_view, requested_role,
};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query, id_param};
use crate::services::managed::Domains;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A user as listed among members.")]
pub struct MemberUser {
    #[doc = "The user."]
    pub id: UserId,
    #[doc = "Their address; `null` for a deleted user."]
    pub email: Option<String>,
    #[doc = "Their name; `\"Deleted user\"` for a deleted user."]
    pub name: String,
    #[doc = "`active`, `deactivated` or `deleted`."]
    pub state: String,
    #[doc = "The picture's address, under `me/avatars`; `null` without one."]
    #[schema(
        example = "/platform/v1/me/avatars/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    )]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org member.")]
pub struct MemberView {
    #[doc = "The membership."]
    pub id: MembershipId,
    #[doc = "The user."]
    pub user: MemberUser,
    #[doc = "Their org role and rights."]
    pub grant: GrantView,
    #[doc = "When they joined."]
    #[serde(with = "time::serde::rfc3339")]
    pub joined_at: OffsetDateTime,
    #[doc = "Whether the org manages their account: their address is at one of its\n\
        verified email domains and it has an SSO connection. Otherwise they are\n\
        a guest, who keeps their own way of signing in."]
    pub managed: bool,
}

impl MemberView {
    pub fn from_row(m: &OrgMember, grant: &OrgGrant, domains: &Domains) -> MemberView {
        let role = RoleRef::of_org(&m.grant());
        MemberView {
            managed: domains
                .manager(m.email.as_deref())
                .is_some_and(|(org, _)| org == m.org_id),
            id: MembershipId::from_uuid(m.id),
            user: MemberUser {
                id: UserId::from_uuid(m.user_id),
                email: m.email.clone(),
                name: crate::platform::views::display_name(m.name.as_deref()),
                state: m.user_state.clone(),
                avatar_url: crate::platform::views::avatar_url(m.avatar.as_deref()),
            },
            grant: org_view(grant, role),
            joined_at: m.created_at,
        }
    }
}

pub(crate) fn grant_of(m: &OrgMember) -> ApiResult<OrgGrant> {
    org_from_store(&m.grant())
}

pub(crate) fn view(m: &OrgMember, domains: &Domains) -> ApiResult<MemberView> {
    Ok(MemberView::from_row(m, &grant_of(m)?, domains))
}

pub(crate) async fn domains(state: &AppState, org: uuid::Uuid) -> ApiResult<Domains> {
    Ok(Domains::load(&state.pg, Some(org)).await?)
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MemberFilter {
    #[doc = "Only members with this org role: a built-in role's name or a custom\n\
        role's id (`role_…`)."]
    pub role: Option<String>,
    #[doc = "Deprecated: only members with this built-in org role; use `role`."]
    pub preset: Option<String>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/members",
    summary = "The org's members.",
    tag = "orgs",
    params(("org" = OrgId, Path), PageParams, MemberFilter),
    responses((status = 200, body = List<MemberView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(page): Query<PageParams>,
    Query(filter): Query<MemberFilter>,
) -> ApiResult<Json<List<MemberView>>> {
    let page = page.page()?;
    let (preset, role) = match filter.role.as_deref().or(filter.preset.as_deref()) {
        None => (None, None),
        Some(r) if r.starts_with("role_") => {
            let id: kaeso_core::id::RoleId = r
                .parse()
                .map_err(|_| ApiError::invalid("role", "`role` is not a role id."))?;
            (None, Some(id.uuid()))
        }
        Some(r) => (Some(r), None),
    };
    let rows = members::list_org(
        &state.pg,
        ctx.org()?.org.id,
        page.after,
        page.fetch(),
        preset,
        role,
    )
    .await?;
    let list = List::from_rows(&page, rows, |m| m.id, |m| m);
    let domains = domains(&state, ctx.org()?.org.id).await?;
    Ok(Json(List {
        data: list
            .data
            .iter()
            .map(|m| view(m, &domains))
            .collect::<ApiResult<_>>()?,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

async fn member(db: impl sqlx::PgExecutor<'_>, ctx: &Ctx, user: &str) -> ApiResult<OrgMember> {
    members::get_org(
        db,
        ctx.org()?.org.id,
        id_param::<UserId>(user, "member")?.uuid(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("member"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/members/{user}",
    summary = "One member.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("user" = UserId, Path)),
    responses((status = 200, body = MemberView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, user)): Path<(String, String)>,
) -> ApiResult<Json<MemberView>> {
    let domains = domains(&state, ctx.org()?.org.id).await?;
    Ok(Json(view(
        &member(&state.pg, &ctx, &user).await?,
        &domains,
    )?))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/members/{user}",
    summary = "Changes a member's org role and individual rights. Only rights the\n\
    caller holds can be given, only to a member whose rights the caller all\n\
    holds; only owners change owners.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("user" = UserId, Path)),
    request_body = GrantRequest,
    responses((status = 200, body = MemberView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, user)): Path<(String, String)>,
    Body(req): Body<GrantRequest>,
) -> ApiResult<Json<MemberView>> {
    let scope = ctx.org()?;
    let mut tx = state.pg.begin().await?;
    let current = member(&mut *tx, &ctx, &user).await?;
    lock_roles(
        &mut tx,
        scope.org.id,
        &[current.role_id, requested_role(&req)],
    )
    .await?;
    let target = member(&mut *tx, &ctx, &user).await?;
    let before = grant_of(&target)?;
    let (after, role) = RoleBook::load(&mut *tx, scope.org.id)
        .await?
        .org_grant(&req)?;
    if (before.is_owner() || after.is_owner()) && !scope.is_owner() {
        return Err(ApiError::PermissionDenied(
            "Only owners can make someone an owner or change an owner's role.".into(),
        ));
    }
    check_org_peer(scope, &before)?;
    check_org(scope, Some(&before), &after)?;
    members::lock_owners(&mut tx, scope.org.id).await?;
    if before.is_owner()
        && !after.is_owner()
        && members::count_owners(&mut *tx, scope.org.id).await? <= 1
    {
        return Err(ApiError::Conflict(
            "This is the organization's last owner; make someone else an owner first.".into(),
        ));
    }
    if !members::update_org(
        &mut *tx,
        scope.org.id,
        target.user_id,
        &org_to_store(&after),
    )
    .await?
    {
        return Err(ApiError::not_found("member"));
    }
    ctx.audit(Some(scope.id()), Action::MemberRightsUpdate)
        .target("user", UserId::from_uuid(target.user_id))
        .before(org_view(&before, RoleRef::of_org(&target.grant())))
        .after(org_view(&after, role))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_user_in_org(&state, scope.org.id, target.user_id).await;
    let domains = domains(&state, scope.org.id).await?;
    Ok(Json(view(
        &member(&state.pg, &ctx, &user).await?,
        &domains,
    )?))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/members/{user}",
    summary = "Removes a member from the org and its projects and archives their user\n\
    keys in the org. Owners are removed only by owners; the last owner never.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("user" = UserId, Path)),
    responses((status = 204, description = "Removed."))
)]
pub async fn remove(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, user)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let scope = ctx.org()?;
    let target = member(&state.pg, &ctx, &user).await?;
    if target.preset == "owner" && !scope.is_owner() {
        return Err(ApiError::PermissionDenied(
            "Only owners can remove an owner.".into(),
        ));
    }
    check_org_peer(scope, &grant_of(&target)?)?;
    remove_member(&state, &ctx, scope.id(), &target).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn remove_member(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    target: &OrgMember,
) -> ApiResult<()> {
    let mut tx = state.pg.begin().await?;
    members::lock_owners(&mut tx, org.uuid()).await?;
    if target.preset == "owner" && members::count_owners(&mut *tx, org.uuid()).await? <= 1 {
        return Err(ApiError::Conflict(
            "This is the organization's last owner; transfer ownership first.".into(),
        ));
    }
    members::delete_org(&mut tx, org.uuid(), target.user_id).await?;
    let limited =
        kaeso_store::pg::limits::delete_of_user(&mut *tx, org.uuid(), None, target.user_id).await?;
    let archived =
        key_lifecycle::archive_user_keys_in_org(&mut *tx, org.uuid(), target.user_id, ctx.now)
            .await?;
    let remaining = members::org_count_of_user(&mut *tx, target.user_id).await?;
    ctx.audit(Some(org), Action::MemberRemove)
        .target("user", UserId::from_uuid(target.user_id))
        .before(serde_json::json!({ "role": RoleRef::of_org(&target.grant()) }))
        .after(serde_json::json!({ "archived_keys": archived.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(state, &archived).await;
    for project in limited {
        crate::services::limits::forget(state, project).await;
    }
    if remaining == 0 {
        sessions::revoke_all(state, UserId::from_uuid(target.user_id), None).await?;
    }
    Ok(())
}
