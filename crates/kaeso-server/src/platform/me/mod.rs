pub mod account;
pub mod agents;
pub mod keys;
pub mod profile;
pub mod security;
pub mod sessions;
pub mod sso;
pub mod whats_new;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use kaeso_core::rights::AccountRight::*;

use super::access::{Access, Secure};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(Access::Me(AccountView), routes!(profile::get)))
        .routes(s.route(Access::Session, routes!(agents::list)))
        .routes(s.route(Access::Session, routes!(agents::catalogue)))
        .routes(s.route(Access::Session, routes!(agents::grantable)))
        .routes(s.route(Access::Session, routes!(agents::get, agents::update, agents::revoke)))
        .routes(s.route(Access::Session, routes!(agents::set_rights)))
        .routes(s.route(Access::Session, routes!(agents::activity)))
        .routes(s.route(Access::Session, routes!(agents::request)))
        .routes(s.route(Access::Session, routes!(agents::approve)))
        .routes(s.route(Access::Session, routes!(agents::deny)))
        .routes(s.route(Access::Me(AccountUpdate), routes!(profile::update)))
        .routes(s.route(Access::Me(AccountUpdate), routes!(profile::set_avatar, profile::remove_avatar)))
        .routes(s.route(Access::Me(AccountView), routes!(crate::platform::avatars::get)))
        .routes(s.route(Access::Account, routes!(profile::change_email)))
        .routes(s.route(Access::Public, routes!(profile::confirm_email)))
        .routes(s.route(Access::Public, routes!(profile::cancel_email)))
        .routes(s.route(Access::Account, routes!(security::change_password)))
        .routes(s.route(Access::Account, routes!(security::totp_begin)))
        .routes(s.route(Access::Account, routes!(security::totp_confirm)))
        .routes(s.route(Access::Account, routes!(security::totp_disable)))
        .routes(s.route(Access::Account, routes!(security::recovery_codes)))
        .routes(s.route(Access::Account, routes!(account::create_org)))
        .routes(s.route(Access::Me(AccountView), routes!(account::memberships)))
        .routes(s.route(Access::Me(AccountMembershipsLeave), routes!(account::leave)))
        .routes(s.route(Access::Me(AccountView), routes!(account::notifications)))
        .routes(s.route(Access::Me(AccountNotificationsManage), routes!(account::set_notifications)))
        .routes(s.route(Access::Account, routes!(account::delete_account)))
        .routes(s.route(Access::Me(AccountKeysView), routes!(keys::list)))
        .routes(s.route(Access::Me(AccountKeysView), routes!(keys::get)))
        .routes(s.route(Access::Session, routes!(sessions::list)))
        .routes(s.route(Access::Session, routes!(sessions::revoke)))
        .routes(s.route(Access::Session, routes!(sessions::revoke_all)))
        .routes(s.route(Access::Me(AccountKeysManage), routes!(keys::disable)))
        .routes(s.route(Access::Me(AccountKeysManage), routes!(keys::enable)))
        .routes(s.route(Access::Me(AccountKeysManage), routes!(keys::archive)))
        .routes(s.route(Access::Me(AccountView), routes!(whats_new::get)))
        .routes(s.route(Access::Session, routes!(whats_new::seen)))
        .routes(s.route(Access::Session, routes!(sso::list)))
        .routes(s.route(Access::Account, routes!(sso::link)))
        .routes(s.route(Access::Account, routes!(sso::unlink)))
        .routes(s.route(Access::Session, routes!(sso::confirm_ways)))
        .routes(s.route(Access::Session, routes!(sso::confirm)))
}
