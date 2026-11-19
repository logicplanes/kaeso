use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

/// How a project follows a model line it added, and where the line points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRow {
    pub catalogue_model_id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub line: String,
    pub target: String,
    pub previous: Option<String>,
    pub since: OffsetDateTime,
    pub policy: String,
    pub delay_days: Option<i32>,
    pub pending: Option<String>,
    pub pending_since: Option<OffsetDateTime>,
    pub kept: Option<String>,
    pub held: Option<String>,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct NewLine<'a> {
    pub catalogue_model_id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub provider: &'a str,
    pub line: &'a str,
    pub target: &'a str,
    pub previous: Option<&'a str>,
    pub policy: &'a str,
    pub delay_days: Option<i32>,
    pub at: OffsetDateTime,
}

/// Records a catalogue model as a line: its state, and the model it calls.
pub async fn insert(db: &mut sqlx::PgConnection, l: &NewLine<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE catalogue_models SET line_target = $2 WHERE id = $1",
        l.catalogue_model_id,
        l.target
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "INSERT INTO catalogue_lines (catalogue_model_id, org_id, project_id, provider, line, previous, since, policy, delay_days, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $7)",
        l.catalogue_model_id,
        l.org_id,
        l.project_id,
        l.provider,
        l.line,
        l.previous,
        l.at,
        l.policy,
        l.delay_days
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn of_project(
    db: impl PgExecutor<'_>,
    project: Uuid,
) -> Result<Vec<LineRow>, StoreError> {
    Ok(sqlx::query_as!(
        LineRow,
        "SELECT l.catalogue_model_id, l.org_id, l.project_id, l.provider, l.line, c.line_target AS \"target!\", \
         l.previous, l.since, l.policy, l.delay_days, l.pending, l.pending_since, l.kept, l.held, l.updated_at \
         FROM catalogue_lines l JOIN catalogue_models c ON c.id = l.catalogue_model_id \
         WHERE l.project_id = $1 ORDER BY l.provider, l.line",
        project
    )
    .fetch_all(db)
    .await?)
}

/// Every line of every project, for the job that moves them.
pub async fn all(db: impl PgExecutor<'_>) -> Result<Vec<LineRow>, StoreError> {
    Ok(sqlx::query_as!(
        LineRow,
        "SELECT l.catalogue_model_id, l.org_id, l.project_id, l.provider, l.line, c.line_target AS \"target!\", \
         l.previous, l.since, l.policy, l.delay_days, l.pending, l.pending_since, l.kept, l.held, l.updated_at \
         FROM catalogue_lines l JOIN catalogue_models c ON c.id = l.catalogue_model_id \
         ORDER BY l.project_id, l.provider, l.line"
    )
    .fetch_all(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    project: Uuid,
    id: Uuid,
) -> Result<Option<LineRow>, StoreError> {
    Ok(sqlx::query_as!(
        LineRow,
        "SELECT l.catalogue_model_id, l.org_id, l.project_id, l.provider, l.line, c.line_target AS \"target!\", \
         l.previous, l.since, l.policy, l.delay_days, l.pending, l.pending_since, l.kept, l.held, l.updated_at \
         FROM catalogue_lines l JOIN catalogue_models c ON c.id = l.catalogue_model_id \
         WHERE l.project_id = $1 AND l.catalogue_model_id = $2 FOR UPDATE OF l, c",
        project,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// Points the line at `target`; `previous` is where it pointed. Clears what waited.
pub async fn move_to(
    db: &mut sqlx::PgConnection,
    id: Uuid,
    target: &str,
    previous: Option<&str>,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE catalogue_models SET line_target = $2, revision = revision + 1, updated_at = $3 WHERE id = $1",
        id,
        target,
        at
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "UPDATE catalogue_lines SET previous = $2, since = $3, pending = NULL, pending_since = NULL, held = NULL, \
         updated_at = $3 WHERE catalogue_model_id = $1",
        id,
        previous,
        at
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

pub async fn set_policy(
    db: impl PgExecutor<'_>,
    id: Uuid,
    policy: &str,
    delay_days: Option<i32>,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE catalogue_lines SET policy = $2, delay_days = $3, updated_at = $4 WHERE catalogue_model_id = $1",
        id,
        policy,
        delay_days,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// What waits for the line: a move to `pending` since `pending_since`, a target an owner chose
/// to keep, and why the line couldn't move.
pub async fn set_waiting(
    db: impl PgExecutor<'_>,
    id: Uuid,
    pending: Option<(&str, OffsetDateTime)>,
    kept: Option<&str>,
    held: Option<&str>,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE catalogue_lines SET pending = $2, pending_since = $3, kept = $4, held = $5, updated_at = $6 \
         WHERE catalogue_model_id = $1",
        id,
        pending.map(|p| p.0),
        pending.map(|p| p.1),
        kept,
        held,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}
