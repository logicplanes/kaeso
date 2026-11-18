use axum::Json;
use axum::extract::State;
use kaeso_core::id::{
    AliasId, ApiKeyId, CatalogueModelId, OrgId, ProjectId, ServiceAccountId, UserId,
};
use kaeso_core::kpmr::{self, Lifecycle};
use kaeso_core::rights::{OrgRight, ProjectRight};
use kaeso_store::clickhouse::logs::{self as ch_logs, Timeline, TimelineGroup};
use kaeso_store::pg::model_lifecycle::{self as store, Settings};
use kaeso_store::pg::projects;
use kaeso_store::pg::usage::{self, Filter, GroupBy, Model as UsageModel};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::{Date, Duration};
use utoipa::ToSchema;

use super::{MODELS_ARCHIVED, ensure_active};
use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Path;
use crate::services::lifecycle::{self as service, Affected, MovePlan};
use crate::state::AppState;

/// How far back the callers of a retiring model are counted.
pub const CALLERS_DAYS: i64 = 30;
/// The longest daily request chart, in days.
pub const CHART_DAYS: i64 = 365;

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "When the provider retires a model and what it recommends instead, as\n\
    KPMR publishes it. Each field is `null` while unknown."
)]
pub struct LifecycleView {
    #[doc = "When the provider announced the deprecation."]
    #[schema(value_type = Option<String>, format = Date, example = "2026-11-04")]
    pub announced_on: Option<String>,
    #[doc = "When the model became deprecated."]
    #[schema(value_type = Option<String>, format = Date, example = "2026-11-04")]
    pub deprecated_on: Option<String>,
    #[doc = "When the provider stops serving the model."]
    #[schema(value_type = Option<String>, format = Date, example = "2027-02-04")]
    pub shutdown_on: Option<String>,
    #[doc = "The provider's model id it recommends instead."]
    #[schema(example = "claude-sonnet-5-5")]
    pub replacement: Option<String>,
    #[doc = "The page where the provider announced it."]
    pub source: Option<String>,
}

impl LifecycleView {
    pub fn of(l: &Lifecycle) -> LifecycleView {
        let day = |d: Option<Date>| d.map(kpmr::day::format);
        LifecycleView {
            announced_on: day(l.announced()),
            deprecated_on: day(l.deprecated_on),
            shutdown_on: day(l.shutdown_on),
            replacement: l.replacement.clone(),
            source: l.source.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "Where a model is in its retirement.")]
pub enum RetirementState {
    #[doc = "The provider announced a date; the model still works and isn't deprecated yet."]
    Announced,
    #[doc = "Deprecated: it still works until its shutdown day."]
    Deprecated,
    #[doc = "Past its shutdown day: calls to it fail, or are answered by the\n\
        replacement where the project allows it."]
    Retired,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A price per million tokens before and after a move.")]
pub struct PriceChange {
    #[doc = "The retiring model's price in USD; `null` when unknown."]
    #[schema(value_type = Option<String>, example = "3")]
    pub from: Option<Decimal>,
    #[doc = "The replacement's price in USD; `null` when unknown."]
    #[schema(value_type = Option<String>, example = "3")]
    pub to: Option<Decimal>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The model the provider recommends instead.")]
pub struct ReplacementView {
    #[doc = "The provider's model id."]
    #[schema(example = "claude-sonnet-5-5")]
    pub model_id: String,
    #[doc = "The project's catalogue model for it; `null` when the project doesn't have\n\
        it yet, and a move adds it."]
    pub catalogue_model_id: Option<CatalogueModelId>,
    #[doc = "Whether KPMR lists it for the provider; one it doesn't can't be added."]
    pub listed: bool,
    #[doc = "The input price per million tokens."]
    pub input_price: PriceChange,
    #[doc = "The output price per million tokens."]
    pub output_price: PriceChange,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An alias whose route calls the model.")]
pub struct AliasUseView {
    #[doc = "The alias."]
    pub alias_id: AliasId,
    #[doc = "Its name."]
    #[schema(example = "chat")]
    pub name: String,
    #[doc = "The steps that call the model, as a trace names them, fallbacks and the\n\
        branches of conditions and splits included."]
    #[schema(example = json!(["route", "route.fallback"]))]
    pub steps: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A caller and how many requests it sent.")]
pub struct CallerCount {
    #[doc = "The key, member or service account id, or the client app's name."]
    #[schema(example = "key_01J9Z8W5D6Q4X7G2R1V3N5M8KT")]
    pub id: String,
    #[doc = "Requests."]
    pub requests: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Requests on one day.")]
pub struct DayCount {
    #[doc = "The day, UTC."]
    #[schema(value_type = String, format = Date, example = "2026-11-04")]
    pub day: String,
    #[doc = "Requests."]
    pub requests: u64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Who still calls a retiring model, and whether the calls go down.")]
pub struct RetiringUsage {
    #[doc = "Requests in the last 30 days."]
    pub requests: u64,
    #[doc = "Keys that called it in the last 30 days, most requests first."]
    pub keys: Vec<CallerCount>,
    #[doc = "Members that called it in the last 30 days."]
    pub members: Vec<CallerCount>,
    #[doc = "Service accounts that called it in the last 30 days."]
    pub service_accounts: Vec<CallerCount>,
    #[doc = "Client apps, by the user agent they send, that called it in the last 30\n\
        days; empty without the request log store."]
    pub clients: Vec<CallerCount>,
    #[doc = "Requests per day from the announcement (at most a year back) until today."]
    pub daily: Vec<DayCount>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A model of the project that its provider retires or will retire.")]
pub struct RetiringModelView {
    #[doc = "The catalogue model."]
    pub id: CatalogueModelId,
    #[doc = "The provider."]
    #[schema(example = "anthropic")]
    pub provider: String,
    #[doc = "The provider's model id."]
    #[schema(example = "claude-sonnet-4-5-20250929")]
    pub model_id: String,
    #[doc = "Where it is in its retirement."]
    pub state: RetirementState,
    #[doc = "Days until the shutdown, negative once it passed; `null` without a date."]
    pub days_left: Option<i64>,
    #[doc = "The dates, the replacement and the source."]
    pub lifecycle: LifecycleView,
    #[doc = "The replacement and how the project stands with it."]
    pub replacement: Option<ReplacementView>,
    #[doc = "The aliases and route steps that call it."]
    pub aliases: Vec<AliasUseView>,
    #[doc = "Who calls it; `null` without the right to see the project's usage."]
    pub usage: Option<RetiringUsage>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The project's models that their providers retire, soonest shutdown first.")]
pub struct RetiringModels {
    #[doc = "The models."]
    pub data: Vec<RetiringModelView>,
    #[doc = "How many of them shut down within 90 days or have already."]
    pub soon: usize,
}

fn state_of(a: &Affected, today: Date) -> RetirementState {
    if a.retired {
        RetirementState::Retired
    } else if a.deprecated || a.lifecycle.deprecated_on.is_some_and(|d| d <= today) {
        RetirementState::Deprecated
    } else {
        RetirementState::Announced
    }
}

fn replacement_view(r: &service::Replacement) -> ReplacementView {
    ReplacementView {
        model_id: r.model_id.clone(),
        catalogue_model_id: r.entry.map(CatalogueModelId::from_uuid),
        listed: r.listed,
        input_price: PriceChange {
            from: r.input_price.0,
            to: r.input_price.1,
        },
        output_price: PriceChange {
            from: r.output_price.0,
            to: r.output_price.1,
        },
    }
}

fn uses_view(a: &Affected) -> Vec<AliasUseView> {
    a.uses
        .iter()
        .map(|u| AliasUseView {
            alias_id: AliasId::from_uuid(u.alias_id),
            name: u.name.clone(),
            steps: u.paths.clone(),
        })
        .collect()
}

fn counts(
    groups: Vec<usage::UsageGroup>,
    keep: impl Fn(&str) -> Option<String>,
) -> Vec<CallerCount> {
    let mut out: Vec<CallerCount> = groups
        .into_iter()
        .filter_map(|g| {
            Some(CallerCount {
                id: keep(&g.key)?,
                requests: u64::try_from(g.requests).unwrap_or(0),
            })
        })
        .filter(|c| c.requests > 0)
        .collect();
    out.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.id.cmp(&b.id)));
    out
}

async fn usage_of(state: &AppState, org: uuid::Uuid, a: &Affected) -> ApiResult<RetiringUsage> {
    let today = service::today(state);
    let from = today - Duration::days(CALLERS_DAYS - 1);
    let filter = Filter {
        model: Some(UsageModel {
            provider: Some(a.row.provider.clone()),
            id: a.row.model_id.clone(),
        }),
        ..Filter::default()
    };
    let project = Some(a.row.project_id);
    let by = |group| usage::usage(&state.pg, org, project, from, today, group, &filter);
    let keys = counts(by(GroupBy::Key).await?, |k| {
        uuid::Uuid::parse_str(k)
            .ok()
            .filter(|u| !u.is_nil())
            .map(|u| ApiKeyId::from_uuid(u).to_string())
    });
    let callers = by(GroupBy::Member).await?;
    let members = counts(callers.clone(), |k| {
        let id = uuid::Uuid::parse_str(k.strip_prefix("user:")?).ok()?;
        Some(UserId::from_uuid(id).to_string())
    });
    let service_accounts = counts(callers, |k| {
        let id = uuid::Uuid::parse_str(k.strip_prefix("service_account:")?).ok()?;
        (!id.is_nil()).then(|| ServiceAccountId::from_uuid(id).to_string())
    });
    let requests = service::requests(
        state,
        org,
        a.row.project_id,
        &a.row.provider,
        &a.row.model_id,
        CALLERS_DAYS,
    )
    .await?;
    let start = a
        .lifecycle
        .announced()
        .unwrap_or(from)
        .clamp(today - Duration::days(CHART_DAYS - 1), from);
    let days = usage::usage(&state.pg, org, project, start, today, GroupBy::Day, &filter).await?;
    let daily = days
        .into_iter()
        .map(|g| DayCount {
            day: g.key,
            requests: u64::try_from(g.requests).unwrap_or(0),
        })
        .collect();
    let clients = match &state.clickhouse {
        Some(ch) => {
            let to = state.clock.now();
            let t = Timeline {
                project,
                filter: filter.clone(),
                group: Some(TimelineGroup::Client),
                ..Timeline::default()
            };
            let seconds = u32::try_from(CALLERS_DAYS * 86_400).unwrap_or(u32::MAX);
            let rows = ch_logs::timeline(
                ch,
                org,
                to - Duration::days(CALLERS_DAYS),
                to,
                seconds,
                &t,
            )
            .await
            .inspect_err(
                |e| tracing::warn!(error = %e, "cannot count the clients of a retiring model"),
            )
            .unwrap_or_default();
            let mut by_client: std::collections::BTreeMap<String, u64> =
                std::collections::BTreeMap::new();
            for r in rows.into_iter().filter(|r| !r.key.is_empty()) {
                *by_client.entry(r.key).or_default() += r.requests;
            }
            let mut out: Vec<CallerCount> = by_client
                .into_iter()
                .map(|(id, requests)| CallerCount { id, requests })
                .collect();
            out.sort_by(|a, b| b.requests.cmp(&a.requests).then_with(|| a.id.cmp(&b.id)));
            out
        }
        None => Vec::new(),
    };
    Ok(RetiringUsage {
        requests,
        keys,
        members,
        service_accounts,
        clients,
        daily,
    })
}

async fn view_of(state: &AppState, ctx: &Ctx, a: &Affected) -> ApiResult<RetiringModelView> {
    let today = service::today(state);
    let sees_usage = ctx.project()?.rights.contains(ProjectRight::UsageView);
    Ok(RetiringModelView {
        id: CatalogueModelId::from_uuid(a.row.id),
        provider: a.row.provider.clone(),
        model_id: a.row.model_id.clone(),
        state: state_of(a, today),
        days_left: a.lifecycle.days_left(today),
        lifecycle: LifecycleView::of(&a.lifecycle),
        replacement: a.replacement.as_ref().map(replacement_view),
        aliases: uses_view(a),
        usage: if sees_usage {
            Some(usage_of(state, a.row.org_id, a).await?)
        } else {
            None
        },
    })
}

fn soon(a: &Affected, today: Date) -> bool {
    a.lifecycle
        .days_left(today)
        .is_some_and(|d| d <= service::SOON_DAYS)
        || a.retired
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/deprecations",
    summary = "The project's models that their providers retire: dates, replacement,\n\
    the aliases and route steps that call them, and who still calls them.",
    description = "`usage` is `null` without `usage.view`.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = RetiringModels))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<RetiringModels>> {
    let project = ctx.project()?.project.id;
    let found = service::affected(&state, project).await?;
    let today = service::today(&state);
    let mut data = Vec::with_capacity(found.len());
    for a in &found {
        data.push(view_of(&state, &ctx, a).await?);
    }
    Ok(Json(RetiringModels {
        soon: found.iter().filter(|a| soon(a, today)).count(),
        data,
    }))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What moving a model to its replacement changes, or changed.")]
pub struct MoveView {
    #[doc = "The retiring model."]
    pub model: RetiringModelView,
    #[doc = "Whether the replacement is (or was) added to the catalogue first."]
    pub adds_replacement: bool,
    #[doc = "How many route steps point at the replacement afterwards."]
    pub steps: usize,
}

async fn plan_of(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<MovePlan> {
    let id: CatalogueModelId = id
        .parse()
        .map_err(|_| ApiError::not_found("catalogue model"))?;
    let project = ctx.project()?.project.id;
    service::plan(service::find(state, project, id.uuid()).await?)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/deprecations/{model}/move",
    summary = "Shows what moving a retiring model to its replacement would change.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = MoveView))
)]
pub async fn preview(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<MoveView>> {
    let plan = plan_of(&state, &ctx, &id).await?;
    Ok(Json(MoveView {
        model: view_of(&state, &ctx, &plan.from).await?,
        adds_replacement: plan.to.entry.is_none(),
        steps: plan.steps,
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/deprecations/{model}/move",
    summary = "Moves every alias and route step that calls a retiring model to its\n\
    replacement, adding the replacement to the catalogue when it's missing.",
    description = "The catalogue model itself stays, so apps that name it directly keep\n\
    working until the shutdown.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("model" = CatalogueModelId, Path)),
    responses((status = 200, body = MoveView))
)]
pub async fn apply(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<Json<MoveView>> {
    let project = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let plan = plan_of(&state, &ctx, &id).await?;
    let org = ctx.org()?.org.id;
    let by = crate::platform::keys::creator_of(&ctx);
    let mut tx = state.pg.begin().await?;
    let moved = service::apply(&mut tx, org, project, &plan, by).await?;
    let (before, after) = service::audited(&plan, &moved);
    ctx.audit(Some(OrgId::from_uuid(org)), Action::CatalogueModelMove)
        .target(
            "catalogue_model",
            CatalogueModelId::from_uuid(plan.from.row.id),
        )
        .before(before)
        .after(after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::catalogue::forget(&state, project).await;
    let now = service::find(&state, project, plan.from.row.id).await?;
    Ok(Json(MoveView {
        model: view_of(&state, &ctx, &now).await?,
        adds_replacement: moved.added,
        steps: plan.steps,
    }))
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "What a project does when a model it uses shuts down.")]
pub struct ModelLifecycleSettings {
    #[doc = "On a model's shutdown day, move the aliases and route steps that call it\n\
        to its replacement, adding the replacement when it's missing. Off by default."]
    pub move_on_shutdown: bool,
    #[doc = "After a model's shutdown, answer requests that name it with its\n\
        replacement instead of failing. The reply names the model that answered and\n\
        the request log marks it. Off by default: price and behaviour change."]
    pub serve_replacement: bool,
}

impl From<Settings> for ModelLifecycleSettings {
    fn from(s: Settings) -> ModelLifecycleSettings {
        ModelLifecycleSettings {
            move_on_shutdown: s.move_on_shutdown,
            serve_replacement: s.serve_replacement,
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/model-lifecycle",
    summary = "What the project does when a model it uses shuts down.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = ModelLifecycleSettings))
)]
pub async fn get_settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<ModelLifecycleSettings>> {
    let project = ctx.project()?.project.id;
    Ok(Json(store::settings(&state.pg, project).await?.into()))
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/projects/{project}/model-lifecycle",
    summary = "Sets what the project does when a model it uses shuts down.",
    tag = "models",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = ModelLifecycleSettings,
    responses((status = 200, body = ModelLifecycleSettings))
)]
pub async fn set_settings(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<ModelLifecycleSettings>,
) -> ApiResult<Json<ModelLifecycleSettings>> {
    let project = ensure_active(&ctx, MODELS_ARCHIVED)?;
    let org = ctx.org()?.id();
    let after = Settings {
        move_on_shutdown: req.move_on_shutdown,
        serve_replacement: req.serve_replacement,
    };
    let mut tx = state.pg.begin().await?;
    let before = store::settings(&mut *tx, project).await?;
    if before != after {
        store::set_settings(&mut *tx, project, after, state.clock.now()).await?;
        ctx.audit(Some(org), Action::ProjectModelLifecycle)
            .target("project", ProjectId::from_uuid(project))
            .before(ModelLifecycleSettings::from(before))
            .after(ModelLifecycleSettings::from(after))
            .write(&mut tx)
            .await?;
    }
    tx.commit().await?;
    if before.serve_replacement != after.serve_replacement {
        crate::services::catalogue::forget(&state, project).await;
    }
    Ok(Json(after.into()))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project's retiring models, in short.")]
pub struct ProjectRetirements {
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "Its name."]
    pub name: String,
    #[doc = "How many of its models their providers retire or will retire."]
    pub models: usize,
    #[doc = "How many of them are past their shutdown day."]
    pub retired: usize,
    #[doc = "The soonest shutdown still ahead; `null` without one."]
    #[schema(value_type = Option<String>, format = Date, example = "2027-02-04")]
    pub next_shutdown_on: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(
    description = "The organization's projects that use retiring models, soonest shutdown first."
)]
pub struct OrgRetirements {
    #[doc = "The projects, only those the caller can see and that use one."]
    pub data: Vec<ProjectRetirements>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/deprecations",
    summary = "The organization's projects that use models their providers retire.",
    description = "Lists the projects the caller can see, as the project list does.",
    tag = "models",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = OrgRetirements))
)]
pub async fn org_summary(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<OrgRetirements>> {
    let scope = ctx.org()?;
    let see_all = scope.rights.contains(OrgRight::ProjectsViewAll)
        || scope
            .every_project
            .contains(kaeso_core::rights::ProjectRight::ProjectView);
    let member = if see_all {
        None
    } else {
        ctx.acting_user().map(|u| u.uuid())
    };
    let only = match ctx.agent().and_then(|a| a.grant.org(scope.org.id)) {
        Some(access) if !see_all => access.projects.only(),
        _ => None,
    };
    let rows = projects::list(
        &state.pg,
        scope.org.id,
        member,
        only.as_deref(),
        false,
        None,
        1000,
    )
    .await?;
    let today = service::today(&state);
    let mut data = Vec::new();
    for p in rows {
        let found = service::affected(&state, p.id).await?;
        if found.is_empty() {
            continue;
        }
        data.push(ProjectRetirements {
            project_id: ProjectId::from_uuid(p.id),
            name: p.name.clone(),
            models: found.len(),
            retired: found.iter().filter(|a| a.retired).count(),
            next_shutdown_on: found
                .iter()
                .filter_map(|a| a.lifecycle.shutdown_on.filter(|d| *d >= today))
                .min()
                .map(kpmr::day::format),
        });
    }
    data.sort_by(|a, b| {
        let at =
            |x: &ProjectRetirements| x.next_shutdown_on.clone().unwrap_or_else(|| "~".to_owned());
        at(a).cmp(&at(b)).then_with(|| a.name.cmp(&b.name))
    });
    Ok(Json(OrgRetirements { data }))
}
