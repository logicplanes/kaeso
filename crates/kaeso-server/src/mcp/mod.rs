pub mod cimd;
pub mod oauth;
pub mod server;
pub mod stats;
pub mod tools;
pub mod upstream;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

use crate::state::AppState;

pub fn router(state: &AppState, platform: Router) -> Router<AppState> {
    let limit = state.config.body_limits.platform_api.as_usize();
    Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth::protected_resource),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(oauth::protected_resource),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp/{project}",
            get(oauth::portal_resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth::authorization_server),
        )
        .route("/oauth/register", post(oauth::register))
        .route("/oauth/authorize", get(oauth::authorize))
        .route("/oauth/token", post(oauth::token))
        .route("/oauth/revoke", post(oauth::revoke))
        .route("/oauth/client.json", get(upstream::client_document))
        .route("/oauth/upstream/callback", get(upstream::callback))
        .route(
            "/mcp",
            post(server::post)
                .get(server::not_allowed)
                .delete(server::not_allowed),
        )
        .layer(axum::Extension(server::Platform(platform)))
        .layer(DefaultBodyLimit::max(limit))
}
