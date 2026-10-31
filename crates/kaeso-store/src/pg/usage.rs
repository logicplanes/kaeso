use rust_decimal::Decimal;
use sqlx::PgExecutor;
use time::Date;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBy {
    Day,
    Week,
    Month,
    Project,
    Model,
    Key,
    Member,
    Route,
}

impl GroupBy {
    fn key(self) -> &'static str {
        match self {
            GroupBy::Day => "to_char(day, 'YYYY-MM-DD')",
            GroupBy::Week => "to_char(date_trunc('week', day), 'YYYY-MM-DD')",
            GroupBy::Month => "to_char(date_trunc('month', day), 'YYYY-MM-DD')",
            GroupBy::Project => "project_id::text",
            GroupBy::Model => "provider || '/' || model_id",
            GroupBy::Key => "coalesce(api_key_id, '00000000-0000-0000-0000-000000000000')::text",
            GroupBy::Member => {
                "CASE WHEN user_id IS NOT NULL THEN 'user:' || user_id::text \
                 ELSE 'service_account:' || coalesce(service_account_id, '00000000-0000-0000-0000-000000000000')::text END"
            }
            GroupBy::Route => "ingress_dialect || ':' || egress_dialect || ':' || provider",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct UsageGroup {
    pub key: String,
    pub requests: i64,
    pub errors: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost: Decimal,
}

pub async fn usage(
    db: impl PgExecutor<'_>,
    org: Uuid,
    project: Option<Uuid>,
    from: Date,
    to: Date,
    group: GroupBy,
) -> Result<Vec<UsageGroup>, StoreError> {
    // The grouping expression comes from the enum above, never from input; the
    // values are bound. "C" orders keys by bytes, as the API always has.
    let sql = format!(
        "SELECT {key} AS key, sum(requests)::bigint AS requests, sum(errors)::bigint AS errors, \
         sum(input_tokens)::bigint AS input_tokens, sum(output_tokens)::bigint AS output_tokens, \
         sum(cache_read_tokens)::bigint AS cache_read_tokens, sum(cache_write_tokens)::bigint AS cache_write_tokens, \
         sum(reasoning_tokens)::bigint AS reasoning_tokens, sum(cost) AS cost \
         FROM usage_daily WHERE org_id = $1 AND ($2::uuid IS NULL OR project_id = $2) AND day >= $3 AND day <= $4 \
         GROUP BY 1 ORDER BY ({key}) COLLATE \"C\"",
        key = group.key()
    );
    Ok(sqlx::query_as::<_, UsageGroup>(sqlx::AssertSqlSafe(sql))
        .bind(org)
        .bind(project)
        .bind(from)
        .bind(to)
        .fetch_all(db)
        .await?)
}

pub async fn span(
    db: impl PgExecutor<'_>,
    org: Uuid,
    project: Option<Uuid>,
) -> Result<Option<(Date, Date)>, StoreError> {
    let r = sqlx::query!(
        "SELECT min(day) AS first, max(day) AS last FROM usage_daily \
         WHERE org_id = $1 AND ($2::uuid IS NULL OR project_id = $2)",
        org,
        project
    )
    .fetch_one(db)
    .await?;
    Ok(r.first.zip(r.last))
}
