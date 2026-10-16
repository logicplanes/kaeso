pub mod access;
pub mod aliases;
pub mod audit;
pub mod keys;
pub mod limits;
pub mod members;
pub mod models;
pub mod playground;
pub mod project;
pub mod service_accounts;

use kaeso_core::rights::{OrgRight, ProjectRight as R};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::access::{Access, Secure};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(Access::Org(OrgRight::OrgView), routes!(project::list)))
        .routes(s.route(Access::Org(OrgRight::ProjectsCreate), routes!(project::create)))
        .routes(s.route(Access::Project(R::ProjectView), routes!(project::get)))
        .routes(s.route(Access::Project(R::ProjectUpdate), routes!(project::update)))
        .routes(s.route(Access::Project(R::SecurityKeyPolicyProjectManage), routes!(project::key_policy)))
        .routes(s.route(Access::Project(R::ProjectUpdate), routes!(project::set_avatar, project::remove_avatar)))
        .routes(s.route(Access::Project(R::ProjectArchive), routes!(project::archive)))
        .routes(s.route(Access::Org(OrgRight::OrgUpdate), routes!(project::set_default)))
        .routes(s.route(Access::Project(R::ProjectMembersView), routes!(members::list)))
        .routes(s.route(Access::Project(R::ProjectMembersView), routes!(members::get)))
        .routes(s.route(Access::Project(R::ProjectMembersAdd), routes!(members::add)))
        .routes(s.route(Access::Project(R::ProjectMembersRightsUpdate), routes!(members::update)))
        .routes(s.route(Access::Project(R::ProjectMembersRemove), routes!(members::remove)))
        .routes(s.route(Access::Project(R::ServiceAccountsView), routes!(service_accounts::list)))
        .routes(s.route(Access::Project(R::ServiceAccountsCreate), routes!(service_accounts::create)))
        .routes(s.route(Access::Project(R::ServiceAccountsView), routes!(service_accounts::get)))
        .routes(s.route(Access::Project(R::ServiceAccountsRightsUpdate), routes!(service_accounts::update)))
        .routes(s.route(Access::Project(R::ServiceAccountsArchive), routes!(service_accounts::archive)))
        .routes(s.route(Access::Project(R::KeysOwnCreate), routes!(keys::create)))
        .routes(s.route(Access::Project(R::ServiceAccountsKeysCreate), routes!(keys::create_for_service_account)))
        .routes(s.route(Access::Project(R::KeysAllView), routes!(keys::list)))
        .routes(s.route(Access::Project(R::KeysAllView), routes!(keys::get)))
        .routes(s.route(Access::Project(R::KeysAllManage), routes!(keys::disable)))
        .routes(s.route(Access::Project(R::KeysAllManage), routes!(keys::enable)))
        .routes(s.route(Access::Project(R::KeysAllManage), routes!(keys::archive)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(models::search)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(models::preview)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(models::list)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(models::create)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(models::batch)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(models::get)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(models::update)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(models::delete)))
        .routes(s.route(Access::Project(R::PlaygroundUse), routes!(playground::responses)))
        .routes(s.route(Access::Project(R::PlaygroundUse), routes!(playground::models)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(aliases::list)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(aliases::create)))
        .routes(s.route(Access::Project(R::ModelsView), routes!(aliases::get)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(aliases::update)))
        .routes(s.route(Access::Project(R::ModelsManage), routes!(aliases::delete)))
        .routes(s.route(Access::Project(R::LimitsView), routes!(access::member_get)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(access::member_set)))
        .routes(s.route(Access::Project(R::LimitsView), routes!(access::account_get)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(access::account_set)))
        .routes(s.route(Access::Project(R::LimitsView), routes!(access::key_get)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(access::key_set)))
        .routes(s.route(Access::Project(R::ProjectView), routes!(crate::platform::logs::get_project_settings)))
        .routes(s.route(Access::Project(R::LogsContentDisable), routes!(crate::platform::logs::update_project_settings)))
        .routes(s.route(Access::Project(R::LogsMetadataView), routes!(crate::platform::logs::list)))
        .routes(s.route(Access::Project(R::LogsExport), routes!(crate::platform::logs::export)))
        .routes(s.route(Access::Project(R::LogsMetadataView), routes!(crate::platform::logs::get)))
        .routes(s.route(Access::Project(R::UsageView), routes!(crate::platform::usage::project)))
        .routes(s.route(Access::Project(R::UsageView), routes!(crate::platform::usage::project_span)))
        .routes(s.route(Access::Project(R::LimitsView), routes!(limits::list)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(limits::create)))
        .routes(s.route(Access::Project(R::LimitsView), routes!(limits::get)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(limits::update)))
        .routes(s.route(Access::Project(R::LimitsManage), routes!(limits::delete)))
        .routes(s.route(Access::Project(R::ProjectMembersView), routes!(crate::platform::rights::project_member)))
        .routes(s.route(Access::Project(R::ProjectView), routes!(crate::platform::rights::mine)))
        .routes(s.route(Access::Project(R::ProjectView), routes!(crate::platform::rights::project_grantors)))
        .routes(s.route(Access::Project(R::ProjectAuditLogView), routes!(audit::list)))
}
