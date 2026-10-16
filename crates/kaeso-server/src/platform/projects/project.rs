use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_core::rights::{OrgRight, ProjectGrant, ProjectPreset, ProjectRight};
use kaeso_store::pg::{avatars, key_lifecycle, members, projects, service_accounts};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::auth::setup::validate_name;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::project_to_store;
use crate::platform::json::Body;
use crate::platform::orgs::security::validate_lifetime;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project.")]
pub struct ProjectView {
    #[doc = "Id."]
    pub id: ProjectId,
    #[doc = "The org."]
    pub org_id: OrgId,
    #[doc = "Name."]
    #[schema(example = "Support bot")]
    pub name: String,
    #[doc = "Whether it is the org's default project."]
    pub is_default: bool,
    #[doc = "Whether bodies are logged (off when the org or the project turned it off)."]
    pub content_logging: bool,
    #[doc = "The longest lifetime of new keys in days; `null` for the org's limit."]
    pub max_key_lifetime_days: Option<i32>,
    #[doc = "When archived."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    #[doc = "The picture's address, under `me/avatars`; `null` without one."]
    #[schema(
        example = "/platform/v1/me/avatars/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    )]
    pub avatar_url: Option<String>,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl ProjectView {
    pub fn from_row(p: &projects::Project) -> ProjectView {
        ProjectView {
            id: ProjectId::from_uuid(p.id),
            org_id: OrgId::from_uuid(p.org_id),
            name: p.name.clone(),
            is_default: p.is_default,
            content_logging: p.content_logging,
            max_key_lifetime_days: p.max_key_lifetime_days,
            archived_at: p.archived_at,
            avatar_url: crate::platform::views::avatar_url(p.avatar.as_deref()),
            created_at: p.created_at,
        }
    }
}

fn audit_view(p: &projects::Project) -> serde_json::Value {
    serde_json::to_value(ProjectView::from_row(p)).unwrap_or_default()
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProjectFilter {
    #[doc = "Include archived projects (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects",
    summary = "The org's projects: every project with `projects.view.all` or an org\n\
    role that sees every project, otherwise the projects the caller is a\n\
    member of.",
    tag = "projects",
    params(("org" = OrgId, Path), PageParams, ProjectFilter),
    responses((status = 200, body = List<ProjectView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(page): Query<PageParams>,
    Query(filter): Query<ProjectFilter>,
) -> ApiResult<Json<List<ProjectView>>> {
    let scope = ctx.org()?;
    let page = page.page()?;
    let see_all = scope.rights.contains(OrgRight::ProjectsViewAll)
        || scope.every_project.contains(ProjectRight::ProjectView);
    let member = if see_all {
        None
    } else {
        ctx.acting_user().map(|u| u.uuid())
    };
    let rows = projects::list(
        &state.pg,
        scope.org.id,
        member,
        filter.include_archived.unwrap_or(false),
        page.after,
        page.fetch(),
    )
    .await?;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |p| p.id,
        |p| ProjectView::from_row(&p),
    )))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new project.")]
pub struct ProjectCreate {
    #[doc = "The name, 1 to 100 characters."]
    #[schema(example = "Support bot")]
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects",
    summary = "Creates a project. Its creator becomes its admin.",
    tag = "projects",
    params(("org" = OrgId, Path)),
    request_body = ProjectCreate,
    responses((status = 201, body = ProjectView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<ProjectCreate>,
) -> ApiResult<(StatusCode, Json<ProjectView>)> {
    let scope = ctx.org()?;
    let name = validate_name("name", &req.name)?;
    let id = ProjectId::new();
    let mut tx = state.pg.begin().await?;
    projects::insert(&mut *tx, id.uuid(), scope.org.id, &name, false).await?;
    if let Some(user) = ctx.acting_user() {
        members::insert_project(
            &mut *tx,
            id.uuid(),
            user.uuid(),
            &project_to_store(&ProjectGrant::preset(ProjectPreset::Admin)),
        )
        .await?;
    }
    let row = projects::get(&mut *tx, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new project vanished"))?;
    ctx.audit(Some(scope.id()), "project.create")
        .target("project", id)
        .after(audit_view(&row))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(ProjectView::from_row(&row))))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}",
    summary = "The project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectView))
)]
pub async fn get(ctx: Ctx, Path(_p): Path<(String, String)>) -> ApiResult<Json<ProjectView>> {
    Ok(Json(ProjectView::from_row(&ctx.project()?.project)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Project changes.")]
pub struct ProjectUpdate {
    #[doc = "A new name, 1 to 100 characters."]
    pub name: Option<String>,
}

fn archived() -> ApiError {
    ApiError::Conflict("The project is archived and cannot be changed.".into())
}

async fn lock_active(tx: &mut PgConnection, ctx: &Ctx) -> ApiResult<projects::Project> {
    let p = &ctx.project()?.project;
    let row = projects::get_for_update(&mut *tx, p.org_id, p.id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    if row.archived_at.is_some() {
        return Err(archived());
    }
    Ok(row)
}

async fn save(
    tx: &mut PgConnection,
    ctx: &Ctx,
    action: &'static str,
    before: &projects::Project,
    after: &projects::Project,
) -> ApiResult<()> {
    projects::update(&mut *tx, after).await?;
    ctx.audit(Some(ctx.org()?.id()), action)
        .target("project", ProjectId::from_uuid(after.id))
        .before(audit_view(before))
        .after(audit_view(after))
        .write(tx)
        .await
}

async fn change(
    state: &AppState,
    ctx: &Ctx,
    action: &'static str,
    change: impl FnOnce(&mut projects::Project) -> ApiResult<()>,
) -> ApiResult<projects::Project> {
    let mut tx = state.pg.begin().await?;
    let before = lock_active(&mut tx, ctx).await?;
    let mut after = before.clone();
    change(&mut after)?;
    save(&mut tx, ctx, action, &before, &after).await?;
    tx.commit().await?;
    Ok(after)
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}",
    summary = "Renames the project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectUpdate,
    responses((status = 200, body = ProjectView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<ProjectUpdate>,
) -> ApiResult<Json<ProjectView>> {
    let after = change(&state, &ctx, "project.update", |p| {
        if let Some(name) = &req.name {
            p.name = validate_name("name", name)?;
        }
        Ok(())
    })
    .await?;
    Ok(Json(ProjectView::from_row(&after)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A project key lifetime policy.")]
pub struct ProjectKeyPolicy {
    #[doc = "The longest lifetime of new keys in days; never above the org's\n\
        maximum; `null` for the org's limit."]
    #[schema(example = 30)]
    pub max_key_lifetime_days: Option<i32>,
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/projects/{project}/key-policy",
    summary = "Sets the project's maximum key lifetime.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ProjectKeyPolicy,
    responses((status = 200, body = ProjectView))
)]
pub async fn key_policy(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<ProjectKeyPolicy>,
) -> ApiResult<Json<ProjectView>> {
    let org_max = ctx.org()?.org.max_key_lifetime_days;
    let after = change(&state, &ctx, "project.key_policy", |p| {
        let days = validate_lifetime(req.max_key_lifetime_days)?;
        if let (Some(d), Some(org_max)) = (days, org_max)
            && d > org_max
        {
            return Err(ApiError::invalid(
                "max_key_lifetime_days",
                format!("The organization allows at most {org_max} days."),
            ));
        }
        p.max_key_lifetime_days = days;
        Ok(())
    })
    .await?;
    Ok(Json(ProjectView::from_row(&after)))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/projects/{project}/avatar",
    summary = "Sets the project's picture: a square PNG, JPEG or WebP image of at most 512 KiB.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = crate::platform::avatars::AvatarUpload,
    responses((status = 200, body = ProjectView))
)]
pub async fn set_avatar(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<crate::platform::avatars::AvatarUpload>,
) -> ApiResult<Json<ProjectView>> {
    let mut tx = state.pg.begin().await?;
    let before = lock_active(&mut tx, &ctx).await?;
    let picture = crate::platform::avatars::check(&req)?;
    let mut after = before.clone();
    after.avatar = Some(picture.hash.clone());
    avatars::put(&mut *tx, &picture.hash, picture.content_type, &picture.data).await?;
    save(&mut tx, &ctx, "project.update", &before, &after).await?;
    if let Some(old) = before.avatar.as_deref().filter(|old| *old != picture.hash) {
        avatars::delete_if_unused(&mut *tx, old).await?;
    }
    tx.commit().await?;
    Ok(Json(ProjectView::from_row(&after)))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/avatar",
    summary = "Removes the project's picture; its initials show again.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectView))
)]
pub async fn remove_avatar(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectView>> {
    let mut tx = state.pg.begin().await?;
    let before = lock_active(&mut tx, &ctx).await?;
    let Some(old) = before.avatar.clone() else {
        return Ok(Json(ProjectView::from_row(&before)));
    };
    let mut after = before.clone();
    after.avatar = None;
    save(&mut tx, &ctx, "project.update", &before, &after).await?;
    avatars::delete_if_unused(&mut *tx, &old).await?;
    tx.commit().await?;
    Ok(Json(ProjectView::from_row(&after)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/archive",
    summary = "Archives the project with all its keys and service accounts. Archiving\n\
    is permanent; the default project cannot be archived.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ProjectView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ProjectView>> {
    let org = ctx.project()?.project.org_id;
    let mut tx = state.pg.begin().await?;
    projects::lock_default(&mut tx, org).await?;
    let before = lock_active(&mut tx, &ctx).await?;
    if before.is_default {
        return Err(ApiError::Conflict(
            "The default project cannot be archived; make another one the default first.".into(),
        ));
    }
    let mut after = before.clone();
    after.archived_at = Some(ctx.now);
    projects::update(&mut *tx, &after).await?;
    let keys = key_lifecycle::archive_project_keys(&mut *tx, before.id, ctx.now).await?;
    let accounts = service_accounts::archive_all_in_project(&mut *tx, before.id, ctx.now).await?;
    ctx.audit(Some(ctx.org()?.id()), "project.archive")
        .target("project", ProjectId::from_uuid(before.id))
        .after(serde_json::json!({ "archived_keys": keys.len(), "archived_service_accounts": accounts }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    Ok(Json(ProjectView::from_row(&after)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The project that becomes the org's default.")]
pub struct DefaultProjectChoice {
    #[doc = "An active project of the org."]
    pub project_id: ProjectId,
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/default-project",
    summary = "Makes an active project the org's default: the previous\n\
    default becomes an ordinary project, in the same transaction, so the org\n\
    always has exactly one. Every org member who isn't in the new default\n\
    joins it with the project preset of their org preset;\n\
    members keep their places in the previous one. Invites from now on add\n\
    people to the new default.",
    tag = "projects",
    params(("org" = OrgId, Path)),
    request_body = DefaultProjectChoice,
    responses((status = 200, body = ProjectView))
)]
pub async fn set_default(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<DefaultProjectChoice>,
) -> ApiResult<Json<ProjectView>> {
    let scope = ctx.org()?;
    let org = scope.org.id;
    let id = req.project_id.uuid();
    let mut tx = state.pg.begin().await?;
    projects::lock_default(&mut tx, org).await?;
    let chosen = projects::get(&mut *tx, org, id)
        .await?
        .ok_or_else(|| ApiError::not_found("project"))?;
    if chosen.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "An archived project cannot be the default.".into(),
        ));
    }
    if chosen.is_default {
        return Ok(Json(ProjectView::from_row(&chosen)));
    }
    let previous = projects::default_of(&mut *tx, org).await?;
    if !projects::set_default(&mut tx, org, id).await? {
        return Err(ApiError::not_found("project"));
    }
    let joining = members::org_presets(&mut *tx, org)
        .await?
        .into_iter()
        .map(|(user, preset)| {
            let org_role = kaeso_store::pg::members::Grant {
                preset,
                ..Default::default()
            };
            (
                user,
                crate::services::orgs::default_project_preset(&org_role)
                    .as_str()
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let joined = members::join_project(&mut *tx, id, &joining).await?;
    let after = projects::get(&mut *tx, org, id)
        .await?
        .ok_or_else(|| ApiError::internal("the new default project vanished"))?;
    ctx.audit(Some(scope.id()), "project.default")
        .target("project", ProjectId::from_uuid(id))
        .before(serde_json::json!({
            "default_project": previous.map(|p| ProjectId::from_uuid(p.id).to_string()),
        }))
        .after(serde_json::json!({
            "default_project": ProjectId::from_uuid(id).to_string(),
            "joined_members": joined,
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(ProjectView::from_row(&after)))
}
