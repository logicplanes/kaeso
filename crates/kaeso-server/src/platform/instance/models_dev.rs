use axum::Json;
use axum::extract::State;
use kaeso_store::pg::catalogue;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::services::models_dev::{self, SyncError, Synced};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "models.dev status.")]
pub struct ModelsDevStatus {
    #[doc = "Whether the gateway fetches models.dev periodically."]
    pub sync_enabled: bool,
    #[doc = "Where it fetches from."]
    pub url: String,
    #[doc = "The fetch interval in seconds."]
    pub interval_seconds: u64,
    #[doc = "Where the data in use came from (`embedded` for the built-in snapshot)."]
    pub source: String,
    #[doc = "When the data in use was fetched."]
    #[serde(with = "time::serde::rfc3339")]
    pub fetched_at: OffsetDateTime,
    #[doc = "How many models it holds (for the providers Kaeso calls)."]
    pub model_count: usize,
    #[doc = "The last sync attempt."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_attempt_at: Option<OffsetDateTime>,
    #[doc = "Why the last attempt failed, if it did."]
    pub last_error: Option<String>,
}

async fn status_of(state: &AppState) -> ApiResult<ModelsDevStatus> {
    let loaded = state.models_dev.current();
    let row = catalogue::sync_state(&state.pg).await?;
    Ok(ModelsDevStatus {
        sync_enabled: state.config.models_dev.sync,
        url: state.config.models_dev.url.to_string(),
        interval_seconds: state.config.models_dev.interval.as_secs(),
        source: loaded.source.clone(),
        fetched_at: loaded.fetched_at,
        model_count: loaded.snapshot.model_count(),
        last_attempt_at: row.as_ref().and_then(|r| r.last_attempt_at),
        last_error: row.and_then(|r| r.last_error),
    })
}

#[utoipa::path(
    get,
    path = "/instance/models-dev",
    summary = "The models.dev data in use and the last sync.",
    tag = "instance",
    responses((status = 200, body = ModelsDevStatus))
)]
pub async fn status(State(state): State<AppState>, _ctx: Ctx) -> ApiResult<Json<ModelsDevStatus>> {
    Ok(Json(status_of(&state).await?))
}

#[utoipa::path(
    post,
    path = "/instance/models-dev/sync",
    summary = "Fetches models.dev now (even when periodic sync is off).",
    tag = "instance",
    responses((status = 200, body = ModelsDevStatus))
)]
pub async fn sync(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<ModelsDevStatus>> {
    let outcome = models_dev::sync(&state).await.map_err(|e| match e {
        SyncError::Store(e) => ApiError::from(e),
        other => {
            tracing::warn!(error = %other, "models.dev sync failed");
            ApiError::Unavailable(
                "The models.dev data could not be fetched; the gateway log says why.".into(),
            )
        }
    })?;
    let changed = matches!(outcome, Synced::Applied(_));
    ctx.audit(None, Action::ModelsDevSync)
        .after(serde_json::json!({ "changed": changed }))
        .write(&mut *state.pg.acquire().await?)
        .await?;
    Ok(Json(status_of(&state).await?))
}
