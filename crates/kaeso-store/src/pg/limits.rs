use kaeso_core::limits::{Kind, Limit, Period, Subject};
use rust_decimal::Decimal;
use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct LimitRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub level: String,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub kind: String,
    pub catalogue_model_id: Option<Uuid>,
    pub period: String,
    pub amount: Decimal,
    pub hard: bool,
    pub alert_thresholds: Vec<i16>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl LimitRow {
    pub fn limit(&self) -> Result<Limit, StoreError> {
        let corrupt = |what: &str| StoreError::Corrupt(format!("limit {}: {what}", self.id));
        let subject = match (
            self.level.as_str(),
            self.user_id,
            self.service_account_id,
            self.api_key_id,
        ) {
            ("project", None, None, None) => Subject::Project(self.project_id),
            ("member", Some(u), None, None) => Subject::User(u),
            ("member", None, Some(s), None) => Subject::ServiceAccount(s),
            ("key", None, None, Some(k)) => Subject::Key(k),
            _ => return Err(corrupt("subject")),
        };
        Ok(Limit {
            id: self.id,
            subject,
            kind: Kind::parse(&self.kind).ok_or_else(|| corrupt("kind"))?,
            model: self.catalogue_model_id,
            period: Period::parse(&self.period).ok_or_else(|| corrupt("period"))?,
            amount: self.amount,
            hard: self.hard,
            thresholds: self
                .alert_thresholds
                .iter()
                .filter_map(|t| u8::try_from(*t).ok())
                .collect(),
        })
    }
}

fn subject_columns(s: Subject) -> (&'static str, Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match s {
        Subject::Project(_) => ("project", None, None, None),
        Subject::User(u) => ("member", Some(u), None, None),
        Subject::ServiceAccount(a) => ("member", None, Some(a), None),
        Subject::Key(k) => ("key", None, None, Some(k)),
    }
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Uuid,
    l: &Limit,
) -> Result<bool, StoreError> {
    let (level, user, account, key) = subject_columns(l.subject);
    let thresholds: Vec<i16> = l.thresholds.iter().map(|t| i16::from(*t)).collect();
    let r = sqlx::query!(
        "INSERT INTO limits (id, org_id, project_id, level, user_id, service_account_id, api_key_id, kind, \
         catalogue_model_id, period, amount, hard, alert_thresholds) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) ON CONFLICT DO NOTHING",
        l.id,
        org_id,
        project_id,
        level,
        user,
        account,
        key,
        l.kind.as_str(),
        l.model,
        l.period.as_str(),
        l.amount,
        l.hard,
        &thresholds
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn update(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    l: &Limit,
) -> Result<bool, StoreError> {
    let thresholds: Vec<i16> = l.thresholds.iter().map(|t| i16::from(*t)).collect();
    Ok(sqlx::query!(
        "UPDATE limits SET amount = $3, hard = $4, alert_thresholds = $5, updated_at = now() \
         WHERE project_id = $1 AND id = $2",
        project_id,
        l.id,
        l.amount,
        l.hard,
        &thresholds
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn delete(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM limits WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn delete_of_user(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    project_id: Option<Uuid>,
    user_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    let mut projects = sqlx::query_scalar!(
        "DELETE FROM limits WHERE org_id = $1 AND ($2::uuid IS NULL OR project_id = $2) AND user_id = $3 \
         RETURNING project_id",
        org_id,
        project_id,
        user_id
    )
    .fetch_all(db)
    .await?;
    projects.sort_unstable();
    projects.dedup();
    Ok(projects)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<LimitRow>, StoreError> {
    Ok(sqlx::query_as!(
        LimitRow,
        "SELECT id, org_id, project_id, level, user_id, service_account_id, api_key_id, kind, catalogue_model_id, \
         period, amount, hard, alert_thresholds, created_at, updated_at FROM limits WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: impl PgExecutor<'_>, project_id: Uuid) -> Result<Vec<LimitRow>, StoreError> {
    Ok(sqlx::query_as!(
        LimitRow,
        "SELECT id, org_id, project_id, level, user_id, service_account_id, api_key_id, kind, catalogue_model_id, \
         period, amount, hard, alert_thresholds, created_at, updated_at FROM limits WHERE project_id = $1 \
         ORDER BY level, id",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn list_org(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<LimitRow>, StoreError> {
    Ok(sqlx::query_as!(
        LimitRow,
        "SELECT id, org_id, project_id, level, user_id, service_account_id, api_key_id, kind, catalogue_model_id, \
         period, amount, hard, alert_thresholds, created_at, updated_at FROM limits WHERE org_id = $1 \
         ORDER BY id",
        org_id
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageRow {
    pub day: Date,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub catalogue_model_id: Option<Uuid>,
    pub model_id: String,
    pub provider: String,
    pub requests: i64,
    pub errors: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost: Decimal,
}

pub async fn add_usage(db: impl PgExecutor<'_>, rows: &[UsageRow]) -> Result<(), StoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    macro_rules! col {
        ($f:ident) => {
            rows.iter().map(|r| r.$f.clone()).collect::<Vec<_>>()
        };
    }
    sqlx::query!(
        "INSERT INTO usage_daily AS u (day, org_id, project_id, user_id, service_account_id, api_key_id, \
           catalogue_model_id, model_id, provider, requests, errors, input_tokens, output_tokens, cache_read_tokens, \
           cache_write_tokens, reasoning_tokens, cost) \
         SELECT * FROM unnest($1::date[], $2::uuid[], $3::uuid[], $4::uuid[], $5::uuid[], $6::uuid[], $7::uuid[], \
           $8::text[], $9::text[], $10::bigint[], $11::bigint[], $12::bigint[], $13::bigint[], $14::bigint[], \
           $15::bigint[], $16::bigint[], $17::numeric[]) \
         ON CONFLICT (project_id, day, api_key_id, user_id, service_account_id, provider, model_id) DO UPDATE SET \
           requests = u.requests + excluded.requests, errors = u.errors + excluded.errors, \
           input_tokens = u.input_tokens + excluded.input_tokens, \
           output_tokens = u.output_tokens + excluded.output_tokens, \
           cache_read_tokens = u.cache_read_tokens + excluded.cache_read_tokens, \
           cache_write_tokens = u.cache_write_tokens + excluded.cache_write_tokens, \
           reasoning_tokens = u.reasoning_tokens + excluded.reasoning_tokens, cost = u.cost + excluded.cost, \
           catalogue_model_id = coalesce(excluded.catalogue_model_id, u.catalogue_model_id)",
        &col!(day),
        &col!(org_id),
        &col!(project_id),
        &col!(user_id) as &[Option<Uuid>],
        &col!(service_account_id) as &[Option<Uuid>],
        &col!(api_key_id) as &[Option<Uuid>],
        &col!(catalogue_model_id) as &[Option<Uuid>],
        &col!(model_id),
        &col!(provider),
        &col!(requests),
        &col!(errors),
        &col!(input_tokens),
        &col!(output_tokens),
        &col!(cache_read_tokens),
        &col!(cache_write_tokens),
        &col!(reasoning_tokens),
        &col!(cost)
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Used {
    pub tokens: i64,
    pub cost: Decimal,
    pub succeeded: i64,
}

pub async fn used(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    subject: Subject,
    model: Option<Uuid>,
    since: Option<Date>,
) -> Result<Used, StoreError> {
    let (level, user, account, key) = subject_columns(subject);
    let r = sqlx::query!(
        "SELECT coalesce(sum(input_tokens + output_tokens + cache_read_tokens + cache_write_tokens), 0)::bigint AS \"tokens!\", \
           coalesce(sum(cost), 0) AS \"cost!\", coalesce(sum(requests - errors), 0)::bigint AS \"succeeded!\" FROM usage_daily WHERE project_id = $1 \
           AND ($2 <> 'member' OR user_id IS NOT DISTINCT FROM $3 AND service_account_id IS NOT DISTINCT FROM $4) \
           AND ($5::uuid IS NULL OR api_key_id = $5) AND ($6::uuid IS NULL OR catalogue_model_id = $6) \
           AND ($7::date IS NULL OR day >= $7)",
        project_id,
        level,
        user,
        account,
        key,
        model,
        since
    )
    .fetch_one(db)
    .await?;
    Ok(Used {
        tokens: r.tokens,
        cost: r.cost,
        succeeded: r.succeeded,
    })
}

pub async fn mark_alert(
    db: impl PgExecutor<'_>,
    limit_id: Uuid,
    period_start: Date,
    threshold: u8,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "INSERT INTO budget_alerts_sent (limit_id, period_start, threshold) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        limit_id,
        period_start,
        i16::from(threshold)
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn alert_recipients(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT DISTINCT u.id, u.email AS \"email!\" FROM users u \
         WHERE u.state = 'active' AND u.notify_budget_alerts AND u.email IS NOT NULL AND ( \
           u.id IN (SELECT user_id FROM project_memberships WHERE project_id = $1 AND preset = 'admin') \
           OR u.id IN (SELECT m.user_id FROM org_memberships m JOIN projects p ON p.org_id = m.org_id \
                       WHERE p.id = $1 AND m.preset IN ('owner', 'admin')))",
        project_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.email)).collect())
}
