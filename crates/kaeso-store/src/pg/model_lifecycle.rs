use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::StoreError;

/// What a project does when a model it uses shuts down. Both are off for a project that never
/// set them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub move_on_shutdown: bool,
    pub serve_replacement: bool,
}

/// The stages of a model's retirement a project hears about, each once per shutdown day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Deprecated,
    Shutdown30,
    Shutdown7,
    Shutdown1,
    Moved,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Deprecated => "deprecated",
            Stage::Shutdown30 => "shutdown_30",
            Stage::Shutdown7 => "shutdown_7",
            Stage::Shutdown1 => "shutdown_1",
            Stage::Moved => "moved",
        }
    }
}

pub async fn settings(db: impl PgExecutor<'_>, project: Uuid) -> Result<Settings, StoreError> {
    Ok(sqlx::query_as!(
        Settings,
        "SELECT move_on_shutdown, serve_replacement FROM project_model_lifecycle WHERE project_id = $1",
        project
    )
    .fetch_optional(db)
    .await?
    .unwrap_or_default())
}

pub async fn set_settings(
    db: impl PgExecutor<'_>,
    project: Uuid,
    s: Settings,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO project_model_lifecycle (project_id, move_on_shutdown, serve_replacement, updated_at) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (project_id) DO UPDATE \
         SET move_on_shutdown = $2, serve_replacement = $3, updated_at = $4",
        project,
        s.move_on_shutdown,
        s.serve_replacement,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Records that a project got a stage's notice for a model and its shutdown day. True when it
/// is new: never sent, or sent for another shutdown day, which a provider can move.
pub async fn claim(
    db: impl PgExecutor<'_>,
    project: Uuid,
    provider: &str,
    model: &str,
    stage: Stage,
    shutdown_on: Option<Date>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query_scalar!(
        "INSERT INTO model_lifecycle_notices (project_id, provider, model_id, stage, shutdown_on, sent_at) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (project_id, provider, model_id, stage) DO UPDATE \
         SET shutdown_on = EXCLUDED.shutdown_on, sent_at = EXCLUDED.sent_at \
         WHERE model_lifecycle_notices.shutdown_on IS DISTINCT FROM EXCLUDED.shutdown_on \
         RETURNING project_id",
        project,
        provider,
        model,
        stage.as_str(),
        shutdown_on,
        at
    )
    .fetch_optional(db)
    .await?
    .is_some())
}
