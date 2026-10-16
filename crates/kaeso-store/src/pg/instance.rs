use sqlx::PgExecutor;
use time::OffsetDateTime;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceSettings {
    pub org_creation: String,
    pub account_creation: String,
    pub allowed_domains: Vec<String>,
    pub metadata_retention_days: i32,
    pub content_retention_days: i32,
    pub max_metadata_retention_days: i32,
    pub max_content_retention_days: i32,
    pub org_deletion: bool,
    pub org_deletion_grace_days: i32,
    pub setup_completed_at: Option<OffsetDateTime>,
}

pub async fn seed(db: &sqlx::PgPool, defaults: &InstanceSettings) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO instance_settings (org_creation, account_creation, allowed_domains, \
         metadata_retention_days, content_retention_days, max_metadata_retention_days, \
         max_content_retention_days, org_deletion, org_deletion_grace_days) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        defaults.org_creation,
        defaults.account_creation,
        &defaults.allowed_domains,
        defaults.metadata_retention_days,
        defaults.content_retention_days,
        defaults.max_metadata_retention_days,
        defaults.max_content_retention_days,
        defaults.org_deletion,
        defaults.org_deletion_grace_days,
    )
    .execute(db)
    .await?;
    sqlx::query!(
        "UPDATE instance_settings SET account_creation = $1, updated_at = now() \
         WHERE account_creation IS NULL",
        defaults.account_creation,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(db: impl PgExecutor<'_>) -> Result<InstanceSettings, StoreError> {
    let row = sqlx::query_as!(
        InstanceSettings,
        r#"SELECT org_creation, COALESCE(account_creation, 'invitation_only') AS "account_creation!",
         allowed_domains, metadata_retention_days, content_retention_days,
         max_metadata_retention_days, max_content_retention_days, org_deletion,
         org_deletion_grace_days, setup_completed_at FROM instance_settings"#
    )
    .fetch_optional(db)
    .await?;
    row.ok_or_else(|| StoreError::Corrupt("the instance settings row is missing".into()))
}

pub async fn update(db: impl PgExecutor<'_>, s: &InstanceSettings) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET org_creation = $1, metadata_retention_days = $2, \
         content_retention_days = $3, max_metadata_retention_days = $4, \
         max_content_retention_days = $5, org_deletion = $6, org_deletion_grace_days = $7, \
         account_creation = $8, allowed_domains = $9, updated_at = now()",
        s.org_creation,
        s.metadata_retention_days,
        s.content_retention_days,
        s.max_metadata_retention_days,
        s.max_content_retention_days,
        s.org_deletion,
        s.org_deletion_grace_days,
        s.account_creation,
        &s.allowed_domains,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn complete_setup(db: impl PgExecutor<'_>, at: OffsetDateTime) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET setup_completed_at = $1, updated_at = now() WHERE setup_completed_at IS NULL",
        at
    )
    .execute(db)
    .await?;
    Ok(())
}
