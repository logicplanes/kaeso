use std::sync::Arc;

use axum::Router;
use axum::middleware;

use crate::state::AppState;
use crate::{health, http, mcp, model_api, platform, portal};

pub fn router(state: AppState) -> Router {
    let (platform, _) = platform::router(&state);
    let in_process = platform.clone().with_state(state.clone());
    let origins = state.config.cors.origins.clone();
    let router = Router::new().merge(health::router()).merge(platform);
    // A read-only demo has no keys anyone could use, and agents could change it through MCP.
    let router = if state.config.auth.demo_user().is_some() {
        router
    } else {
        router
            .merge(model_api::router())
            .merge(mcp::router(&state, in_process))
            .merge(portal::router(&state))
    };
    let router = router
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            http::metrics::middleware,
        ))
        .layer(middleware::from_fn_with_state(
            http::body_timeout::BodyWaits {
                between_frames: state.config.server.body_read_timeout,
                whole_body: state.config.server.body_read_deadline,
            },
            http::body_timeout::middleware,
        ))
        .with_state(state)
        .layer(middleware::from_fn(http::request_id::middleware));
    if origins.is_empty() {
        return router;
    }
    router.layer(middleware::from_fn_with_state(
        Arc::new(origins),
        http::cors::middleware,
    ))
}
