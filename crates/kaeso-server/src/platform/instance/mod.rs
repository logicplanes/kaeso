pub mod agents;
pub mod orgs;
pub mod settings;
pub mod system;
pub mod updates;
pub mod users;

use axum::Json;
use axum::extract::State;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use kaeso_core::rights::InstanceRight;

use super::access::{Access, Secure};
use super::audit_log::{self, AuditEventView, AuditFilter};
use super::error::ApiResult;
use super::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    use InstanceRight::*;
    let a = Access::InstanceAdmin;
    let i = Access::Instance;
    OpenApiRouter::new()
        .routes(s.route(i(InstanceUsersView), routes!(users::list)))
        .routes(s.route(i(InstanceUsersView), routes!(users::get)))
        .routes(s.route(i(InstanceUsersView), routes!(users::orgs)))
        .routes(s.route(i(InstanceUsersManage), routes!(users::invite)))
        .routes(s.route(i(InstanceUsersManage), routes!(users::deactivate)))
        .routes(s.route(i(InstanceUsersManage), routes!(users::reactivate)))
        .routes(s.route(a, routes!(users::delete)))
        .routes(s.route(i(InstanceUsersManage), routes!(users::end_sessions)))
        .routes(s.route(i(InstanceUsersManage), routes!(users::reset_2fa)))
        .routes(s.route(a, routes!(users::set_admin)))
        .routes(s.route(i(InstanceOrgsView), routes!(orgs::list)))
        .routes(s.route(i(InstanceOrgsManage), routes!(orgs::create)))
        .routes(s.route(i(InstanceOrgsView), routes!(orgs::get)))
        .routes(s.route(i(InstanceOrgsView), routes!(orgs::members_list)))
        .routes(s.route(i(InstanceOrgsManage), routes!(orgs::appoint_owner)))
        .routes(s.route(i(InstanceOrgsManage), routes!(orgs::rename)))
        .routes(s.route(i(InstanceOrgsManage), routes!(orgs::schedule_deletion)))
        .routes(s.route(i(InstanceOrgsManage), routes!(orgs::restore)))
        .routes(s.route(a, routes!(orgs::delete_now)))
        .routes(s.route(i(InstanceSystemView), routes!(settings::get)))
        .routes(s.route(i(InstanceSettingsManage), routes!(settings::update)))
        .routes(s.route(a, routes!(settings::set_admin_sso)))
        .routes(s.route(i(InstanceSystemView), routes!(agents::get)))
        .routes(s.route(a, routes!(agents::set)))
        .routes(s.route(i(InstanceSettingsManage), routes!(settings::test_email)))
        .routes(s.route(i(InstanceSystemView), routes!(system::get)))
        .routes(s.route(i(InstanceSystemView), routes!(updates::get)))
        .routes(s.route(i(InstanceUpdatesManage), routes!(updates::check)))
        .routes(s.route(i(InstanceUpdatesManage), routes!(updates::settings)))
        .routes(s.route(i(InstanceUpdatesApply), routes!(updates::apply)))
        .routes(s.route(i(InstanceSystemView), routes!(updates::jobs)))
        .routes(s.route(i(InstanceSystemView), routes!(updates::job)))
        .routes(s.route(i(InstanceAuditLogView), routes!(audit_list)))
        .routes(s.route(i(InstanceAuditLogView), routes!(audit_get)))
        .routes(s.route(i(InstanceOrgsView), routes!(orgs::projects_list)))
        .routes(s.route(i(InstanceOrgsView), routes!(orgs::usage)))
}

#[utoipa::path(
    get,
    path = "/instance/audit-log/{event}",
    summary = "One event of the instance audit log.",
    tag = "instance",
    params(("event" = kaeso_core::id::AuditEventId, Path)),
    responses((status = 200, body = AuditEventView))
)]
pub async fn audit_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<AuditEventView>> {
    Ok(Json(audit_log::one(&state.pg, None, &id).await?))
}

#[utoipa::path(
    get,
    path = "/instance/audit-log",
    summary = "The instance audit log, newest first.",
    tag = "instance",
    params(PageParams, AuditFilter),
    responses((status = 200, body = List<AuditEventView>))
)]
pub async fn audit_list(
    State(state): State<AppState>,
    Query(page): Query<PageParams>,
    Query(filter): Query<AuditFilter>,
) -> ApiResult<Json<List<AuditEventView>>> {
    audit_log::check_range(&filter)?;
    Ok(Json(
        audit_log::page(&state.pg, None, page.page()?, &filter).await?,
    ))
}
