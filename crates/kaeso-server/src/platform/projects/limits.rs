use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::decimal_input::LimitAmount;
use kaeso_core::id::{
    ApiKeyId, CatalogueModelId, LimitId, OrgId, ProjectId, ServiceAccountId, UserId,
};
use kaeso_core::limits::{self, Kind, Limit, Period, Subject};
use kaeso_store::pg::limits as store;
use kaeso_store::pg::{catalogue, keys, members, service_accounts};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::pagination::List;
use crate::platform::params::{Path, Query, id_param};
use crate::platform::precondition::{IfMatch, etag};
use crate::services::limits as engine;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Whom a limit applies to.")]
pub struct SubjectView {
    #[doc = "`project`, `user`, `service_account` or `key`."]
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[doc = "Its id."]
    pub id: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A limit with its use in the current period.")]
pub struct LimitView {
    #[doc = "Id."]
    pub id: LimitId,
    #[doc = "`project`, `member` or `key`."]
    pub level: &'static str,
    #[doc = "Whom it applies to."]
    pub subject: SubjectView,
    #[doc = "`request_rate`, `token_rate`, `token_budget` or `spend_budget`."]
    pub kind: &'static str,
    #[doc = "One catalogue model, or `null` for every model."]
    pub model: Option<CatalogueModelId>,
    #[doc = "`minute` (rates), `day`, `month` or `total` (budgets; UTC calendar periods)."]
    pub period: &'static str,
    #[doc = "Requests, tokens, or USD (a decimal string)."]
    #[schema(value_type = String, example = "50")]
    pub amount: String,
    #[doc = "Hard budgets refuse requests once used up; soft budgets only alert."]
    pub hard: bool,
    #[doc = "Alert thresholds in percent."]
    pub alert_thresholds: Vec<u8>,
    #[doc = "Used in the current period (same unit as `amount`), when known."]
    #[schema(value_type = Option<String>)]
    pub used: Option<String>,
    #[doc = "When the current period ends; `null` for `total`."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub resets_at: Option<OffsetDateTime>,
    #[doc = "A description, e.g. \"monthly spend budget of 50 USD\"."]
    pub description: String,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "Last changed."]
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[doc = "Goes up with every change. Send it as `If-Match: \"<revision>\"` to change\n\
        or remove the limit only if nobody changed it since."]
    #[schema(example = 1)]
    pub revision: i64,
}

fn subject_view(s: Subject) -> SubjectView {
    match s {
        Subject::Project(id) => SubjectView {
            kind: "project",
            id: ProjectId::from_uuid(id).to_string(),
        },
        Subject::User(id) => SubjectView {
            kind: "user",
            id: UserId::from_uuid(id).to_string(),
        },
        Subject::ServiceAccount(id) => SubjectView {
            kind: "service_account",
            id: ServiceAccountId::from_uuid(id).to_string(),
        },
        Subject::Key(id) => SubjectView {
            kind: "key",
            id: ApiKeyId::from_uuid(id).to_string(),
        },
    }
}

fn amount_text(l: &Limit) -> String {
    l.amount.normalize().to_string()
}

async fn view(
    state: &AppState,
    project: Uuid,
    row: &store::LimitRow,
    now: OffsetDateTime,
) -> ApiResult<LimitView> {
    let l = row.limit()?;
    let used = engine::current(state, project, &l, now)
        .await
        .map(|units| match l.kind {
            Kind::SpendBudget => kaeso_core::cost::from_nano(units).to_string(),
            _ => units.to_string(),
        });
    Ok(LimitView {
        id: LimitId::from_uuid(l.id),
        level: l.subject.level().as_str(),
        subject: subject_view(l.subject),
        kind: l.kind.as_str(),
        model: l.model.map(CatalogueModelId::from_uuid),
        period: l.period.as_str(),
        amount: amount_text(&l),
        hard: l.hard,
        alert_thresholds: l.thresholds.clone(),
        used,
        resets_at: if l.period == Period::Minute {
            None
        } else {
            l.period.resets_at(now)
        },
        description: l.describe(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    })
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(
    description = "A limit to set. Exactly one of `user`, `service_account` or `key` for\n\
    `member` and `key` levels; none for `project`."
)]
pub struct LimitCreate {
    #[doc = "`project`, `member` or `key`."]
    #[schema(example = "key")]
    pub level: String,
    #[doc = "The member, for `member`."]
    pub user: Option<UserId>,
    #[doc = "The member, for `member`."]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "The key, for `key`."]
    pub key: Option<ApiKeyId>,
    #[doc = "What it counts."]
    #[schema(example = "spend_budget")]
    pub kind: String,
    #[doc = "One catalogue model; every model when absent."]
    pub model: Option<CatalogueModelId>,
    #[doc = "`minute` for rates; `day`, `month` or `total` for budgets."]
    #[schema(example = "month")]
    pub period: String,
    #[doc = "Requests, tokens, or USD; at most 10^12, and 1,000,000,000 USD for a spend\n\
        budget."]
    #[schema(value_type = LimitAmount, example = "50")]
    #[serde(deserialize_with = "kaeso_core::limits::amount")]
    pub amount: Option<Decimal>,
    #[doc = "Hard (default) or soft; rates are always hard."]
    pub hard: Option<bool>,
    #[doc = "Budget alert thresholds in percent; default 50, 80 and 100. Rate limits\n\
        have no alerts; leave `alert_thresholds` out."]
    pub alert_thresholds: Option<Vec<u8>>,
}

async fn subject_of(state: &AppState, project: Uuid, req: &LimitCreate) -> ApiResult<Subject> {
    match (req.level.as_str(), req.user, req.service_account, req.key) {
        ("project", None, None, None) => Ok(Subject::Project(project)),
        ("member", Some(u), None, None) => {
            members::get_project(&state.pg, project, u.uuid())
                .await?
                .ok_or_else(|| {
                    ApiError::invalid("user", "This user is not a member of the project.")
                })?;
            Ok(Subject::User(u.uuid()))
        }
        ("member", None, Some(s), None) => {
            service_accounts::get(&state.pg, project, s.uuid())
                .await?
                .filter(|a| a.archived_at.is_none())
                .ok_or_else(|| {
                    ApiError::invalid(
                        "service_account",
                        "No active service account with this id in the project.",
                    )
                })?;
            Ok(Subject::ServiceAccount(s.uuid()))
        }
        ("key", None, None, Some(k)) => {
            keys::get(&state.pg, project, k.uuid())
                .await?
                .filter(|k| k.archived_at.is_none())
                .ok_or_else(|| {
                    ApiError::invalid("key", "No active key with this id in the project.")
                })?;
            Ok(Subject::Key(k.uuid()))
        }
        ("project", ..) => Err(ApiError::bad_request(
            "A project limit names no `user`, `service_account` or `key`.",
        )),
        ("member", ..) => Err(ApiError::bad_request(
            "A member limit names either `user` or `service_account`.",
        )),
        ("key", ..) => Err(ApiError::bad_request("A key limit names only `key`.")),
        _ => Err(ApiError::invalid(
            "level",
            "Use `project`, `member` or `key`.",
        )),
    }
}

pub(crate) async fn limit_from(
    state: &AppState,
    project: Uuid,
    subject: Subject,
    req: &LimitCreate,
) -> ApiResult<Limit> {
    let kind = Kind::parse(&req.kind).ok_or_else(|| {
        ApiError::invalid(
            "kind",
            "Use `request_rate`, `token_rate`, `token_budget` or `spend_budget`.",
        )
    })?;
    let period = Period::parse(&req.period)
        .ok_or_else(|| ApiError::invalid("period", "Use `minute`, `day`, `month` or `total`."))?;
    let amount = req
        .amount
        .ok_or_else(|| ApiError::invalid("amount", "An amount is required."))?;
    if let Some(m) = req.model {
        catalogue::get(&state.pg, project, m.uuid())
            .await?
            .ok_or_else(|| {
                ApiError::invalid("model", "No such model in this project's catalogue.")
            })?;
    }
    let l = Limit {
        id: Uuid::now_v7(),
        subject,
        kind,
        model: req.model.map(|m| m.uuid()),
        period,
        amount,
        hard: kind.is_rate() || req.hard.unwrap_or(true),
        thresholds: req
            .alert_thresholds
            .clone()
            .unwrap_or_else(|| kind.default_thresholds()),
    };
    limits::validate(&l).map_err(|e| ApiError::invalid(e.field, e.message))?;
    Ok(l)
}

async fn key_member(state: &AppState, project: Uuid, key: Uuid) -> ApiResult<Option<Subject>> {
    Ok(keys::get(&state.pg, project, key).await?.and_then(|k| {
        k.user_id
            .map(Subject::User)
            .or(k.service_account_id.map(Subject::ServiceAccount))
    }))
}

fn who(s: Subject) -> String {
    match s {
        Subject::Project(_) => "the project".into(),
        Subject::User(u) => format!("member {}", UserId::from_uuid(u)),
        Subject::ServiceAccount(a) => format!("member {}", ServiceAccountId::from_uuid(a)),
        Subject::Key(k) => format!("key {}", ApiKeyId::from_uuid(k)),
    }
}

pub(crate) async fn check_hierarchy(
    state: &AppState,
    project: Uuid,
    l: &Limit,
    all: &[Limit],
    member_hint: Option<Subject>,
) -> ApiResult<()> {
    let others: Vec<&Limit> = all.iter().filter(|o| o.id != l.id).collect();
    let member = match (l.subject, member_hint) {
        (Subject::Key(_), Some(m)) => Some(m),
        (Subject::Key(k), None) => key_member(state, project, k).await?,
        _ => None,
    };
    for parent in &others {
        let is_parent = match (l.subject, parent.subject) {
            (Subject::User(_) | Subject::ServiceAccount(_), Subject::Project(_)) => true,
            (Subject::Key(_), Subject::Project(_)) => true,
            (Subject::Key(_), s) => Some(s) == member,
            _ => false,
        };
        if is_parent && limits::exceeds(l, parent) {
            return Err(ApiError::invalid(
                "amount",
                format!(
                    "This limit is higher than the {} of {} ({}).",
                    parent.describe(),
                    who(parent.subject),
                    amount_text(parent)
                ),
            ));
        }
    }
    let mut over = Vec::new();
    for child in &others {
        let is_child = match (l.subject, child.subject) {
            (Subject::Project(_), Subject::Project(_)) => false,
            (Subject::Project(_), _) => true,
            (Subject::User(_) | Subject::ServiceAccount(_), Subject::Key(k)) => {
                key_member(state, project, k).await? == Some(l.subject)
            }
            _ => false,
        };
        if is_child && limits::exceeds(child, l) {
            over.push(format!("{} ({})", who(child.subject), amount_text(child)));
        }
    }
    if !over.is_empty() {
        return Err(ApiError::Conflict(format!(
            "First lower the limits that allow more than this one: {}.",
            over.join(", ")
        )));
    }
    Ok(())
}

pub(crate) async fn all_limits(
    db: impl sqlx::PgExecutor<'_>,
    project: Uuid,
) -> ApiResult<Vec<Limit>> {
    store::list(db, project)
        .await?
        .iter()
        .map(|r| r.limit().map_err(ApiError::from))
        .collect()
}

pub(crate) async fn create_limit(
    state: &AppState,
    ctx: &Ctx,
    l: &Limit,
) -> ApiResult<store::LimitRow> {
    let project = ctx.project()?.project.clone();
    let mut tx = state.pg.begin().await?;
    store::lock_project(&mut *tx, project.id).await?;
    let all = all_limits(&mut *tx, project.id).await?;
    check_hierarchy(state, project.id, l, &all, None).await?;
    if !store::insert(&mut *tx, project.org_id, project.id, l).await? {
        return Err(ApiError::Conflict(
            "This subject already has a limit of this kind, model and period.".into(),
        ));
    }
    let row = store::get(&mut *tx, project.id, l.id)
        .await?
        .ok_or_else(|| ApiError::internal("new limit vanished"))?;
    ctx.audit(Some(ctx.org()?.id()), Action::LimitCreate)
        .target("limit", LimitId::from_uuid(l.id))
        .after(serde_json::json!({ "subject": subject_view(l.subject).id, "description": l.describe(), "hard": l.hard }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    engine::forget(state, project.id).await;
    Ok(row)
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/projects/{project}/limits",
    summary = "Sets a limit on the project, a member or a key.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path)),
    request_body = LimitCreate,
    responses((status = 201, body = LimitView, headers(("ETag" = String, description = "The limit's revision as an entity tag."))))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Body(req): Body<LimitCreate>,
) -> ApiResult<(
    StatusCode,
    [(axum::http::HeaderName, axum::http::HeaderValue); 1],
    Json<LimitView>,
)> {
    super::ensure_active(&ctx, super::MODELS_ARCHIVED)?;
    let project = ctx.project()?.project.id;
    let subject = subject_of(&state, project, &req).await?;
    let l = limit_from(&state, project, subject, &req).await?;
    let row = create_limit(&state, &ctx, &l).await?;
    Ok((
        StatusCode::CREATED,
        etag(row.revision),
        Json(view(&state, project, &row, ctx.now).await?),
    ))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LimitFilter {
    #[doc = "Only this level."]
    pub level: Option<String>,
    #[doc = "Only this user's limits."]
    pub user: Option<UserId>,
    #[doc = "Only this service account's limits."]
    pub service_account: Option<ServiceAccountId>,
    #[doc = "Only this key's limits."]
    pub key: Option<ApiKeyId>,
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/limits",
    summary = "The project's limits with their current use.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), LimitFilter),
    responses((status = 200, body = List<LimitView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_p): Path<(String, String)>,
    Query(f): Query<LimitFilter>,
) -> ApiResult<Json<List<LimitView>>> {
    let project = ctx.project()?.project.id;
    let mut out = Vec::new();
    for row in store::list(&state.pg, project).await? {
        let keep = f.level.as_deref().is_none_or(|l| l == row.level)
            && f.user.is_none_or(|u| row.user_id == Some(u.uuid()))
            && f.service_account
                .is_none_or(|s| row.service_account_id == Some(s.uuid()))
            && f.key.is_none_or(|k| row.api_key_id == Some(k.uuid()));
        if keep {
            out.push(view(&state, project, &row, ctx.now).await?);
        }
    }
    Ok(Json(List::complete(out)))
}

async fn row(state: &AppState, ctx: &Ctx, id: &str) -> ApiResult<store::LimitRow> {
    let id: LimitId = id.parse().map_err(|_| ApiError::not_found("limit"))?;
    store::get(&state.pg, ctx.project()?.project.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("limit"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/projects/{project}/limits/{limit}",
    summary = "One limit with its current use.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("limit" = LimitId, Path)),
    responses((status = 200, body = LimitView, headers(("ETag" = String, description = "The limit's revision as an entity tag."))))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
) -> ApiResult<(Tagged, Json<LimitView>)> {
    let r = row(&state, &ctx, &id).await?;
    Ok((
        etag(r.revision),
        Json(view(&state, r.project_id, &r, ctx.now).await?),
    ))
}

type Tagged = [(axum::http::HeaderName, axum::http::HeaderValue); 1];

fn parse_id(id: &str) -> ApiResult<Uuid> {
    Ok(id_param::<LimitId>(id, "limit")?.uuid())
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "Limit changes.")]
pub struct LimitUpdate {
    #[doc = "A new amount, within the range the kind allows."]
    #[schema(value_type = Option<LimitAmount>)]
    #[serde(default, deserialize_with = "kaeso_core::limits::amount")]
    pub amount: Option<Decimal>,
    #[doc = "Hard or soft (budgets)."]
    pub hard: Option<bool>,
    #[doc = "New budget alert thresholds; rates have none."]
    pub alert_thresholds: Option<Vec<u8>>,
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/projects/{project}/limits/{limit}",
    summary = "Changes a limit's amount, hardness or alert thresholds.",
    description = "With `If-Match`, only while the limit is still at that revision; a\n\
    newer one answers 412.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("limit" = LimitId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    request_body = LimitUpdate,
    responses((status = 200, body = LimitView, headers(("ETag" = String, description = "The limit's new revision as an entity tag."))))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    if_match: IfMatch,
    Body(req): Body<LimitUpdate>,
) -> ApiResult<(Tagged, Json<LimitView>)> {
    super::ensure_active(&ctx, super::MODELS_ARCHIVED)?;
    let project = ctx.project()?.project.id;
    let id = parse_id(&id)?;
    let mut tx = state.pg.begin().await?;
    store::lock_project(&mut *tx, project).await?;
    let r = store::get(&mut *tx, project, id)
        .await?
        .ok_or_else(|| ApiError::not_found("limit"))?;
    if_match.check(r.revision, "limit")?;
    let before = r.limit()?;
    let mut l = before.clone();
    if let Some(a) = req.amount {
        l.amount = a;
    }
    if let Some(h) = req.hard {
        l.hard = l.kind.is_rate() || h;
    }
    if let Some(t) = req.alert_thresholds {
        l.thresholds = t;
    }
    limits::validate(&l).map_err(|e| ApiError::invalid(e.field, e.message))?;
    let all = all_limits(&mut *tx, project).await?;
    check_hierarchy(&state, project, &l, &all, None).await?;
    if !store::update(&mut *tx, project, &l).await? {
        return Err(ApiError::not_found("limit"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::LimitUpdate)
        .target("limit", LimitId::from_uuid(l.id))
        .before(serde_json::json!({ "description": before.describe(), "hard": before.hard, "alert_thresholds": before.thresholds }))
        .after(serde_json::json!({ "description": l.describe(), "hard": l.hard, "alert_thresholds": l.thresholds }))
        .write(&mut tx)
        .await?;
    let r = store::get(&mut *tx, project, id)
        .await?
        .ok_or_else(|| ApiError::internal("changed limit vanished"))?;
    tx.commit().await?;
    engine::forget(&state, project).await;
    Ok((
        etag(r.revision),
        Json(view(&state, project, &r, ctx.now).await?),
    ))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/projects/{project}/limits/{limit}",
    summary = "Removes a limit.",
    description = "With `If-Match`, only while the limit is still at that revision; a\n\
    newer one answers 412.",
    tag = "projects",
    params(("org" = OrgId, Path), ("project" = ProjectId, Path), ("limit" = LimitId, Path), ("If-Match" = Option<String>, Header, description = "The `ETag` the change is meant for, such as `\"3\"`.")),
    responses((status = 204))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_o, _p, id)): Path<(String, String, String)>,
    if_match: IfMatch,
) -> ApiResult<StatusCode> {
    super::ensure_active(&ctx, super::MODELS_ARCHIVED)?;
    let project = ctx.project()?.project.id;
    let id = parse_id(&id)?;
    let mut tx = state.pg.begin().await?;
    store::lock_project(&mut *tx, project).await?;
    let r = store::get(&mut *tx, project, id)
        .await?
        .ok_or_else(|| ApiError::not_found("limit"))?;
    if_match.check(r.revision, "limit")?;
    let l = r.limit()?;
    if !store::delete(&mut *tx, project, r.id).await? {
        return Err(ApiError::not_found("limit"));
    }
    ctx.audit(Some(ctx.org()?.id()), Action::LimitDelete)
        .target("limit", LimitId::from_uuid(l.id))
        .before(serde_json::json!({ "description": l.describe() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    engine::forget(&state, project).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A limit set when a key is created: only tighter than\n\
    the member's and the project's.")]
pub struct KeyLimit {
    #[doc = "What it counts."]
    #[schema(example = "spend_budget")]
    pub kind: String,
    #[doc = "One catalogue model; every model when absent."]
    pub model: Option<CatalogueModelId>,
    #[doc = "`minute` for rates; `day`, `month` or `total` for budgets."]
    #[schema(example = "day")]
    pub period: String,
    #[doc = "Requests, tokens, or USD; at most 10^12, and 1,000,000,000 USD for a spend\n\
        budget."]
    #[schema(value_type = LimitAmount, example = "5")]
    #[serde(deserialize_with = "kaeso_core::limits::amount")]
    pub amount: Option<Decimal>,
    #[doc = "Hard (default) or soft."]
    pub hard: Option<bool>,
    #[doc = "Budget alert thresholds in percent; default 50, 80 and 100. Rate limits\n\
        have no alerts; leave `alert_thresholds` out."]
    pub alert_thresholds: Option<Vec<u8>>,
}

pub(crate) async fn key_limits(
    state: &AppState,
    tx: &mut kaeso_store::pg::Tx<'_>,
    project: Uuid,
    key: Uuid,
    member: Option<Subject>,
    specs: &[KeyLimit],
) -> ApiResult<Vec<Limit>> {
    store::lock_project(&mut **tx, project).await?;
    let existing = all_limits(&mut **tx, project).await?;
    let mut out: Vec<Limit> = Vec::new();
    for spec in specs {
        let req = LimitCreate {
            level: "key".into(),
            user: None,
            service_account: None,
            key: None,
            kind: spec.kind.clone(),
            model: spec.model,
            period: spec.period.clone(),
            amount: spec.amount,
            hard: spec.hard,
            alert_thresholds: spec.alert_thresholds.clone(),
        };
        let l = limit_from(state, project, Subject::Key(key), &req).await?;
        if out
            .iter()
            .any(|o| o.kind == l.kind && o.model == l.model && o.period == l.period)
        {
            return Err(ApiError::invalid(
                "limits",
                "Each kind, model and period may appear once.",
            ));
        }
        check_hierarchy(state, project, &l, &existing, member).await?;
        out.push(l);
    }
    Ok(out)
}
