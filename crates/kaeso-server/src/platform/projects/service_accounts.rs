use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::rights::ProjectGrant;
use kaeso_store::pg::key_lifecycle;
use kaeso_store::pg::service_accounts::{self, ServiceAccount};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::auth::setup::validate_name;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{
    GrantRequest, GrantView, RoleBook, RoleRef, check_project, project_from_store,
    project_to_store, project_view,
};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A service account.")]
pub struct ServiceAccountView {
    #[doc = "Id."]
    pub id: ServiceAccountId,
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "Name."]
    #[schema(example = "ci-deploy")]
    pub name: String,
    #[doc = "Its project role and rights; its keys act with them."]
    pub grant: GrantView,
    #[doc = "Who created it."]
    pub created_by: Option<UserId>,
    #[doc = "When archived."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub fn grant_of(s: &ServiceAccount) -> ApiResult<ProjectGrant> {
    project_from_store(&s.grant())
}

fn view(s: &ServiceAccount) -> ApiResult<ServiceAccountView> {
    Ok(ServiceAccountView {
        id: ServiceAccountId::from_uuid(s.id),
        project_id: ProjectId::from_uuid(s.project_id),
        name: s.name.clone(),
        grant: project_view(&grant_of(s)?, RoleRef::of_project(&s.grant())),
        created_by: s.created_by.map(UserId::from_uuid),
        archived_at: s.archived_at,
        created_at: s.created_at,
    })
}

fn writable(ctx: &Ctx) -> ApiResult<uuid::Uuid> {
    let p = &ctx.project()?.project;
    if p.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The project is archived and cannot be changed.".into(),
        ));
    }
    Ok(p.id)
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AccountFilter {
    #[doc = "Include archived accounts (default false)."]
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/service-accounts",
    summary = "The project's service accounts.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), PageParams, AccountFilter),
    responses((status = 200, body = List<ServiceAccountView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(page): Query<PageParams>,
    Query(f): Query<AccountFilter>,
) -> ApiResult<Json<List<ServiceAccountView>>> {
    let page = page.page()?;
    let rows = service_accounts::list(
        &state.pg,
        ctx.project()?.project.id,
        f.include_archived.unwrap_or(false),
        page.after,
        page.fetch(),
    )
    .await?;
    let list = List::from_rows(&page, rows, |s| s.id, |s| s);
    Ok(Json(List {
        data: list.data.iter().map(view).collect::<ApiResult<_>>()?,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new service account.")]
pub struct AccountCreate {
    #[doc = "The name, 1 to 100 characters."]
    #[schema(example = "ci-deploy")]
    pub name: String,
    #[doc = "Its project rights."]
    #[serde(flatten)]
    pub grant: GrantRequest,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/service-accounts",
    summary = "Creates a service account with project rights the caller holds.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = AccountCreate,
    responses((status = 201, body = ServiceAccountView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<AccountCreate>,
) -> ApiResult<(StatusCode, Json<ServiceAccountView>)> {
    let project = writable(&ctx)?;
    let name = validate_name("name", &req.name)?;
    let (grant, role) = RoleBook::load(&state.pg, ctx.org()?.org.id)
        .await?
        .project_grant(&req.grant)?;
    check_project(ctx.project()?.rights, None, &grant)?;
    let id = ServiceAccountId::new();
    let mut tx = state.pg.begin().await?;
    service_accounts::insert(
        &mut *tx,
        id.uuid(),
        project,
        &name,
        &project_to_store(&grant),
        ctx.acting_user().map(|u| u.uuid()),
    )
    .await?;
    ctx.audit(Some(ctx.org()?.id()), "service_account.create")
        .target("service_account", id)
        .after(serde_json::json!({ "name": name, "grant": project_view(&grant, role) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let row = service_accounts::get(&state.pg, project, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("service account"))?;
    Ok((StatusCode::CREATED, Json(view(&row)?)))
}

fn account_id(id: &str) -> ApiResult<ServiceAccountId> {
    id.parse()
        .map_err(|_| ApiError::not_found("service account"))
}

async fn account(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<ServiceAccount> {
    service_accounts::get(&state.pg, ctx.project()?.project.id, account_id(id)?.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("service account"))
}

async fn lock_active(tx: &mut PgConnection, ctx: &Ctx, id: &str) -> ApiResult<ServiceAccount> {
    let row = service_accounts::get_for_update(
        &mut *tx,
        ctx.project()?.project.id,
        account_id(id)?.uuid(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("service account"))?;
    if row.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The service account is archived.".into(),
        ));
    }
    Ok(row)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}",
    summary = "One service account.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    responses((status = 200, body = ServiceAccountView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ServiceAccountView>> {
    Ok(Json(view(&account(&state, &ctx, &id).await?)?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to a service account.")]
pub struct AccountUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "New rights."]
    pub grant: Option<GrantRequest>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}",
    summary = "Renames a service account or changes its rights.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    request_body = AccountUpdate,
    responses((status = 200, body = ServiceAccountView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    Body(req): Body<AccountUpdate>,
) -> ApiResult<Json<ServiceAccountView>> {
    writable(&ctx)?;
    let book = match &req.grant {
        Some(_) => Some(RoleBook::load(&state.pg, ctx.org()?.org.id).await?),
        None => None,
    };
    let mut tx = state.pg.begin().await?;
    let before = lock_active(&mut tx, &ctx, &id).await?;
    let mut after = before.clone();
    if let Some(name) = req.name {
        after.name = validate_name("name", &name)?;
    }
    if let (Some(g), Some(book)) = (req.grant, book) {
        let (grant, _) = book.project_grant(&g)?;
        check_project(ctx.project()?.rights, Some(&grant_of(&before)?), &grant)?;
        let mut stored = project_to_store(&grant);
        if let Some(role) = stored.role_id.and_then(|id| book.get(id)) {
            stored.role_name = Some(role.name.clone());
            stored.role_rights = Some(role.rights.clone());
        }
        after.set_grant(stored);
    }
    service_accounts::update(&mut *tx, &after).await?;
    ctx.audit(Some(ctx.org()?.id()), "service_account.update")
        .target("service_account", ServiceAccountId::from_uuid(after.id))
        .before(serde_json::to_value(view(&before)?).unwrap_or_default())
        .after(serde_json::to_value(view(&after)?).unwrap_or_default())
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let keys = sqlx::query_scalar!(
        "SELECT id FROM api_keys WHERE service_account_id = $1",
        after.id
    )
    .fetch_all(&state.pg)
    .await?;
    crate::services::keys::forget(&state, &keys).await;
    Ok(Json(view(&after)?))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/service-accounts/{account}/archive",
    summary = "Archives a service account and its keys. Permanent.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("account" = ServiceAccountId, Path)),
    responses((status = 200, body = ServiceAccountView))
)]
pub async fn archive(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ServiceAccountView>> {
    writable(&ctx)?;
    let mut tx = state.pg.begin().await?;
    let mut row = service_accounts::get_for_update(
        &mut *tx,
        ctx.project()?.project.id,
        account_id(&id)?.uuid(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("service account"))?;
    if row.archived_at.is_some() {
        return Err(ApiError::Conflict(
            "The service account is already archived.".into(),
        ));
    }
    row.archived_at = Some(ctx.now);
    service_accounts::update(&mut *tx, &row).await?;
    let keys = key_lifecycle::archive_service_account_keys(&mut *tx, row.id, ctx.now).await?;
    kaeso_store::pg::catalogue::clear_service_account(&mut *tx, row.id).await?;
    ctx.audit(Some(ctx.org()?.id()), "service_account.archive")
        .target("service_account", ServiceAccountId::from_uuid(row.id))
        .after(serde_json::json!({ "archived_keys": keys.len() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget(&state, &keys).await;
    crate::services::catalogue::forget(&state, row.project_id).await;
    Ok(Json(view(&row)?))
}
