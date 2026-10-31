use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, PoisonError, RwLock};
use std::time::Duration;

use kaeso_core::catalogue::Fields;
use kaeso_core::id::{CatalogueModelId, OrgId};
use kaeso_core::kpmr::{self, Catalogue, TrustedKey};
use kaeso_store::StoreError;
use kaeso_store::pg::{self, catalogue, locks};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;

use crate::config::KpmrInterval;
use crate::platform::audit::Action;
use crate::state::AppState;

pub const EMBEDDED_SOURCE: &str = "embedded";

const MAX_DOCUMENT: usize = 64 << 20;

const MAX_REDIRECTS: usize = 3;

// A build that adds a provider or a KPMR field gets a new format: data an older build stored
// is then completed from the built-in snapshot and fetched again, however new it is.
static FORMAT: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let model = kpmr::Model {
        modalities: Some(kpmr::Modalities::default()),
        limits: Some(kpmr::Limits::default()),
        pricing: Some(kpmr::Pricing::default()),
        capabilities: Some(kpmr::Capabilities::default()),
        reasoning: Some(kpmr::Reasoning::default()),
        gateway: Some(kpmr::Gateway::default()),
        ..kpmr::Model::default()
    };
    let providers = kaeso_core::provider::known_ids()
        .into_iter()
        .map(|id| {
            let provider = kpmr::Provider {
                id: id.to_string(),
                name: None,
                models: BTreeMap::from([(String::new(), model.clone())]),
            };
            (id.to_string(), provider)
        })
        .collect();
    let shape = serde_json::to_vec(&Catalogue {
        providers,
        ..Catalogue::default()
    })
    .unwrap_or_default();
    Sha256::digest(shape).to_vec()
});

fn in_this_format(format: Option<&[u8]>) -> bool {
    format == Some(FORMAT.as_slice())
}

#[derive(Clone, Debug)]
pub struct Loaded {
    pub snapshot: Catalogue,
    pub fetched_at: OffsetDateTime,
    pub source: String,
}

#[derive(Debug)]
pub struct Kpmr {
    current: RwLock<Arc<Loaded>>,
}

impl Kpmr {
    pub fn embedded() -> Kpmr {
        let snapshot = kpmr::embedded().unwrap_or_else(|e| {
            tracing::error!(error = %e, "the built-in KPMR snapshot does not parse");
            Catalogue::default()
        });
        Kpmr {
            current: RwLock::new(Arc::new(Loaded {
                fetched_at: embedded_fetched_at(&snapshot),
                snapshot,
                source: EMBEDDED_SOURCE.into(),
            })),
        }
    }

    pub fn current(&self) -> Arc<Loaded> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn replace_source(&self, source: &str) {
        let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
        if current.source != source {
            let mut loaded = (**current).clone();
            loaded.source = source.to_owned();
            *current = Arc::new(loaded);
        }
    }

    fn replace(&self, loaded: Loaded) {
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(loaded);
    }
}

pub fn embedded_fetched_at(embedded: &Catalogue) -> OffsetDateTime {
    embedded.generated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

pub async fn init(state: &AppState) -> Result<(), StoreError> {
    let stored = catalogue::snapshot(&state.pg).await?;
    let embedded = state.kpmr.current();
    match stored {
        Some(row) if preferred_over_embedded(&row, embedded.fetched_at) => {
            load_row(state, row);
            Ok(())
        }
        _ => {
            let hash = Sha256::digest(kpmr::EMBEDDED.as_bytes()).to_vec();
            apply(
                state,
                embedded.snapshot.clone(),
                embedded.fetched_at,
                EMBEDDED_SOURCE,
                &hash,
                None,
            )
            .await
            .map(|_| ())
        }
    }
}

fn preferred_over_embedded(row: &catalogue::SnapshotRow, embedded: OffsetDateTime) -> bool {
    match row.fetched_at.cmp(&embedded) {
        Ordering::Greater => true,
        Ordering::Equal => in_this_format(row.format.as_deref()),
        Ordering::Less => false,
    }
}

fn load_row(state: &AppState, row: catalogue::SnapshotRow) {
    let complete = in_this_format(row.format.as_deref());
    match serde_json::from_value::<Catalogue>(row.data) {
        Ok(mut snapshot) => {
            if !complete {
                complete_from_embedded(&mut snapshot);
            }
            state.kpmr.replace(Loaded {
                snapshot,
                fetched_at: row.fetched_at,
                source: row.source_url,
            });
        }
        Err(e) => {
            tracing::error!(error = %e, "the stored KPMR data does not parse; keeping the current data")
        }
    }
}

fn complete_from_embedded(snapshot: &mut Catalogue) {
    let Ok(embedded) = kpmr::embedded() else {
        return;
    };
    if snapshot.provider_ids.is_empty() {
        snapshot.provider_ids = embedded.provider_ids;
    }
    for (id, provider) in embedded.providers {
        snapshot.providers.entry(id).or_insert(provider);
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
    let notify = state.caches.kpmr_changed.clone();
    tokio::spawn(async move {
        loop {
            notify.notified().await;
            if let Err(e) = reload(&state).await {
                tracing::warn!(error = %e, "cannot reload KPMR data");
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
    new: Catalogue,
    fetched_at: OffsetDateTime,
    source: &str,
    hash: &[u8],
    etag: Option<&str>,
) -> Result<Applied, StoreError> {
    let st = state.clone();
    let source = source.to_owned();
    let hash = hash.to_vec();
    let etag = etag.map(str::to_owned);
    let outcome = pg::with_advisory_lock(&state.pg, locks::KPMR, || async move {
        apply_locked(&st, new, fetched_at, &source, &hash, etag.as_deref()).await
    })
    .await?;
    let (applied, loaded, projects) = outcome;
    state.kpmr.replace(loaded);
    for project in projects {
        crate::services::catalogue::forget(state, project).await;
    }
    if state
        .valkey
        .publish_invalidation("kpmr:reload")
        .await
        .is_err()
    {
        tracing::warn!(
            "cannot tell other instances about new KPMR data; they reload on their next resubscription"
        );
    }
    Ok(applied)
}

type ApplyOutcome = (Applied, Loaded, Vec<uuid::Uuid>);

async fn apply_locked(
    state: &AppState,
    new: Catalogue,
    fetched_at: OffsetDateTime,
    source: &str,
    hash: &[u8],
    etag: Option<&str>,
) -> Result<ApplyOutcome, StoreError> {
    let old = match catalogue::snapshot(&state.pg).await? {
        Some(row) => serde_json::from_value::<Catalogue>(row.data).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "the stored KPMR data does not parse; this update audits no price changes");
            Catalogue::default()
        }),
        None => Catalogue::default(),
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
                prices(&Fields::from_kpmr(before)),
                prices(&Fields::from_kpmr(model)),
            );
            if b == a {
                continue;
            }
            applied.price_changes += 1;
            let target = format!("{pid}/{mid}");
            audit(
                &mut tx,
                None,
                Action::KpmrPriceChange,
                ("model", target),
                b,
                a,
                now,
            )
            .await?;
            for entry in catalogue::of_model(&mut *tx, pid, mid).await? {
                let Ok(e) = entry.entry() else { continue };
                let was = e.overrides.clone().over(&Fields::from_kpmr(before));
                let is = e.overrides.clone().over(&Fields::from_kpmr(model));
                if prices(&was) != prices(&is) {
                    audit(
                        &mut tx,
                        Some(OrgId::from_uuid(entry.org_id)),
                        Action::CatalogueModelPriceChange,
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
                    Action::KpmrModelDelisted,
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
    catalogue::put_snapshot(
        &mut *tx,
        catalogue::NewSnapshot {
            fetched_at,
            source_url: source,
            content_hash: hash,
            etag,
            format: &FORMAT,
            data: &data,
            model_count: count,
        },
    )
    .await?;
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
        "cache_write_1h_price": f.cache_write_1h_price,
    }))
}

async fn audit(
    tx: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: Action,
    target: (&'static str, String),
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    at: OffsetDateTime,
) -> Result<(), StoreError> {
    crate::platform::audit::system_event(tx, org, action, Some(target), before, after, at).await
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("cannot fetch KPMR data: {0}")]
    Fetch(String),
    #[error("refused the KPMR catalogue: {0}")]
    Signature(#[from] kpmr::SignatureError),
    #[error("{0}")]
    Parse(#[from] kpmr::ParseError),
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
        tracing::warn!(error = %e, "cannot record the KPMR sync attempt");
    }
    match &result {
        Ok(Synced::Applied(a)) => tracing::info!(
            price_changes = a.price_changes,
            delisted = a.delisted,
            relisted = a.relisted,
            "applied new KPMR data"
        ),
        Ok(Synced::Unchanged) => tracing::debug!("KPMR data unchanged"),
        Err(e) => tracing::warn!(error = %e, "KPMR sync failed"),
    }
    result
}

pub fn trusted_keys(state: &AppState) -> Vec<TrustedKey> {
    TrustedKey::built_in()
        .inspect_err(|e| tracing::error!(error = %e, "the built-in KPMR key does not parse"))
        .into_iter()
        .chain(state.config.kpmr.public_keys.iter().cloned())
        .collect()
}

async fn fetch_and_apply(state: &AppState, now: OffsetDateTime) -> Result<Synced, SyncError> {
    let url = state.config.kpmr.url.clone();
    let stored = catalogue::sync_state(&state.pg).await?;
    let known = stored
        .as_ref()
        .filter(|s| in_this_format(s.format.as_deref()));
    let etag = known
        .filter(|s| s.source_url == url.as_str())
        .and_then(|s| s.etag.as_deref());
    let body = match fetch(&url, etag).await.map_err(SyncError::Fetch)? {
        Fetched::NotModified => return Ok(Synced::Unchanged),
        Fetched::Document(body) => body,
    };
    kpmr::verify(&body.bytes, body.signature.as_deref(), &trusted_keys(state))?;
    let hash = Sha256::digest(&body.bytes).to_vec();
    if known.is_some_and(|s| s.content_hash == hash) {
        catalogue::put_source(&state.pg, url.as_str(), body.etag.as_deref()).await?;
        state.kpmr.replace_source(url.as_str());
        return Ok(Synced::Unchanged);
    }
    let parsed = kpmr::parse(&body.bytes, &kaeso_core::provider::known_ids())?;
    if !parsed.skipped.is_empty() {
        tracing::warn!(skipped = ?parsed.skipped, "KPMR models skipped because they do not parse");
    }
    let applied = apply(
        state,
        parsed.catalogue,
        now,
        url.as_str(),
        &hash,
        body.etag.as_deref(),
    )
    .await?;
    Ok(Synced::Applied(applied))
}

fn redirect_refusal(from: &Url, to: &Url, hops: usize) -> Option<String> {
    if hops > MAX_REDIRECTS {
        return Some(format!("{from} redirected more than {MAX_REDIRECTS} times"));
    }
    if to.host() != from.host() {
        return Some(format!("{from} redirected to another host, {to}"));
    }
    if to.scheme() != "https" && to.scheme() != from.scheme() {
        return Some(format!("{from} redirected to {to}, which is not https"));
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
struct Document {
    bytes: Vec<u8>,
    etag: Option<String>,
    signature: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Fetched {
    NotModified,
    Document(Document),
}

fn catalogue_url(url: &Url) -> Url {
    let mut ids: Vec<&str> = kaeso_core::provider::PROVIDERS
        .iter()
        .map(|p| p.id)
        .collect();
    ids.sort_unstable();
    let mut with = url.clone();
    with.query_pairs_mut()
        .append_pair("providers", &ids.join(","));
    with
}

async fn fetch(url: &Url, etag: Option<&str>) -> Result<Fetched, String> {
    let origin = url.clone();
    let redirects = reqwest::redirect::Policy::custom(move |attempt| {
        match redirect_refusal(&origin, attempt.url(), attempt.previous().len()) {
            Some(refused) => attempt.error(refused),
            None => attempt.follow(),
        }
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .redirect(redirects)
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client
        .get(catalogue_url(url))
        .header(reqwest::header::ACCEPT, kpmr::MEDIA_TYPE);
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let mut resp = request.send().await.map_err(|e| {
        let mut message = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(c) = cause {
            message = format!("{message}: {c}");
            cause = c.source();
        }
        message
    })?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED && etag.is_some() {
        return Ok(Fetched::NotModified);
    }
    if status == reqwest::StatusCode::NOT_ACCEPTABLE {
        return Err(format!(
            "{url} does not serve catalogue format {}",
            kpmr::FORMAT
        ));
    }
    if !status.is_success() {
        return Err(format!("{url} answered {status}"));
    }
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let etag = header(reqwest::header::ETAG.as_str());
    let signature = header(kpmr::SIGNATURE_HEADER);
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_DOCUMENT {
            return Err(format!("{url} sent more than {} MiB", MAX_DOCUMENT >> 20));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Fetched::Document(Document {
        bytes,
        etag,
        signature,
    }))
}

const SHORTEST_INTERVAL: time::Duration = time::Duration::minutes(5);

const LONGEST_INTERVAL: time::Duration = time::Duration::DAY;

pub fn interval(config: KpmrInterval, refresh_seconds: Option<u64>) -> time::Duration {
    match config {
        KpmrInterval::Every(every) => time::Duration::try_from(every).unwrap_or(LONGEST_INTERVAL),
        KpmrInterval::Auto => refresh_seconds
            .and_then(|s| i64::try_from(s).ok())
            .map_or(time::Duration::HOUR, time::Duration::seconds)
            .clamp(SHORTEST_INTERVAL, LONGEST_INTERVAL),
    }
}

pub async fn due(state: &AppState) -> Result<bool, StoreError> {
    if !state.config.kpmr.sync {
        return Ok(false);
    }
    let mut interval = interval(
        state.config.kpmr.interval,
        state.kpmr.current().snapshot.refresh_seconds,
    );
    let Some(row) = catalogue::sync_state(&state.pg).await? else {
        return Ok(true);
    };
    if !in_this_format(row.format.as_deref()) && row.last_error.is_none() {
        return Ok(true);
    }
    if row.last_error.is_some() {
        interval = interval.min(time::Duration::HOUR);
    }
    Ok(match row.last_attempt_at {
        None => true,
        Some(at) => state.clock.now() - at >= interval,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Redirect};
    use axum::routing::get;

    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn redirects_stay_on_https_and_the_same_host_a_few_times() {
        let from = url("https://kpmr.kaeso.app/v1/catalogue");
        assert_eq!(
            redirect_refusal(&from, &url("https://kpmr.kaeso.app/v2/catalogue"), 1),
            None
        );
        assert_eq!(
            redirect_refusal(&from, &url("https://kpmr.kaeso.app:8443/a"), 3),
            None
        );
        for (to, hops) in [
            ("http://kpmr.kaeso.app/v1/catalogue", 1),
            ("https://evil.example/v1/catalogue", 1),
            ("https://cdn.kpmr.kaeso.app/v1/catalogue", 1),
            ("https://kpmr.kaeso.app/v1/catalogue", 4),
        ] {
            assert!(
                redirect_refusal(&from, &url(to), hops).is_some(),
                "{to} after {hops}"
            );
        }
        let local = url("http://127.0.0.1:9/v1/catalogue");
        assert_eq!(
            redirect_refusal(&local, &url("http://127.0.0.1:9/b"), 1),
            None
        );
        assert_eq!(
            redirect_refusal(&local, &url("https://127.0.0.1:9/b"), 1),
            None
        );
        assert!(redirect_refusal(&local, &url("http://localhost:9/b"), 1).is_some());
    }

    #[test]
    fn the_gateway_asks_for_the_providers_it_calls() {
        let asked = catalogue_url(&url("https://kpmr.kaeso.app/v1/catalogue"));
        let providers = asked
            .query_pairs()
            .find(|(k, _)| k == "providers")
            .unwrap()
            .1
            .into_owned();
        let ids: Vec<&str> = providers.split(',').collect();
        assert!(ids.is_sorted());
        assert_eq!(ids.len(), kaeso_core::provider::PROVIDERS.len());
        assert!(ids.contains(&"anthropic"));
    }

    #[test]
    fn auto_follows_kpmrs_hint_within_five_minutes_and_a_day() {
        let auto = KpmrInterval::Auto;
        assert_eq!(interval(auto, Some(3600)), time::Duration::HOUR);
        assert_eq!(interval(auto, Some(60)), time::Duration::minutes(5));
        assert_eq!(interval(auto, Some(10 * 86_400)), time::Duration::DAY);
        assert_eq!(interval(auto, None), time::Duration::HOUR);
        assert_eq!(
            interval(KpmrInterval::Every(Duration::from_secs(60)), Some(3600)),
            time::Duration::minutes(1)
        );
    }

    #[tokio::test]
    async fn the_fetch_asks_for_format_1_sends_the_etag_and_follows_only_the_redirects_it_allows() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let catalogue = |headers: HeaderMap| async move {
            if headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) != Some(kpmr::MEDIA_TYPE) {
                return StatusCode::NOT_ACCEPTABLE.into_response();
            }
            if headers
                .get(header::IF_NONE_MATCH)
                .and_then(|v| v.to_str().ok())
                == Some("\"r1\"")
            {
                return StatusCode::NOT_MODIFIED.into_response();
            }
            (
                [
                    (header::ETAG, "\"r1\""),
                    (
                        header::HeaderName::from_static("kpmr-signature"),
                        "keyid=k, ed25519=AA==",
                    ),
                ],
                "{}",
            )
                .into_response()
        };
        let router = axum::Router::new()
            .route("/v1/catalogue", get(catalogue))
            .route("/v9", get(|| async { StatusCode::NOT_ACCEPTABLE }))
            .route(
                "/moved",
                get(|| async { Redirect::temporary("/v1/catalogue") }),
            )
            .route(
                "/elsewhere",
                get(move || async move {
                    Redirect::temporary(&format!("http://localhost:{port}/v1/catalogue"))
                }),
            )
            .route("/loop", get(|| async { Redirect::temporary("/loop") }));
        tokio::spawn(async move { axum::serve(listener, router).await });
        let at = |path: &str| url(&format!("http://127.0.0.1:{port}{path}"));
        assert_eq!(
            fetch(&at("/moved"), None).await.unwrap(),
            Fetched::Document(Document {
                bytes: b"{}".to_vec(),
                etag: Some("\"r1\"".into()),
                signature: Some("keyid=k, ed25519=AA==".into()),
            })
        );
        assert_eq!(
            fetch(&at("/v1/catalogue"), Some("\"r1\"")).await.unwrap(),
            Fetched::NotModified
        );
        let err = fetch(&at("/v9"), None).await.unwrap_err();
        assert!(err.ends_with("does not serve catalogue format 1"), "{err}");
        let err = fetch(&at("/elsewhere"), None).await.unwrap_err();
        assert!(err.contains("another host"), "{err}");
        let err = fetch(&at("/loop"), None).await.unwrap_err();
        assert!(err.contains("more than 3 times"), "{err}");
    }
}
