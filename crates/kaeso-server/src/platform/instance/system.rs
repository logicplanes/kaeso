use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;

use crate::platform::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Storage of the install.")]
pub struct InstanceStorageView {
    #[doc = "PostgreSQL database size in bytes."]
    pub postgres_bytes: Option<i64>,
    #[doc = "ClickHouse bytes on disk, all tables of the logs database."]
    pub clickhouse_bytes_on_disk: Option<u64>,
    #[doc = "ClickHouse rows in the request log."]
    pub request_log_rows: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The state of the install.")]
pub struct SystemView {
    #[doc = "The gateway version."]
    #[schema(example = json!(env!("CARGO_PKG_VERSION")))]
    pub version: &'static str,
    #[doc = "`ready`, `degraded` or `not_ready`."]
    pub status: &'static str,
    #[doc = "`ok` or `down`."]
    pub postgres: &'static str,
    #[doc = "`ok` or `down`."]
    pub valkey: &'static str,
    #[doc = "`ok` or `down`."]
    pub clickhouse: &'static str,
    #[doc = "Storage."]
    pub storage: InstanceStorageView,
    #[doc = "Request log rows waiting for ClickHouse on this instance."]
    pub log_buffer: LogBufferView,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The request log buffer of this instance.")]
pub struct LogBufferView {
    #[doc = "Rows waiting."]
    pub rows: usize,
    #[doc = "Rows dropped since start because the buffer was full."]
    pub dropped: u64,
}

#[utoipa::path(
    get,
    path = "/instance/system",
    summary = "Version, component health and storage.",
    tag = "instance",
    responses((status = 200, body = SystemView))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<SystemView>> {
    let health = crate::health::check(&state).await;
    let postgres_bytes = kaeso_store::pg::database_size(&state.pg).await.ok();
    let disk = kaeso_store::clickhouse::logs::disk(&state.clickhouse)
        .await
        .ok();
    Ok(Json(SystemView {
        version: env!("CARGO_PKG_VERSION"),
        status: health.status,
        postgres: health.postgres,
        valkey: health.valkey,
        clickhouse: health.clickhouse,
        storage: InstanceStorageView {
            postgres_bytes,
            clickhouse_bytes_on_disk: disk.as_ref().map(|d| d.bytes),
            request_log_rows: disk.as_ref().map(|d| d.rows),
        },
        log_buffer: LogBufferView {
            rows: state.logs.len(),
            dropped: state.logs.dropped(),
        },
    }))
}
