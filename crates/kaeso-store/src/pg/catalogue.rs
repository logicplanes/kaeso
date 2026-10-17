use std::collections::{BTreeMap, BTreeSet};

use kaeso_core::catalogue::resolve::Alias;
use kaeso_core::catalogue::{Availability, Entry, Fields};
use kaeso_core::models_dev;
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub fetched_at: OffsetDateTime,
    pub source_url: String,
    pub content_hash: Vec<u8>,
    pub data: serde_json::Value,
    pub model_count: i32,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

pub async fn snapshot(db: impl PgExecutor<'_>) -> Result<Option<SnapshotRow>, StoreError> {
    Ok(sqlx::query_as!(
        SnapshotRow,
        "SELECT fetched_at, source_url, content_hash, data, model_count, last_attempt_at, last_error \
         FROM models_dev_snapshot WHERE id = 1"
    )
    .fetch_optional(db)
    .await?)
}

pub async fn put_snapshot(
    db: impl PgExecutor<'_>,
    fetched_at: OffsetDateTime,
    source_url: &str,
    content_hash: &[u8],
    data: &serde_json::Value,
    model_count: i32,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO models_dev_snapshot (id, fetched_at, source_url, content_hash, data, model_count, last_attempt_at) \
         VALUES (1, $1, $2, $3, $4, $5, $1) \
         ON CONFLICT (id) DO UPDATE SET fetched_at = $1, source_url = $2, content_hash = $3, data = $4, \
         model_count = $5, last_attempt_at = $1, last_error = NULL, updated_at = now()",
        fetched_at,
        source_url,
        content_hash,
        data,
        model_count
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn record_attempt(
    db: impl PgExecutor<'_>,
    at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE models_dev_snapshot SET last_attempt_at = $1, last_error = $2, updated_at = now() WHERE id = 1",
        at,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct EntryRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub model_id: String,
    pub overrides: serde_json::Value,
    pub custom: bool,
    pub delisted_at: Option<OffsetDateTime>,
    pub last_known: Option<serde_json::Value>,
    pub availability: Option<String>,
    pub availability_checked_at: Option<OffsetDateTime>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub revision: i64,
}

impl EntryRow {
    pub fn entry(&self) -> Result<Entry, StoreError> {
        let overrides: Fields = serde_json::from_value(self.overrides.clone()).map_err(|e| {
            StoreError::Corrupt(format!("overrides of catalogue model {}: {e}", self.id))
        })?;
        let last_known = match &self.last_known {
            Some(v) => Some(
                serde_json::from_value::<models_dev::Model>(v.clone()).map_err(|e| {
                    StoreError::Corrupt(format!("catalogue model {}: {e}", self.id))
                })?,
            ),
            None => None,
        };
        let availability = match self.availability.as_deref() {
            Some(text) => Some(Availability::parse(text).ok_or_else(|| {
                StoreError::Corrupt(format!(
                    "availability `{text}` of catalogue model {}",
                    self.id
                ))
            })?),
            None => None,
        };
        Ok(Entry {
            id: self.id,
            provider: self.provider.clone(),
            model_id: self.model_id.clone(),
            overrides,
            custom: self.custom,
            delisted: self.delisted_at.is_some(),
            last_known,
            availability,
        })
    }
}

#[derive(Clone, Debug)]
pub struct NewEntry<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub provider: &'a str,
    pub model_id: &'a str,
    pub overrides: &'a serde_json::Value,
    pub custom: bool,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
}

pub async fn insert(db: impl PgExecutor<'_>, e: &NewEntry<'_>) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "INSERT INTO catalogue_models (id, org_id, project_id, provider, model_id, overrides, custom, created_by_kind, created_by_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (project_id, provider, model_id) DO NOTHING",
        e.id,
        e.org_id,
        e.project_id,
        e.provider,
        e.model_id,
        e.overrides,
        e.custom,
        e.created_by_kind,
        e.created_by_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<EntryRow>, StoreError> {
    Ok(sqlx::query_as!(
        EntryRow,
        "SELECT id, org_id, project_id, provider, model_id, overrides, custom, delisted_at, last_known, \
         availability, availability_checked_at, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM catalogue_models WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<EntryRow>, StoreError> {
    Ok(sqlx::query_as!(
        EntryRow,
        "SELECT id, org_id, project_id, provider, model_id, overrides, custom, delisted_at, last_known, \
         availability, availability_checked_at, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM catalogue_models \
         WHERE project_id = $1 AND id = $2 FOR UPDATE",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: impl PgExecutor<'_>, project_id: Uuid) -> Result<Vec<EntryRow>, StoreError> {
    Ok(sqlx::query_as!(
        EntryRow,
        "SELECT id, org_id, project_id, provider, model_id, overrides, custom, delisted_at, last_known, \
         availability, availability_checked_at, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM catalogue_models WHERE project_id = $1 \
         ORDER BY provider, model_id",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_overrides(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    overrides: &serde_json::Value,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE catalogue_models SET overrides = $3, revision = revision + 1, updated_at = now() \
         WHERE project_id = $1 AND id = $2",
        project_id,
        id,
        overrides
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
        "DELETE FROM catalogue_models WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn of_model(
    db: impl PgExecutor<'_>,
    provider: &str,
    model_id: &str,
) -> Result<Vec<EntryRow>, StoreError> {
    Ok(sqlx::query_as!(
        EntryRow,
        "SELECT id, org_id, project_id, provider, model_id, overrides, custom, delisted_at, last_known, \
         availability, availability_checked_at, created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM catalogue_models \
         WHERE provider = $1 AND model_id = $2 AND NOT custom",
        provider,
        model_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn listed_models(
    db: impl PgExecutor<'_>,
) -> Result<Vec<(String, String, bool)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT provider, model_id, bool_or(delisted_at IS NOT NULL) AS \"delisted!\" FROM catalogue_models \
         WHERE NOT custom GROUP BY provider, model_id"
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.provider, r.model_id, r.delisted))
        .collect())
}

pub async fn mark_delisted(
    db: impl PgExecutor<'_>,
    provider: &str,
    model_id: &str,
    at: OffsetDateTime,
    last_known: &serde_json::Value,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE catalogue_models SET delisted_at = $3, last_known = $4, revision = revision + 1, updated_at = now() \
         WHERE provider = $1 AND model_id = $2 AND NOT custom AND delisted_at IS NULL RETURNING project_id",
        provider,
        model_id,
        at,
        last_known
    )
    .fetch_all(db)
    .await?)
}

pub async fn mark_listed(
    db: impl PgExecutor<'_>,
    provider: &str,
    model_id: &str,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE catalogue_models SET delisted_at = NULL, last_known = NULL, revision = revision + 1, updated_at = now() \
         WHERE provider = $1 AND model_id = $2 AND delisted_at IS NOT NULL RETURNING project_id",
        provider,
        model_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn set_availability(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    availability: Availability,
    at: OffsetDateTime,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE catalogue_models SET availability = $3, availability_checked_at = $4 \
         WHERE project_id = $1 AND id = $2",
        project_id,
        id,
        availability.as_str(),
        at
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

pub async fn clear_availability(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE catalogue_models SET availability = NULL, availability_checked_at = NULL \
         WHERE org_id = $1 AND provider = $2 AND availability IS NOT NULL RETURNING project_id",
        org_id,
        provider
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct AliasRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub catalogue_model_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub async fn insert_alias(
    db: impl PgExecutor<'_>,
    id: Uuid,
    org_id: Uuid,
    project_id: Uuid,
    name: &str,
    target: Uuid,
) -> Result<bool, StoreError> {
    let r = sqlx::query!(
        "INSERT INTO aliases (id, org_id, project_id, name, catalogue_model_id) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (project_id, name) DO NOTHING",
        id,
        org_id,
        project_id,
        name,
        target
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn list_aliases(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<AliasRow>, StoreError> {
    Ok(sqlx::query_as!(
        AliasRow,
        "SELECT id, project_id, name, catalogue_model_id, created_at, updated_at FROM aliases \
         WHERE project_id = $1 ORDER BY name",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get_alias(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<Option<AliasRow>, StoreError> {
    Ok(sqlx::query_as!(
        AliasRow,
        "SELECT id, project_id, name, catalogue_model_id, created_at, updated_at FROM aliases \
         WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn update_alias(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
    name: &str,
    target: Uuid,
) -> Result<Option<bool>, StoreError> {
    let r = sqlx::query!(
        "UPDATE aliases SET name = $3, catalogue_model_id = $4, updated_at = now() WHERE project_id = $1 AND id = $2",
        project_id,
        id,
        name,
        target
    )
    .execute(db)
    .await;
    match r {
        Ok(r) => Ok(Some(r.rows_affected() == 1)),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn delete_alias(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    id: Uuid,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "DELETE FROM aliases WHERE project_id = $1 AND id = $2",
        project_id,
        id
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    Membership(Uuid),
    ServiceAccount(Uuid),
    Key(Uuid),
}

pub async fn access(
    db: &mut sqlx::PgConnection,
    project_id: Uuid,
    subject: Subject,
) -> Result<Option<BTreeSet<Uuid>>, StoreError> {
    let restricted = match subject {
        Subject::Membership(id) => sqlx::query_scalar!(
            "SELECT models_restricted FROM project_memberships WHERE project_id = $1 AND id = $2",
            project_id,
            id
        )
        .fetch_optional(&mut *db)
        .await?,
        Subject::ServiceAccount(id) => {
            sqlx::query_scalar!(
                "SELECT models_restricted FROM service_accounts WHERE project_id = $1 AND id = $2",
                project_id,
                id
            )
            .fetch_optional(&mut *db)
            .await?
        }
        Subject::Key(id) => {
            sqlx::query_scalar!(
                "SELECT models_restricted FROM api_keys WHERE project_id = $1 AND id = $2",
                project_id,
                id
            )
            .fetch_optional(&mut *db)
            .await?
        }
    };
    if restricted != Some(true) {
        return Ok(None);
    }
    let (m, s, k) = ids(subject);
    let models = sqlx::query_scalar!(
        "SELECT catalogue_model_id FROM model_access WHERE project_id = $1 \
         AND membership_id IS NOT DISTINCT FROM $2 AND service_account_id IS NOT DISTINCT FROM $3 \
         AND api_key_id IS NOT DISTINCT FROM $4",
        project_id,
        m,
        s,
        k
    )
    .fetch_all(&mut *db)
    .await?;
    Ok(Some(models.into_iter().collect()))
}

fn ids(subject: Subject) -> (Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match subject {
        Subject::Membership(id) => (Some(id), None, None),
        Subject::ServiceAccount(id) => (None, Some(id), None),
        Subject::Key(id) => (None, None, Some(id)),
    }
}

pub async fn set_access(
    db: &mut sqlx::PgConnection,
    project_id: Uuid,
    subject: Subject,
    models: Option<&BTreeSet<Uuid>>,
) -> Result<(), StoreError> {
    let restricted = models.is_some();
    match subject {
        Subject::Membership(id) => {
            sqlx::query!(
                "UPDATE project_memberships SET models_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
                project_id,
                id,
                restricted
            )
            .execute(&mut *db)
            .await?;
        }
        Subject::ServiceAccount(id) => {
            sqlx::query!(
                "UPDATE service_accounts SET models_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
                project_id,
                id,
                restricted
            )
            .execute(&mut *db)
            .await?;
        }
        Subject::Key(id) => {
            sqlx::query!(
                "UPDATE api_keys SET models_restricted = $3, updated_at = now() WHERE project_id = $1 AND id = $2",
                project_id,
                id,
                restricted
            )
            .execute(&mut *db)
            .await?;
        }
    }
    let (m, s, k) = ids(subject);
    sqlx::query!(
        "DELETE FROM model_access WHERE project_id = $1 AND membership_id IS NOT DISTINCT FROM $2 \
         AND service_account_id IS NOT DISTINCT FROM $3 AND api_key_id IS NOT DISTINCT FROM $4",
        project_id,
        m,
        s,
        k
    )
    .execute(&mut *db)
    .await?;
    if let Some(models) = models {
        let list: Vec<Uuid> = models.iter().copied().collect();
        sqlx::query!(
            "INSERT INTO model_access (project_id, membership_id, service_account_id, api_key_id, catalogue_model_id) \
             SELECT $1, $2, $3, $4, unnest($5::uuid[])",
            project_id,
            m,
            s,
            k,
            &list
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn prune_key_lists(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
    user_id: Option<Uuid>,
    service_account_id: Option<Uuid>,
    allowed: &BTreeSet<Uuid>,
) -> Result<Vec<Uuid>, StoreError> {
    let list: Vec<Uuid> = allowed.iter().copied().collect();
    Ok(sqlx::query_scalar!(
        "DELETE FROM model_access ma USING api_keys k WHERE ma.api_key_id = k.id AND k.project_id = $1 \
         AND k.user_id IS NOT DISTINCT FROM $2 AND k.service_account_id IS NOT DISTINCT FROM $3 \
         AND ma.catalogue_model_id <> ALL($4::uuid[]) RETURNING k.id",
        project_id,
        user_id,
        service_account_id,
        &list
    )
    .fetch_all(db)
    .await?)
}

pub async fn clear_service_account(
    db: impl PgExecutor<'_>,
    service_account_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "DELETE FROM model_access WHERE service_account_id = $1",
        service_account_id
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectModels {
    pub entries: Vec<Entry>,
    pub aliases: Vec<Alias>,
    pub users: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub service_accounts: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub keys: BTreeMap<Uuid, BTreeSet<Uuid>>,
}

pub async fn project_models(
    db: &mut sqlx::PgConnection,
    project_id: Uuid,
) -> Result<ProjectModels, StoreError> {
    let entries = list(&mut *db, project_id)
        .await?
        .iter()
        .map(EntryRow::entry)
        .collect::<Result<Vec<_>, _>>()?;
    let aliases = list_aliases(&mut *db, project_id)
        .await?
        .into_iter()
        .map(|a| Alias {
            name: a.name,
            target: a.catalogue_model_id,
        })
        .collect();
    let mut out = ProjectModels {
        entries,
        aliases,
        ..ProjectModels::default()
    };
    let users = sqlx::query!(
        "SELECT pm.user_id, ma.catalogue_model_id AS \"model?\" FROM project_memberships pm \
         LEFT JOIN model_access ma ON ma.membership_id = pm.id WHERE pm.project_id = $1 AND pm.models_restricted",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in users {
        let set = out.users.entry(r.user_id).or_default();
        set.extend(r.model);
    }
    let accounts = sqlx::query!(
        "SELECT s.id, ma.catalogue_model_id AS \"model?\" FROM service_accounts s \
         LEFT JOIN model_access ma ON ma.service_account_id = s.id WHERE s.project_id = $1 AND s.models_restricted",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in accounts {
        out.service_accounts
            .entry(r.id)
            .or_default()
            .extend(r.model);
    }
    let keys = sqlx::query!(
        "SELECT k.id, ma.catalogue_model_id AS \"model?\" FROM api_keys k \
         LEFT JOIN model_access ma ON ma.api_key_id = k.id \
         WHERE k.project_id = $1 AND k.models_restricted AND k.archived_at IS NULL",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in keys {
        out.keys.entry(r.id).or_default().extend(r.model);
    }
    Ok(out)
}

impl ProjectModels {
    pub fn access(
        &self,
        key: Option<Uuid>,
        user: Option<Uuid>,
        service_account: Option<Uuid>,
    ) -> kaeso_core::catalogue::resolve::Access {
        let member = match (user, service_account) {
            (Some(u), _) => self.users.get(&u).cloned(),
            (None, Some(s)) => self.service_accounts.get(&s).cloned(),
            (None, None) => Some(BTreeSet::new()),
        };
        kaeso_core::catalogue::resolve::Access {
            member,
            key: key.and_then(|k| self.keys.get(&k).cloned()),
        }
    }
}
