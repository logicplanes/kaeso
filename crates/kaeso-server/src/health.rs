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
    let clickhouse = async {
        match &state.clickhouse {
            Some(ch) => Some(tokio::time::timeout(timeout, ch.ping()).await),
            None => None,
        }
    };
    let (pg, valkey, clickhouse) = tokio::join!(
        tokio::time::timeout(timeout, kaeso_store::pg::ping(&state.pg)),
        tokio::time::timeout(timeout, state.valkey.ping()),
        clickhouse,
    );
    let up = |r: Result<Result<(), kaeso_store::StoreError>, _>| matches!(r, Ok(Ok(())));
    let (pg, valkey) = (up(pg), up(valkey));
    let clickhouse = clickhouse.map(|r| up(r) && state.clickhouse_migrated());
    let word = |ok: bool| if ok { "ok" } else { "down" };
    Readiness {
        status: match (pg && valkey, clickhouse) {
            (false, _) => "not_ready",
            (true, Some(false)) => "degraded",
            (true, Some(true) | None) => "ready",
        },
        postgres: word(pg),
        valkey: word(valkey),
        clickhouse: clickhouse.map_or("off", word),
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
