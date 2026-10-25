use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyAuth {
    pub id: Uuid,
    pub secret_hash: Vec<u8>,
    pub org_id: Uuid,
    pub org_state: String,
    pub project_id: Uuid,
    #[serde(with = "time::serde::rfc3339::option")]
    pub project_archived_at: Option<OffsetDateTime>,
    pub user_id: Option<Uuid>,
    pub user_state: Option<String>,
    pub service_account_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub service_account_archived_at: Option<OffsetDateTime>,
    pub inference: bool,
    pub platform: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub disabled_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    pub org_preset: Option<String>,
    pub org_role_id: Option<Uuid>,
    pub org_role_rights: Option<Vec<String>>,
    pub org_role_project_rights: Option<Vec<String>>,
    pub org_rights_added: Vec<String>,
    pub org_rights_removed: Vec<String>,
    pub project_preset: Option<String>,
    pub project_role_id: Option<Uuid>,
    pub project_role_rights: Option<Vec<String>>,
    pub project_rights_added: Vec<String>,
    pub project_rights_removed: Vec<String>,
}

pub async fn auth_by_hash(
    db: impl PgExecutor<'_>,
    hash: &[u8],
) -> Result<Option<KeyAuth>, StoreError> {
    Ok(sqlx::query_as!(
        KeyAuth,
        "SELECT k.id, k.secret_hash, k.org_id, o.state AS org_state, k.project_id, p.archived_at AS project_archived_at, \
         k.user_id, u.state AS \"user_state?\", k.service_account_id, s.archived_at AS \"service_account_archived_at?\", \
         k.inference, k.platform, k.expires_at, k.disabled_at, k.archived_at, k.last_used_at, \
         om.preset AS \"org_preset?\", COALESCE(om.rights_added, '{}') AS \"org_rights_added!\", \
         COALESCE(om.rights_removed, '{}') AS \"org_rights_removed!\", \
         om.role_id AS \"org_role_id?\", orr.rights AS \"org_role_rights?\", orr.project_rights AS \"org_role_project_rights?\", \
         COALESCE(pm.preset, s.preset) AS \"project_preset?\", \
         COALESCE(pm.role_id, s.role_id) AS \"project_role_id?\", prr.rights AS \"project_role_rights?\", \
         COALESCE(pm.rights_added, s.rights_added, '{}') AS \"project_rights_added!\", \
         COALESCE(pm.rights_removed, s.rights_removed, '{}') AS \"project_rights_removed!\" \
         FROM api_keys k JOIN organizations o ON o.id = k.org_id JOIN projects p ON p.id = k.project_id \
         LEFT JOIN users u ON u.id = k.user_id LEFT JOIN service_accounts s ON s.id = k.service_account_id \
         LEFT JOIN org_memberships om ON om.org_id = k.org_id AND om.user_id = k.user_id \
         LEFT JOIN project_memberships pm ON pm.project_id = k.project_id AND pm.user_id = k.user_id \
         LEFT JOIN org_roles orr ON orr.id = om.role_id \
         LEFT JOIN org_roles prr ON prr.id = COALESCE(pm.role_id, s.role_id) \
         WHERE k.secret_hash = $1",
        hash
    )
    .fetch_optional(db)
    .await?)
}

pub async fn hashes_of(db: impl PgExecutor<'_>, ids: &[Uuid]) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT secret_hash FROM api_keys WHERE id = ANY($1)", ids)
            .fetch_all(db)
            .await?,
    )
}

pub async fn ids_of_service_account(
    db: impl PgExecutor<'_>,
    service_account_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM api_keys WHERE service_account_id = $1",
        service_account_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn hashes_of_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT secret_hash AS \"h!\" FROM api_keys WHERE org_id = $1 \
         UNION ALL SELECT secret_hash FROM org_platform_keys WHERE org_id = $1",
        org_id
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub name: String,
    pub secret_prefix: String,
    pub secret_last4: String,
    pub inference: bool,
    pub platform: bool,
    pub expires_at: Option<OffsetDateTime>,
    pub disabled_at: Option<OffsetDateTime>,
    pub archived_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub rotated_from: Option<Uuid>,
    pub replaced_by: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct NewKey<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub name: &'a str,
    pub secret_hash: &'a [u8],
    pub secret_prefix: &'a str,
    pub secret_last4: &'a str,
    pub inference: bool,
    pub platform: bool,
    pub expires_at: Option<OffsetDateTime>,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
    pub rotated_from: Option<Uuid>,
}

pub async fn insert(db: impl PgExecutor<'_>, k: &NewKey<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO api_keys (id, org_id, project_id, user_id, service_account_id, name, secret_hash, secret_prefix, \
         secret_last4, inference, platform, expires_at, created_by_kind, created_by_id, rotated_from) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        k.id,
        k.org_id,
        k.project_id,
        k.user_id,
        k.service_account_id,
        k.name,
        k.secret_hash,
        k.secret_prefix,
        k.secret_last4,
        k.inference,
        k.platform,
        k.expires_at,
        k.created_by_kind,
        k.created_by_id,
        k.rotated_from
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<KeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        KeyRow,
        "SELECT id, org_id, project_id, user_id, service_account_id, name, secret_prefix, secret_last4, inference, \
         platform, expires_at, disabled_at, archived_at, last_used_at, created_by_kind, created_by_id, created_at, rotated_from, replaced_by \
         FROM api_keys WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<KeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        KeyRow,
        "SELECT id, org_id, project_id, user_id, service_account_id, name, secret_prefix, secret_last4, inference, \
         platform, expires_at, disabled_at, archived_at, last_used_at, created_by_kind, created_by_id, created_at, rotated_from, replaced_by \
         FROM api_keys WHERE user_id = $1 AND id = $2",
        user_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyFilter {
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub include_archived: bool,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    f: KeyFilter,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<KeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        KeyRow,
        "SELECT id, org_id, project_id, user_id, service_account_id, name, secret_prefix, secret_last4, inference, \
         platform, expires_at, disabled_at, archived_at, last_used_at, created_by_kind, created_by_id, created_at, rotated_from, replaced_by \
         FROM api_keys WHERE project_id = $1 AND ($2::uuid IS NULL OR user_id = $2) \
         AND ($3::uuid IS NULL OR service_account_id = $3) AND ($4 OR archived_at IS NULL) \
         AND ($5::uuid IS NULL OR id > $5) ORDER BY id LIMIT $6",
        project_id,
        f.user_id,
        f.service_account_id,
        f.include_archived,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn list_of_user(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    orgs: &[Uuid],
    projects: Option<&[Uuid]>,
    include_archived: bool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<KeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        KeyRow,
        "SELECT id, org_id, project_id, user_id, service_account_id, name, secret_prefix, secret_last4, inference, \
         platform, expires_at, disabled_at, archived_at, last_used_at, created_by_kind, created_by_id, created_at, rotated_from, replaced_by \
         FROM api_keys WHERE user_id = $1 AND org_id = ANY($5) AND ($6::uuid[] IS NULL OR project_id = ANY($6)) \
         AND ($2 OR archived_at IS NULL) AND ($3::uuid IS NULL OR id > $3) ORDER BY id LIMIT $4",
        user_id,
        include_archived,
        after,
        limit,
        orgs,
        projects
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actor<'a> {
    pub kind: &'a str,
    pub id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disabler {
    pub kind: Option<String>,
    pub id: Option<Uuid>,
}

pub async fn set_disabled(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    at: Option<OffsetDateTime>,
    by: Actor<'_>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE api_keys SET disabled_at = $3, disabled_reason = CASE WHEN $3::timestamptz IS NULL THEN NULL ELSE 'manual' END, \
         disabled_by_kind = CASE WHEN $3::timestamptz IS NULL THEN NULL ELSE $4 END, \
         disabled_by_id = CASE WHEN $3::timestamptz IS NULL THEN NULL ELSE $5::uuid END, \
         updated_at = now() WHERE project_id = $1 AND id = $2 AND archived_at IS NULL",
        project_id,
        id,
        at,
        by.kind,
        by.id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn disabled_by(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<Disabler>, StoreError> {
    Ok(sqlx::query_as!(
        Disabler,
        "SELECT disabled_by_kind AS kind, disabled_by_id AS id FROM api_keys WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn archive(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE api_keys SET archived_at = $3, updated_at = now() \
         WHERE project_id = $1 AND id = $2 AND archived_at IS NULL",
        project_id,
        id,
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn retire(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    replaced_by: Uuid,
    stops_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE api_keys SET replaced_by = $3, expires_at = LEAST(expires_at, $4), updated_at = now() \
         WHERE project_id = $1 AND id = $2 AND archived_at IS NULL AND replaced_by IS NULL",
        project_id,
        id,
        replaced_by,
        stops_at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE api_keys SET last_used_at = $2 WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $2::timestamptz - interval '1 minute')",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Expiring {
    pub id: Uuid,
    pub name: String,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub user_id: Option<Uuid>,
    pub expires_at: OffsetDateTime,
}

pub async fn expiring(
    db: impl PgExecutor<'_>,
    now: OffsetDateTime,
    days: i32,
) -> Result<Vec<Expiring>, StoreError> {
    Ok(sqlx::query_as!(
        Expiring,
        "SELECT k.id, k.name, k.org_id, k.project_id, p.name AS project_name, k.user_id, k.expires_at AS \"expires_at!\" \
         FROM api_keys k JOIN projects p ON p.id = k.project_id \
         WHERE k.archived_at IS NULL AND k.disabled_at IS NULL AND k.replaced_by IS NULL AND k.expires_at > $1 \
         AND k.expires_at <= $1 + make_interval(days => $2) \
         AND NOT EXISTS (SELECT 1 FROM key_expiry_warnings w WHERE w.key_id = k.id AND w.days_before = $2)",
        now,
        days
    )
    .fetch_all(db)
    .await?)
}

pub async fn claim_warnings(
    db: impl PgExecutor<'_>,
    ids: &[Uuid],
    days: i32,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "INSERT INTO key_expiry_warnings (key_id, days_before) SELECT unnest($1::uuid[]), $2 \
         ON CONFLICT DO NOTHING RETURNING key_id",
        ids,
        days
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgKeyAuth {
    pub id: Uuid,
    pub secret_hash: Vec<u8>,
    pub org_id: Uuid,
    pub org_state: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub disabled_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

pub async fn org_auth_by_hash(
    db: impl PgExecutor<'_>,
    hash: &[u8],
) -> Result<Option<OrgKeyAuth>, StoreError> {
    Ok(sqlx::query_as!(
        OrgKeyAuth,
        "SELECT k.id, k.secret_hash, k.org_id, o.state AS org_state, k.expires_at, k.disabled_at, k.archived_at, k.last_used_at \
         FROM org_platform_keys k JOIN organizations o ON o.id = k.org_id WHERE k.secret_hash = $1",
        hash
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrgKeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub secret_prefix: String,
    pub secret_last4: String,
    pub expires_at: Option<OffsetDateTime>,
    pub disabled_at: Option<OffsetDateTime>,
    pub archived_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub rotated_from: Option<Uuid>,
    pub replaced_by: Option<Uuid>,
}

pub struct NewOrgKey<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: &'a str,
    pub secret_hash: &'a [u8],
    pub secret_prefix: &'a str,
    pub secret_last4: &'a str,
    pub expires_at: Option<OffsetDateTime>,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
    pub rotated_from: Option<Uuid>,
}

pub async fn insert_org(db: impl PgExecutor<'_>, k: &NewOrgKey<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO org_platform_keys (id, org_id, name, secret_hash, secret_prefix, secret_last4, expires_at, created_by_kind, \
         created_by_id, rotated_from) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        k.id,
        k.org_id,
        k.name,
        k.secret_hash,
        k.secret_prefix,
        k.secret_last4,
        k.expires_at,
        k.created_by_kind,
        k.created_by_id,
        k.rotated_from
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<OrgKeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        OrgKeyRow,
        "SELECT id, org_id, name, secret_prefix, secret_last4, expires_at, disabled_at, archived_at, last_used_at, \
         created_by_kind, created_by_id, created_at, rotated_from, replaced_by FROM org_platform_keys WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    include_archived: bool,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<OrgKeyRow>, StoreError> {
    Ok(sqlx::query_as!(
        OrgKeyRow,
        "SELECT id, org_id, name, secret_prefix, secret_last4, expires_at, disabled_at, archived_at, last_used_at, \
         created_by_kind, created_by_id, created_at, rotated_from, replaced_by FROM org_platform_keys \
         WHERE org_id = $1 AND ($2 OR archived_at IS NULL) AND ($3::uuid IS NULL OR id > $3) ORDER BY id LIMIT $4",
        org_id,
        include_archived,
        after,
        limit
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_org_disabled(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    at: Option<OffsetDateTime>,
    by: Actor<'_>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE org_platform_keys SET disabled_at = $3, \
         disabled_by_kind = CASE WHEN $3::timestamptz IS NULL THEN NULL ELSE $4 END, \
         disabled_by_id = CASE WHEN $3::timestamptz IS NULL THEN NULL ELSE $5::uuid END, \
         updated_at = now() WHERE org_id = $1 AND id = $2 AND archived_at IS NULL",
        org_id,
        id,
        at,
        by.kind,
        by.id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn org_disabled_by(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Disabler>, StoreError> {
    Ok(sqlx::query_as!(
        Disabler,
        "SELECT disabled_by_kind AS kind, disabled_by_id AS id FROM org_platform_keys WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn archive_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE org_platform_keys SET archived_at = $3, updated_at = now() WHERE org_id = $1 AND id = $2 AND archived_at IS NULL",
        org_id,
        id,
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn retire_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    id: Uuid,
    replaced_by: Uuid,
    stops_at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE org_platform_keys SET replaced_by = $3, expires_at = LEAST(expires_at, $4), updated_at = now() \
         WHERE org_id = $1 AND id = $2 AND archived_at IS NULL AND replaced_by IS NULL",
        org_id,
        id,
        replaced_by,
        stops_at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn touch_org(
    db: impl PgExecutor<'_>,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE org_platform_keys SET last_used_at = $2 WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $2::timestamptz - interval '1 minute')",
        id,
        at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn org_hash_of(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<Vec<u8>>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT secret_hash FROM org_platform_keys WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn ids_of_user_in_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM api_keys WHERE org_id = $1 AND user_id = $2",
        org_id,
        user_id
    )
    .fetch_all(db)
    .await?)
}
