use axum::Json;
use axum::extract::State;
use kaeso_store::pg::catalogue;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Storage of the install.")]
pub struct InstanceStorageView {
    #[doc = "PostgreSQL database size in bytes."]
    pub postgres_bytes: Option<i64>,
    #[doc = "ClickHouse bytes on disk, all tables of the logs database; `null`\n\
        when request logs are off or ClickHouse doesn't answer."]
    pub clickhouse_bytes_on_disk: Option<u64>,
    #[doc = "ClickHouse rows in the request log; `null` when request logs are off\n\
        or ClickHouse doesn't answer."]
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
    #[doc = "`ok`, `down`, or `off` when the install keeps no request logs."]
    pub clickhouse: &'static str,
    #[doc = "Storage."]
    pub storage: InstanceStorageView,
    #[doc = "Request log rows waiting for ClickHouse on this instance."]
    pub log_buffer: LogBufferView,
    #[doc = "The model data from KPMR, the Kaeso Provider & Model Registry."]
    pub model_data: ModelDataView,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The model data in use and how the gateway keeps it current.")]
pub struct ModelDataView {
    #[doc = "Whether the gateway fetches the KPMR catalogue by itself."]
    pub sync_enabled: bool,
    #[doc = "Where it fetches the catalogue from."]
    pub url: String,
    #[doc = "Whether the data in use is the snapshot built into this version."]
    pub built_in: bool,
    #[doc = "The KPMR revision of the data in use."]
    pub revision: Option<String>,
    #[doc = "When the data in use was fetched, or generated for the built-in snapshot."]
    #[serde(with = "time::serde::rfc3339")]
    pub fetched_at: OffsetDateTime,
    #[doc = "How many models it holds for the providers Kaeso calls."]
    pub model_count: usize,
    #[doc = "The last time the gateway asked KPMR for changes."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_attempt_at: Option<OffsetDateTime>,
    #[doc = "Why the last attempt failed, if it did."]
    pub last_error: Option<String>,
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
    summary = "Version, component health, storage and model data.",
    tag = "instance",
    responses((status = 200, body = SystemView))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<SystemView>> {
    let health = crate::health::check(&state).await;
    let postgres_bytes = kaeso_store::pg::database_size(&state.pg)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "cannot read the PostgreSQL database size"))
        .ok();
    let disk = match &state.clickhouse {
        Some(ch) => kaeso_store::clickhouse::logs::disk(ch)
            .await
            .inspect_err(|e| tracing::warn!(error = %e, "cannot read the request log's disk usage"))
            .ok(),
        None => None,
    };
    let kpmr = state.kpmr.current();
    let sync = catalogue::sync_state(&state.pg)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "cannot read the KPMR sync state"))
        .ok()
        .flatten();
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
        model_data: ModelDataView {
            sync_enabled: state.config.kpmr.sync,
            url: state.config.kpmr.url.to_string(),
            built_in: kpmr.source == crate::services::kpmr::EMBEDDED_SOURCE,
            revision: kpmr.snapshot.revision.clone(),
            fetched_at: kpmr.fetched_at,
            model_count: kpmr.snapshot.model_count(),
            last_attempt_at: sync.as_ref().and_then(|r| r.last_attempt_at),
            last_error: sync.and_then(|r| r.last_error),
        },
    }))
}
