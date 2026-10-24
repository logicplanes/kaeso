use std::collections::BTreeMap;

use kaeso_core::provider::ProviderId;
use kaeso_core::providers::{CustomAuth, CustomProvider, Draft};
use kaeso_kpp::DialectId;
use sqlx::{PgConnection, PgExecutor};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomProviderRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub name: String,
    pub base_url: String,
    pub auth: String,
    pub generation: String,
    pub embeddings: Option<String>,
    pub count_tokens: Option<String>,
    pub created_by_kind: String,
    pub created_by_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub revision: i64,
}

fn corrupt(row: &CustomProviderRow, what: &str) -> StoreError {
    StoreError::Corrupt(format!("{what} of custom provider {}", row.id))
}

fn dialect(row: &CustomProviderRow, text: &str) -> Result<DialectId, StoreError> {
    text.parse().map_err(|_| corrupt(row, "a dialect"))
}

impl CustomProviderRow {
    pub fn custom_provider(&self) -> Result<CustomProvider, StoreError> {
        let draft = Draft {
            uuid: self.id,
            id: ProviderId::parse(&self.provider).map_err(|_| corrupt(self, "the id"))?,
            name: self.name.clone(),
            base_url: self.base_url.clone(),
            auth: CustomAuth::parse(&self.auth).ok_or_else(|| corrupt(self, "the auth style"))?,
            generation: dialect(self, &self.generation)?,
            embeddings: self
                .embeddings
                .as_deref()
                .map(|d| dialect(self, d))
                .transpose()?,
            count_tokens: self
                .count_tokens
                .as_deref()
                .map(|d| dialect(self, d))
                .transpose()?,
        };
        CustomProvider::new(draft).map_err(|p| corrupt(self, &format!("{p:?}")))
    }
}

#[derive(Clone, Debug)]
pub struct Fields<'a> {
    pub name: &'a str,
    pub base_url: &'a str,
    pub auth: &'a str,
    pub generation: &'a str,
    pub embeddings: Option<&'a str>,
    pub count_tokens: Option<&'a str>,
}

impl<'a> Fields<'a> {
    pub fn of(p: &'a CustomProvider) -> Fields<'a> {
        Fields {
            name: p.name(),
            base_url: p.base_url().as_str(),
            auth: p.auth().as_str(),
            generation: p.generation().as_str(),
            embeddings: p.embeddings().map(DialectId::as_str),
            count_tokens: p.count_tokens().map(DialectId::as_str),
        }
    }
}

pub async fn list(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Vec<CustomProviderRow>, StoreError> {
    Ok(sqlx::query_as!(
        CustomProviderRow,
        "SELECT id, org_id, provider, name, base_url, auth, generation, embeddings, count_tokens, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM custom_providers WHERE org_id = $1 ORDER BY provider",
        org_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn of_project(
    db: impl PgExecutor<'_>,
    project_id: Uuid,
) -> Result<Vec<CustomProviderRow>, StoreError> {
    Ok(sqlx::query_as!(
        CustomProviderRow,
        "SELECT c.id, c.org_id, c.provider, c.name, c.base_url, c.auth, c.generation, c.embeddings, \
         c.count_tokens, c.created_by_kind, c.created_by_id, c.created_at, c.updated_at, c.revision \
         FROM custom_providers c JOIN projects p ON p.org_id = c.org_id \
         WHERE p.id = $1 ORDER BY c.provider",
        project_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn get(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
) -> Result<Option<CustomProviderRow>, StoreError> {
    Ok(sqlx::query_as!(
        CustomProviderRow,
        "SELECT id, org_id, provider, name, base_url, auth, generation, embeddings, count_tokens, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM custom_providers WHERE org_id = $1 AND provider = $2",
        org_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub async fn get_for_update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
) -> Result<Option<CustomProviderRow>, StoreError> {
    Ok(sqlx::query_as!(
        CustomProviderRow,
        "SELECT id, org_id, provider, name, base_url, auth, generation, embeddings, count_tokens, \
         created_by_kind, created_by_id, created_at, updated_at, revision \
         FROM custom_providers WHERE org_id = $1 AND provider = $2 FOR UPDATE",
        org_id,
        provider
    )
    .fetch_optional(db)
    .await?)
}

pub struct NewCustomProvider<'a> {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider: &'a str,
    pub fields: Fields<'a>,
    pub created_by_kind: &'a str,
    pub created_by_id: Option<Uuid>,
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    p: &NewCustomProvider<'_>,
) -> Result<bool, StoreError> {
    let f = &p.fields;
    let r = sqlx::query!(
        "INSERT INTO custom_providers (id, org_id, provider, name, base_url, auth, generation, embeddings, \
         count_tokens, created_by_kind, created_by_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         ON CONFLICT (org_id, provider) DO NOTHING",
        p.id,
        p.org_id,
        p.provider,
        f.name,
        f.base_url,
        f.auth,
        f.generation,
        f.embeddings,
        f.count_tokens,
        p.created_by_kind,
        p.created_by_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn update(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    provider: &str,
    f: &Fields<'_>,
) -> Result<bool, StoreError> {
    Ok(sqlx::query!(
        "UPDATE custom_providers SET name = $3, base_url = $4, auth = $5, generation = $6, embeddings = $7, \
         count_tokens = $8, revision = revision + 1, updated_at = now() WHERE org_id = $1 AND provider = $2",
        org_id,
        provider,
        f.name,
        f.base_url,
        f.auth,
        f.generation,
        f.embeddings,
        f.count_tokens
    )
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Removed {
    pub credentials: u64,
    pub models: u64,
    pub projects: Vec<Uuid>,
}

pub async fn delete(
    db: &mut PgConnection,
    org_id: Uuid,
    provider: &str,
) -> Result<Option<Removed>, StoreError> {
    let gone = sqlx::query!(
        "DELETE FROM custom_providers WHERE org_id = $1 AND provider = $2",
        org_id,
        provider
    )
    .execute(&mut *db)
    .await?
    .rows_affected();
    if gone == 0 {
        return Ok(None);
    }
    let credentials = sqlx::query!(
        "DELETE FROM provider_credentials WHERE org_id = $1 AND provider = $2",
        org_id,
        provider
    )
    .execute(&mut *db)
    .await?
    .rows_affected();
    let mut projects = sqlx::query_scalar!(
        "DELETE FROM catalogue_models WHERE org_id = $1 AND provider = $2 RETURNING project_id",
        org_id,
        provider
    )
    .fetch_all(&mut *db)
    .await?;
    let models = u64::try_from(projects.len()).unwrap_or(u64::MAX);
    projects.sort_unstable();
    projects.dedup();
    Ok(Some(Removed {
        credentials,
        models,
        projects,
    }))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InUse {
    pub keys: i64,
    pub models: i64,
    pub projects: i64,
}

pub async fn in_use(
    db: &mut PgConnection,
    org_id: Uuid,
) -> Result<BTreeMap<String, InUse>, StoreError> {
    let mut out: BTreeMap<String, InUse> = BTreeMap::new();
    let keys = sqlx::query!(
        "SELECT c.provider, count(k.id) AS \"keys!\" FROM custom_providers c \
         JOIN provider_credentials k ON k.org_id = c.org_id AND k.provider = c.provider \
         WHERE c.org_id = $1 GROUP BY c.provider",
        org_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in keys {
        out.entry(r.provider).or_default().keys = r.keys;
    }
    let models = sqlx::query!(
        "SELECT c.provider, count(m.id) AS \"models!\", count(DISTINCT m.project_id) AS \"projects!\" \
         FROM custom_providers c JOIN catalogue_models m ON m.org_id = c.org_id AND m.provider = c.provider \
         WHERE c.org_id = $1 GROUP BY c.provider",
        org_id
    )
    .fetch_all(&mut *db)
    .await?;
    for r in models {
        let u = out.entry(r.provider).or_default();
        u.models = r.models;
        u.projects = r.projects;
    }
    Ok(out)
}

pub async fn org_projects(db: impl PgExecutor<'_>, org_id: Uuid) -> Result<Vec<Uuid>, StoreError> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM projects WHERE org_id = $1", org_id)
            .fetch_all(db)
            .await?,
    )
}
