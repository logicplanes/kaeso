use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use kaeso_core::catalogue::{Fields, Supplement};
use kaeso_core::id::{CatalogueModelId, OrgId};
use kaeso_core::models_dev::{self, Snapshot};
use kaeso_store::StoreError;
use kaeso_store::pg::{self, catalogue, locks};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::state::AppState;

pub const EMBEDDED_SOURCE: &str = "embedded";

const MAX_DOCUMENT: usize = 64 << 20;

#[derive(Clone, Debug)]
pub struct Loaded {
    pub snapshot: Snapshot,
    pub fetched_at: OffsetDateTime,
    pub source: String,
}

#[derive(Debug)]
pub struct ModelsDev {
    current: RwLock<Arc<Loaded>>,
    supplement: Supplement,
}

impl ModelsDev {
    pub fn embedded() -> ModelsDev {
        let snapshot = models_dev::embedded().unwrap_or_else(|e| {
            tracing::error!(error = %e, "the built-in models.dev snapshot does not parse");
            Snapshot::default()
        });
        let supplement = Supplement::embedded().unwrap_or_else(|e| {
            tracing::error!(error = %e, "the built-in model supplement does not parse");
            Supplement::default()
        });
        ModelsDev {
            current: RwLock::new(Arc::new(Loaded {
                snapshot,
                fetched_at: models_dev::EMBEDDED_FETCHED_AT,
                source: EMBEDDED_SOURCE.into(),
            })),
            supplement,
        }
    }

    pub fn current(&self) -> Arc<Loaded> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn supplement(&self) -> &Supplement {
        &self.supplement
    }

    fn replace(&self, loaded: Loaded) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(loaded);
    }
}

fn provider_ids() -> Vec<&'static str> {
    kaeso_core::provider::PROVIDERS
        .iter()
        .map(|p| p.id)
        .collect()
}

pub async fn init(state: &AppState) -> Result<(), StoreError> {
    let stored = catalogue::snapshot(&state.pg).await?;
    match stored {
        Some(row) if row.fetched_at >= models_dev::EMBEDDED_FETCHED_AT => {
            load_row(state, row);
            Ok(())
        }
        _ => {
            let embedded = state.models_dev.current();
            let hash = Sha256::digest(models_dev::EMBEDDED.as_bytes()).to_vec();
            apply(
                state,
                embedded.snapshot.clone(),
                models_dev::EMBEDDED_FETCHED_AT,
                EMBEDDED_SOURCE,
                &hash,
            )
            .await
            .map(|_| ())
        }
    }
}

fn load_row(state: &AppState, row: catalogue::SnapshotRow) {
    match serde_json::from_value::<Snapshot>(row.data) {
        Ok(snapshot) => state.models_dev.replace(Loaded {
            snapshot,
            fetched_at: row.fetched_at,
            source: row.source_url,
        }),
        Err(e) => {
            tracing::error!(error = %e, "the stored models.dev data does not parse; keeping the current data")
        }
    }
}

pub async fn reload(state: &AppState) -> Result<(), StoreError> {
    if let Some(row) = catalogue::snapshot(&state.pg).await? {
        load_row(state, row);
    }
    Ok(())
}

pub fn spawn_reloader(state: &AppState) {
    let state = state.clone();
    let notify = state.caches.models_dev_changed.clone();
    tokio::spawn(async move {
        loop {
            notify.notified().await;
            if let Err(e) = reload(&state).await {
                tracing::warn!(error = %e, "cannot reload models.dev data");
            }
        }
    });
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub price_changes: usize,
    pub delisted: usize,
    pub relisted: usize,
}

pub async fn apply(
    state: &AppState,
    new: Snapshot,
    fetched_at: OffsetDateTime,
    source: &str,
    hash: &[u8],
) -> Result<Applied, StoreError> {
    let st = state.clone();
    let source = source.to_owned();
    let hash = hash.to_vec();
    let outcome = pg::with_advisory_lock(&state.pg, locks::MODELS_DEV, || async move {
        apply_locked(&st, new, fetched_at, &source, &hash).await
    })
    .await?;
    let (applied, loaded, projects) = outcome;
    state.models_dev.replace(loaded);
    for project in projects {
        crate::services::catalogue::forget(state, project).await;
    }
    if state
        .valkey
        .publish_invalidation("mdev:reload")
        .await
        .is_err()
    {
        tracing::warn!(
            "cannot tell other instances about new models.dev data; they reload on their next resubscription"
        );
    }
    Ok(applied)
}

type ApplyOutcome = (Applied, Loaded, Vec<uuid::Uuid>);

async fn apply_locked(
    state: &AppState,
    new: Snapshot,
    fetched_at: OffsetDateTime,
    source: &str,
    hash: &[u8],
) -> Result<ApplyOutcome, StoreError> {
    let old = match catalogue::snapshot(&state.pg).await? {
        Some(row) => serde_json::from_value::<Snapshot>(row.data).unwrap_or_default(),
        None => Snapshot::default(),
    };
    let now = state.clock.now();
    let mut applied = Applied::default();
    let mut projects = Vec::new();
    let mut tx = state.pg.begin().await?;
    for (pid, provider) in &new.providers {
        for (mid, model) in &provider.models {
            let Some(before) = old.model(pid, mid) else {
                continue;
            };
            let (b, a) = (
                before.cost.clone().unwrap_or_default(),
                model.cost.clone().unwrap_or_default(),
            );
            if b == a {
                continue;
            }
            applied.price_changes += 1;
            let target = format!("{pid}/{mid}");
            audit(
                &mut tx,
                None,
                "models_dev.price_change",
                ("model", target),
                prices(&Fields::from_models_dev(before)),
                prices(&Fields::from_models_dev(model)),
                now,
            )
            .await?;
            for entry in catalogue::of_model(&mut *tx, pid, mid).await? {
                let Ok(e) = entry.entry() else { continue };
                let was = e.overrides.clone().over(&Fields::from_models_dev(before));
                let is = e.overrides.clone().over(&Fields::from_models_dev(model));
                if prices(&was) != prices(&is) {
                    audit(
                        &mut tx,
                        Some(OrgId::from_uuid(entry.org_id)),
                        "catalogue_model.price_change",
                        (
                            "catalogue_model",
                            CatalogueModelId::from_uuid(entry.id).to_string(),
                        ),
                        prices(&was),
                        prices(&is),
                        now,
                    )
                    .await?;
                }
            }
        }
    }
    for (provider, model, delisted) in catalogue::listed_models(&mut *tx).await? {
        if !new.providers.contains_key(&provider) {
            continue;
        }
        let present = new.model(&provider, &model).is_some();
        if !delisted && !present {
            let last = old
                .model(&provider, &model)
                .and_then(|m| serde_json::to_value(m).ok())
                .unwrap_or(serde_json::Value::Null);
            let affected =
                catalogue::mark_delisted(&mut *tx, &provider, &model, now, &last).await?;
            if !affected.is_empty() {
                applied.delisted += 1;
                audit(
                    &mut tx,
                    None,
                    "models_dev.model_delisted",
                    ("model", format!("{provider}/{model}")),
                    None,
                    None,
                    now,
                )
                .await?;
            }
            projects.extend(affected);
        } else if delisted && present {
            let affected = catalogue::mark_listed(&mut *tx, &provider, &model).await?;
            applied.relisted += usize::from(!affected.is_empty());
            projects.extend(affected);
        }
    }
    let data = serde_json::to_value(&new).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let count = i32::try_from(new.model_count()).unwrap_or(i32::MAX);
    catalogue::put_snapshot(&mut *tx, fetched_at, source, hash, &data, count).await?;
    tx.commit().await?;
    Ok((
        applied,
        Loaded {
            snapshot: new,
            fetched_at,
            source: source.to_owned(),
        },
        projects,
    ))
}

fn prices(f: &Fields) -> Option<serde_json::Value> {
    Some(json!({
        "input_price": f.input_price,
        "output_price": f.output_price,
        "cache_read_price": f.cache_read_price,
        "cache_write_price": f.cache_write_price,
    }))
}

async fn audit(
    tx: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: &'static str,
    target: (&'static str, String),
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    crate::platform::audit::system_change(tx, org, action, Some(target), before, after, at)
        .await
        .map_err(|e| StoreError::Corrupt(format!("audit: {e}")))
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("cannot fetch models.dev data: {0}")]
    Fetch(String),
    #[error("{0}")]
    Parse(#[from] models_dev::ParseError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Synced {
    Unchanged,
    Applied(Applied),
}

pub async fn sync(state: &AppState) -> Result<Synced, SyncError> {
    let now = state.clock.now();
    let result = fetch_and_apply(state, now).await;
    let error = result.as_ref().err().map(ToString::to_string);
    if let Err(e) = catalogue::record_attempt(&state.pg, now, error.as_deref()).await {
        tracing::warn!(error = %e, "cannot record the models.dev sync attempt");
    }
    match &result {
        Ok(Synced::Applied(a)) => tracing::info!(
            price_changes = a.price_changes,
            delisted = a.delisted,
            relisted = a.relisted,
            "applied new models.dev data"
        ),
        Ok(Synced::Unchanged) => tracing::debug!("models.dev data unchanged"),
        Err(e) => tracing::warn!(error = %e, "models.dev sync failed"),
    }
    result
}

async fn fetch_and_apply(state: &AppState, now: OffsetDateTime) -> Result<Synced, SyncError> {
    let url = state.config.models_dev.url.clone();
    let body = fetch(url.as_str()).await.map_err(SyncError::Fetch)?;
    let hash = Sha256::digest(&body).to_vec();
    if let Some(row) = catalogue::snapshot(&state.pg).await?
        && row.content_hash == hash
    {
        return Ok(Synced::Unchanged);
    }
    let parsed = models_dev::parse(&body, &provider_ids())?;
    if !parsed.skipped.is_empty() {
        tracing::warn!(skipped = ?parsed.skipped, "models.dev models skipped because they do not parse");
    }
    let applied = apply(state, parsed.snapshot, now, url.as_str(), &hash).await?;
    Ok(Synced::Applied(applied))
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("{url} answered {}", resp.status()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_DOCUMENT {
            return Err(format!("{url} sent more than {} MiB", MAX_DOCUMENT >> 20));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub async fn due(state: &AppState) -> Result<bool, StoreError> {
    if !state.config.models_dev.sync {
        return Ok(false);
    }
    let mut interval =
        time::Duration::try_from(state.config.models_dev.interval).unwrap_or(time::Duration::DAY);
    let Some(row) = catalogue::snapshot(&state.pg).await? else {
        return Ok(true);
    };
    if row.last_error.is_some() {
        interval = interval.min(time::Duration::HOUR);
    }
    Ok(match row.last_attempt_at {
        None => true,
        Some(at) => state.clock.now() - at >= interval,
    })
}
