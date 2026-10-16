use std::sync::Arc;

use kaeso_core::catalogue::{self as core, Effective};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, ProjectModels};
use uuid::Uuid;

use crate::state::AppState;

pub async fn project_models(
    state: &AppState,
    project: Uuid,
) -> Result<Arc<ProjectModels>, StoreError> {
    let pg = state.pg.clone();
    let found = state
        .caches
        .project_models
        .get_or_load(&project.to_string(), || async move {
            let mut conn = pg.acquire().await?;
            catalogue::project_models(&mut conn, project)
                .await
                .map(Some)
        })
        .await?;
    Ok(found.unwrap_or_default())
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .project_models
        .invalidate(&project.to_string())
        .await;
}

pub fn effective(state: &AppState, entry: &core::Entry) -> Option<Effective> {
    let loaded = state.models_dev.current();
    core::effective(entry, &loaded.snapshot, state.models_dev.supplement())
}
