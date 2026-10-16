use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{MembershipId, OrgId, ProjectId, UserId};
use kaeso_core::rights::ProjectGrant;
use kaeso_store::pg::key_lifecycle;
use kaeso_store::pg::members::{self, ProjectMember};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{
    GrantRequest, GrantView, RoleBook, RoleRef, check_project, check_project_peer,
    project_from_store, project_to_store, project_view,
};
use crate::platform::json::Body;
use crate::platform::orgs::members::MemberUser;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query, id_param};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project member.")]
pub struct ProjectMemberView {
    #[doc = "The membership."]
    pub id: MembershipId,
    #[doc = "The user."]
    pub user: MemberUser,
    #[doc = "Their project role and rights."]
    pub grant: GrantView,
    #[doc = "When they joined the project."]
    #[serde(with = "time::serde::rfc3339")]
    pub joined_at: OffsetDateTime,
}

pub(crate) fn grant_of(m: &ProjectMember) -> ApiResult<ProjectGrant> {
    project_from_store(&m.grant())
}

fn view(m: &ProjectMember) -> ApiResult<ProjectMemberView> {
    Ok(ProjectMemberView {
        id: MembershipId::from_uuid(m.id),
        user: MemberUser {
            id: UserId::from_uuid(m.user_id),
            email: m.email.clone(),
            name: crate::platform::views::display_name(m.name.as_deref()),
            state: if m.email.is_some() {
                "active".into()
            } else {
                "deleted".into()
            },
            avatar_url: crate::platform::views::avatar_url(m.avatar.as_deref()),
        },
        grant: project_view(&grant_of(m)?, RoleRef::of_project(&m.grant())),
        joined_at: m.created_at,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/members",
    summary = "The project's members.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams),
    responses((status = 200, body = List<ProjectMemberView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<List<ProjectMemberView>>> {
    let page = page.page()?;
    let rows = members::list_project(
        &state.pg,
        ctx.project()?.project.id,
        page.after,
        page.fetch(),
    )
    .await?;
    let list = List::from_rows(&page, rows, |m| m.id, |m| m);
    Ok(Json(List {
        data: list.data.iter().map(view).collect::<ApiResult<_>>()?,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "An org member to add.")]
pub struct AddMember {
    #[doc = "A member of the org."]
    pub user_id: UserId,
    #[doc = "Their project role and rights."]
    #[serde(flatten)]
    pub grant: GrantRequest,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/members",
    summary = "Adds an org member to the project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = AddMember,
    responses((status = 201, body = ProjectMemberView))
)]
pub async fn add(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<AddMember>,
) -> ApiResult<(StatusCode, Json<ProjectMemberView>)> {
    let project = ensure_active(&ctx, ARCHIVED)?;
    let (grant, role) = RoleBook::load(&state.pg, ctx.org()?.org.id)
        .await?
        .project_grant(&req.grant)?;
    check_project(ctx.project()?.rights, None, &grant)?;
    members::get_org(&state.pg, ctx.org()?.org.id, req.user_id.uuid())
        .await?
        .filter(|m| m.user_state == "active")
        .ok_or_else(|| {
            ApiError::invalid(
                "user_id",
                "Only active members of the organization can join its projects.",
            )
        })?;
    let mut tx = state.pg.begin().await?;
    if !members::insert_project(
        &mut *tx,
        project,
        req.user_id.uuid(),
        &project_to_store(&grant),
    )
    .await?
    {
        return Err(ApiError::Conflict(
            "The user is already a member of this project.".into(),
        ));
    }
    ctx.audit(Some(ctx.org()?.id()), "project.member_add")
        .target("user", req.user_id)
        .after(serde_json::json!({ "project": ProjectId::from_uuid(project).to_string(), "grant": project_view(&grant, role) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_user_in_org(&state, ctx.org()?.org.id, req.user_id.uuid()).await;
    let row = members::get_project(&state.pg, project, req.user_id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("member"))?;
    Ok((StatusCode::CREATED, Json(view(&row)?)))
}

async fn member(state: &AppState, ctx: &Ctx, user: &str) -> ApiResult<ProjectMember> {
    let user: UserId = id_param(user, "member")?;
    members::get_project(&state.pg, ctx.project()?.project.id, user.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("member"))
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/members/{user}",
    summary = "Changes a project member's role and individual rights.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    request_body = GrantRequest,
    responses((status = 200, body = ProjectMemberView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
    Body(req): Body<GrantRequest>,
) -> ApiResult<Json<ProjectMemberView>> {
    let project = ensure_active(&ctx, ARCHIVED)?;
    let target = member(&state, &ctx, &user).await?;
    let before = grant_of(&target)?;
    let (after, role) = RoleBook::load(&state.pg, ctx.org()?.org.id)
        .await?
        .project_grant(&req)?;
    check_project_peer(ctx.project()?.rights, &before)?;
    check_project(ctx.project()?.rights, Some(&before), &after)?;
    let mut tx = state.pg.begin().await?;
    if !members::update_project(&mut *tx, project, target.user_id, &project_to_store(&after))
        .await?
    {
        return Err(ApiError::not_found("member"));
    }
    ctx.audit(Some(ctx.org()?.id()), "project.member_rights_update")
        .target("user", UserId::from_uuid(target.user_id))
        .before(project_view(&before, RoleRef::of_project(&target.grant())))
        .after(project_view(&after, role))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_user_in_org(&state, ctx.org()?.org.id, target.user_id).await;
    Ok(Json(view(&member(&state, &ctx, &user).await?)?))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/members/{user}",
    summary = "Removes a member from the project and archives their keys in it. Their\n\
    model access and limits in the project go with the membership.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    responses((status = 204, description = "Removed."))
)]
pub async fn remove(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    let project = ensure_active(&ctx, ARCHIVED)?;
    let target = member(&state, &ctx, &user).await?;
    check_project_peer(ctx.project()?.rights, &grant_of(&target)?)?;
    let mut tx = state.pg.begin().await?;
    members::delete_project(&mut *tx, project, target.user_id).await?;
    let limits = kaeso_store::pg::limits::delete_of_user(
        &mut *tx,
        ctx.org()?.org.id,
        Some(project),
        target.user_id,
    )
    .await?;
    let keys =
        key_lifecycle::archive_user_keys_in_project(&mut *tx, project, target.user_id, ctx.now)
            .await?;
    ctx.audit(Some(ctx.org()?.id()), "project.member_remove")
        .target("user", UserId::from_uuid(target.user_id))
        .after(serde_json::json!({ "project": ProjectId::from_uuid(project).to_string(), "archived_keys": keys.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    crate::services::catalogue::forget(&state, project).await;
    if !limits.is_empty() {
        crate::services::limits::forget(&state, project).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/members/{user}",
    summary = "One project member.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    responses((status = 200, body = ProjectMemberView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
) -> ApiResult<Json<ProjectMemberView>> {
    Ok(Json(view(&member(&state, &ctx, &user).await?)?))
}
