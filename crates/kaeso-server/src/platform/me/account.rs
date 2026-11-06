use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, ProjectId, UserId};
use kaeso_store::pg::{instance, members, users};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::platform::access::{Area, Ctx, agent_scope, require_policy};
use crate::platform::audit::Action;
use crate::platform::auth::setup::validate_name;
use crate::platform::cookies;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::orgs::members::{Removal, remove_member};
use crate::platform::orgs::org::OrgView;
use crate::platform::params::Path;
use crate::services::accounts;
use crate::services::users as lifecycle;
use crate::state::AppState;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new org.")]
pub struct OrgCreate {
    #[doc = "The name."]
    #[schema(example = "Acme")]
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/me/orgs",
    summary = "Creates an org; the caller becomes its owner. Allowed for instance admins,\n\
    and for others as the instance setting `org_creation` says;\n\
    `auth/session` reports it as `can_create_orgs`.",
    tag = "me",
    request_body = OrgCreate,
    responses((status = 201, body = OrgView))
)]
pub async fn create_org(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<OrgCreate>,
) -> ApiResult<(StatusCode, Json<OrgView>)> {
    let session = ctx.session()?;
    let email = users::get(&state.pg, session.user_id)
        .await?
        .and_then(|u| u.email);
    let policy = accounts::policy(&instance::get(&state.pg).await?);
    if let Some(refused) =
        accounts::refuse_org_creation(&policy, session.is_instance_admin, email.as_deref())
    {
        return Err(refused);
    }
    let name = validate_name("name", &req.name)?;
    let me = ctx.user_id()?;
    let mut tx = state.pg.begin().await?;
    let org = crate::services::orgs::create(&mut tx, &name, me).await?;
    ctx.audit(Some(org), Action::OrgCreate)
        .target("org", org)
        .after(serde_json::json!({ "name": name, "owner": me.to_string() }))
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgCreate)
        .target("org", org)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let row = kaeso_store::pg::orgs::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    Ok((StatusCode::CREATED, Json(OrgView::from_row(&row))))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project the user belongs to.")]
pub struct MyProject {
    #[doc = "The project."]
    pub id: ProjectId,
    #[doc = "Its name."]
    pub name: String,
    #[doc = "The project's picture; `null` without one."]
    pub avatar_url: Option<String>,
    #[doc = "The user's project role."]
    pub role: crate::platform::grants::RoleRef,
    #[doc = "Deprecated: the built-in role's name, `custom` for a custom role."]
    #[schema(deprecated)]
    pub preset: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org the user belongs to, with their projects in it.")]
pub struct MyMembership {
    #[doc = "The org."]
    pub org_id: OrgId,
    #[doc = "Its name."]
    pub org_name: String,
    #[doc = "`active` or `scheduled_for_deletion`."]
    pub org_state: String,
    #[doc = "When a scheduled deletion becomes final; `null` while active."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub org_deletion_due_at: Option<time::OffsetDateTime>,
    #[doc = "The org's picture; `null` without one."]
    pub org_avatar_url: Option<String>,
    #[doc = "The user's org role."]
    pub role: crate::platform::grants::RoleRef,
    #[doc = "Deprecated: the built-in role's name, `custom` for a custom role."]
    #[schema(deprecated)]
    pub preset: String,
    #[doc = "The user's projects there."]
    pub projects: Vec<MyProject>,
    #[doc = "The user is the org's only owner, so `DELETE me/memberships/{org}` is\n\
        refused until someone else is an owner too."]
    pub last_owner: bool,
    #[doc = "The user muted emails about the org; security emails still arrive."]
    pub notifications_muted: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The user's memberships.")]
pub struct Memberships {
    #[doc = "One entry per org."]
    pub data: Vec<MyMembership>,
}

#[utoipa::path(
    get,
    path = "/me/memberships",
    summary = "Every org and project the user belongs to. An agent sees only the orgs\n\
    and projects it can open.",
    tag = "me",
    responses((status = 200, body = Memberships))
)]
pub async fn memberships(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<Memberships>> {
    let me = ctx.user_id()?;
    let mut orgs = members::orgs_of_user(&state.pg, me.uuid()).await?;
    let mut projects = members::projects_of_user(&state.pg, me.uuid()).await?;
    if let Some(agent) = ctx.agent() {
        let reach = crate::services::agents::reachable(&state, agent).await?;
        orgs.retain(|o| reach.contains_key(&o.org_id));
        projects.retain(|p| {
            reach
                .get(&p.org_id)
                .is_some_and(|r| r.contains(&p.project_id))
        });
    }
    let last_owner = members::orgs_where_last_owner(&state.pg, me.uuid()).await?;
    let muted = kaeso_store::pg::notifications::muted_orgs(&state.pg, me.uuid()).await?;
    Ok(Json(Memberships {
        data: orgs
            .into_iter()
            .map(|o| MyMembership {
                role: crate::platform::grants::RoleRef::of_org(&o.grant()),
                org_id: OrgId::from_uuid(o.org_id),
                projects: projects
                    .iter()
                    .filter(|p| p.org_id == o.org_id)
                    .map(|p| MyProject {
                        id: ProjectId::from_uuid(p.project_id),
                        name: p.project_name.clone(),
                        avatar_url: crate::platform::views::avatar_url(p.project_avatar.as_deref()),
                        role: crate::platform::grants::RoleRef::of_project(&p.grant()),
                        preset: p.preset.clone(),
                    })
                    .collect(),
                org_avatar_url: crate::platform::views::avatar_url(o.org_avatar.as_deref()),
                org_name: o.org_name,
                org_state: o.org_state,
                org_deletion_due_at: o.org_deletion_due_at,
                preset: o.preset,
                last_owner: last_owner.iter().any(|(id, _)| *id == o.org_id),
                notifications_muted: muted.contains(&o.org_id),
            })
            .collect(),
    }))
}

#[utoipa::path(
    delete,
    path = "/me/memberships/{org}",
    summary = "Leaves an org: removes the user from it and its projects and archives\n\
    their keys there. Refused for the last owner, and while the org is\n\
    scheduled for deletion.",
    tag = "me",
    params(("org" = OrgId, Path)),
    responses((status = 204, description = "Left."))
)]
pub async fn leave(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(org): Path<String>,
) -> ApiResult<StatusCode> {
    let me = ctx.user_id()?;
    let org: OrgId = org.parse().map_err(|_| ApiError::not_found("membership"))?;
    let session = ctx.sign_in()?;
    let leaving = Area::Org {
        id: org.uuid(),
        require_sso: false,
        require_2fa: false,
    };
    require_policy(&state, session, leaving).await?;
    if let Some(agent) = ctx.agent() {
        agent_scope(&state, agent, org, None, true)
            .await
            .map_err(|e| match e {
                ApiError::NotFound(_) => ApiError::not_found("membership"),
                e => e,
            })?;
    }
    members::get_org(&state.pg, org.uuid(), me.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("membership"))?;
    crate::services::orgs::ensure_changeable(&state.pg, org.uuid()).await?;
    remove_member(&state, &ctx, org, me, Removal::Leaving).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/me",
    summary = "Deletes the user's account. Needs a recent sign-in;\n\
    refused while the user is the last owner of an org.",
    tag = "me",
    responses((status = 204, description = "Deleted; the session cookie is cleared."))
)]
pub async fn delete_account(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Response> {
    ctx.require_recent_sign_in()?;
    let me: UserId = ctx.user_id()?;
    let mut tx = state.pg.begin().await?;
    let keys = lifecycle::delete(&state, &mut tx, me, ctx.now).await?;
    ctx.audit(None, Action::UserDelete)
        .target("user", me)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookies::clear(&state))],
    )
        .into_response())
}
