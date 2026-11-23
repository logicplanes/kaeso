use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Source {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub name: String,
    pub secret_prefix: String,
    pub secret_last4: String,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub match_members: bool,
    pub content: bool,
    pub disabled_at: Option<OffsetDateTime>,
    pub archived_at: Option<OffsetDateTime>,
    pub first_event_at: Option<OffsetDateTime>,
    pub last_event_at: Option<OffsetDateTime>,
    pub last_error_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct NewSource<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub secret_hash: &'a [u8],
    pub secret_prefix: &'a str,
    pub secret_last4: &'a str,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub match_members: bool,
    pub content: bool,
    pub created_by: Option<Uuid>,
    pub at: OffsetDateTime,
}

pub async fn insert(db: impl PgExecutor<'_>, s: &NewSource<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO sources (id, org_id, project_id, kind, name, secret_hash, secret_prefix, secret_last4, \
           user_id, service_account_id, match_members, content, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)",
        s.id,
        s.org_id,
        s.project_id,
        s.kind,
        s.name,
        s.secret_hash,
        s.secret_prefix,
        s.secret_last4,
        s.user_id,
        s.service_account_id,
        s.match_members,
        s.content,
        s.created_by,
        s.at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list(
    db: impl PgExecutor<'_>,
    project: Uuid,
    include_archived: bool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Source>, StoreError> {
    Ok(sqlx::query_as!(
        Source,
        "SELECT id, org_id, project_id, kind, name, secret_prefix, secret_last4, user_id, service_account_id, \
           match_members, content, disabled_at, archived_at, first_event_at, last_event_at, last_error_at, \
           last_error, created_by, created_at, updated_at \
         FROM sources WHERE project_id = $1 AND ($2 OR archived_at IS NULL) \
           AND ($3::uuid IS NULL OR id < $3) \
         ORDER BY id DESC LIMIT $4",
        project,
        include_archived,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

/// The organization's sources in the projects given, newest first; archived ones are left out.
pub async fn list_org(
    db: impl PgExecutor<'_>,
    org: Uuid,
    projects: &[Uuid],
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Source>, StoreError> {
    Ok(sqlx::query_as!(
        Source,
        "SELECT id, org_id, project_id, kind, name, secret_prefix, secret_last4, user_id, service_account_id, \
           match_members, content, disabled_at, archived_at, first_event_at, last_event_at, last_error_at, \
           last_error, created_by, created_at, updated_at \
         FROM sources WHERE org_id = $1 AND project_id = ANY($2) AND archived_at IS NULL \
           AND ($3::uuid IS NULL OR id < $3) \
         ORDER BY id DESC LIMIT $4",
        org,
        projects,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project: Uuid,
    id: Uuid,
) -> Result<Option<Source>, StoreError> {
    Ok(sqlx::query_as!(
        Source,
        "SELECT id, org_id, project_id, kind, name, secret_prefix, secret_last4, user_id, service_account_id, \
           match_members, content, disabled_at, archived_at, first_event_at, last_event_at, last_error_at, \
           last_error, created_by, created_at, updated_at \
         FROM sources WHERE project_id = $1 AND id = $2",
        project,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    project: Uuid,
    id: Uuid,
) -> Result<Option<Source>, StoreError> {
    Ok(sqlx::query_as!(
        Source,
        "SELECT id, org_id, project_id, kind, name, secret_prefix, secret_last4, user_id, service_account_id, \
           match_members, content, disabled_at, archived_at, first_event_at, last_event_at, last_error_at, \
           last_error, created_by, created_at, updated_at \
         FROM sources WHERE project_id = $1 AND id = $2 FOR UPDATE",
        project,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// Saves what people may change: the name, attribution, content capture and the disabled and
/// archived marks.
pub async fn update(db: impl PgExecutor<'_>, s: &Source) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sources SET name = $3, user_id = $4, service_account_id = $5, match_members = $6, content = $7, \
           disabled_at = $8, archived_at = $9, updated_at = $10 \
         WHERE project_id = $1 AND id = $2",
        s.project_id,
        s.id,
        s.name,
        s.user_id,
        s.service_account_id,
        s.match_members,
        s.content,
        s.disabled_at,
        s.archived_at,
        s.updated_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_secret(
    db: impl PgExecutor<'_>,
    project: Uuid,
    id: Uuid,
    hash: &[u8],
    prefix: &str,
    last4: &str,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sources SET secret_hash = $3, secret_prefix = $4, secret_last4 = $5, updated_at = $6 \
         WHERE project_id = $1 AND id = $2",
        project,
        id,
        hash,
        prefix,
        last4,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// A source with what decides whether it may report now: its project's and organization's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presented {
    pub source: Source,
    pub project_archived: bool,
    pub org_active: bool,
}

pub async fn by_secret(
    db: impl PgExecutor<'_>,
    hash: &[u8],
) -> Result<Option<Presented>, StoreError> {
    let row = sqlx::query!(
        "SELECT s.id, s.org_id, s.project_id, s.kind, s.name, s.secret_prefix, s.secret_last4, s.user_id, \
           s.service_account_id, s.match_members, s.content, s.disabled_at, s.archived_at, s.first_event_at, \
           s.last_event_at, s.last_error_at, s.last_error, s.created_by, s.created_at, s.updated_at, \
           p.archived_at IS NOT NULL AS \"project_archived!\", \
           (o.state = 'active') AS \"org_active!\" \
         FROM sources s JOIN projects p ON p.id = s.project_id JOIN organizations o ON o.id = s.org_id \
         WHERE s.secret_hash = $1",
        hash
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| Presented {
        source: Source {
            id: r.id,
            org_id: r.org_id,
            project_id: r.project_id,
            kind: r.kind,
            name: r.name,
            secret_prefix: r.secret_prefix,
            secret_last4: r.secret_last4,
            user_id: r.user_id,
            service_account_id: r.service_account_id,
            match_members: r.match_members,
            content: r.content,
            disabled_at: r.disabled_at,
            archived_at: r.archived_at,
            first_event_at: r.first_event_at,
            last_event_at: r.last_event_at,
            last_error_at: r.last_error_at,
            last_error: r.last_error,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        },
        project_archived: r.project_archived,
        org_active: r.org_active,
    }))
}

/// Notes that a source reported: the first and last event, or the last refused batch and why.
pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE sources SET \
           first_event_at = CASE WHEN $3::text IS NULL THEN coalesce(first_event_at, $2) ELSE first_event_at END, \
           last_event_at = CASE WHEN $3::text IS NULL THEN greatest(last_event_at, $2) ELSE last_event_at END, \
           last_error_at = CASE WHEN $3::text IS NULL THEN last_error_at ELSE $2 END, \
           last_error = coalesce($3, last_error) \
         WHERE id = $1",
        id,
        at,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    pub batches: i64,
    pub requests: i64,
    pub tool_calls: i64,
    pub duplicates: i64,
    pub refused: i64,
}

pub async fn add_activity(
    db: impl PgExecutor<'_>,
    source: Uuid,
    day: Date,
    a: Activity,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO source_activity AS a (source_id, day, batches, requests, tool_calls, duplicates, refused) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (source_id, day) DO UPDATE SET batches = a.batches + excluded.batches, \
           requests = a.requests + excluded.requests, tool_calls = a.tool_calls + excluded.tool_calls, \
           duplicates = a.duplicates + excluded.duplicates, refused = a.refused + excluded.refused",
        source,
        day,
        a.batches,
        a.requests,
        a.tool_calls,
        a.duplicates,
        a.refused
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Day {
    pub source_id: Uuid,
    pub day: Date,
    pub activity: Activity,
}

/// What the sources reported on each day from `from` on, oldest first.
pub async fn activity(
    db: impl PgExecutor<'_>,
    sources: &[Uuid],
    from: Date,
) -> Result<Vec<Day>, StoreError> {
    let rows = sqlx::query!(
        "SELECT source_id, day, batches, requests, tool_calls, duplicates, refused FROM source_activity \
         WHERE source_id = ANY($1) AND day >= $2 ORDER BY day, source_id",
        sources,
        from
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Day {
            source_id: r.source_id,
            day: r.day,
            activity: Activity {
                batches: r.batches,
                requests: r.requests,
                tool_calls: r.tool_calls,
                duplicates: r.duplicates,
                refused: r.refused,
            },
        })
        .collect())
}
