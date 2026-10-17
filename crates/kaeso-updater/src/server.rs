use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use kaeso_update::policy;
use kaeso_update::protocol::StartRequest;
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::engine::Engine;

const ID_PREFIX: &str = "upd_";
const ID_ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";

#[derive(Clone)]
struct App {
    engine: Arc<Engine>,
    token: Arc<SecretString>,
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

pub fn router(engine: Arc<Engine>, token: SecretString) -> Router {
    let app = App {
        engine,
        token: Arc::new(token),
    };
    let api = Router::new()
        .route("/v1/status", get(status))
        .route("/v1/updates", post(start))
        .route("/v1/updates/{id}", get(job))
        .layer(middleware::from_fn_with_state(app.clone(), authorize));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(api)
        .layer(DefaultBodyLimit::max(4096))
        .with_state(app)
}

pub fn authorized(expected: &SecretString, header: Option<&str>) -> bool {
    let Some(given) = header.and_then(|h| h.strip_prefix("Bearer ")) else {
        return false;
    };
    let expected = expected.expose_secret().as_bytes();
    let given = given.as_bytes();
    expected.len() == given.len() && bool::from(expected.ct_eq(given))
}

async fn authorize(State(app): State<App>, req: Request, next: Next) -> Response {
    let header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if !authorized(&app.token, header) {
        tracing::warn!(
            path = req.uri().path(),
            "refused a request without the updater token"
        );
        return error(
            StatusCode::UNAUTHORIZED,
            "the updater token is missing or wrong",
        );
    }
    next.run(req).await
}

async fn status(State(app): State<App>) -> Response {
    Json(app.engine.status().await).into_response()
}

pub fn valid_id(id: &str) -> bool {
    id.strip_prefix(ID_PREFIX)
        .is_some_and(|rest| rest.len() == 26 && rest.bytes().all(|b| ID_ALPHABET.contains(&b)))
}

async fn start(State(app): State<App>, body: axum::body::Bytes) -> Response {
    let Ok(req) = serde_json::from_slice::<StartRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, "the body is not a start request");
    };
    if !valid_id(&req.id) {
        return error(StatusCode::BAD_REQUEST, "the update id is malformed");
    }
    let Some(version) = policy::parse_tag(&req.version) else {
        return error(StatusCode::BAD_REQUEST, "the version is malformed");
    };
    match app.engine.begin(req.id, version) {
        Ok(job) => (StatusCode::ACCEPTED, Json(job)).into_response(),
        Err(busy) => error(StatusCode::CONFLICT, &busy.to_string()),
    }
}

async fn job(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.engine.job(&id) {
        Some(job) => Json(job).into_response(),
        None => error(StatusCode::NOT_FOUND, "no such update"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_bearer_token_is_authorized() {
        let token = SecretString::from("0123456789abcdef0123456789abcdef");
        assert!(authorized(
            &token,
            Some("Bearer 0123456789abcdef0123456789abcdef")
        ));
        assert!(!authorized(
            &token,
            Some("Bearer 0123456789abcdef0123456789abcdee")
        ));
        assert!(!authorized(&token, Some("Bearer 0123456789abcdef")));
        assert!(!authorized(
            &token,
            Some("0123456789abcdef0123456789abcdef")
        ));
        assert!(!authorized(
            &token,
            Some("Basic 0123456789abcdef0123456789abcdef")
        ));
        assert!(!authorized(&token, None));
    }

    #[test]
    fn update_ids_are_the_gateways_ids() {
        assert!(valid_id("upd_01k7a3f8m2xq4v9c5r6t8w0yz1"));
        assert!(!valid_id("upd_01k7a3f8m2xq4v9c5r6t8w0yz"));
        assert!(!valid_id("upd_01k7a3f8m2xq4v9c5r6t8w0yzi"));
        assert!(!valid_id("usr_01k7a3f8m2xq4v9c5r6t8w0yz1"));
        assert!(!valid_id("upd_../../../etc/passwd000000"));
    }
}
