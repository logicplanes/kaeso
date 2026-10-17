use std::sync::Arc;

use axum::Router;
use axum::middleware;

use crate::state::AppState;
use crate::{health, http, model_api, platform};

pub fn router(state: AppState) -> Router {
    let (platform, _) = platform::router(&state);
    let origins = state.config.cors.origins.clone();
    let router = Router::new()
        .merge(health::router())
        .merge(model_api::router())
        .merge(platform)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            http::metrics::middleware,
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
