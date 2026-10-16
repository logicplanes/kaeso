use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::catalogue::resolve::{shadows, validate_alias_name};
use kaeso_core::id::{AliasId, CatalogueModelId, OrgId, ProjectId};
use kaeso_store::pg::catalogue::{self, AliasRow};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::models::writable;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::services::catalogue as service;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An alias.")]
pub struct AliasView {
    #[doc = "Id."]
    pub id: AliasId,
    #[doc = "The name apps send as `model`."]
    #[schema(example = "chat-default")]
    pub name: String,
    #[doc = "The catalogue entry it points at."]
    pub catalogue_model_id: CatalogueModelId,
    #[doc = "The target as `provider/model`."]
    #[schema(example = "anthropic/claude-sonnet-4-5")]
    pub target: String,
    #[doc = "Whether the name is also a real model's name, which the alias hides\n\
        in this project."]
    pub shadows_model: bool,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

async fn views(
    state: &AppState,
    project: uuid::Uuid,
    rows: &[AliasRow],
) -> ApiResult<Vec<AliasView>> {
    let entries = catalogue::list(&state.pg, project).await?;
    let domain: Vec<_> = entries
        .iter()
        .map(catalogue::EntryRow::entry)
        .collect::<Result<_, _>>()?;
    let loaded = state.models_dev.current();
    Ok(rows
        .iter()
        .map(|a| AliasView {
            id: AliasId::from_uuid(a.id),
            name: a.name.clone(),
            catalogue_model_id: CatalogueModelId::from_uuid(a.catalogue_model_id),
            target: entries
                .iter()
                .find(|e| e.id == a.catalogue_model_id)
                .map(|e| format!("{}/{}", e.provider, e.model_id))
                .unwrap_or_default(),
            shadows_model: shadows(&a.name, &domain, &loaded.snapshot),
            created_at: a.created_at,
            updated_at: a.updated_at,
        })
        .collect())
}

async fn target(
    state: &AppState,
    project: uuid::Uuid,
    id: CatalogueModelId,
) -> ApiResult<uuid::Uuid> {
    catalogue::get(&state.pg, project, id.uuid())
        .await?
        .map(|e| e.id)
        .ok_or_else(|| {
            ApiError::invalid(
                "catalogue_model_id",
                "No such model in this project's catalogue.",
            )
        })
}

fn check_name(name: &str) -> ApiResult<()> {
    validate_alias_name(name).map_err(|m| ApiError::invalid("name", format!("An alias {m}.")))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new alias.")]
pub struct AliasCreate {
    #[doc = "The name."]
    #[schema(example = "chat-default")]
    pub name: String,
    #[doc = "The catalogue entry."]
    pub catalogue_model_id: CatalogueModelId,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/aliases",
    summary = "Creates an alias.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = AliasCreate,
    responses((status = 201, body = AliasView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<AliasCreate>,
) -> ApiResult<(StatusCode, Json<AliasView>)> {
    writable(&ctx)?;
    check_name(&req.name)?;
    let project = ctx.project()?.project.clone();
    let target = target(&state, project.id, req.catalogue_model_id).await?;
    let id = AliasId::new();
    let mut tx = state.pg.begin().await?;
    if !catalogue::insert_alias(
        &mut *tx,
        id.uuid(),
        project.org_id,
        project.id,
        &req.name,
        target,
    )
    .await?
    {
        return Err(ApiError::Conflict(format!(
            "The alias `{}` exists in this project.",
            req.name
        )));
    }
    ctx.audit(Some(ctx.org()?.id()), "alias.create")
        .target("alias", id)
        .after(
            serde_json::json!({ "name": req.name, "catalogue_model_id": req.catalogue_model_id }),
        )
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, project.id).await;
    let row = catalogue::get_alias(&state.pg, project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("alias"))?;
    let mut v = views(&state, project.id, &[row]).await?;
    let view = v.pop().ok_or_else(|| ApiError::internal("alias view"))?;
    Ok((StatusCode::CREATED, Json(view)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/aliases",
    summary = "The project's aliases.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = List<AliasView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<List<AliasView>>> {
    let project = ctx.project()?.project.id;
    let rows = catalogue::list_aliases(&state.pg, project).await?;
    Ok(Json(List::complete(views(&state, project, &rows).await?)))
}

async fn alias_row(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<AliasRow> {
    let id: AliasId = id.parse().map_err(|_| ApiError::not_found("alias"))?;
    catalogue::get_alias(&state.pg, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("alias"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/aliases/{alias}",
    summary = "One alias.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("alias" = AliasId, Path)),
    responses((status = 200, body = AliasView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<AliasView>> {
    let row = alias_row(&state, &ctx, &id).await?;
    let mut v = views(&state, row.project_id, &[row]).await?;
    v.pop()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("alias"))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Alias changes.")]
pub struct AliasUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new target."]
    pub catalogue_model_id: Option<CatalogueModelId>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/aliases/{alias}",
    summary = "Renames or retargets an alias. Requests use the new target at once.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("alias" = AliasId, Path)),
    request_body = AliasUpdate,
    responses((status = 200, body = AliasView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    Body(req): Body<AliasUpdate>,
) -> ApiResult<Json<AliasView>> {
    writable(&ctx)?;
    let row = alias_row(&state, &ctx, &id).await?;
    let name = req.name.clone().unwrap_or_else(|| row.name.clone());
    check_name(&name)?;
    let target = match req.catalogue_model_id {
        Some(t) => target(&state, row.project_id, t).await?,
        None => row.catalogue_model_id,
    };
    let mut tx = state.pg.begin().await?;
    match catalogue::update_alias(&mut *tx, row.project_id, row.id, &name, target).await? {
        None => {
            return Err(ApiError::Conflict(format!(
                "The alias `{name}` exists in this project."
            )));
        }
        Some(false) => return Err(ApiError::not_found("alias")),
        Some(true) => {}
    }
    ctx.audit(Some(ctx.org()?.id()), "alias.update")
        .target("alias", AliasId::from_uuid(row.id))
        .before(serde_json::json!({ "name": row.name, "catalogue_model_id": CatalogueModelId::from_uuid(row.catalogue_model_id) }))
        .after(serde_json::json!({ "name": name, "catalogue_model_id": CatalogueModelId::from_uuid(target) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    let row = catalogue::get_alias(&state.pg, row.project_id, row.id)
        .await?
        .ok_or_else(|| ApiError::not_found("alias"))?;
    let mut v = views(&state, row.project_id, &[row]).await?;
    v.pop()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("alias"))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/aliases/{alias}",
    summary = "Removes an alias.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("alias" = AliasId, Path)),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    writable(&ctx)?;
    let row = alias_row(&state, &ctx, &id).await?;
    let mut tx = state.pg.begin().await?;
    if !catalogue::delete_alias(&mut *tx, row.project_id, row.id).await? {
        return Err(ApiError::not_found("alias"));
    }
    ctx.audit(Some(ctx.org()?.id()), "alias.delete")
        .target("alias", AliasId::from_uuid(row.id))
        .before(serde_json::json!({ "name": row.name, "catalogue_model_id": CatalogueModelId::from_uuid(row.catalogue_model_id) }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    Ok(StatusCode::NO_CONTENT)
}
