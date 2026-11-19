use serde_json::Value;
use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq)]
pub struct SharedLink {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
    pub kind: String,
    pub member_id: Option<Uuid>,
    pub title: String,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
    pub live: bool,
    pub period_days: Option<i32>,
    pub from_day: Option<Date>,
    pub to_day: Option<Date>,
    pub snapshot: Option<Value>,
    pub metric: String,
    pub show_costs: bool,
    pub hide_members: bool,
    pub hide_keys: bool,
    pub hide_projects: bool,
    pub audience: String,
    pub views: i64,
    pub last_opened_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct NewLink<'a> {
    pub link: &'a SharedLink,
    pub token_hash: &'a [u8],
}

pub async fn insert(db: impl PgExecutor<'_>, new: &NewLink<'_>) -> Result<(), StoreError> {
    let l = new.link;
    sqlx::query!(
        "INSERT INTO shared_links (id, org_id, project_id, kind, member_id, title, token_hash, \
         created_by, created_at, expires_at, live, period_days, from_day, to_day, snapshot, metric, \
         show_costs, hide_members, hide_keys, hide_projects, audience) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)",
        l.id,
        l.org_id,
        l.project_id,
        l.kind,
        l.member_id,
        l.title,
        new.token_hash,
        l.created_by,
        l.created_at,
        l.expires_at,
        l.live,
        l.period_days,
        l.from_day,
        l.to_day,
        l.snapshot,
        l.metric,
        l.show_costs,
        l.hide_members,
        l.hide_keys,
        l.hide_projects,
        l.audience
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn by_token(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
) -> Result<Option<SharedLink>, StoreError> {
    Ok(sqlx::query_as!(
        SharedLink,
        "SELECT id, org_id, project_id, kind, member_id, title, created_by, created_at, expires_at, \
         revoked_at, live, period_days, from_day, to_day, snapshot, metric, show_costs, hide_members, \
         hide_keys, hide_projects, audience, views, last_opened_at \
         FROM shared_links WHERE token_hash = $1",
        token_hash
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org: Uuid,
    id: Uuid,
) -> Result<Option<SharedLink>, StoreError> {
    Ok(sqlx::query_as!(
        SharedLink,
        "SELECT id, org_id, project_id, kind, member_id, title, created_by, created_at, expires_at, \
         revoked_at, live, period_days, from_day, to_day, snapshot, metric, show_costs, hide_members, \
         hide_keys, hide_projects, audience, views, last_opened_at \
         FROM shared_links WHERE org_id = $1 AND id = $2",
        org,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// The links of an organization that still work, newest first; only `creator`'s when given.
pub async fn list(
    db: impl PgExecutor<'_>,
    org: Uuid,
    creator: Option<Uuid>,
    now: OffsetDateTime,
) -> Result<Vec<SharedLink>, StoreError> {
    Ok(sqlx::query_as!(
        SharedLink,
        "SELECT id, org_id, project_id, kind, member_id, title, created_by, created_at, expires_at, \
         revoked_at, live, period_days, from_day, to_day, snapshot, metric, show_costs, hide_members, \
         hide_keys, hide_projects, audience, views, last_opened_at \
         FROM shared_links WHERE org_id = $1 AND ($2::uuid IS NULL OR created_by = $2) \
         AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $3) \
         ORDER BY created_at DESC, id DESC",
        org,
        creator,
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn revoke(
    db: impl PgExecutor<'_>,
    id: Uuid,
    by: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE shared_links SET revoked_at = $3, revoked_by = $2 WHERE id = $1 AND revoked_at IS NULL",
        id,
        by,
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0)
}

pub async fn opened(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE shared_links SET views = views + 1, last_opened_at = $2 WHERE id = $1",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub allow: String,
    pub max_days: Option<i32>,
}

pub async fn policy(db: impl PgExecutor<'_>, org: Uuid) -> Result<Policy, StoreError> {
    Ok(sqlx::query_as!(
        Policy,
        "SELECT shared_links AS allow, shared_link_max_days AS max_days FROM organizations WHERE id = $1",
        org
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(Policy {
        allow: "off".into(),
        max_days: Some(1),
    }))
}

pub async fn set_policy(
    db: impl PgExecutor<'_>,
    org: Uuid,
    policy: &Policy,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE organizations SET shared_links = $2, shared_link_max_days = $3 WHERE id = $1",
        org,
        policy.allow,
        policy.max_days
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstanceSwitches {
    pub shared_links: bool,
    pub share_mark: bool,
}

pub async fn instance(db: impl PgExecutor<'_>) -> Result<InstanceSwitches, StoreError> {
    Ok(sqlx::query_as!(
        InstanceSwitches,
        "SELECT shared_links, share_mark FROM instance_settings"
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(InstanceSwitches {
        shared_links: true,
        share_mark: true,
    }))
}

pub async fn set_instance(
    db: impl PgExecutor<'_>,
    switches: InstanceSwitches,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET shared_links = $1, share_mark = $2, updated_at = now()",
        switches.shared_links,
        switches.share_mark
    )
    .execute(db)
    .await?;
    Ok(())
}

/// The names a shared link shows: people by their name or address, keys, service accounts and
/// projects by theirs.
pub async fn user_names(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT id, coalesce(name, email, '') AS \"name!\" FROM users WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect())
}

pub async fn key_names(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(
        sqlx::query!("SELECT id, name FROM api_keys WHERE id = ANY($1)", ids)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect(),
    )
}

pub async fn service_account_names(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT id, name FROM service_accounts WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect())
}

pub async fn project_names(
    db: impl PgExecutor<'_>,
    org: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(
        sqlx::query!("SELECT id, name FROM projects WHERE org_id = $1", org)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect(),
    )
}

pub async fn model_names(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    Ok(sqlx::query!(
        "SELECT id, provider || '/' || model_id AS \"name!\" FROM catalogue_models WHERE id = ANY($1)",
        ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect())
}
