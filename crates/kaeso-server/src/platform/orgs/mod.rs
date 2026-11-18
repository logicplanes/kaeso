pub mod agents;
pub mod credential_accounts;
pub mod credential_links;
pub mod credentials;
pub mod custom_providers;
pub mod guardrails;
pub mod invites;
pub mod linked_billing;
pub mod mcp_servers;
pub mod members;
pub mod org;
pub mod platform_keys;
pub mod roles;
pub mod scim;
pub mod security;
pub mod sso_groups;
pub mod webhooks;

use axum::Json;
use axum::extract::State;
use axum::response::Response;
use kaeso_core::id::OrgId;
use kaeso_core::rights::OrgRight as R;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::access::{Access, Ctx, Secure};
use super::audit_log::{self, AuditEventView, AuditFilter};
use super::error::ApiResult;
use super::pagination::{List, PageParams};
use crate::platform::audit::Action;
use crate::platform::params::{Path, Query};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(Access::Org(R::OrgView), routes!(org::get)))
        .routes(s.route(Access::Org(R::OrgUpdate), routes!(org::update)))
        .routes(s.route(Access::Org(R::OrgUpdate), routes!(org::set_avatar, org::remove_avatar)))
        .routes(s.route(Access::Org(R::OrgDelete), routes!(org::delete)))
        .routes(s.route(Access::Org(R::OrgDelete), routes!(org::restore)))
        .routes(s.route(Access::Org(R::OrgOwnershipTransfer), routes!(org::transfer)))
        .routes(s.route(Access::Org(R::MembersView), routes!(members::list)))
        .routes(s.route(Access::Org(R::MembersView), routes!(members::get)))
        .routes(s.route(
            Access::Org(R::MembersRightsUpdate),
            routes!(members::update),
        ))
        .routes(s.route(Access::Org(R::MembersRemove), routes!(members::remove)))
        .routes(s.route(Access::Org(R::MembersInvite), routes!(invites::create)))
        .routes(s.route(Access::Org(R::MembersView), routes!(invites::list)))
        .routes(s.route(Access::Org(R::MembersInvite), routes!(invites::resend)))
        .routes(s.route(
            Access::Org(R::MembersInvitesRevoke),
            routes!(invites::revoke),
        ))
        .routes(s.route(Access::Org(R::OrgView), routes!(security::get)))
        .routes(s.route(Access::Org(R::OrgView), routes!(scim::get)))
        .routes(s.route(Access::Org(R::SecuritySsoManage), routes!(scim::rotate, scim::delete)))
        .routes(s.route(Access::Org(R::OrgView), routes!(sso_groups::get)))
        .routes(s.route(Access::OrgSession(R::SecuritySsoManage), routes!(sso_groups::put)))
        .routes(s.route(Access::Org(R::MembersView), routes!(agents::list)))
        .routes(s.route(Access::OrgSession(R::MembersRightsUpdate), routes!(agents::revoke)))
        .routes(s.route(Access::Org(R::OrgView), routes!(agents::policy)))
        .routes(s.route(Access::OrgSession(R::MembersRightsUpdate), routes!(agents::set_policy)))
        .routes(s.route(
            Access::Org(R::SecuritySsoManage),
            routes!(security::put_sso),
        ))
        .routes(s.route(
            Access::Org(R::SecuritySsoManage),
            routes!(security::delete_sso),
        ))
        .routes(s.route(
            Access::Org(R::SecuritySsoManage),
            routes!(security::verify_domains),
        ))
        .routes(s.route(
            Access::Org(R::SecuritySsoRequire),
            routes!(security::require_sso),
        ))
        .routes(s.route(
            Access::Org(R::Security2faRequire),
            routes!(security::require_2fa),
        ))
        .routes(s.route(
            Access::Org(R::SecurityKeyPolicyManage),
            routes!(security::key_policy),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysCreate),
            routes!(platform_keys::create),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysView),
            routes!(platform_keys::list),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysRevoke),
            routes!(platform_keys::disable),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysRevoke),
            routes!(platform_keys::enable),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysRevoke),
            routes!(platform_keys::archive),
        ))
        .routes(s.route(
            Access::Org(R::PlatformKeysCreate),
            routes!(platform_keys::rotate),
        ))
        .routes(s.route(Access::Org(R::ProviderCredentialsCreate), routes!(credentials::create)))
        .routes(s.route(Access::Org(R::ProviderCredentialsView), routes!(credentials::list)))
        .routes(s.route(Access::Org(R::ProviderCredentialsView), routes!(credentials::get)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credentials::replace)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credentials::rename)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credentials::make_default)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credentials::assign)))
        .routes(s.route(Access::Org(R::ProviderCredentialsDelete), routes!(credentials::delete)))
        .routes(s.route(Access::Org(R::ProviderCredentialsCreate), routes!(credential_accounts::start)))
        .routes(s.route(Access::Org(R::ProviderCredentialsCreate), routes!(credential_accounts::finish)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credential_accounts::start_again)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(credential_accounts::finish_again)))
        .routes(s.route(Access::Org(R::ProviderCredentialsView), routes!(linked_billing::fallbacks)))
        .routes(s.route(Access::Org(R::ProviderCredentialsReplace), routes!(linked_billing::set_fallback)))
        .routes(s.route(Access::Org(R::CostsOrgView), routes!(linked_billing::costs)))
        .routes(s.route(Access::Org(R::OrgView), routes!(crate::platform::logs::get_org_settings)))
        .routes(s.route(Access::Org(R::LogsSettingsManage), routes!(crate::platform::logs::update_org_settings)))
        .routes(s.route(Access::Org(R::OrgView), routes!(crate::platform::response_cache::get_org)))
        .routes(s.route(Access::Org(R::OrgUpdate), routes!(crate::platform::response_cache::update_org)))
        .routes(s.route(Access::Org(R::OrgView), routes!(crate::platform::stored_responses::get_org)))
        .routes(s.route(Access::Org(R::OrgUpdate), routes!(crate::platform::stored_responses::update_org)))
        .routes(s.route(Access::Org(R::LogsStorageView), routes!(crate::platform::logs::storage)))
        .routes(s.route(Access::Org(R::LogsDelete), routes!(crate::platform::logs::delete)))
        .routes(s.route(Access::Org(R::LogsStorageView), routes!(crate::platform::logs::deletions)))
        .routes(s.route(Access::Org(R::UsageOrgView), routes!(crate::platform::usage::org)))
        .routes(s.route(Access::Org(R::UsageOrgView), routes!(crate::platform::usage::org_span)))
        .routes(s.route(Access::Org(R::UsageOrgView), routes!(crate::platform::usage::org_timeline)))
        .routes(s.route(Access::Org(R::AuditLogView), routes!(audit_list)))
        .routes(s.route(Access::Org(R::AuditLogView), routes!(audit_get)))
        .routes(s.route(Access::Org(R::MembersView), routes!(invites::get)))
        .routes(s.route(Access::Org(R::PlatformKeysView), routes!(platform_keys::get)))
        .routes(s.route(Access::Org(R::AuditLogExport), routes!(audit_export)))
        .routes(s.route(Access::Org(R::OrgView), routes!(roles::list)))
        .routes(s.route(Access::Org(R::OrgView), routes!(roles::get)))
        .routes(s.route(Access::Org(R::RolesManage), routes!(roles::create)))
        .routes(s.route(Access::Org(R::RolesManage), routes!(roles::update)))
        .routes(s.route(Access::Org(R::RolesManage), routes!(roles::delete)))
        .routes(s.route(Access::Org(R::OrgView), routes!(crate::platform::rights::catalogue)))
        .routes(s.route(Access::Org(R::OrgView), routes!(crate::platform::providers::list)))
        .routes(s.route(Access::Org(R::ProvidersCustomView), routes!(custom_providers::list)))
        .routes(s.route(Access::Org(R::ProvidersCustomView), routes!(custom_providers::get)))
        .routes(s.route(Access::Org(R::ProvidersCustomManage), routes!(custom_providers::create)))
        .routes(s.route(Access::Org(R::ProvidersCustomManage), routes!(custom_providers::update)))
        .routes(s.route(Access::Org(R::ProvidersCustomManage), routes!(custom_providers::delete)))
        .routes(s.route(Access::Org(R::ProvidersCustomManage), routes!(custom_providers::detect)))
        .routes(s.route(Access::Org(R::ProvidersCustomView), routes!(custom_providers::servers)))
        .routes(s.route(Access::Org(R::McpServersView), routes!(mcp_servers::list)))
        .routes(s.route(Access::Org(R::McpServersView), routes!(mcp_servers::get)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::probe)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::create)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::update)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::delete)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::connect)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::refresh)))
        .routes(s.route(Access::Org(R::McpServersView), routes!(mcp_servers::list_tools)))
        .routes(s.route(Access::Org(R::McpServersView), routes!(mcp_servers::change)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::approve)))
        .routes(s.route(Access::Org(R::McpServersManage), routes!(mcp_servers::reject)))
        .routes(s.route(Access::Org(R::UsageShare), routes!(crate::platform::share::create)))
        .routes(s.route(Access::Org(R::UsageShare), routes!(crate::platform::share::list)))
        .routes(s.route(Access::Org(R::UsageShare), routes!(crate::platform::share::revoke)))
        .routes(s.route(Access::Org(R::UsageShareManage), routes!(crate::platform::share::set_policy)))
        .routes(s.route(Access::Org(R::WebhooksView), routes!(webhooks::list)))
        .routes(s.route(Access::Org(R::WebhooksView), routes!(webhooks::get)))
        .routes(s.route(Access::Org(R::WebhooksManage), routes!(webhooks::create)))
        .routes(s.route(Access::Org(R::WebhooksManage), routes!(webhooks::update)))
        .routes(s.route(Access::Org(R::WebhooksManage), routes!(webhooks::delete)))
        .routes(s.route(Access::Org(R::WebhooksManage), routes!(webhooks::rotate)))
        .routes(s.route(Access::Org(R::WebhooksManage), routes!(webhooks::test)))
        .routes(s.route(Access::Org(R::WebhooksView), routes!(webhooks::deliveries)))
        .routes(s.route(Access::Org(R::GuardrailsView), routes!(guardrails::list)))
        .routes(s.route(Access::Org(R::GuardrailsView), routes!(guardrails::get)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::create)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::update)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::delete)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::rotate_secret)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::test)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::try_rules)))
        .routes(s.route(Access::Project(kaeso_core::rights::ProjectRight::ProjectView), routes!(guardrails::list_project)))
        .routes(s.route(Access::Org(R::GuardrailsManage), routes!(guardrails::set_project)))
        .routes(s.route(Access::Org(R::MembersView), routes!(crate::platform::rights::member)))
        .routes(s.route(Access::Org(R::MembersView), routes!(crate::platform::rights::org_grantors)))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/audit-log",
    summary = "The org's audit log, newest first.",
    tag = "orgs",
    params(("org" = OrgId, Path), PageParams, AuditFilter),
    responses((status = 200, body = List<AuditEventView>))
)]
pub async fn audit_list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(page): Query<PageParams>,
    Query(filter): Query<AuditFilter>,
) -> ApiResult<Json<List<AuditEventView>>> {
    audit_log::check_range(&filter)?;
    Ok(Json(
        audit_log::page(&state.pg, Some(ctx.org()?.id()), page.page()?, &filter).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/audit-log/{event}",
    summary = "One event of the org's audit log.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("event" = kaeso_core::id::AuditEventId, Path)),
    responses((status = 200, body = AuditEventView))
)]
pub async fn audit_get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<AuditEventView>> {
    Ok(Json(
        audit_log::one(&state.pg, Some(ctx.org()?.id()), &id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/audit-log/export",
    summary = "The org's audit log as JSON Lines (one event per line, newest first).",
    tag = "orgs",
    params(("org" = OrgId, Path), AuditFilter),
    responses((status = 200, description = "JSON Lines of audit events.", content_type = "application/x-ndjson", body = AuditEventView))
)]
pub async fn audit_export(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(filter): Query<AuditFilter>,
) -> ApiResult<Response> {
    audit_log::check_range(&filter)?;
    let org = ctx.org()?.id();
    ctx.audit(Some(org), Action::AuditLogExport)
        .write(&mut *state.pg.acquire().await?)
        .await?;
    let user = ctx.user_id().ok().map(|u| u.uuid());
    let pg = state.pg.clone();
    let done = async move {
        if let Some(user) = user {
            crate::services::notifications::system::export_finished(
                &state,
                org.uuid(),
                None,
                user,
                kaeso_mail::notices::Export::AuditLog,
            )
            .await;
        }
    };
    audit_log::export(pg, Some(org), filter, done)
}
