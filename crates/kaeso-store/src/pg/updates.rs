use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub channel: String,
    pub mode: String,
    pub etag: Option<String>,
    pub last_checked_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub next_check_at: Option<OffsetDateTime>,
    pub blocked_until: Option<OffsetDateTime>,
    pub auto_frequency: String,
    pub auto_weekday: i16,
    pub auto_hour: i16,
    pub auto_window_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    pub channel: String,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub frequency: String,
    pub weekday: i16,
    pub hour: i16,
}

pub async fn seed(db: impl PgExecutor<'_>, defaults: &Choices) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO update_settings (id, channel, mode) \
         VALUES (1, $1, $2) ON CONFLICT (id) DO NOTHING",
        defaults.channel,
        defaults.mode,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn settings(db: impl PgExecutor<'_>) -> Result<Settings, StoreError> {
    sqlx::query_as!(
        Settings,
        "SELECT channel, mode, etag, last_checked_at, last_success_at, last_error, next_check_at, blocked_until, \
         auto_frequency, auto_weekday, auto_hour, auto_window_at \
         FROM update_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| StoreError::Corrupt("the update settings row is missing".into()))
}

pub async fn settings_for_update(db: &mut sqlx::PgConnection) -> Result<Settings, StoreError> {
    sqlx::query_as!(
        Settings,
        "SELECT channel, mode, etag, last_checked_at, last_success_at, last_error, next_check_at, blocked_until, \
         auto_frequency, auto_weekday, auto_hour, auto_window_at \
         FROM update_settings WHERE id = 1 FOR NO KEY UPDATE"
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| StoreError::Corrupt("the update settings row is missing".into()))
}

pub async fn choose(
    db: impl PgExecutor<'_>,
    choices: &Choices,
    window: &Window,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE update_settings SET channel = $1, \
         auto_window_at = CASE WHEN mode <> $2 OR auto_frequency <> $3 OR auto_weekday <> $4 OR auto_hour <> $5 \
         THEN $6 ELSE auto_window_at END, \
         mode = $2, auto_frequency = $3, auto_weekday = $4, auto_hour = $5, updated_at = now() WHERE id = 1",
        choices.channel,
        choices.mode,
        window.frequency,
        window.weekday,
        window.hour,
        at,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn claim_window(
    db: impl PgExecutor<'_>,
    start: OffsetDateTime,
) -> Result<bool, StoreError> {
    let claimed = sqlx::query!(
        "UPDATE update_settings SET auto_window_at = $1 \
         WHERE id = 1 AND (auto_window_at IS NULL OR auto_window_at < $1)",
        start,
    )
    .execute(db)
    .await?;
    Ok(claimed.rows_affected() == 1)
}

pub async fn record_check(
    db: impl PgExecutor<'_>,
    at: OffsetDateTime,
    etag: Option<&str>,
    error: Option<&str>,
    next: OffsetDateTime,
    blocked_until: Option<OffsetDateTime>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE update_settings SET last_checked_at = $1, etag = COALESCE($2, etag), \
         last_error = $3, next_check_at = $4, blocked_until = $5, \
         last_success_at = CASE WHEN $3::text IS NULL THEN $1 ELSE last_success_at END, \
         updated_at = now() WHERE id = 1",
        at,
        etag,
        error,
        next,
        blocked_until,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseRow {
    pub version: String,
    pub published_at: OffsetDateTime,
    pub prerelease: bool,
    pub notes: String,
    pub url: String,
    pub manifest: Option<serde_json::Value>,
    pub manifest_error: Option<String>,
    pub art: Option<serde_json::Value>,
}

pub async fn releases(db: impl PgExecutor<'_>) -> Result<Vec<ReleaseRow>, StoreError> {
    Ok(sqlx::query_as!(
        ReleaseRow,
        "SELECT version, published_at, prerelease, notes, url, manifest, manifest_error, art \
         FROM update_releases ORDER BY published_at DESC, version DESC"
    )
    .fetch_all(db)
    .await?)
}

pub async fn replace_releases(
    tx: &mut sqlx::PgConnection,
    rows: &[ReleaseRow],
    etag: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!("DELETE FROM update_releases")
        .execute(&mut *tx)
        .await?;
    for r in rows {
        sqlx::query!(
            "INSERT INTO update_releases (version, published_at, prerelease, notes, url, manifest, manifest_error, art) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            r.version,
            r.published_at,
            r.prerelease,
            r.notes,
            r.url,
            r.manifest,
            r.manifest_error,
            r.art,
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        "UPDATE update_settings SET etag = $1, updated_at = now() WHERE id = 1",
        etag
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRow {
    pub id: Uuid,
    pub from_version: String,
    pub to_version: String,
    pub state: String,
    pub step: String,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub started_by: Option<Uuid>,
    pub started_by_name: Option<String>,
    pub automatic: bool,
    pub started_at: OffsetDateTime,
    pub last_contact_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct NewJob<'a> {
    pub id: Uuid,
    pub from_version: &'a str,
    pub to_version: &'a str,
    pub started_by: Option<Uuid>,
    pub automatic: bool,
    pub started_at: OffsetDateTime,
}

pub async fn start_job(db: impl PgExecutor<'_>, job: &NewJob<'_>) -> Result<bool, StoreError> {
    let inserted = sqlx::query!(
        "INSERT INTO update_jobs (id, from_version, to_version, state, step, started_by, automatic, started_at) \
         VALUES ($1, $2, $3, 'running', 'verifying', $4, $5, $6)",
        job.id,
        job.from_version,
        job.to_version,
        job.started_by,
        job.automatic,
        job.started_at,
    )
    .execute(db)
    .await;
    match inserted {
        Ok(_) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub async fn job(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<JobRow>, StoreError> {
    Ok(sqlx::query_as!(
        JobRow,
        "SELECT j.id, j.from_version, j.to_version, j.state, j.step, j.reason, j.detail, \
         j.started_by, u.name AS started_by_name, j.automatic, j.started_at, j.last_contact_at, j.finished_at \
         FROM update_jobs j LEFT JOIN users u ON u.id = j.started_by WHERE j.id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn running_job(db: impl PgExecutor<'_>) -> Result<Option<JobRow>, StoreError> {
    Ok(sqlx::query_as!(
        JobRow,
        "SELECT j.id, j.from_version, j.to_version, j.state, j.step, j.reason, j.detail, \
         j.started_by, u.name AS started_by_name, j.automatic, j.started_at, j.last_contact_at, j.finished_at \
         FROM update_jobs j LEFT JOIN users u ON u.id = j.started_by WHERE j.state = 'running'"
    )
    .fetch_optional(db)
    .await?)
}

pub async fn latest_job(db: impl PgExecutor<'_>) -> Result<Option<JobRow>, StoreError> {
    Ok(sqlx::query_as!(
        JobRow,
        "SELECT j.id, j.from_version, j.to_version, j.state, j.step, j.reason, j.detail, \
         j.started_by, u.name AS started_by_name, j.automatic, j.started_at, j.last_contact_at, j.finished_at \
         FROM update_jobs j LEFT JOIN users u ON u.id = j.started_by \
         ORDER BY j.started_at DESC, j.id DESC LIMIT 1"
    )
    .fetch_optional(db)
    .await?)
}

pub async fn jobs(
    db: impl PgExecutor<'_>,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<JobRow>, StoreError> {
    Ok(sqlx::query_as!(
        JobRow,
        "SELECT j.id, j.from_version, j.to_version, j.state, j.step, j.reason, j.detail, \
         j.started_by, u.name AS started_by_name, j.automatic, j.started_at, j.last_contact_at, j.finished_at \
         FROM update_jobs j LEFT JOIN users u ON u.id = j.started_by \
         WHERE $1::uuid IS NULL OR j.id < $1 ORDER BY j.id DESC LIMIT $2",
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn seen(
    db: impl PgExecutor<'_>,
    id: Uuid,
    step: &str,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE update_jobs SET step = $2, last_contact_at = $3 WHERE id = $1 AND state = 'running'",
        id,
        step,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Finish<'a> {
    pub state: &'a str,
    pub step: &'a str,
    pub reason: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub at: OffsetDateTime,
}

pub async fn finish_job(
    db: impl PgExecutor<'_>,
    id: Uuid,
    finish: &Finish<'_>,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "UPDATE update_jobs SET state = $2, step = $3, reason = $4, detail = $5, finished_at = $6 \
         WHERE id = $1 AND state = 'running'",
        id,
        finish.state,
        finish.step,
        finish.reason,
        finish.detail,
        finish.at,
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}
