use std::collections::{BTreeMap, BTreeSet};

use kaeso_core::catalogue::resolve::{Access, Alias, Catalogue};
use kaeso_core::catalogue::{Availability, Entry, Fields};
use kaeso_core::kpmr;
use kaeso_core::providers::OrgProviders;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use super::credentials::{self, KeyInUse};
use crate::StoreError;

#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub fetched_at: OffsetDateTime,
    pub source_url: String,
    pub content_hash: Vec<u8>,
    pub format: Option<Vec<u8>>,
    pub data: serde_json::Value,
    pub model_count: i32,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

pub async fn snapshot(db: impl PgExecutor<'_>) -> Result<Option<SnapshotRow>, StoreError> {
    Ok(sqlx::query_as!(
        SnapshotRow,
        "SELECT fetched_at, source_url, content_hash, format, data, model_count, last_attempt_at, last_error \
         FROM kpmr_snapshot WHERE id = 1"
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct SyncState {
    pub source_url: String,
    pub content_hash: Vec<u8>,
    pub etag: Option<String>,
    pub format: Option<Vec<u8>>,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

pub async fn sync_state(db: impl PgExecutor<'_>) -> Result<Option<SyncState>, StoreError> {
    Ok(sqlx::query_as!(
        SyncState,
        "SELECT source_url, content_hash, etag, format, last_attempt_at, last_error FROM kpmr_snapshot WHERE id = 1"
    )
    .fetch_optional(db)
    .await?)
}

pub struct NewSnapshot<'a> {
    pub fetched_at: OffsetDateTime,
    pub source_url: &'a str,
    pub content_hash: &'a [u8],
    pub etag: Option<&'a str>,
    pub format: &'a [u8],
    pub data: &'a serde_json::Value,
    pub model_count: i32,
}

pub async fn put_snapshot(db: impl PgExecutor<'_>, s: NewSnapshot<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO kpmr_snapshot (id, fetched_at, source_url, content_hash, etag, format, data, model_count, last_attempt_at) \
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, $1) \
         ON CONFLICT (id) DO UPDATE SET fetched_at = $1, source_url = $2, content_hash = $3, etag = $4, format = $5, data = $6, \
         model_count = $7, last_attempt_at = $1, last_error = NULL, updated_at = now()",
        s.fetched_at,
        s.source_url,
        s.content_hash,
        s.etag,
        s.format,
        s.data,
        s.model_count
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn put_source(
    db: impl PgExecutor<'_>,
    source_url: &str,
    etag: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE kpmr_snapshot SET source_url = $1, etag = $2, updated_at = now() WHERE id = 1",
        source_url,
        etag
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
        "UPDATE kpmr_snapshot SET last_attempt_at = $1, last_error = $2, updated_at = now() WHERE id = 1",
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
                serde_json::from_value::<kpmr::Model>(v.clone()).map_err(|e| {
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
            availability_checked_at: self.availability_checked_at,
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

#[derive(Clone, Copy, Debug)]
pub struct Answer<'a> {
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub entry: Uuid,
    pub provider: &'a str,
    pub key: KeyInUse,
    pub availability: Availability,
    pub at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    Changed,
    Unchanged,
    KeyChanged,
    EntryGone,
}

pub async fn record_availability(
    tx: &mut PgConnection,
    answer: &Answer<'_>,
) -> Result<Recorded, StoreError> {
    credentials::lock_provider(&mut *tx, answer.org_id, answer.provider).await?;
    let current =
        credentials::key_in_use(&mut *tx, answer.org_id, answer.project_id, answer.provider)
            .await?;
    if current != Some(answer.key) {
        return Ok(Recorded::KeyChanged);
    }
    let changed = sqlx::query_scalar!(
        "UPDATE catalogue_models c SET availability = $5, availability_checked_at = $6 \
         FROM catalogue_models prev \
         WHERE c.org_id = $1 AND c.project_id = $2 AND c.id = $3 AND c.provider = $4 AND prev.id = c.id \
         RETURNING prev.availability IS DISTINCT FROM c.availability AS \"changed!\"",
        answer.org_id,
        answer.project_id,
        answer.entry,
        answer.provider,
        answer.availability.as_str(),
        answer.at
    )
    .fetch_optional(&mut *tx)
    .await?;
    Ok(match changed {
        Some(true) => Recorded::Changed,
        Some(false) => Recorded::Unchanged,
        None => Recorded::EntryGone,
    })
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "Stored")]
pub struct ProjectModels {
    pub catalogue: Catalogue,
    pub providers: OrgProviders,
    pub users: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub service_accounts: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub keys: BTreeMap<Uuid, BTreeSet<Uuid>>,
    pub open: BTreeSet<Uuid>,
}

#[derive(Deserialize)]
struct Stored {
    entries: Vec<Entry>,
    aliases: Vec<Alias>,
    users: BTreeMap<Uuid, BTreeSet<Uuid>>,
    service_accounts: BTreeMap<Uuid, BTreeSet<Uuid>>,
    keys: BTreeMap<Uuid, BTreeSet<Uuid>>,
    #[serde(default)]
    open: BTreeSet<Uuid>,
    #[serde(default)]
    providers: OrgProviders,
}

impl From<Stored> for ProjectModels {
    fn from(s: Stored) -> ProjectModels {
        ProjectModels {
            catalogue: Catalogue::new(s.entries, s.aliases),
            providers: s.providers,
            users: s.users,
            service_accounts: s.service_accounts,
            keys: s.keys,
            open: s.open,
        }
    }
}

impl Serialize for ProjectModels {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("ProjectModels", 7)?;
        out.serialize_field("entries", self.catalogue.entries())?;
        out.serialize_field("aliases", self.catalogue.aliases())?;
        out.serialize_field("users", &self.users)?;
        out.serialize_field("service_accounts", &self.service_accounts)?;
        out.serialize_field("keys", &self.keys)?;
        out.serialize_field("open", &self.open)?;
        out.serialize_field("providers", &self.providers)?;
        out.end()
    }
}

static NOTHING: BTreeSet<Uuid> = BTreeSet::new();

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
    let providers = super::custom_providers::of_project(&mut *db, project_id)
        .await?
        .iter()
        .map(super::custom_providers::CustomProviderRow::custom_provider)
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = ProjectModels {
        catalogue: Catalogue::new(entries, aliases),
        providers: OrgProviders::new(providers),
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
        "SELECT s.id, s.models_restricted, ma.catalogue_model_id AS \"model?\" FROM service_accounts s \
         LEFT JOIN model_access ma ON ma.service_account_id = s.id AND s.models_restricted \
         WHERE s.project_id = $1",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in accounts {
        if r.models_restricted {
            out.service_accounts
                .entry(r.id)
                .or_default()
                .extend(r.model);
        } else {
            out.open.insert(r.id);
        }
    }
    let keys = sqlx::query!(
        "SELECT k.id, k.models_restricted, ma.catalogue_model_id AS \"model?\" FROM api_keys k \
         LEFT JOIN model_access ma ON ma.api_key_id = k.id AND k.models_restricted \
         WHERE k.project_id = $1 AND k.archived_at IS NULL",
        project_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in keys {
        if r.models_restricted {
            out.keys.entry(r.id).or_default().extend(r.model);
        } else {
            out.open.insert(r.id);
        }
    }
    Ok(out)
}

impl ProjectModels {
    // A key or service account this value doesn't list was created after it was loaded,
    // perhaps with a model list: it gets no model rather than every one.
    fn listed<'a>(
        &self,
        id: Uuid,
        restricted: &'a BTreeMap<Uuid, BTreeSet<Uuid>>,
    ) -> Option<&'a BTreeSet<Uuid>> {
        match restricted.get(&id) {
            Some(models) => Some(models),
            None if self.open.contains(&id) => None,
            None => Some(&NOTHING),
        }
    }

    pub fn knows(&self, key: Option<Uuid>, service_account: Option<Uuid>) -> bool {
        let known = |id: Uuid, restricted: &BTreeMap<Uuid, BTreeSet<Uuid>>| {
            restricted.contains_key(&id) || self.open.contains(&id)
        };
        key.is_none_or(|k| known(k, &self.keys))
            && service_account.is_none_or(|s| known(s, &self.service_accounts))
    }

    pub fn access(
        &self,
        key: Option<Uuid>,
        user: Option<Uuid>,
        service_account: Option<Uuid>,
    ) -> Access<'_> {
        let member = match (user, service_account) {
            (Some(u), _) => self.users.get(&u),
            (None, Some(s)) => self.listed(s, &self.service_accounts),
            (None, None) => Some(&NOTHING),
        };
        Access {
            member,
            key: key.and_then(|k| self.listed(k, &self.keys)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(provider: &str, model: &str) -> Entry {
        Entry {
            id: Uuid::now_v7(),
            provider: provider.into(),
            model_id: model.into(),
            overrides: Fields::default(),
            custom: false,
            delisted: false,
            last_known: None,
            availability: None,
            availability_checked_at: None,
        }
    }

    #[test]
    fn cached_project_models_keep_their_shape_and_resolve_after_reading_back() {
        let gpt = entry("openai", "gpt-4o");
        let user = Uuid::now_v7();
        let models = ProjectModels {
            catalogue: Catalogue::new(
                vec![gpt.clone()],
                vec![Alias {
                    name: "fast".into(),
                    target: gpt.id,
                }],
            ),
            users: BTreeMap::from([(user, BTreeSet::from([gpt.id]))]),
            ..ProjectModels::default()
        };
        let json = serde_json::to_value(&models).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "aliases",
                "entries",
                "keys",
                "open",
                "providers",
                "service_accounts",
                "users"
            ]
        );
        assert_eq!(json["aliases"][0]["name"], "fast");
        let mut older = json.clone();
        let back: ProjectModels = serde_json::from_value(json).unwrap();
        assert_eq!(back, models);
        assert_eq!(back.catalogue.resolve("fast").unwrap().0, &gpt);
        assert_eq!(back.catalogue.resolve("openai/gpt-4o").unwrap().0, &gpt);
        assert!(back.access(None, Some(user), None).allows(gpt.id));
        assert!(!back.access(None, None, None).allows(gpt.id));
        older.as_object_mut().unwrap().remove("open");
        older.as_object_mut().unwrap().remove("providers");
        let older: ProjectModels = serde_json::from_value(older).unwrap();
        assert!(
            older.open.is_empty() && older.providers == OrgProviders::default(),
            "a value cached before `open` and `providers` reads back"
        );
    }

    #[test]
    fn a_key_or_service_account_the_cached_lists_do_not_know_may_use_no_model() {
        let gpt = entry("openai", "gpt-4o");
        let (restricted_user, restricted_key, open_key, open_account) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let models = ProjectModels {
            catalogue: Catalogue::new(vec![gpt.clone()], vec![]),
            users: BTreeMap::from([(restricted_user, BTreeSet::new())]),
            keys: BTreeMap::from([(restricted_key, BTreeSet::new())]),
            open: BTreeSet::from([open_key, open_account]),
            ..ProjectModels::default()
        };
        let (new_key, new_account, other_user) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        assert!(
            !models
                .access(Some(restricted_key), Some(other_user), None)
                .allows(gpt.id)
        );
        assert!(
            !models
                .access(Some(open_key), Some(restricted_user), None)
                .allows(gpt.id)
        );
        assert!(
            models
                .access(Some(open_key), Some(other_user), None)
                .allows(gpt.id)
        );
        assert!(
            models
                .access(Some(open_key), None, Some(open_account))
                .allows(gpt.id)
        );
        assert!(
            !models
                .access(Some(new_key), Some(other_user), None)
                .allows(gpt.id),
            "a key created after the value was cached gets nothing"
        );
        assert!(
            !models
                .access(Some(open_key), None, Some(new_account))
                .allows(gpt.id)
        );
        assert!(models.knows(Some(open_key), Some(open_account)));
        assert!(models.knows(Some(restricted_key), None));
        assert!(!models.knows(Some(new_key), None));
        assert!(!models.knows(Some(open_key), Some(new_account)));
        assert!(models.knows(None, None));
    }
}
