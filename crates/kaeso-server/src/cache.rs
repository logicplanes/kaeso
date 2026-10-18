use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use moka::future::Cache;
use serde::Serialize;
use serde::de::DeserializeOwned;

use kaeso_store::valkey::{Invalidation, Valkey};

pub const TTL: Duration = Duration::from_secs(30);

const KEY_BLOCK_TTL: Duration = Duration::from_secs(3600);

pub const TOUCH_EVERY: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct TieredCache<V: Clone + Send + Sync + 'static> {
    name: &'static str,
    memory: Cache<String, Arc<V>>,
    valkey: Valkey,
    subscribed: Arc<AtomicBool>,
}

impl<V: Clone + Send + Sync + 'static> std::fmt::Debug for TieredCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TieredCache")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<V> TieredCache<V>
where
    V: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn new(name: &'static str, valkey: Valkey, subscribed: Arc<AtomicBool>) -> Self {
        TieredCache {
            name,
            memory: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(TTL)
                .build(),
            valkey,
            subscribed,
        }
    }

    fn valkey_key(&self, key: &str) -> String {
        self.valkey.key(&format!("cache:{}:{key}", self.name))
    }

    pub async fn cached(&self, key: &str) -> Option<Arc<V>> {
        if !self.subscribed.load(Ordering::Acquire) {
            return None;
        }
        self.memory.get(key).await
    }

    pub async fn get_or_load<E, F, Fut>(&self, key: &str, load: F) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        let use_memory = self.subscribed.load(Ordering::Acquire);
        if use_memory && let Some(v) = self.memory.get(key).await {
            return Ok(Some(v));
        }
        if use_memory && let Some(v) = self.valkey_get(key).await {
            let v = Arc::new(v);
            self.memory.insert(key.to_owned(), v.clone()).await;
            return Ok(Some(v));
        }
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        if use_memory {
            self.valkey_set(key, &value).await;
            self.memory.insert(key.to_owned(), value.clone()).await;
        }
        Ok(Some(value))
    }

    pub async fn put(&self, key: &str, value: V) {
        self.valkey_set(key, &value).await;
        self.memory.insert(key.to_owned(), Arc::new(value)).await;
    }

    pub async fn invalidate(&self, key: &str) {
        self.memory.invalidate(key).await;
        let mut conn = self.valkey.conn();
        let deleted: Result<i64, _> = redis::cmd("DEL")
            .arg(self.valkey_key(key))
            .query_async(&mut conn)
            .await;
        let published = self
            .valkey
            .publish_invalidation(&format!("{}:{key}", self.name))
            .await;
        if deleted.is_err() || published.is_err() {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "invalidate")
                .increment(1);
            tracing::warn!(
                cache = self.name,
                "cannot publish a cache invalidation; other instances drop the entry within 30 seconds"
            );
        }
    }

    async fn drop_local(&self, key: &str) {
        self.memory.invalidate(key).await;
    }

    fn clear_local(&self) {
        self.memory.invalidate_all();
    }

    async fn valkey_get(&self, key: &str) -> Option<V> {
        let mut conn = self.valkey.conn();
        let raw: Option<Vec<u8>> = redis::cmd("GET")
            .arg(self.valkey_key(key))
            .query_async(&mut conn)
            .await
            .inspect_err(|e| {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "cache_get")
                    .increment(1);
                tracing::debug!(cache = self.name, error = %e, "cannot read a shared cache entry; loading it from PostgreSQL");
            })
            .ok()
            .flatten();
        raw.and_then(|bytes| {
            serde_json::from_slice(&bytes)
                .inspect_err(|e| {
                    tracing::warn!(cache = self.name, error = %e, "a shared cache entry does not parse; loading it from PostgreSQL");
                })
                .ok()
        })
    }

    async fn valkey_set(&self, key: &str, value: &V) {
        let Ok(bytes) = serde_json::to_vec(value) else {
            return;
        };
        let mut conn = self.valkey.conn();
        let set: Result<(), _> = redis::cmd("SET")
            .arg(self.valkey_key(key))
            .arg(bytes)
            .arg("PX")
            .arg(u64::try_from(TTL.as_millis()).unwrap_or(30_000))
            .query_async(&mut conn)
            .await;
        if set.is_err() {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "cache_set")
                .increment(1);
        }
    }
}

pub struct MemoryCache<V: Send + Sync + 'static> {
    name: &'static str,
    memory: Cache<String, Arc<V>>,
    valkey: Valkey,
    subscribed: Arc<AtomicBool>,
}

impl<V: Send + Sync + 'static> Clone for MemoryCache<V> {
    fn clone(&self) -> Self {
        MemoryCache {
            name: self.name,
            memory: self.memory.clone(),
            valkey: self.valkey.clone(),
            subscribed: self.subscribed.clone(),
        }
    }
}

impl<V: Send + Sync + 'static> std::fmt::Debug for MemoryCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryCache")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<V: Send + Sync + 'static> MemoryCache<V> {
    fn new(name: &'static str, valkey: Valkey, subscribed: Arc<AtomicBool>) -> Self {
        MemoryCache {
            name,
            memory: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(TTL)
                .support_invalidation_closures()
                .build(),
            valkey,
            subscribed,
        }
    }

    pub async fn get_or_load<E, F, Fut>(
        &self,
        group: &str,
        rest: &str,
        load: F,
    ) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        let key = format!("{group}:{rest}");
        let use_memory = self.subscribed.load(Ordering::Acquire);
        if use_memory && let Some(v) = self.memory.get(&key).await {
            return Ok(Some(v));
        }
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        if use_memory {
            self.memory.insert(key, value.clone()).await;
        }
        Ok(Some(value))
    }

    pub async fn invalidate_group(&self, group: &str) {
        self.drop_group(group);
        if self
            .valkey
            .publish_invalidation(&format!("{}:{group}", self.name))
            .await
            .is_err()
        {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "invalidate")
                .increment(1);
            tracing::warn!(
                cache = self.name,
                "cannot publish a cache invalidation; other instances drop the entry within 30 seconds"
            );
        }
    }

    fn drop_group(&self, group: &str) {
        let prefix = format!("{group}:");
        if self
            .memory
            .invalidate_entries_if(move |k, _| k.starts_with(&prefix))
            .is_err()
        {
            self.memory.invalidate_all();
        }
    }

    fn clear_local(&self) {
        self.memory.invalidate_all();
    }
}

#[derive(Clone, Debug)]
pub struct Caches {
    pub sessions: TieredCache<kaeso_store::pg::sessions::SessionRow>,
    pub api_keys: TieredCache<kaeso_store::pg::keys::KeyAuth>,
    pub org_keys: TieredCache<kaeso_store::pg::keys::OrgKeyAuth>,
    pub credentials: MemoryCache<crate::services::credentials::Resolved>,
    pub project_models: TieredCache<kaeso_store::pg::catalogue::ProjectModels>,
    pub project_limits: TieredCache<Vec<kaeso_core::limits::Limit>>,
    pub log_settings: TieredCache<crate::services::logs::Cached>,
    pub models_dev_changed: Arc<tokio::sync::Notify>,
    pub touched: Cache<uuid::Uuid, ()>,
    pub unknown_keys: Cache<String, ()>,
    pub key_blocks: Cache<String, time::OffsetDateTime>,
    pub time_zone: Cache<(), Arc<str>>,
    subscribed: Arc<AtomicBool>,
}

impl Caches {
    pub fn new(valkey: &Valkey) -> Caches {
        let subscribed = Arc::new(AtomicBool::new(false));
        Caches {
            sessions: TieredCache::new("ses", valkey.clone(), subscribed.clone()),
            api_keys: TieredCache::new("key", valkey.clone(), subscribed.clone()),
            org_keys: TieredCache::new("opk", valkey.clone(), subscribed.clone()),
            credentials: MemoryCache::new("cred", valkey.clone(), subscribed.clone()),
            project_models: TieredCache::new("pmod", valkey.clone(), subscribed.clone()),
            project_limits: TieredCache::new("plim", valkey.clone(), subscribed.clone()),
            log_settings: TieredCache::new("lcfg", valkey.clone(), subscribed.clone()),
            models_dev_changed: Arc::new(tokio::sync::Notify::new()),
            touched: Cache::builder()
                .max_capacity(1_000_000)
                .time_to_live(TOUCH_EVERY)
                .build(),
            unknown_keys: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(TTL)
                .build(),
            key_blocks: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(KEY_BLOCK_TTL)
                .build(),
            time_zone: Cache::builder().max_capacity(1).time_to_live(TTL).build(),
            subscribed,
        }
    }

    pub fn listen(&self, valkey: &Valkey) {
        let mut rx = valkey.subscribe_invalidations();
        let caches = self.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                caches.apply(event).await;
            }
            caches.subscribed.store(false, Ordering::Release);
        });
    }

    pub fn is_subscribed(&self) -> bool {
        self.subscribed.load(Ordering::Acquire)
    }

    async fn apply(&self, event: Invalidation) {
        match event {
            Invalidation::Connected => {
                self.clear_local();
                self.subscribed.store(true, Ordering::Release);
                self.models_dev_changed.notify_one();
            }
            Invalidation::Disconnected => {
                self.subscribed.store(false, Ordering::Release);
                self.clear_local();
            }
            Invalidation::Message(m) => match m.split_once(':') {
                Some(("ses", key)) => self.sessions.drop_local(key).await,
                Some(("key", key)) => self.api_keys.drop_local(key).await,
                Some(("opk", key)) => self.org_keys.drop_local(key).await,
                Some(("cred", group)) => self.credentials.drop_group(group),
                Some(("pmod", key)) => self.project_models.drop_local(key).await,
                Some(("plim", key)) => self.project_limits.drop_local(key).await,
                Some(("lcfg", key)) => self.log_settings.drop_local(key).await,
                Some(("mdev", _)) => self.models_dev_changed.notify_one(),
                _ => self.clear_local(),
            },
        }
    }

    fn clear_local(&self) {
        self.sessions.clear_local();
        self.api_keys.clear_local();
        self.org_keys.clear_local();
        self.credentials.clear_local();
        self.project_models.clear_local();
        self.project_limits.clear_local();
        self.log_settings.clear_local();
    }
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}
