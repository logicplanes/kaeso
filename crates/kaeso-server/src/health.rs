use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use serde::Serialize;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Debug, Serialize)]
pub struct Readiness {
    pub status: &'static str,
    pub postgres: &'static str,
    pub valkey: &'static str,
    pub clickhouse: &'static str,
}

pub async fn check(state: &AppState) -> Readiness {
    let timeout = Duration::from_secs(2);
    let (pg, valkey, clickhouse) = tokio::join!(
        tokio::time::timeout(timeout, kaeso_store::pg::ping(&state.pg)),
        tokio::time::timeout(timeout, state.valkey.ping()),
        tokio::time::timeout(timeout, state.clickhouse.ping()),
    );
    let up = |r: Result<Result<(), kaeso_store::StoreError>, _>| matches!(r, Ok(Ok(())));
    let (pg, valkey, clickhouse) = (
        up(pg),
        up(valkey),
        up(clickhouse) && state.clickhouse_migrated(),
    );
    let word = |ok: bool| if ok { "ok" } else { "down" };
    Readiness {
        status: match (pg && valkey, clickhouse) {
            (false, _) => "not_ready",
            (true, false) => "degraded",
            (true, true) => "ready",
        },
        postgres: word(pg),
        valkey: word(valkey),
        clickhouse: word(clickhouse),
    }
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    let r = check(&state).await;
    let status = if r.status == "not_ready" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (status, Json(r))
}
