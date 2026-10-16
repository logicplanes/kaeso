use axum::Json;
use axum::extract::State;
use kaeso_core::id::{OrgId, ProjectId, UserId};
use kaeso_core::rights::{
    Category, OrgGrant, OrgRight, ProjectGrant, ProjectRight, Source, effective_project_rights,
    every_project_sources, org_sources, project_sources,
};
use kaeso_store::pg::members;
use serde::Serialize;
use utoipa::ToSchema;

use super::access::{Caller, Ctx};
use super::error::{ApiError, ApiResult};
use super::grants::{RoleRef, org_from_store, project_from_store};
use super::orgs::members::MemberUser;
use super::params::{Path, id_param};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "One right of the catalogue.")]
pub struct RightView {
    #[doc = "The stable id, e.g. `logs.export`."]
    #[schema(example = "logs.export")]
    pub id: &'static str,
    #[doc = "`org` or `project`."]
    #[schema(example = "project")]
    pub scope: &'static str,
    #[doc = "The short name shown in the console."]
    #[schema(example = "Export logs")]
    pub name: &'static str,
    #[doc = "What it allows, as one sentence."]
    #[schema(example = "Export the project's request logs.")]
    pub description: &'static str,
    #[doc = "Whether it exposes sensitive data or is hard to undo."]
    pub sensitive: bool,
    #[doc = "Whether using it in the console needs a sign-in within 15 minutes."]
    pub recent_sign_in: bool,
    #[doc = "Whether it exists only in Kaeso Cloud."]
    pub cloud_only: bool,
    #[doc = "The rights it needs directly; a change that gives it without them, or\n\
        takes one of them away and keeps it, is refused."]
    #[schema(example = json!(["project.view", "logs.metadata.view"]))]
    pub needs: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A group of rights, in display order.")]
pub struct CategoryView {
    #[doc = "The stable id, e.g. `members`."]
    #[schema(example = "logs")]
    pub id: &'static str,
    #[doc = "The name shown in the console."]
    #[schema(example = "Logs & data")]
    pub name: &'static str,
    #[doc = "Its rights, in display order."]
    pub rights: Vec<RightView>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The rights of one scope.")]
pub struct ScopeCatalogue {
    #[doc = "The categories, in display order."]
    pub categories: Vec<CategoryView>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Every right, grouped for the console.")]
pub struct RightsCatalogue {
    #[doc = "Org rights."]
    pub org: ScopeCatalogue,
    #[doc = "Project rights."]
    pub project: ScopeCatalogue,
}

fn group<R: Copy>(
    all: &[R],
    category: impl Fn(R) -> Category,
    view: impl Fn(R) -> RightView,
) -> ScopeCatalogue {
    let mut categories: Vec<CategoryView> = Vec::new();
    for r in all.iter().copied() {
        let c = category(r);
        match categories.last_mut() {
            Some(last) if last.name == c.name() => last.rights.push(view(r)),
            _ => categories.push(CategoryView {
                id: c.id(),
                name: c.name(),
                rights: vec![view(r)],
            }),
        }
    }
    ScopeCatalogue { categories }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/rights",
    summary = "The rights catalogue: every org and project right with its category,\n\
    name, description, flags and the rights it needs.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = RightsCatalogue))
)]
pub async fn catalogue(Path(_org): Path<String>, _ctx: Ctx) -> Json<RightsCatalogue> {
    Json(RightsCatalogue {
        org: group(OrgRight::ALL, OrgRight::category, |r| RightView {
            id: r.as_str(),
            scope: "org",
            name: r.label(),
            description: r.description(),
            sensitive: r.is_sensitive(),
            recent_sign_in: r.needs_recent_sign_in(),
            cloud_only: r.cloud_only(),
            needs: r.needs().into_iter().map(OrgRight::as_str).collect(),
        }),
        project: group(ProjectRight::ALL, ProjectRight::category, |r| RightView {
            id: r.as_str(),
            scope: "project",
            name: r.label(),
            description: r.description(),
            sensitive: r.is_sensitive(),
            recent_sign_in: r.needs_recent_sign_in(),
            cloud_only: r.cloud_only(),
            needs: r.needs().into_iter().map(ProjectRight::as_str).collect(),
        }),
    })
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "One reason a right is held.")]
pub struct SourceView {
    #[doc = "`owner` (owners hold every right), `role` (the org role, for org\n\
        rights), `project_role` (the project role), `added` (added to this\n\
        member individually), `org_role` (the org role's rights in every\n\
        project), `org_right` (an org right that reaches into every\n\
        project) or `key` (an API key acting in the project)."]
    #[schema(example = "role")]
    pub kind: &'static str,
    #[doc = "The role, for `role`, `project_role` and `org_role`."]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<RoleRef>,
    #[doc = "The org right, for `org_right`."]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "projects.view.all")]
    pub right: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A held right and why.")]
pub struct HeldRight {
    #[doc = "The right."]
    #[schema(example = "logs.export")]
    pub right: &'static str,
    #[doc = "Every reason it is held."]
    pub sources: Vec<SourceView>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A right the role gives but that was removed for this member.")]
pub struct RemovedRight {
    #[doc = "The right."]
    #[schema(example = "logs.content.view")]
    pub right: &'static str,
    #[doc = "The role that would give it."]
    pub role: RoleRef,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A member's org rights.")]
pub struct OrgRightsDetail {
    #[doc = "Their org role."]
    pub role: RoleRef,
    #[doc = "Their effective org rights, in catalogue order."]
    pub rights: Vec<HeldRight>,
    #[doc = "Rights of their role removed for them."]
    pub removed: Vec<RemovedRight>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project as named in rights.")]
pub struct ProjectRefView {
    #[doc = "The project."]
    pub id: ProjectId,
    #[doc = "Its name."]
    pub name: String,
    #[doc = "Its picture; `null` without one."]
    pub avatar_url: Option<String>,
    #[doc = "Whether it is the org's default project."]
    pub is_default: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Rights in one project.")]
pub struct ProjectRightsDetail {
    #[doc = "The project."]
    pub project: ProjectRefView,
    #[doc = "The project role; `null` when the rights come only from the org."]
    pub role: Option<RoleRef>,
    #[doc = "The effective project rights, in catalogue order."]
    pub rights: Vec<HeldRight>,
    #[doc = "Rights of the project role removed for this member."]
    pub removed: Vec<RemovedRight>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Everything a member holds in an org and its projects.")]
pub struct MemberRights {
    #[doc = "The member."]
    pub user_id: UserId,
    #[doc = "Their org rights."]
    pub org: OrgRightsDetail,
    #[doc = "The project rights their org role holds in every project."]
    pub every_project: Vec<HeldRight>,
    #[doc = "Their rights in each project they are a member of that the caller\n\
        can see."]
    pub projects: Vec<ProjectRightsDetail>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Rights in one project, for one principal.")]
pub struct ProjectRightsView {
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "The user; `null` for an API key."]
    pub user_id: Option<UserId>,
    #[doc = "The project role; `null` when the rights come only from the org or a key."]
    pub role: Option<RoleRef>,
    #[doc = "The effective project rights, in catalogue order."]
    pub rights: Vec<HeldRight>,
    #[doc = "Rights of the project role removed for this member."]
    pub removed: Vec<RemovedRight>,
}

fn source(s: Source, own: Option<&RoleRef>, org_role: &RoleRef, project: bool) -> SourceView {
    let (kind, role, right) = match s {
        Source::Owner => ("owner", None, None),
        Source::Role if project => ("project_role", own.cloned(), None),
        Source::Role => ("role", own.cloned(), None),
        Source::Added => ("added", None, None),
        Source::OrgRole => ("org_role", Some(org_role.clone()), None),
        Source::OrgRight(r) => ("org_right", None, Some(r.as_str())),
    };
    SourceView { kind, role, right }
}

fn org_detail(grant: &OrgGrant, role: &RoleRef) -> (OrgRightsDetail, Vec<HeldRight>) {
    let rights = org_sources(grant)
        .into_iter()
        .map(|(r, sources)| HeldRight {
            right: r.as_str(),
            sources: sources
                .into_iter()
                .map(|s| source(s, Some(role), role, false))
                .collect(),
        })
        .collect();
    let removed = grant
        .removed
        .intersect(grant.base)
        .iter()
        .map(|r| RemovedRight {
            right: r.as_str(),
            role: role.clone(),
        })
        .collect();
    let every = every_project_sources(grant)
        .into_iter()
        .map(|(r, sources)| HeldRight {
            right: r.as_str(),
            sources: sources
                .into_iter()
                .map(|s| source(s, Some(role), role, false))
                .collect(),
        })
        .collect();
    (
        OrgRightsDetail {
            role: role.clone(),
            rights,
            removed,
        },
        every,
    )
}

fn project_detail(
    org: Option<(&OrgGrant, &RoleRef)>,
    project: Option<(&ProjectGrant, &RoleRef)>,
) -> (Vec<HeldRight>, Vec<RemovedRight>) {
    let none = RoleRef::custom(None, None);
    let org_role = org.map_or(&none, |(_, r)| r);
    let rights = project_sources(org.map(|(g, _)| g), project.map(|(g, _)| g))
        .into_iter()
        .map(|(r, sources)| HeldRight {
            right: r.as_str(),
            sources: sources
                .into_iter()
                .map(|s| source(s, project.map(|(_, r)| r), org_role, true))
                .collect(),
        })
        .collect();
    let removed = project.map_or_else(Vec::new, |(g, role)| {
        g.removed
            .intersect(g.base)
            .iter()
            .map(|r| RemovedRight {
                right: r.as_str(),
                role: role.clone(),
            })
            .collect()
    });
    (rights, removed)
}

fn sees_every_project(ctx: &Ctx) -> ApiResult<bool> {
    let scope = ctx.org()?;
    Ok(scope.rights.contains(OrgRight::ProjectsViewAll)
        || scope.every_project.contains(ProjectRight::ProjectView))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/members/{user}/rights",
    summary = "A member's effective rights in the org and in each of their projects\n\
    the caller can see, each right with its sources.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("user" = UserId, Path)),
    responses((status = 200, body = MemberRights))
)]
pub async fn member(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, user)): Path<(String, String)>,
) -> ApiResult<Json<MemberRights>> {
    let scope = ctx.org()?;
    let user = id_param::<UserId>(&user, "member")?;
    let m = members::get_org(&state.pg, scope.org.id, user.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("member"))?;
    let stored = m.grant();
    let grant = org_from_store(&stored)?;
    let org_role = RoleRef::of_org(&stored);
    let (org, every_project) = org_detail(&grant, &org_role);
    let visible = if sees_every_project(&ctx)? {
        None
    } else {
        Some(match ctx.acting_user() {
            Some(me) => members::projects_of_user(&state.pg, me.uuid())
                .await?
                .into_iter()
                .filter(|p| p.org_id == scope.org.id)
                .map(|p| p.project_id)
                .collect::<Vec<_>>(),
            None => Vec::new(),
        })
    };
    let mut projects = Vec::new();
    for p in members::projects_of_user(&state.pg, user.uuid()).await? {
        if p.org_id != scope.org.id || visible.as_ref().is_some_and(|v| !v.contains(&p.project_id))
        {
            continue;
        }
        let stored = p.grant();
        let pg = project_from_store(&stored)?;
        let role = RoleRef::of_project(&stored);
        let (rights, removed) = project_detail(Some((&grant, &org_role)), Some((&pg, &role)));
        projects.push(ProjectRightsDetail {
            project: ProjectRefView {
                id: ProjectId::from_uuid(p.project_id),
                name: p.project_name.clone(),
                avatar_url: super::views::avatar_url(p.project_avatar.as_deref()),
                is_default: p.is_default,
            },
            role: Some(role),
            rights,
            removed,
        });
    }
    Ok(Json(MemberRights {
        user_id: user,
        org,
        every_project,
        projects,
    }))
}

async fn rights_in_project(
    state: &AppState,
    ctx: &Ctx,
    user: UserId,
) -> ApiResult<ProjectRightsView> {
    let scope = ctx.org()?;
    let project = ctx.project()?;
    let m = members::get_org(&state.pg, scope.org.id, user.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("member"))?;
    let stored = m.grant();
    let org_grant = org_from_store(&stored)?;
    let org_role = RoleRef::of_org(&stored);
    let membership = members::get_project(&state.pg, project.project.id, user.uuid()).await?;
    let own = match &membership {
        Some(pm) => {
            let s = pm.grant();
            Some((project_from_store(&s)?, RoleRef::of_project(&s)))
        }
        None => None,
    };
    let (rights, removed) = project_detail(
        Some((&org_grant, &org_role)),
        own.as_ref().map(|(g, r)| (g, r)),
    );
    Ok(ProjectRightsView {
        project_id: project.id(),
        user_id: Some(user),
        role: own.map(|(_, r)| r),
        rights,
        removed,
    })
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/members/{user}/rights",
    summary = "A member's effective rights in this project, each with its sources:\n\
    their project role, individual changes, their org role's rights in\n\
    every project and org rights that reach into every project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("user" = UserId, Path)),
    responses((status = 200, body = ProjectRightsView))
)]
pub async fn project_member(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, user)): Path<(String, String, String)>,
) -> ApiResult<Json<ProjectRightsView>> {
    Ok(Json(
        rights_in_project(&state, &ctx, id_param::<UserId>(&user, "member")?).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/my-rights",
    summary = "The caller's own effective rights in this project, with sources. The\n\
    console decides what to show in a project from this.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectRightsView))
)]
pub async fn mine(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectRightsView>> {
    if let Caller::Session(s) = &ctx.caller {
        return Ok(Json(
            rights_in_project(&state, &ctx, UserId::from_uuid(s.user_id)).await?,
        ));
    }
    let project = ctx.project()?;
    Ok(Json(ProjectRightsView {
        project_id: project.id(),
        user_id: None,
        role: None,
        rights: project
            .rights
            .iter()
            .map(|r| HeldRight {
                right: r.as_str(),
                sources: vec![SourceView {
                    kind: "key",
                    role: None,
                    right: None,
                }],
            })
            .collect(),
        removed: Vec::new(),
    }))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "People who can grant a right.")]
pub struct Grantors {
    #[doc = "Up to 10 active members who hold the right and may change roles,\n\
        owners first."]
    pub data: Vec<MemberUser>,
}

fn member_user(m: &members::OrgMember) -> MemberUser {
    MemberUser {
        id: UserId::from_uuid(m.user_id),
        email: m.email.clone(),
        name: super::views::display_name(m.name.as_deref()),
        state: m.user_state.clone(),
        avatar_url: super::views::avatar_url(m.avatar.as_deref()),
    }
}

fn ranked(mut found: Vec<(bool, MemberUser)>) -> Grantors {
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    Grantors {
        data: found.into_iter().take(10).map(|(_, u)| u).collect(),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/rights/{right}/grantors",
    summary = "Who can grant an org right: active members holding it and\n\
    `members.rights.update`.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("right" = String, Path, description = "An org right, e.g. `audit_log.view`.")),
    responses((status = 200, body = Grantors))
)]
pub async fn org_grantors(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, right)): Path<(String, String)>,
) -> ApiResult<Json<Grantors>> {
    let right: OrgRight = right
        .parse()
        .map_err(|_| ApiError::not_found("org right"))?;
    let mut found = Vec::new();
    for m in members::all_org(&state.pg, ctx.org()?.org.id).await? {
        if m.user_state != "active" {
            continue;
        }
        let g = org_from_store(&m.grant())?;
        let held = g.effective();
        if held.contains(right) && held.contains(OrgRight::MembersRightsUpdate) {
            found.push((g.is_owner(), member_user(&m)));
        }
    }
    Ok(Json(ranked(found)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/rights/{right}/grantors",
    summary = "Who can grant a project right here: active members holding it and\n\
    `project.members.rights.update` in this project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("right" = String, Path, description = "A project right, e.g. `logs.export`.")),
    responses((status = 200, body = Grantors))
)]
pub async fn project_grantors(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, right)): Path<(String, String, String)>,
) -> ApiResult<Json<Grantors>> {
    let right: ProjectRight = right
        .parse()
        .map_err(|_| ApiError::not_found("project right"))?;
    let project = ctx.project()?.project.id;
    let in_project = members::all_project(&state.pg, project).await?;
    let mut found = Vec::new();
    for m in members::all_org(&state.pg, ctx.org()?.org.id).await? {
        if m.user_state != "active" {
            continue;
        }
        let g = org_from_store(&m.grant())?;
        let own = in_project
            .iter()
            .find(|p| p.user_id == m.user_id)
            .map(|p| project_from_store(&p.grant()))
            .transpose()?;
        let held = effective_project_rights(Some(&g), own.as_ref());
        if held.contains(right) && held.contains(ProjectRight::ProjectMembersRightsUpdate) {
            found.push((g.is_owner(), member_user(&m)));
        }
    }
    Ok(Json(ranked(found)))
}
