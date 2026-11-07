use axum::Json;
use axum::extract::State;
use kaeso_core::cost::cost_from_raw;
use kaeso_core::id::{ApiKeyId, OrgId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::rights::{OrgRight, ProjectRight};
use kaeso_kpp::DialectId;
use kaeso_store::clickhouse::logs::{self as ch_logs, TimelineGroup};
use kaeso_store::pg::usage::{self as store, GroupBy, UsageGroup};
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
        `month` (named by its first day), `model`, `provider`, `key`, `member`,\n\
        `route`, or for orgs `project`. A week or month that starts before `from`\n\
        or ends after `to` counts only the days in the period."]
    pub group_by: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UsageFilter {
    #[doc = "Only this model, as `provider/model` like the `key` of a row grouped\n\
        by `model`, such as `openai/gpt-5`; a model id without `/` matches it at\n\
        every provider."]
    pub model: Option<String>,
    #[doc = "Only this provider, such as `openai`."]
    #[param(value_type = Option<kaeso_core::provider::ProviderId>)]
    pub provider: Option<String>,
    #[doc = "Only this key: a key id, or `playground` for the requests members made\n\
        in the console's Playground."]
    pub key: Option<String>,
    #[doc = "Only this member: `user:<id>` or `service_account:<id>`, as grouping by\n\
        `member` names them."]
    pub member: Option<String>,
    #[doc = "Only this route, `<app format>:<provider format>:<provider>` as\n\
        grouping by `route` names it, such as\n\
        `openai_responses:anthropic_messages:anthropic`."]
    pub route: Option<String>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProjectFilter {
    #[doc = "Only this project."]
    pub project: Option<ProjectId>,
}

fn not_a_key(field: &str) -> ApiError {
    ApiError::invalid(
        field,
        format!("`{field}` must be a `key` that grouping by `{field}` returns."),
    )
}

impl UsageFilter {
    fn parse(&self) -> ApiResult<store::Filter> {
        let model = self.model.as_deref().map(|m| match m.split_once('/') {
            Some((provider, id)) => store::Model {
                provider: Some(provider.to_owned()),
                id: id.to_owned(),
            },
            None => store::Model {
                provider: None,
                id: m.to_owned(),
            },
        });
        let key = match self.key.as_deref() {
            None => None,
            Some("playground") => Some(Uuid::nil()),
            Some(k) => Some(k.parse::<ApiKeyId>().map_err(|_| not_a_key("key"))?.uuid()),
        };
        let member = match self.member.as_deref().map(|m| m.split_once(':')) {
            None => None,
            Some(Some(("user", id))) => Some(store::Member::User(
                id.parse::<UserId>()
                    .map_err(|_| not_a_key("member"))?
                    .uuid(),
            )),
            Some(Some(("service_account", id))) => Some(store::Member::ServiceAccount(
                id.parse::<ServiceAccountId>()
                    .map_err(|_| not_a_key("member"))?
                    .uuid(),
            )),
            Some(_) => return Err(not_a_key("member")),
        };
        let route = match self.route.as_deref() {
            None => None,
            Some(r) => match r.split(':').collect::<Vec<_>>().as_slice() {
                [ingress, egress, provider] => Some(store::Route {
                    ingress_dialect: (*ingress).to_owned(),
                    egress_dialect: (*egress).to_owned(),
                    provider: (*provider).to_owned(),
                }),
                _ => return Err(not_a_key("route")),
            },
        };
        Ok(store::Filter {
            provider: self
                .provider
                .as_deref()
                .map(super::logs::provider_filter)
                .transpose()?,
            model,
            key,
            member,
            route,
        })
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Usage of one group.")]
pub struct UsageRow {
    #[doc = "The day, the Monday of the week, the first day of the month,\n\
        `provider/model`, provider, key id (`playground` for the requests members\n\
        made in the console's Playground, without a key), member (`user:<id>` or\n\
        `service_account:<id>`), route (`<app format>:<provider format>:<provider>`,\n\
        such as `openai_responses:anthropic_messages:anthropic`; both formats are\n\
        empty for requests made before Kaeso recorded them, and the provider\n\
        format for requests refused before a provider was chosen), or project\n\
        id. The filter of the same name takes it as it is."]
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
    #[doc = "The share of the input read from the cache, from 0 to 1: cache reads\n\
        over uncached input, cache reads and cache writes together; `null` for\n\
        a group without input and for routes to or from `openai_embeddings`,\n\
        which never cache."]
    pub cache_hit_rate: Option<f64>,
    #[doc = "Cost in USD; `null` without the right to see costs."]
    #[schema(value_type = Option<String>)]
    pub cost: Option<String>,
    #[doc = "Tool calls through the project's MCP portals, refusals included. Counted by\n\
        day and project, so 0 when grouped or filtered by model, provider, key,\n\
        member or route."]
    pub tool_calls: u64,
    #[doc = "Of those, the ones refused or failed."]
    pub tool_call_errors: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ids {
    Plain,
    Project,
    Key,
    Member,
}

fn ids_of(group: GroupBy) -> Ids {
    match group {
        GroupBy::Day
        | GroupBy::Week
        | GroupBy::Month
        | GroupBy::Model
        | GroupBy::Provider
        | GroupBy::Route => Ids::Plain,
        GroupBy::Project => Ids::Project,
        GroupBy::Key => Ids::Key,
        GroupBy::Member => Ids::Member,
    }
}

fn public_key(ids: Ids, key: String) -> String {
    let id = |s: &str| Uuid::parse_str(s).ok();
    match ids {
        Ids::Plain => key,
        Ids::Project => id(&key).map_or(key, |u| ProjectId::from_uuid(u).to_string()),
        Ids::Key => match id(&key) {
            Some(u) if u.is_nil() => "playground".to_owned(),
            Some(u) => ApiKeyId::from_uuid(u).to_string(),
            None => key,
        },
        Ids::Member => match key.split_once(':') {
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

fn embedding_route(group: GroupBy, key: &str) -> bool {
    let embeddings = DialectId::OpenAiEmbeddings.as_str();
    group == GroupBy::Route && key.split(':').take(2).any(|format| format == embeddings)
}

fn cache_hit_rate(group: GroupBy, r: &UsageGroup) -> Option<f64> {
    if embedding_route(group, &r.key) {
        return None;
    }
    let input = count(r.input_tokens)
        .saturating_add(count(r.cache_read_tokens))
        .saturating_add(count(r.cache_write_tokens));
    (input > 0).then(|| count(r.cache_read_tokens) as f64 / input as f64)
}

fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
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

#[derive(Clone, Copy)]
struct Scope {
    org: Uuid,
    project: Option<Uuid>,
    org_wide: bool,
    costs: bool,
}

impl Scope {
    fn narrowed(self, filter: &ProjectFilter) -> Scope {
        Scope {
            project: self.project.or(filter.project.map(|p| p.uuid())),
            ..self
        }
    }
}

async fn usage(
    state: &AppState,
    ctx: &Ctx,
    scope: Scope,
    q: &UsageQuery,
    f: &UsageFilter,
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
        "provider" => GroupBy::Provider,
        "key" => GroupBy::Key,
        "member" => GroupBy::Member,
        "route" => GroupBy::Route,
        "project" if scope.org_wide => GroupBy::Project,
        _ => {
            return Err(ApiError::invalid(
                "group_by",
                if scope.org_wide {
                    "Use `day`, `week`, `month`, `model`, `provider`, `key`, `member`, `route` or `project`."
                } else {
                    "Use `day`, `week`, `month`, `model`, `provider`, `key`, `member` or `route`."
                },
            ));
        }
    };
    if group == GroupBy::Day && (to - from).whole_days() > 400 {
        return Err(ApiError::invalid(
            "from",
            "By day, `from` and `to` are at most 400 days apart; use `week` or `month`.",
        ));
    }
    let filter = f.parse()?;
    let (org, project, costs) = (scope.org, scope.project, scope.costs);
    let rows = store::usage(&state.pg, org, project, from, to, group, &filter)
        .await
        .map_err(|e| ApiError::unavailable(USAGE_UNAVAILABLE, e))?;
    let tools = store::tool_usage(&state.pg, org, project, from, to, group, &filter)
        .await
        .map_err(|e| ApiError::unavailable(USAGE_UNAVAILABLE, e))?;
    let ids = ids_of(group);
    let mut merged: std::collections::BTreeMap<String, UsageRow> = rows
        .into_iter()
        .map(|r| {
            let row = UsageRow {
                cache_hit_rate: cache_hit_rate(group, &r),
                key: public_key(ids, r.key.clone()),
                requests: count(r.requests),
                errors: count(r.errors),
                input_tokens: count(r.input_tokens),
                output_tokens: count(r.output_tokens),
                cache_read_tokens: count(r.cache_read_tokens),
                cache_write_tokens: count(r.cache_write_tokens),
                reasoning_tokens: count(r.reasoning_tokens),
                cost: costs.then(|| r.cost.normalize().to_string()),
                tool_calls: 0,
                tool_call_errors: 0,
            };
            (r.key, row)
        })
        .collect();
    for t in tools {
        let row = merged.entry(t.key.clone()).or_insert_with(|| UsageRow {
            key: public_key(ids, t.key.clone()),
            requests: 0,
            errors: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            cache_hit_rate: None,
            cost: costs.then(|| "0".to_owned()),
            tool_calls: 0,
            tool_call_errors: 0,
        });
        row.tool_calls = count(t.calls);
        row.tool_call_errors = count(t.errors);
    }
    Ok(Json(List::complete(merged.into_values().collect())))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/usage",
    summary = "The org's usage by day, week, month, model, provider, key, member, route or project.",
    description = "Requests (refusals included), errors, tokens, cost and MCP tool calls\n\
    across the org's projects, from daily totals that reach back to the first\n\
    request (`usage/span`) and never expire; days are UTC. One row per group of\n\
    `group_by`, sorted by `key`; groups without usage are left out. Every filter\n\
    takes a `key` as its grouping returns it, so filters and groupings combine:\n\
    `group_by=day&model=openai/gpt-5&key=key_…` is the daily usage of one model\n\
    through one key. `cost` is in USD and `null` without `costs.org.view`. For\n\
    minutes, hours, latency or error kinds use `usage/timeline`; for single\n\
    requests, a project's `logs`.",
    tag = "orgs",
    params(("org" = OrgId, Path), UsageQuery, UsageFilter, ProjectFilter),
    responses((status = 200, body = List<UsageRow>))
)]
pub async fn org(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    Query(q): Query<UsageQuery>,
    Query(f): Query<UsageFilter>,
    Query(p): Query<ProjectFilter>,
) -> ApiResult<Json<List<UsageRow>>> {
    let scope = org_scope(&ctx)?.narrowed(&p);
    usage(&state, &ctx, scope, &q, &f).await
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/usage",
    summary = "The project's usage by day, week, month, model, provider, key, member or route.",
    description = "Requests (refusals included), errors, tokens, cost and MCP tool calls\n\
    of the project, from daily totals that reach back to the first request\n\
    (`usage/span`) and never expire; days are UTC. One row per group of\n\
    `group_by`, sorted by `key`; groups without usage are left out. Every filter\n\
    takes a `key` as its grouping returns it, so filters and groupings combine:\n\
    `group_by=day&model=openai/gpt-5&key=key_…` is the daily usage of one model\n\
    through one key. `cost` is in USD and `null` without `costs.view`. For\n\
    minutes, hours, latency or error kinds use `usage/timeline`; for single\n\
    requests, `logs`.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), UsageQuery, UsageFilter),
    responses((status = 200, body = List<UsageRow>))
)]
pub async fn project(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(q): Query<UsageQuery>,
    Query(f): Query<UsageFilter>,
) -> ApiResult<Json<List<UsageRow>>> {
    usage(&state, &ctx, project_scope(&ctx)?, &q, &f).await
}

fn org_scope(ctx: &Ctx) -> ApiResult<Scope> {
    let scope = ctx.org()?;
    Ok(Scope {
        org: scope.org.id,
        project: None,
        org_wide: true,
        costs: scope.rights.contains(OrgRight::CostsOrgView),
    })
}

fn project_scope(ctx: &Ctx) -> ApiResult<Scope> {
    let scope = ctx.project()?;
    Ok(Scope {
        org: scope.project.org_id,
        project: Some(scope.project.id),
        org_wide: false,
        costs: scope.rights.contains(ProjectRight::CostsView),
    })
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
    let span = store::span(&state.pg, org, project)
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

const MINUTE_SPAN: time::Duration = time::Duration::days(2);
const HOUR_SPAN: time::Duration = time::Duration::days(62);
const MOST_TOP: u32 = 50;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimelineQuery {
    #[doc = "Start (RFC 3339, inclusive); default 24 hours before `to`."]
    pub from: Option<String>,
    #[doc = "End (RFC 3339, exclusive); default now."]
    pub to: Option<String>,
    #[doc = "`minute` (at most 2 days) or `hour` (at most 62 days); default `minute`\n\
        for up to 2 days, `hour` beyond."]
    pub step: Option<String>,
    #[doc = "A bucket per step and `model`, `provider`, `key`, `member`, `route`,\n\
        `error_kind` (the request log's, empty for requests that succeeded), or\n\
        for orgs `project`; default one bucket per step."]
    pub group_by: Option<String>,
    #[doc = "With `group_by`, only the groups with the most requests in the whole\n\
        range, 1 to 50; the others are left out."]
    #[param(minimum = 1, maximum = 50)]
    pub top: Option<u32>,
    #[doc = "Only failed (`true`) or successful (`false`) requests."]
    pub failed: Option<bool>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "The requests of one minute or hour, and of one group with `group_by`, from the request log."
)]
pub struct TimelineBucket {
    #[doc = "The minute or hour it starts at (RFC 3339, UTC)."]
    pub start: String,
    #[doc = "The group, as usage grouped the same way names it; `null` without\n\
        `group_by`."]
    pub key: Option<String>,
    #[doc = "Model requests that started in it, refusals included."]
    pub requests: u64,
    #[doc = "Of those, the ones answered with a status of 400 or above."]
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
    #[doc = "The median total time of its requests, in milliseconds."]
    pub duration_p50_ms: u32,
    #[doc = "The 95th percentile of the total time of its requests, in milliseconds."]
    pub duration_p95_ms: u32,
    #[doc = "The longest total time of its requests, in milliseconds."]
    pub duration_max_ms: u32,
    #[doc = "The median time to the provider's first byte, in milliseconds; `null`\n\
        when no request reached a provider, such as refusals and cache hits."]
    pub ttfb_p50_ms: Option<u32>,
    #[doc = "The 95th percentile of the time to the provider's first byte, in\n\
        milliseconds; `null` when no request reached a provider."]
    pub ttfb_p95_ms: Option<u32>,
}

fn parse_instant(
    field: &str,
    s: Option<&str>,
    default: time::OffsetDateTime,
) -> ApiResult<time::OffsetDateTime> {
    match s {
        None => Ok(default),
        Some(s) => time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
            .map_err(|_| {
                ApiError::invalid(
                    field,
                    format!("`{field}` must be a time such as 2026-10-01T00:00:00Z."),
                )
            }),
    }
}

fn millis(quantiles: &[f64], at: usize) -> Option<u32> {
    let q = *quantiles.get(at)?;
    q.is_finite().then(|| q.round() as u32)
}

fn timeline_ids(group: Option<TimelineGroup>) -> Ids {
    match group {
        Some(TimelineGroup::Project) => Ids::Project,
        Some(TimelineGroup::Key) => Ids::Key,
        Some(TimelineGroup::Member) => Ids::Member,
        _ => Ids::Plain,
    }
}

async fn timeline(
    state: &AppState,
    ctx: &Ctx,
    scope: Scope,
    q: &TimelineQuery,
    f: &UsageFilter,
) -> ApiResult<Json<List<TimelineBucket>>> {
    let to = parse_instant("to", q.to.as_deref(), ctx.now)?;
    let from = parse_instant("from", q.from.as_deref(), to - time::Duration::days(1))?;
    if from >= to {
        return Err(ApiError::invalid("from", "`from` must be before `to`."));
    }
    let span = to - from;
    let (step, most) = match q.step.as_deref() {
        Some("minute") => (60, MINUTE_SPAN),
        Some("hour") => (3600, HOUR_SPAN),
        None if span <= MINUTE_SPAN => (60, MINUTE_SPAN),
        None => (3600, HOUR_SPAN),
        Some(_) => return Err(ApiError::invalid("step", "Use `minute` or `hour`.")),
    };
    if span > most {
        return Err(ApiError::invalid(
            "from",
            if step == 60 {
                "By minute, `from` and `to` are at most 2 days apart; use `hour`."
            } else {
                "By hour, `from` and `to` are at most 62 days apart; use the daily usage."
            },
        ));
    }
    let group = match q.group_by.as_deref() {
        None => None,
        Some("model") => Some(TimelineGroup::Model),
        Some("provider") => Some(TimelineGroup::Provider),
        Some("key") => Some(TimelineGroup::Key),
        Some("member") => Some(TimelineGroup::Member),
        Some("route") => Some(TimelineGroup::Route),
        Some("error_kind") => Some(TimelineGroup::ErrorKind),
        Some("project") if scope.org_wide => Some(TimelineGroup::Project),
        Some(_) => {
            return Err(ApiError::invalid(
                "group_by",
                if scope.org_wide {
                    "Use `model`, `provider`, `key`, `member`, `route`, `error_kind` or `project`."
                } else {
                    "Use `model`, `provider`, `key`, `member`, `route` or `error_kind`."
                },
            ));
        }
    };
    if q.top.is_some_and(|n| !(1..=MOST_TOP).contains(&n)) {
        return Err(ApiError::invalid("top", "`top` is out of range."));
    }
    let query = ch_logs::Timeline {
        project: scope.project,
        filter: f.parse()?,
        failed: q.failed,
        group,
        top: q.top,
    };
    let rows = ch_logs::timeline(
        super::logs::log_store(state)?,
        scope.org,
        from,
        to,
        step,
        &query,
    )
    .await
    .map_err(super::logs::unavailable)?;
    let format = time::format_description::well_known::Rfc3339;
    let ids = timeline_ids(group);
    let mut buckets: Vec<TimelineBucket> = rows
        .into_iter()
        .filter_map(|r| {
            let start = time::OffsetDateTime::from_unix_timestamp(r.start_unix)
                .ok()?
                .format(&format)
                .ok()?;
            let reached = r.with_ttfb > 0;
            Some(TimelineBucket {
                start,
                key: group.map(|_| public_key(ids, r.key)),
                requests: r.requests,
                errors: r.errors,
                input_tokens: r.input_tokens,
                output_tokens: r.output_tokens,
                cache_read_tokens: r.cache_read_tokens,
                cache_write_tokens: r.cache_write_tokens,
                reasoning_tokens: r.reasoning_tokens,
                cost: scope
                    .costs
                    .then(|| cost_from_raw(r.cost).normalize().to_string()),
                duration_p50_ms: millis(&r.duration, 0).unwrap_or(r.duration_max),
                duration_p95_ms: millis(&r.duration, 1).unwrap_or(r.duration_max),
                duration_max_ms: r.duration_max,
                ttfb_p50_ms: millis(&r.ttfb, 0).filter(|_| reached),
                ttfb_p95_ms: millis(&r.ttfb, 1).filter(|_| reached),
            })
        })
        .collect();
    buckets.sort_by(|a, b| (&a.start, &a.key).cmp(&(&b.start, &b.key)));
    Ok(Json(List::complete(buckets)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/usage/timeline",
    summary = "The org's requests and latency by minute or hour, from the request log.",
    description = "A bucket per `step` (a minute or an hour, UTC) with the requests that\n\
    started in it across the org's projects: requests, errors, tokens, cost, and\n\
    latency in milliseconds (`duration_*` for the whole request, `ttfb_*` to the\n\
    provider's first byte; percentiles are estimated from a sample in buckets of\n\
    more than 8,192 requests). Steps without requests are left out. It reads the\n\
    request log, so it reaches back only as far as the org keeps metadata; for\n\
    longer periods use `usage`. It takes the filters of `usage` plus `failed`;\n\
    with `group_by` there is a bucket per step and group, `key` naming the group\n\
    as `usage` does, and `top` keeps the busiest groups. `cost` is in USD and\n\
    `null` without `costs.org.view`.",
    tag = "orgs",
    params(("org" = OrgId, Path), TimelineQuery, UsageFilter, ProjectFilter),
    responses((status = 200, body = List<TimelineBucket>))
)]
pub async fn org_timeline(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_o): Path<String>,
    Query(q): Query<TimelineQuery>,
    Query(f): Query<UsageFilter>,
    Query(p): Query<ProjectFilter>,
) -> ApiResult<Json<List<TimelineBucket>>> {
    let scope = org_scope(&ctx)?.narrowed(&p);
    timeline(&state, &ctx, scope, &q, &f).await
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/usage/timeline",
    summary = "The project's requests and latency by minute or hour, from the request log.",
    description = "A bucket per `step` (a minute or an hour, UTC) with the project's\n\
    requests that started in it: requests, errors, tokens, cost, and latency in\n\
    milliseconds (`duration_*` for the whole request, `ttfb_*` to the provider's\n\
    first byte; percentiles are estimated from a sample in buckets of more than\n\
    8,192 requests). Steps without requests are left out. It reads the request\n\
    log, so it reaches back only as far as the org keeps metadata; for longer\n\
    periods use `usage`. It takes the filters of `usage` plus `failed`; with\n\
    `group_by` there is a bucket per step and group, `key` naming the group as\n\
    `usage` does, and `top` keeps the busiest groups. `cost` is in USD and\n\
    `null` without `costs.view`.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), TimelineQuery, UsageFilter),
    responses((status = 200, body = List<TimelineBucket>))
)]
pub async fn project_timeline(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(q): Query<TimelineQuery>,
    Query(f): Query<UsageFilter>,
) -> ApiResult<Json<List<TimelineBucket>>> {
    timeline(&state, &ctx, project_scope(&ctx)?, &q, &f).await
}
