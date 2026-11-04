use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::catalogue::resolve::{Catalogue, validate_alias_name};
use std::collections::BTreeSet;

use kaeso_codecs::HeaderList;
use kaeso_core::catalogue::route::{self, AppFormat, Feature, Step, Traced};
use kaeso_core::id::{
    AliasId, ApiKeyId, CatalogueModelId, OrgId, ProjectId, ServiceAccountId, UserId,
};
use kaeso_store::pg::catalogue::{self, AliasRow};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{MODELS_ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::services::catalogue as service;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An alias: a name apps send as `model`, and the route requests take.")]
pub struct AliasView {
    #[doc = "Id."]
    pub id: AliasId,
    #[doc = "The name apps send as `model`."]
    #[schema(example = "chat-default")]
    pub name: String,
    #[doc = "The route: the steps a request takes, from the first model to its\n\
        fallbacks."]
    pub route: Step,
    #[doc = "Every model the route names, in the order the route first names them."]
    pub models: Vec<RouteModel>,
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

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A model a route names.")]
pub struct RouteModel {
    #[doc = "The catalogue entry."]
    pub catalogue_model_id: CatalogueModelId,
    #[doc = "The model as `provider/model`, or none when it was removed from the\n\
        catalogue: requests then skip its steps."]
    #[schema(example = "anthropic/claude-sonnet-4-5")]
    pub target: Option<String>,
}

async fn views(
    state: &AppState,
    project: uuid::Uuid,
    rows: Vec<AliasRow>,
) -> ApiResult<Vec<AliasView>> {
    let entries = catalogue::list(&state.pg, project).await?;
    let domain = Catalogue::new(
        entries
            .iter()
            .map(catalogue::EntryRow::entry)
            .collect::<Result<_, _>>()?,
        Vec::new(),
    );
    let loaded = state.kpmr.current();
    Ok(rows
        .into_iter()
        .map(|a| AliasView {
            id: AliasId::from_uuid(a.id),
            shadows_model: domain.shadows(&a.name, &loaded.snapshot),
            models: a
                .route
                .models_in_order()
                .into_iter()
                .map(|id| RouteModel {
                    catalogue_model_id: id,
                    target: domain
                        .model(id)
                        .map(|e| format!("{}/{}", e.provider, e.model_id)),
                })
                .collect(),
            name: a.name,
            route: a.route,
            created_at: a.created_at,
            updated_at: a.updated_at,
        })
        .collect())
}

async fn one(state: &AppState, row: AliasRow) -> ApiResult<AliasView> {
    let project = row.project_id;
    views(state, project, vec![row])
        .await?
        .pop()
        .ok_or_else(|| ApiError::not_found("alias"))
}

async fn check_route(state: &AppState, project: uuid::Uuid, route: &Step) -> ApiResult<()> {
    let known: std::collections::BTreeSet<uuid::Uuid> = catalogue::list(&state.pg, project)
        .await?
        .iter()
        .map(|e| e.id)
        .collect();
    route
        .validate(&|m| known.contains(&m.uuid()))
        .map_err(|e| ApiError::invalid(&e.param, e.message))
}

fn check_name(name: &str) -> ApiResult<()> {
    validate_alias_name(name).map_err(|m| ApiError::invalid("name", format!("An alias {m}.")))
}

fn audited(name: &str, route: &Step) -> serde_json::Value {
    serde_json::json!({ "name": name, "route": route })
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new alias.")]
pub struct AliasCreate {
    #[doc = "The name."]
    #[schema(example = "chat-default")]
    pub name: String,
    #[doc = "The route. A single model is `{\"type\": \"model\", \"model\": \"model_…\"}`."]
    pub route: Step,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/aliases",
    summary = "Creates an alias.",
    description = "Every model the route names must be in the project's catalogue. A\n\
    route has at most 32 steps and is at most 12 steps deep; a refused route\n\
    names the field in `param`, such as `route.fallback.model`. Send a route to\n\
    `POST …/aliases/trace` first to see the models a request would try.",
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
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    check_name(&req.name)?;
    let project = ctx.project()?.project.clone();
    check_route(&state, project.id, &req.route).await?;
    let id = AliasId::new();
    let mut tx = state.pg.begin().await?;
    if !catalogue::insert_alias(
        &mut *tx,
        id.uuid(),
        project.org_id,
        project.id,
        &req.name,
        &req.route,
    )
    .await?
    {
        return Err(ApiError::Conflict(format!(
            "The alias `{}` exists in this project.",
            req.name
        )));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::AliasCreate)
        .target("alias", id)
        .after(audited(&req.name, &req.route))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, project.id).await;
    let row = catalogue::get_alias(&state.pg, project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("alias"))?;
    Ok((StatusCode::CREATED, Json(one(&state, row).await?)))
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
    Ok(Json(List::complete(views(&state, project, rows).await?)))
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
    Ok(Json(one(&state, row).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Alias changes.")]
pub struct AliasUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new route."]
    pub route: Option<Step>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/aliases/{alias}",
    summary = "Renames an alias or changes its route. Requests take the new route at once.",
    description = "Send only what changes. The route is checked as on create; send it to\n\
    `POST …/aliases/trace` first to see the models a request would try.",
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
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let row = alias_row(&state, &ctx, &id).await?;
    let name = req.name.clone().unwrap_or_else(|| row.name.clone());
    check_name(&name)?;
    let route = req.route.unwrap_or_else(|| row.route.clone());
    check_route(&state, row.project_id, &route).await?;
    let mut tx = state.pg.begin().await?;
    match catalogue::update_alias(&mut *tx, row.project_id, row.id, &name, &route).await? {
        None => {
            return Err(ApiError::Conflict(format!(
                "The alias `{name}` exists in this project."
            )));
        }
        Some(false) => return Err(ApiError::not_found("alias")),
        Some(true) => {}
    }
    ctx.audit(Some(ctx.org()?.id()), Action::AliasUpdate)
        .target("alias", AliasId::from_uuid(row.id))
        .before(audited(&row.name, &row.route))
        .after(audited(&name, &route))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    let row = catalogue::get_alias(&state.pg, row.project_id, row.id)
        .await?
        .ok_or_else(|| ApiError::not_found("alias"))?;
    Ok(Json(one(&state, row).await?))
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
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let row = alias_row(&state, &ctx, &id).await?;
    let mut tx = state.pg.begin().await?;
    if !catalogue::delete_alias(&mut *tx, row.project_id, row.id).await? {
        return Err(ApiError::not_found("alias"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::AliasDelete)
        .target("alias", AliasId::from_uuid(row.id))
        .before(audited(&row.name, &row.route))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    service::forget(&state, row.project_id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A route, saved or not, and a request to send through it.")]
pub struct RouteTraceRequest {
    #[doc = "The route."]
    pub route: Step,
    #[doc = "The alias the route belongs to, so splits draw as they do for its\n\
        requests; none for an alias that isn't saved yet."]
    #[serde(default)]
    pub alias: Option<AliasId>,
    #[doc = "The request."]
    pub request: TracedRequest,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "What the gateway knows about a request when it picks a route's steps.")]
pub struct TracedRequest {
    #[doc = "Headers the app sends, up to 20."]
    #[serde(default)]
    pub headers: Vec<TracedHeader>,
    #[doc = "The API format the app speaks."]
    #[serde(default)]
    pub format: Option<AppFormat>,
    #[doc = "What the request uses."]
    #[serde(default)]
    pub features: BTreeSet<Feature>,
    #[doc = "The request's input tokens."]
    #[serde(default)]
    pub input_tokens: u64,
    #[doc = "The API key it is sent with."]
    #[serde(default)]
    pub key: Option<ApiKeyId>,
    #[doc = "The member it is sent by, such as the owner of `key`."]
    #[serde(default)]
    pub member: Option<UserId>,
    #[doc = "The service account it is sent by, such as the owner of `key`."]
    #[serde(default)]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "Stands in for the request's id where a split draws anew: the same\n\
        seed draws the same branches."]
    #[serde(default)]
    pub seed: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A header of a traced request.")]
pub struct TracedHeader {
    #[doc = "The name, lowercase."]
    #[schema(example = "x-plan")]
    pub name: String,
    #[doc = "The value."]
    #[schema(example = "enterprise")]
    pub value: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The steps a request takes through a route, in order.")]
pub struct RouteTrace {
    #[doc = "Every condition and split the request passes and every model it would\n\
        try, in order; each model after the first is tried when the one before\n\
        it fails."]
    pub steps: Vec<TraceStep>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schema(description = "A step a traced request passes.")]
pub enum TraceStep {
    #[doc = "A model the request would try."]
    Model {
        #[doc = "Where the step is in the route, such as `route.fallback`."]
        path: String,
        #[doc = "The model."]
        catalogue_model_id: CatalogueModelId,
        #[doc = "Which try it is, from 1."]
        attempt: u32,
    },
    #[doc = "A condition the request passes."]
    Condition {
        #[doc = "Where the step is in the route."]
        path: String,
        #[doc = "Whether the request takes `then`."]
        holds: bool,
        #[doc = "Whether each rule holds, in the condition's order."]
        rules: Vec<bool>,
    },
    #[doc = "A split the request passes."]
    Split {
        #[doc = "Where the step is in the route."]
        path: String,
        #[doc = "The branch it takes, from 0."]
        branch: u32,
        #[doc = "The branch's weight in percent."]
        weight: u8,
        #[doc = "Its draw from 0 to 99, which falls into the branch's share."]
        draw: u8,
    },
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/aliases/trace",
    summary = "Shows the steps a request would take through a route, without sending it.",
    description = "Checks the route as saving it would, then sends the request's facts\n\
    through it: the conditions it passes, the branch each split draws and every\n\
    model it would try in order. Nothing is called or logged.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = RouteTraceRequest,
    responses((status = 200, body = RouteTrace))
)]
pub async fn trace(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<RouteTraceRequest>,
) -> ApiResult<Json<RouteTrace>> {
    let project = ctx.project()?.project.id;
    check_route(&state, project, &req.route).await?;
    let r = req.request;
    let pairs: Vec<(String, String)> = r.headers.into_iter().map(|h| (h.name, h.value)).collect();
    route::check_trace_headers(&pairs).map_err(|e| ApiError::invalid(&e.param, e.message))?;
    let mut headers = HeaderList::new();
    for (name, value) in pairs {
        headers.push(name, value);
    }
    let caller = r
        .key
        .map(|k| k.uuid())
        .or(r.member.map(|m| m.uuid()))
        .or(r.service_account.map(|s| s.uuid()));
    let facts = route::Facts {
        alias: req.alias.map(|a| a.uuid()).unwrap_or_default(),
        request: uuid::Uuid::from_u64_pair(r.seed, 0),
        caller,
        format: r.format,
        headers: &headers,
        uses: &r.features,
        input_tokens: r.input_tokens,
        key: r.key,
        member: r.member,
        service_account: r.service_account,
    };
    let mut attempt = 0;
    let steps = req
        .route
        .trace(&facts)
        .into_iter()
        .map(|t| match t {
            Traced::Model { path, step } => {
                attempt += 1;
                TraceStep::Model {
                    path,
                    catalogue_model_id: step.model,
                    attempt,
                }
            }
            Traced::Condition { path, rules, holds } => TraceStep::Condition { path, holds, rules },
            Traced::Split {
                path,
                branch,
                weight,
                draw,
            } => TraceStep::Split {
                path,
                branch: u32::try_from(branch).unwrap_or(u32::MAX),
                weight,
                draw,
            },
        })
        .collect();
    Ok(Json(RouteTrace { steps }))
}
