use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::catalogue::Fields;
use kaeso_core::id::{CatalogueModelId, OrgId, ProjectId};
use kaeso_core::provider::ProviderId;
use kaeso_mail::notices::LineChange;
use kaeso_store::pg::catalogue::{self, NewEntry};
use kaeso_store::pg::lines::{self as store, LineRow, NewLine};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};

use super::{MODELS_ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::creator_of;
use crate::platform::params::{Path, Query};
use crate::services::lines::{self as service, Policy};
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "How a project follows a model line when KPMR moves it.")]
pub enum LinePolicy {
    #[doc = "Move when KPMR moves (the default)."]
    Follow,
    #[doc = "Wait until a holder of `models.manage` moves or keeps it."]
    Ask,
    #[doc = "Move `delay_days` days after KPMR moved."]
    Delay,
    #[doc = "Stay on the current model until someone unpins."]
    Pin,
}

fn policy_of(p: LinePolicy, delay_days: Option<i32>) -> ApiResult<Policy> {
    match (p, delay_days) {
        (LinePolicy::Delay, Some(d)) if (1..=30).contains(&d) => Ok(Policy::Delay(d)),
        (LinePolicy::Delay, _) | (_, Some(_)) => Err(ApiError::invalid(
            "delay_days",
            "`delay_days` must be 1 to 30 with the `delay` policy, and absent otherwise.",
        )),
        (LinePolicy::Follow, None) => Ok(Policy::Follow),
        (LinePolicy::Ask, None) => Ok(Policy::Ask),
        (LinePolicy::Pin, None) => Ok(Policy::Pin),
    }
}

fn view_policy(p: Policy) -> LinePolicy {
    match p {
        Policy::Follow => LinePolicy::Follow,
        Policy::Ask => LinePolicy::Ask,
        Policy::Delay(_) => LinePolicy::Delay,
        Policy::Pin => LinePolicy::Pin,
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A move KPMR made that waits: for approval, or for the delay.")]
pub struct PendingMove {
    #[doc = "The model the line would point at."]
    #[schema(example = "claude-opus-5-5")]
    pub model: String,
    #[doc = "Since when it waits."]
    #[serde(with = "time::serde::rfc3339")]
    pub since: OffsetDateTime,
    #[doc = "When the delay ends and the line moves; `null` when it waits for approval."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub moves_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "A model line the project follows: a name such as `claude-opus-latest` that\n\
    points at a provider's newest stable model."
)]
pub struct ModelLineView {
    #[doc = "Its catalogue model: apps call it as `provider/line`, and aliases, model\n\
        access and limits name it like any model."]
    pub id: CatalogueModelId,
    #[doc = "The provider."]
    #[schema(value_type = ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The line's name."]
    #[schema(example = "claude-opus-latest")]
    pub line: String,
    #[doc = "The model requests go to now."]
    #[schema(example = "claude-opus-5-5")]
    pub model: String,
    #[doc = "The model before it, for Move back; `null` when there was none."]
    pub previous: Option<String>,
    #[doc = "Since when the line points at `model` in this project."]
    #[serde(with = "time::serde::rfc3339")]
    pub since: OffsetDateTime,
    #[doc = "How the project follows the line."]
    pub policy: LinePolicy,
    #[doc = "With `delay`: the days a move waits, 1 to 30."]
    pub delay_days: Option<i32>,
    #[doc = "Where KPMR points the line now; `null` when KPMR no longer lists it."]
    pub kpmr_model: Option<String>,
    #[doc = "A move that waits, if one does."]
    pub pending: Option<PendingMove>,
    #[doc = "A model an owner chose not to move to; the line asks again only for a newer one."]
    pub kept: Option<String>,
    #[doc = "Why the line couldn't move to `kpmr_model`, such as a missing provider key."]
    pub held: Option<String>,
}

fn view(state: &AppState, row: &LineRow) -> ModelLineView {
    let policy = Policy::of(&row.policy, row.delay_days);
    let loaded = state.kpmr.current();
    ModelLineView {
        id: CatalogueModelId::from_uuid(row.catalogue_model_id),
        provider: row.provider.clone(),
        line: row.line.clone(),
        model: row.target.clone(),
        previous: row.previous.clone(),
        since: row.since,
        policy: view_policy(policy),
        delay_days: row.delay_days,
        kpmr_model: service::kpmr_target(&loaded.snapshot, &row.provider, &row.line)
            .map(|l| l.model.clone()),
        pending: match (&row.pending, row.pending_since) {
            (Some(model), Some(since)) => Some(PendingMove {
                model: model.clone(),
                since,
                moves_at: policy
                    .delay_days()
                    .map(|d| since + Duration::days(i64::from(d))),
            }),
            _ => None,
        },
        kept: row.kept.clone(),
        held: row.held.clone(),
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The project's model lines.")]
pub struct ModelLines {
    #[doc = "The lines, by provider and name."]
    pub data: Vec<ModelLineView>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/lines",
    summary = "The project's model lines, where each points and how the project follows it.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ModelLines))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ModelLines>> {
    let project = ctx.project()?.project.id;
    let rows = store::of_project(&state.pg, project).await?;
    Ok(Json(ModelLines {
        data: rows.iter().map(|r| view(&state, r)).collect(),
    }))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A line KPMR offers for a provider of the organization.")]
pub struct OfferedLine {
    #[doc = "The provider."]
    #[schema(value_type = ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The line's name."]
    #[schema(example = "claude-opus-latest")]
    pub line: String,
    #[doc = "The model it points at now."]
    #[schema(example = "claude-opus-5-5")]
    pub model: String,
    #[doc = "Since when, as KPMR has it."]
    pub since: Option<String>,
    #[doc = "The model before it."]
    pub previous: Option<String>,
    #[doc = "The project's catalogue model for the line, when it has added it."]
    pub added: Option<CatalogueModelId>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The lines a project can add.")]
pub struct OfferedLines {
    #[doc = "The lines, by provider and name."]
    pub data: Vec<OfferedLine>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OfferedFilter {
    #[doc = "Only this provider's lines."]
    pub provider: Option<String>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/lines/offered",
    summary = "The model lines KPMR offers for the organization's providers.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), OfferedFilter),
    responses((status = 200, body = OfferedLines))
)]
pub async fn offered(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(filter): Query<OfferedFilter>,
) -> ApiResult<Json<OfferedLines>> {
    let providers = super::models::org_providers(&state, &ctx).await?;
    let project = ctx.project()?.project.id;
    let added = store::of_project(&state.pg, project).await?;
    let loaded = state.kpmr.current();
    let mut data = Vec::new();
    for (id, provider) in &loaded.snapshot.providers {
        if filter.provider.as_deref().is_some_and(|p| p != id)
            || providers.get(id).is_none_or(|p| p.custom().is_some())
        {
            continue;
        }
        for name in provider.lines.keys() {
            let Some(line) = provider.line(name) else {
                continue;
            };
            data.push(OfferedLine {
                provider: id.clone(),
                line: name.clone(),
                model: line.model.clone(),
                since: line.since.clone(),
                previous: line.previous.clone(),
                added: added
                    .iter()
                    .find(|r| r.provider == *id && r.line == *name)
                    .map(|r| CatalogueModelId::from_uuid(r.catalogue_model_id)),
            });
        }
    }
    Ok(Json(OfferedLines { data }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A model line to add to the catalogue.")]
pub struct LineCreate {
    #[doc = "The provider."]
    #[schema(value_type = ProviderId, example = "anthropic")]
    pub provider: String,
    #[doc = "The line's name, as KPMR lists it."]
    #[schema(example = "claude-opus-latest")]
    pub line: String,
    #[doc = "How to follow it; `follow` when absent."]
    pub policy: Option<LinePolicy>,
    #[doc = "With `delay`: the days a move waits, 1 to 30."]
    pub delay_days: Option<i32>,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/lines",
    summary = "Adds a model line to the catalogue; apps then call it as `provider/line`.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = LineCreate,
    responses((status = 201, body = ModelLineView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<LineCreate>,
) -> ApiResult<(StatusCode, Json<ModelLineView>)> {
    let project = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let policy = policy_of(req.policy.unwrap_or(LinePolicy::Follow), req.delay_days)?;
    let providers = super::models::org_providers(&state, &ctx).await?;
    if providers
        .get(&req.provider)
        .is_none_or(|p| p.custom().is_some())
    {
        return Err(ApiError::invalid(
            "provider",
            "This organization has no built-in provider by that name; add one under Providers.",
        ));
    }
    let loaded = state.kpmr.current();
    let Some(line) = service::kpmr_target(&loaded.snapshot, &req.provider, &req.line) else {
        return Err(ApiError::invalid(
            "line",
            format!("KPMR has no line `{}` for `{}`.", req.line, req.provider),
        ));
    };
    let org = ctx.org()?.org.id;
    let id = CatalogueModelId::new().uuid();
    let (kind, by) = creator_of(&ctx);
    let overrides = serde_json::to_value(Fields::default()).map_err(ApiError::internal)?;
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let added = catalogue::insert(
        &mut *tx,
        &NewEntry {
            id,
            org_id: org,
            project_id: project,
            provider: &req.provider,
            model_id: &req.line,
            overrides: &overrides,
            custom: false,
            created_by_kind: kind,
            created_by_id: by,
        },
    )
    .await?;
    if !added {
        return Err(ApiError::Conflict(format!(
            "The catalogue already has `{}/{}`.",
            req.provider, req.line
        )));
    }
    store::insert(
        &mut tx,
        &NewLine {
            catalogue_model_id: id,
            org_id: org,
            project_id: project,
            provider: &req.provider,
            line: &req.line,
            target: &line.model,
            previous: None,
            policy: policy.as_str(),
            delay_days: policy.delay_days(),
            at: now,
        },
    )
    .await?;
    ctx.audit(Some(OrgId::from_uuid(org)), Action::CatalogueModelCreate)
        .target("catalogue_model", CatalogueModelId::from_uuid(id))
        .after(serde_json::json!({ "provider": req.provider, "model_id": req.line, "line": true, "target": line.model, "policy": policy.as_str() }))
        .write(&mut tx)
        .await?;
    let row = store::get_for_update(&mut *tx, project, id)
        .await?
        .ok_or_else(|| ApiError::internal("new line vanished"))?;
    tx.commit().await?;
    crate::services::catalogue::forget(&state, project).await;
    Ok((StatusCode::CREATED, Json(view(&state, &row))))
}

async fn locked(tx: &mut sqlx::PgConnection, ctx: &Ctx, id: &str) -> ApiResult<LineRow> {
    let id: CatalogueModelId = id.parse().map_err(|_| ApiError::not_found("model line"))?;
    store::get_for_update(&mut *tx, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("model line"))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "How to follow a model line.")]
pub struct LineUpdate {
    #[doc = "The policy."]
    pub policy: LinePolicy,
    #[doc = "With `delay`: the days a move waits, 1 to 30."]
    pub delay_days: Option<i32>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/lines/{model}",
    summary = "Sets how the project follows a model line.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    request_body = LineUpdate,
    responses((status = 200, body = ModelLineView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    Body(req): Body<LineUpdate>,
) -> ApiResult<Json<ModelLineView>> {
    let project = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let policy = policy_of(req.policy, req.delay_days)?;
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let row = locked(&mut tx, &ctx, &id).await?;
    let before = Policy::of(&row.policy, row.delay_days);
    if before != policy {
        store::set_policy(
            &mut *tx,
            row.catalogue_model_id,
            policy.as_str(),
            policy.delay_days(),
            now,
        )
        .await?;
        ctx.audit(Some(ctx.org()?.id()), Action::CatalogueModelLineUpdate)
            .target(
                "catalogue_model",
                CatalogueModelId::from_uuid(row.catalogue_model_id),
            )
            .before(
                serde_json::json!({ "policy": before.as_str(), "delay_days": before.delay_days() }),
            )
            .after(
                serde_json::json!({ "policy": policy.as_str(), "delay_days": policy.delay_days() }),
            )
            .write(&mut tx)
            .await?;
    }
    let row = store::get_for_update(&mut *tx, project, row.catalogue_model_id)
        .await?
        .ok_or_else(|| ApiError::not_found("model line"))?;
    tx.commit().await?;
    Ok(Json(view(&state, &row)))
}

async fn moved(
    state: &AppState,
    ctx: &Ctx,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    row: LineRow,
    to: &str,
    how: &'static str,
) -> ApiResult<Json<ModelLineView>> {
    let now = state.clock.now();
    let from = service::move_line(&mut tx, &row, to, now).await?;
    if how == "back" {
        store::set_policy(&mut *tx, row.catalogue_model_id, "pin", None, now).await?;
    }
    let (before, after) = service::audited(&row, &from, to, how);
    ctx.audit(Some(ctx.org()?.id()), Action::CatalogueModelLineMove)
        .target(
            "catalogue_model",
            CatalogueModelId::from_uuid(row.catalogue_model_id),
        )
        .before(before)
        .after(after)
        .write(&mut tx)
        .await?;
    let after = store::get_for_update(&mut *tx, row.project_id, row.catalogue_model_id)
        .await?
        .ok_or_else(|| ApiError::not_found("model line"))?;
    tx.commit().await?;
    crate::services::catalogue::forget(state, row.project_id).await;
    let loaded = state.kpmr.current();
    service::notify(
        state,
        &row,
        LineChange::Moved {
            from: from.clone(),
            to: to.to_owned(),
            prices: service::prices(&loaded.snapshot, &row.provider, &from, to),
            how,
        },
    )
    .await?;
    Ok(Json(view(state, &after)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/lines/{model}/move",
    summary = "Moves a model line to where KPMR points it now, such as a move that waits\n\
    for approval or for its delay.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = ModelLineView))
)]
pub async fn move_now(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelLineView>> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let mut tx = state.pg.begin().await?;
    let row = locked(&mut tx, &ctx, &id).await?;
    let loaded = state.kpmr.current();
    let to = service::kpmr_target(&loaded.snapshot, &row.provider, &row.line)
        .map(|l| l.model.clone())
        .filter(|m| *m != row.target)
        .ok_or_else(|| {
            ApiError::Conflict(
                "The line points where KPMR does; there is nothing to move to.".to_owned(),
            )
        })?;
    let models = crate::services::catalogue::project_models(&state, row.project_id).await?;
    if let Some(reason) = service::held(&state, &row, &to, &models).await {
        return Err(ApiError::Conflict(reason));
    }
    moved(&state, &ctx, tx, row, &to, "approved").await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/lines/{model}/keep",
    summary = "Keeps a model line where it is instead of a move that waits for approval.",
    description = "The line asks again only when KPMR moves it to yet another model.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = ModelLineView))
)]
pub async fn keep(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelLineView>> {
    let project = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let row = locked(&mut tx, &ctx, &id).await?;
    let Some(pending) = row.pending.clone() else {
        return Err(ApiError::Conflict(
            "No move waits for this line.".to_owned(),
        ));
    };
    store::set_waiting(
        &mut *tx,
        row.catalogue_model_id,
        None,
        Some(&pending),
        None,
        now,
    )
    .await?;
    ctx.audit(Some(ctx.org()?.id()), Action::CatalogueModelLineUpdate)
        .target(
            "catalogue_model",
            CatalogueModelId::from_uuid(row.catalogue_model_id),
        )
        .before(serde_json::json!({ "pending": pending }))
        .after(serde_json::json!({ "kept": pending }))
        .write(&mut tx)
        .await?;
    let row = store::get_for_update(&mut *tx, project, row.catalogue_model_id)
        .await?
        .ok_or_else(|| ApiError::not_found("model line"))?;
    tx.commit().await?;
    Ok(Json(view(&state, &row)))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/lines/{model}/back",
    summary = "Moves a model line back to its previous model and pins it there.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = ModelLineView))
)]
pub async fn back(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<ModelLineView>> {
    ensure_active(&ctx, MODELS_ARCHIVED)?;
    let mut tx = state.pg.begin().await?;
    let row = locked(&mut tx, &ctx, &id).await?;
    let Some(previous) = row.previous.clone() else {
        return Err(ApiError::Conflict(
            "The line has no earlier model to go back to.".to_owned(),
        ));
    };
    moved(&state, &ctx, tx, row, &previous, "back").await
}
