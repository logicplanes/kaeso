use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::rights::{OrgGrant, OrgPreset};
use kaeso_store::pg::{members, orgs, users};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::org_to_store;
use crate::platform::json::Body;
use crate::platform::orgs::members::MemberView;
use crate::platform::orgs::org::OrgView;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::validate::validate_name;
use crate::services::orgs as org_service;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org with its size.")]
pub struct InstanceOrgView {
    #[doc = "The org."]
    #[serde(flatten)]
    pub org: OrgView,
    #[doc = "Its members."]
    pub members: i64,
    #[doc = "Its active projects."]
    pub projects: i64,
    #[doc = "Its owners with active accounts."]
    pub owners: i64,
}

fn sized(o: &orgs::Org, size: members::OrgSize) -> InstanceOrgView {
    InstanceOrgView {
        org: OrgView::from_row(o),
        members: size.members,
        projects: size.projects,
        owners: size.owners,
    }
}

async fn view(state: &AppState, o: &orgs::Org) -> ApiResult<InstanceOrgView> {
    let size = members::org_sizes(&state.pg, &[o.id])
        .await?
        .into_iter()
        .next()
        .map(|(_, s)| s)
        .unwrap_or_default();
    Ok(sized(o, size))
}

#[derive(Clone, Copy, Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "The order of the list.")]
pub enum OrgSort {
    #[default]
    Created,
    Deletion,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OrgFilter {
    #[doc = "Part of the name."]
    pub search: Option<String>,
    #[doc = "`created` (default) or `deletion`. A cursor continues the order it\n\
        came from."]
    #[param(inline)]
    pub sort: Option<OrgSort>,
    #[doc = "Only orgs with (`true`) or without (`false`) an SSO connection."]
    pub sso: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/instance/orgs",
    summary = "Every org of the install.",
    tag = "instance",
    params(PageParams, OrgFilter),
    responses((status = 200, body = List<InstanceOrgView>))
)]
pub async fn list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
    Query(f): Query<OrgFilter>,
) -> ApiResult<Json<List<InstanceOrgView>>> {
    let page = page.page()?;
    let order = match f.sort.unwrap_or_default() {
        OrgSort::Created => orgs::OrgOrder::Created,
        OrgSort::Deletion => orgs::OrgOrder::Deletion,
    };
    let rows = orgs::list(
        &state.pg,
        page.after,
        page.fetch(),
        f.search.as_deref(),
        order,
        f.sso,
    )
    .await?;
    let list = List::from_rows(&page, rows, |o| o.id, |o| o);
    let ids: Vec<uuid::Uuid> = list.data.iter().map(|o| o.id).collect();
    let sizes: std::collections::HashMap<uuid::Uuid, members::OrgSize> =
        members::org_sizes(&state.pg, &ids)
            .await?
            .into_iter()
            .collect();
    let data = list
        .data
        .iter()
        .map(|o| sized(o, sizes.get(&o.id).copied().unwrap_or_default()))
        .collect();
    Ok(Json(List {
        data,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new org.")]
pub struct InstanceOrgCreate {
    #[doc = "The name."]
    #[schema(example = "Acme")]
    pub name: String,
    #[doc = "The owner: an existing user. Default: the instance admin creating it."]
    pub owner: Option<UserId>,
}

#[utoipa::path(
    post,
    path = "/instance/orgs",
    summary = "Creates an org with an owner.",
    tag = "instance",
    request_body = InstanceOrgCreate,
    responses((status = 201, body = InstanceOrgView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<InstanceOrgCreate>,
) -> ApiResult<(StatusCode, Json<InstanceOrgView>)> {
    let name = validate_name("name", &req.name)?;
    let owner = match req.owner {
        Some(o) => o,
        None => ctx.user_id()?,
    };
    users::get(&state.pg, owner.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::invalid("owner", "The owner must be an active user."))?;
    let mut tx = state.pg.begin().await?;
    let (org, _) = org_service::create(&mut tx, &name, owner).await?;
    ctx.audit(Some(org), Action::OrgCreate)
        .target("org", org)
        .after(serde_json::json!({ "name": name, "owner": owner.to_string() }))
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgCreate)
        .target("org", org)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let row = orgs::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    Ok((StatusCode::CREATED, Json(view(&state, &row).await?)))
}

async fn org(state: &AppState, id: &str) -> ApiResult<orgs::Org> {
    let id: OrgId = id
        .parse()
        .map_err(|_| ApiError::not_found("organization"))?;
    orgs::get(&state.pg, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))
}

#[utoipa::path(
    get,
    path = "/instance/orgs/{org}",
    summary = "One org's metadata.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = InstanceOrgView))
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceOrgView>> {
    Ok(Json(view(&state, &org(&state, &id).await?).await?))
}

#[utoipa::path(
    get,
    path = "/instance/orgs/{org}/members",
    summary = "One org's members.",
    tag = "instance",
    params(("org" = OrgId, Path), PageParams),
    responses((status = 200, body = List<MemberView>))
)]
pub async fn members_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<List<MemberView>>> {
    let o = org(&state, &id).await?;
    let page = page.page()?;
    let rows = members::list_org(&state.pg, o.id, page.after, page.fetch(), None, None).await?;
    let list = List::from_rows(&page, rows, |m| m.id, |m| m);
    let domains = crate::platform::orgs::members::domains(&state, o.id).await?;
    Ok(Json(List {
        data: list
            .data
            .iter()
            .map(|m| crate::platform::orgs::members::view(m, &domains))
            .collect::<ApiResult<_>>()?,
        has_more: list.has_more,
        next_cursor: list.next_cursor,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The user to appoint.")]
pub struct AppointOwner {
    #[doc = "An active user who becomes owner."]
    pub user_id: UserId,
}

#[utoipa::path(
    post,
    path = "/instance/orgs/{org}/owner",
    summary = "Makes an active user an owner of the org, whether it has owners or not:\n\
    how an instance admin takes over an org\n\
    whose owners left, or gets in to manage it. Recorded in the org's audit\n\
    log, which its members see, and in the instance audit log. An owner\n\
    already is one: nothing changes. Needs a recent sign-in.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    request_body = AppointOwner,
    responses((status = 200, body = InstanceOrgView))
)]
pub async fn appoint_owner(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Body(req): Body<AppointOwner>,
) -> ApiResult<Json<InstanceOrgView>> {
    ctx.require_recent_sign_in()?;
    let o = org(&state, &id).await?;
    let org_id = OrgId::from_uuid(o.id);
    users::get(&state.pg, req.user_id.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(|| ApiError::invalid("user_id", "The new owner must be an active user."))?;
    if o.state == "scheduled_for_deletion" {
        return Err(ApiError::Conflict(
            "Restore the organization before appointing an owner.".into(),
        ));
    }
    let mut tx = state.pg.begin().await?;
    if members::get_org(&mut *tx, o.id, req.user_id.uuid())
        .await?
        .is_some_and(|m| m.preset == "owner")
    {
        tx.rollback().await?;
        return Ok(Json(view(&state, &o).await?));
    }
    let owner = org_to_store(&OrgGrant::preset(OrgPreset::Owner));
    if !org_service::add_member(&mut tx, org_id, req.user_id, &owner).await? {
        members::update_org(&mut *tx, o.id, req.user_id.uuid(), &owner).await?;
    }
    ctx.audit(Some(org_id), Action::OrgOwnerAppointedByInstanceAdmin)
        .target("user", req.user_id)
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgOwnerAppointed)
        .target("org", org_id)
        .after(serde_json::json!({ "owner": req.user_id.to_string() }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_user_in_org(&state, o.id, req.user_id.uuid()).await;
    Ok(Json(view(&state, &org(&state, &id).await?).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A new name for an org.")]
pub struct InstanceOrgUpdate {
    #[doc = "1 to 100 characters."]
    #[schema(example = "Acme Inc.")]
    pub name: String,
}

#[utoipa::path(
    patch,
    path = "/instance/orgs/{org}",
    summary = "Renames an org. Recorded in the org's audit log and the instance's.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    request_body = InstanceOrgUpdate,
    responses((status = 200, body = InstanceOrgView))
)]
pub async fn rename(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
    Body(req): Body<InstanceOrgUpdate>,
) -> ApiResult<Json<InstanceOrgView>> {
    let o = org(&state, &id).await?;
    let org_id = OrgId::from_uuid(o.id);
    let name = validate_name("name", &req.name)?;
    let mut tx = state.pg.begin().await?;
    let mut row = orgs::get_for_update(&mut *tx, o.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    let before = OrgView::from_row(&row);
    row.name = name;
    orgs::update(&mut *tx, &row).await?;
    let after = OrgView::from_row(&row);
    ctx.audit(Some(org_id), Action::OrgUpdate)
        .target("org", org_id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgUpdate)
        .target("org", org_id)
        .before(&before)
        .after(&after)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(view(&state, &row).await?))
}

#[utoipa::path(
    post,
    path = "/instance/orgs/{org}/schedule-deletion",
    summary = "Schedules an org for deletion, as its owner can: its\n\
    keys stop working at once, members and instance admins are told, and it\n\
    stays read-only for the instance's grace period. Needs a recent sign-in\n\
    and an instance that allows deleting orgs.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = InstanceOrgView))
)]
pub async fn schedule_deletion(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceOrgView>> {
    ctx.require_recent_sign_in()?;
    let o = org(&state, &id).await?;
    let org_id = OrgId::from_uuid(o.id);
    let mut tx = state.pg.begin().await?;
    let (allowed, grace) = org_service::deletion_policy(&mut *tx).await?;
    org_service::ensure_deletion_allowed(allowed)?;
    let mut row = orgs::get_for_update(&mut *tx, o.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    if row.state == "scheduled_for_deletion" {
        return Err(ApiError::Conflict(
            "The organization is already scheduled for deletion.".into(),
        ));
    }
    row.state = "scheduled_for_deletion".into();
    row.deletion_due_at = Some(ctx.now + grace);
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(Some(org_id), Action::OrgScheduleDeletion)
        .target("org", org_id)
        .after(serde_json::json!({ "deletion_due_at": row.deletion_due_at }))
        .write(&mut tx)
        .await?;
    ctx.audit(None, Action::OrgScheduleDeletion)
        .target("org", org_id)
        .after(serde_json::json!({ "deletion_due_at": row.deletion_due_at }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_org(&state, row.id).await;
    org_service::notify_deletion(&state, &ctx, &row).await?;
    Ok(Json(view(&state, &row).await?))
}

#[utoipa::path(
    post,
    path = "/instance/orgs/{org}/restore",
    summary = "Restores an org scheduled for deletion.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = InstanceOrgView))
)]
pub async fn restore(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceOrgView>> {
    let o = org(&state, &id).await?;
    let row = org_service::restore(&state, &ctx, OrgId::from_uuid(o.id)).await?;
    Ok(Json(view(&state, &row).await?))
}

#[utoipa::path(
    delete,
    path = "/instance/orgs/{org}",
    summary = "Deletes an org scheduled for deletion now, instead of at its date.\n\
    Needs a recent sign-in and an instance that allows\n\
    deleting orgs. `204` once it is gone; `202` when its logs cannot be\n\
    deleted yet, and the next run of the deletion job finishes it.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    responses(
        (status = 204, description = "Deleted for good."),
        (status = 202, description = "Due now; deleted for good within minutes.")
    )
)]
pub async fn delete_now(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    ctx.require_recent_sign_in()?;
    let o = org(&state, &id).await?;
    let org_id = OrgId::from_uuid(o.id);
    let mut tx = state.pg.begin().await?;
    let (allowed, _) = org_service::deletion_policy(&mut *tx).await?;
    org_service::ensure_deletion_allowed(allowed)?;
    let mut row = orgs::get_for_update(&mut *tx, o.id)
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    if row.state != "scheduled_for_deletion" {
        return Err(ApiError::Conflict(
            "Only an organization scheduled for deletion can be deleted now.".into(),
        ));
    }
    let before = row.deletion_due_at;
    row.deletion_due_at = Some(ctx.now);
    orgs::update(&mut *tx, &row).await?;
    ctx.audit(None, Action::OrgDeleteNow)
        .target("org", org_id)
        .before(serde_json::json!({ "name": row.name, "deletion_due_at": before }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    match org_service::purge(&state, o.id, ctx.now).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::warn!(org = %org_id, error = %e, "the org is deleted by the next job run");
            Ok(StatusCode::ACCEPTED)
        }
    }
}

#[utoipa::path(
    get,
    path = "/instance/orgs/{org}/projects",
    summary = "One org's projects.",
    tag = "instance",
    params(("org" = OrgId, Path), PageParams),
    responses((status = 200, body = List<crate::platform::projects::project::ProjectView>))
)]
pub async fn projects_list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<List<crate::platform::projects::project::ProjectView>>> {
    let o = org(&state, &id).await?;
    let page = page.page()?;
    let rows = kaeso_store::pg::projects::list(
        &state.pg,
        o.id,
        None,
        None,
        true,
        page.after,
        page.fetch(),
    )
    .await?;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |p| p.id,
        |p| crate::platform::projects::project::ProjectView::from_row(&p),
    )))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An org's usage over the last 30 days and its log storage.")]
pub struct InstanceOrgUsage {
    #[doc = "Requests in the last 30 days."]
    pub requests_30d: u64,
    #[doc = "Tokens (input and output, cache included) in the last 30 days."]
    pub tokens_30d: u64,
    #[doc = "Cost in USD in the last 30 days."]
    #[schema(value_type = String)]
    pub cost_30d: String,
    #[doc = "Log entries; `null` when the install keeps no request logs."]
    pub log_entries: Option<u64>,
    #[doc = "Logged content as received, in bytes; `null` when the install keeps\n\
        no request logs."]
    pub log_logical_bytes: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/instance/orgs/{org}/usage",
    summary = "An org's usage totals and log storage.",
    tag = "instance",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = InstanceOrgUsage))
)]
pub async fn usage(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<InstanceOrgUsage>> {
    use kaeso_store::pg::usage as store;
    let o = org(&state, &id).await?;
    let to = ctx.now.date();
    let unavailable = |e: kaeso_store::StoreError| {
        ApiError::unavailable(crate::platform::usage::USAGE_UNAVAILABLE, e)
    };
    let rows = store::usage(
        &state.pg,
        o.id,
        None,
        to - time::Duration::days(29),
        to,
        store::GroupBy::Project,
    )
    .await
    .map_err(unavailable)?;
    let storage = match &state.clickhouse {
        Some(ch) => Some(
            kaeso_store::clickhouse::logs::storage(ch, o.id)
                .await
                .map_err(unavailable)?,
        ),
        None => None,
    };
    let count = |n: i64| u64::try_from(n).unwrap_or(0);
    let cost: rust_decimal::Decimal = rows.iter().map(|r| r.cost).sum();
    Ok(Json(InstanceOrgUsage {
        requests_30d: rows.iter().map(|r| count(r.requests)).sum(),
        tokens_30d: rows
            .iter()
            .map(|r| {
                count(r.input_tokens)
                    + count(r.output_tokens)
                    + count(r.cache_read_tokens)
                    + count(r.cache_write_tokens)
            })
            .sum(),
        cost_30d: cost.normalize().to_string(),
        log_entries: storage.as_ref().map(|s| s.iter().map(|p| p.entries).sum()),
        log_logical_bytes: storage
            .as_ref()
            .map(|s| s.iter().map(|p| p.logical_bytes).sum()),
    }))
}
