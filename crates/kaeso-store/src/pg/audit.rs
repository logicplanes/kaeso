use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct NewEvent<'a> {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub actor_kind: &'a str,
    pub actor_id: Option<Uuid>,
    pub action: &'a str,
    pub target_kind: Option<&'a str>,
    pub target_id: Option<&'a str>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub ip: Option<std::net::IpAddr>,
    pub user_agent: Option<&'a str>,
    pub request_id: Option<Uuid>,
    pub at: OffsetDateTime,
}

pub async fn insert(db: impl PgExecutor<'_>, e: &NewEvent<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO audit_events (id, scope, org_id, actor_kind, actor_id, action, target_kind, target_id, \
         before, after, ip, user_agent, request_id, at, project_id) \
         VALUES ($1, CASE WHEN $2::uuid IS NULL THEN 'instance' ELSE 'org' END, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        e.id,
        e.org_id,
        e.actor_kind,
        e.actor_id,
        e.action,
        e.target_kind,
        e.target_id,
        e.before,
        e.after,
        e.ip.map(ipnet::IpNet::from),
        e.user_agent,
        e.request_id,
        e.at,
        e.project_id,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Event {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub actor_kind: String,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub ip: Option<ipnet::IpNet>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>,
    pub at: OffsetDateTime,
}

#[derive(Clone, Debug, Default)]
pub struct Filter<'a> {
    pub actor_id: Option<Uuid>,
    pub action_prefix: Option<&'a str>,
    pub target_id: Option<&'a str>,
    pub project_id: Option<Uuid>,
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    before: Option<Uuid>,
    limit: i64,
    f: &Filter<'_>,
) -> Result<Vec<Event>, StoreError> {
    let prefix = f.action_prefix.map(|p| {
        format!(
            "{}%",
            p.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    Ok(sqlx::query_as!(
        Event,
        "SELECT id, org_id, project_id, actor_kind, actor_id, action, target_kind, target_id, before, after, ip, \
         user_agent, request_id, at FROM audit_events \
         WHERE (($1::uuid IS NULL AND scope = 'instance') OR (scope = 'org' AND org_id = $1)) \
         AND ($2::uuid IS NULL OR id < $2) \
         AND ($4::uuid IS NULL OR actor_id = $4) \
         AND ($5::text IS NULL OR action LIKE $5) \
         AND ($6::text IS NULL OR target_id = $6) \
         AND ($7::timestamptz IS NULL OR at >= $7) \
         AND ($8::timestamptz IS NULL OR at < $8) \
         AND ($9::uuid IS NULL OR project_id = $9) \
         ORDER BY id DESC LIMIT $3",
        org_id,
        before,
        limit,
        f.actor_id,
        prefix,
        f.target_id,
        f.from,
        f.to,
        f.project_id,
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Option<Uuid>,
    id: Uuid,
) -> Result<Option<Event>, StoreError> {
    Ok(sqlx::query_as!(
        Event,
        "SELECT id, org_id, project_id, actor_kind, actor_id, action, target_kind, target_id, before, after, ip, \
         user_agent, request_id, at FROM audit_events \
         WHERE id = $2 AND (($1::uuid IS NULL AND scope = 'instance') OR (scope = 'org' AND org_id = $1))",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}
