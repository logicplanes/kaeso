use axum::Json;
use axum::extract::State;
use kaeso_core::cost::cost_from_raw;
use kaeso_core::id::{ApiKeyId, OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::rights::{OrgRight, ProjectRight};
use kaeso_store::clickhouse::logs::{self as ch, GroupBy};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::access::Ctx;
use super::error::{ApiError, ApiResult};
use super::pagination::List;
use crate::platform::params::{Path, Query};
use crate::state::AppState;

pub(crate) const USAGE_UNAVAILABLE: &str = "Usage is unavailable right now; try again shortly.";

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UsageQuery {
    #[doc = "First day (UTC, `YYYY-MM-DD`); default 30 days ago."]
    pub from: Option<String>,
    #[doc = "Last day (UTC, inclusive); default today. By `day`, at most 400 days\n\
        after `from`; other groupings take any period."]
    pub to: Option<String>,
    #[doc = "`day` (default), `week` (Monday to Sunday, named by its Monday),\n\
        `month` (named by its first day), `model`, `key`, `member`, or for\n\
        orgs `project`. A week or month that starts before `from` or ends\n\
        after `to` counts only the days in the period."]
    pub group_by: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Usage of one group.")]
pub struct UsageRow {
    #[doc = "The day, the Monday of the week, the first day of the month,\n\
        `provider/model`, key id (`playground` for the requests members made\n\
        in the console's Playground, without a key), member (`user:<id>` or\n\
        `service_account:<id>`), or project id."]
    pub key: String,
    #[doc = "Model requests of the group, made with a key or from the Playground:\n\
        every one, the gateway's own refusals (limits, unknown models,\n\
        capability checks) included."]
    pub requests: u64,
    #[doc = "Of those, the ones answered with a status of 400 or above, the\n\
        gateway's own refusals and streams that failed after their headers\n\
        included."]
    pub errors: u64,
    #[doc = "Uncached input tokens."]
    pub input_tokens: u64,
    #[doc = "Output tokens."]
    pub output_tokens: u64,
    #[doc = "Cache reads."]
    pub cache_read_tokens: u64,
    #[doc = "Cache writes."]
    pub cache_write_tokens: u64,
    #[doc = "Reasoning tokens."]
    pub reasoning_tokens: u64,
    #[doc = "Cost in USD; `null` without the right to see costs."]
    #[schema(value_type = Option<String>)]
    pub cost: Option<String>,
}

fn public_key(group: GroupBy, key: String) -> String {
    let id = |s: &str| Uuid::parse_str(s).ok();
    match group {
        GroupBy::Day | GroupBy::Week | GroupBy::Month | GroupBy::Model => key,
        GroupBy::Project => id(&key).map_or(key, |u| ProjectId::from_uuid(u).to_string()),
        GroupBy::Key => match id(&key) {
            Some(u) if u.is_nil() => "playground".to_owned(),
            Some(u) => ApiKeyId::from_uuid(u).to_string(),
            None => key,
        },
        GroupBy::Member => match key.split_once(':') {
            Some(("user", rest)) => {
                id(rest).map_or(key.clone(), |u| format!("user:{}", UserId::from_uuid(u)))
            }
            Some(("service_account", rest)) => id(rest).map_or(key.clone(), |u| {
                format!("service_account:{}", ServiceAccountId::from_uuid(u))
            }),
            _ => key,
        },
    }
}

fn parse_day(field: &str, s: Option<&str>, default: Date) -> ApiResult<Date> {
    match s {
        None => Ok(default),
        Some(s) => {
            let format = time::macros::format_description!("[year]-[month]-[day]");
            Date::parse(s, &format).map_err(|_| {
                ApiError::invalid(field, format!("`{field}` must be a date, YYYY-MM-DD."))
            })
        }
    }
}

async fn usage(
    state: &AppState,
    ctx: &Ctx,
    org: Uuid,
    project: Option<Uuid>,
    q: &UsageQuery,
    costs: bool,
) -> ApiResult<Json<List<UsageRow>>> {
    let today = ctx.now.date();
    let to = parse_day("to", q.to.as_deref(), today)?;
    let from = parse_day("from", q.from.as_deref(), today - time::Duration::days(30))?;
    if from > to {
        return Err(ApiError::invalid(
            "from",
            "`from` must be on or before `to`.",
        ));
    }
    let group = match q.group_by.as_deref().unwrap_or("day") {
        "day" => GroupBy::Day,
        "week" => GroupBy::Week,
        "month" => GroupBy::Month,
        "model" => GroupBy::Model,
        "key" => GroupBy::Key,
        "member" => GroupBy::Member,
        "project" if project.is_none() => GroupBy::Project,
        _ => {
            return Err(ApiError::invalid(
                "group_by",
                "Use `day`, `week`, `month`, `model`, `key`, `member`, or `project` for orgs.",
            ));
        }
    };
    if group == GroupBy::Day && (to - from).whole_days() > 400 {
        return Err(ApiError::invalid(
            "from",
            "By day, `from` and `to` are at most 400 days apart; use `week` or `month`.",
        ));
    }
    let rows = ch::usage(&state.clickhouse, org, project, from, to, group)
        .await
        .map_err(|e| ApiError::unavailable(USAGE_UNAVAILABLE, e))?;
    Ok(Json(List::complete(
        rows.into_iter()
            .map(|r| UsageRow {
                key: public_key(group, r.key),
                requests: r.requests,
                errors: r.errors,
                input_tokens: r.input_tokens,
                output_tokens: r.output_tokens,
                cache_read_tokens: r.cache_read_tokens,
                cache_write_tokens: r.cache_write_tokens,
                reasoning_tokens: r.reasoning_tokens,
                cost: costs.then(|| cost_from_raw(r.cost).to_string()),
            })
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/usage",
    summary = "Usage across the org's projects; costs with `costs.org.view`.",
    tag = "orgs",
    params(("org" = OrgId, Path), UsageQuery),
    responses((status = 200, body = List<UsageRow>))
)]
pub async fn org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    Query(q): Query<UsageQuery>,
) -> ApiResult<Json<List<UsageRow>>> {
    let scope = ctx.org()?;
    let costs = scope.rights.contains(OrgRight::CostsOrgView);
    let org = scope.org.id;
    usage(&state, &ctx, org, None, &q, costs).await
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/usage",
    summary = "The project's usage; costs with `costs.view`.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), UsageQuery),
    responses((status = 200, body = List<UsageRow>))
)]
pub async fn project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(q): Query<UsageQuery>,
) -> ApiResult<Json<List<UsageRow>>> {
    let scope = ctx.project()?;
    let costs = scope.rights.contains(ProjectRight::CostsView);
    let (org, project) = (scope.project.org_id, scope.project.id);
    usage(&state, &ctx, org, Some(project), &q, costs).await
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The days with usage.")]
pub struct UsageSpan {
    #[doc = "The first day with usage (UTC, `YYYY-MM-DD`); `null` before the first\n\
        request. \"All time\" starts here."]
    pub first_day: Option<String>,
    #[doc = "The last day with usage (UTC); `null` before the first request."]
    pub last_day: Option<String>,
}

async fn span(state: &AppState, org: Uuid, project: Option<Uuid>) -> ApiResult<Json<UsageSpan>> {
    let span = ch::usage_span(&state.clickhouse, org, project)
        .await
        .map_err(|e| ApiError::unavailable(USAGE_UNAVAILABLE, e))?;
    Ok(Json(UsageSpan {
        first_day: span.map(|(first, _)| first.to_string()),
        last_day: span.map(|(_, last)| last.to_string()),
    }))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/usage/span",
    summary = "The first and last day with usage across the org's projects.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = UsageSpan))
)]
pub async fn org_span(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
) -> ApiResult<Json<UsageSpan>> {
    let scope = ctx.org()?;
    span(&state, scope.org.id, None).await
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/usage/span",
    summary = "The first and last day with usage in the project.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    responses((status = 200, body = UsageSpan))
)]
pub async fn project_span(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
) -> ApiResult<Json<UsageSpan>> {
    let scope = ctx.project()?;
    span(&state, scope.project.org_id, Some(scope.project.id)).await
}
