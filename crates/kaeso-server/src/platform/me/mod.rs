pub mod account;
pub mod keys;
pub mod profile;
pub mod security;
pub mod sessions;
pub mod whats_new;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::access::{Access, Secure};
use crate::state::AppState;

#[rustfmt::skip]
pub fn routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(Access::Session, routes!(profile::get)))
        .routes(s.route(Access::Session, routes!(profile::update)))
        .routes(s.route(Access::Session, routes!(profile::set_avatar, profile::remove_avatar)))
        .routes(s.route(Access::Session, routes!(crate::platform::avatars::get)))
        .routes(s.route(Access::Session, routes!(profile::change_email)))
        .routes(s.route(Access::Public, routes!(profile::confirm_email)))
        .routes(s.route(Access::Public, routes!(profile::cancel_email)))
        .routes(s.route(Access::Session, routes!(security::change_password)))
        .routes(s.route(Access::Session, routes!(security::totp_begin)))
        .routes(s.route(Access::Session, routes!(security::totp_confirm)))
        .routes(s.route(Access::Session, routes!(security::totp_disable)))
        .routes(s.route(Access::Session, routes!(security::recovery_codes)))
        .routes(s.route(Access::Session, routes!(account::create_org)))
        .routes(s.route(Access::Session, routes!(account::memberships)))
        .routes(s.route(Access::Session, routes!(account::leave)))
        .routes(s.route(Access::Session, routes!(account::notifications)))
        .routes(s.route(Access::Session, routes!(account::set_notifications)))
        .routes(s.route(Access::Session, routes!(account::delete_account)))
        .routes(s.route(Access::Session, routes!(keys::list)))
        .routes(s.route(Access::Session, routes!(keys::get)))
        .routes(s.route(Access::Session, routes!(sessions::list)))
        .routes(s.route(Access::Session, routes!(sessions::revoke)))
        .routes(s.route(Access::Session, routes!(sessions::revoke_all)))
        .routes(s.route(Access::Session, routes!(keys::disable)))
        .routes(s.route(Access::Session, routes!(keys::enable)))
        .routes(s.route(Access::Session, routes!(keys::archive)))
        .routes(s.route(Access::Session, routes!(whats_new::get)))
        .routes(s.route(Access::Session, routes!(whats_new::seen)))
}
