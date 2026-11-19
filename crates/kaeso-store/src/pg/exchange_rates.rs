use rust_decimal::Decimal;
use sqlx::PgExecutor;
use time::{Date, OffsetDateTime};

use crate::StoreError;

pub async fn all(db: impl PgExecutor<'_>) -> Result<Vec<(Date, String, Decimal)>, StoreError> {
    Ok(
        sqlx::query!("SELECT day, currency, per_eur FROM exchange_rates ORDER BY day, currency")
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| (r.day, r.currency, r.per_eur))
            .collect(),
    )
}

// A day already stored keeps its first rates: the ECB never revises a published day.
pub async fn insert_day(
    db: impl PgExecutor<'_>,
    day: Date,
    rates: &[(String, Decimal)],
) -> Result<u64, StoreError> {
    let (codes, values): (Vec<String>, Vec<Decimal>) = rates.iter().cloned().unzip();
    Ok(sqlx::query!(
        "INSERT INTO exchange_rates (currency, day, per_eur) \
         SELECT c, $1, r FROM unnest($2::text[], $3::numeric[]) AS t(c, r) \
         ON CONFLICT (currency, day) DO NOTHING",
        day,
        &codes,
        &values
    )
    .execute(db)
    .await?
    .rows_affected())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub enabled: bool,
    pub checked_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}

pub async fn status(db: impl PgExecutor<'_>) -> Result<Status, StoreError> {
    Ok(sqlx::query_as!(
        Status,
        "SELECT exchange_rates AS enabled, exchange_rates_checked_at AS checked_at, \
         exchange_rates_error AS error FROM instance_settings"
    )
    .fetch_optional(db)
    .await?
    .unwrap_or(Status {
        enabled: true,
        checked_at: None,
        error: None,
    }))
}

pub async fn set_enabled(db: impl PgExecutor<'_>, enabled: bool) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET exchange_rates = $1, updated_at = now()",
        enabled
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn checked(
    db: impl PgExecutor<'_>,
    at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET exchange_rates_checked_at = $1, exchange_rates_error = $2",
        at,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}
