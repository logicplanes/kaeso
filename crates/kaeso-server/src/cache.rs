use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use moka::future::Cache;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, OwnedMutexGuard};

use kaeso_store::valkey::{Invalidation, Valkey};

pub const TTL: Duration = Duration::from_secs(30);

pub const UNSUBSCRIBED_TTL: Duration = Duration::from_secs(2);

pub const REFRESH_AFTER: Duration = Duration::from_secs(20);

const KEY_BLOCK_TTL: Duration = Duration::from_secs(3600);

const FLIGHT_IDLE: Duration = Duration::from_secs(60);

pub const TOUCH_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
struct Subscription {
    subscribed: AtomicBool,
    epoch: AtomicU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seen {
    subscribed: bool,
    epoch: u64,
}

impl Subscription {
    fn seen(&self) -> Seen {
        let epoch = self.epoch.load(Ordering::Acquire);
        Seen {
            subscribed: self.subscribed.load(Ordering::Acquire),
            epoch,
        }
    }

    fn still(&self, seen: Seen) -> bool {
        seen.subscribed && self.seen() == seen
    }

    fn set(&self, subscribed: bool) {
        self.subscribed.store(subscribed, Ordering::Release);
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Clone)]
struct Flights(Cache<String, Arc<Mutex<()>>>);

struct Flight {
    _guard: OwnedMutexGuard<()>,
    waited: bool,
}

impl Flights {
    fn new(capacity: u64) -> Flights {
        Flights(
            Cache::builder()
                .max_capacity(capacity)
                .time_to_idle(FLIGHT_IDLE)
                .build(),
        )
    }

    async fn lane(&self, key: &str) -> Arc<Mutex<()>> {
        self.0
            .get_with_by_ref(key, async { Arc::new(Mutex::new(())) })
            .await
    }

    async fn board(&self, key: &str) -> Flight {
        let lane = self.lane(key).await;
        match lane.clone().try_lock_owned() {
            Ok(guard) => Flight {
                _guard: guard,
                waited: false,
            },
            Err(_) => Flight {
                _guard: lane.lock_owned().await,
                waited: true,
            },
        }
    }
}

#[derive(Clone)]
struct Held<V> {
    value: Arc<V>,
    at: Instant,
}

#[derive(Clone)]
pub struct TieredCache<V: Clone + Send + Sync + 'static> {
    name: &'static str,
    memory: Cache<String, Held<V>>,
    fallback: Cache<String, Arc<V>>,
    flights: Flights,
    valkey: Valkey,
    subscription: Arc<Subscription>,
    refresh_after: Duration,
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
    fn new(name: &'static str, valkey: Valkey, subscription: Arc<Subscription>) -> Self {
        TieredCache {
            name,
            memory: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(TTL)
                .build(),
            fallback: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(UNSUBSCRIBED_TTL)
                .build(),
            flights: Flights::new(100_000),
            valkey,
            subscription,
            refresh_after: REFRESH_AFTER,
        }
    }

    fn valkey_key(&self, key: &str) -> String {
        self.valkey.key(&format!("cache:{}:{key}", self.name))
    }

    async fn held(&self, key: &str, seen: Seen) -> Option<Held<V>> {
        if seen.subscribed
            && let Some(h) = self.memory.get(key).await
        {
            return Some(h);
        }
        self.fallback.get(key).await.map(|value| Held {
            value,
            at: Instant::now(),
        })
    }

    pub async fn cached(&self, key: &str) -> Option<Arc<V>> {
        self.held(key, self.subscription.seen())
            .await
            .map(|h| h.value)
    }

    pub async fn get_or_load<E, F, Fut>(&self, key: &str, load: F) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        let seen = self.subscription.seen();
        if let Some(h) = self.held(key, seen).await {
            return Ok(Some(h.value));
        }
        self.load_once(key, seen, load).await
    }

    pub async fn get_or_refresh<E, F, Fut>(&self, key: &str, load: F) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let seen = self.subscription.seen();
        if let Some(h) = self.held(key, seen).await {
            if seen.subscribed
                && h.at.elapsed() >= self.refresh_after
                && let Ok(guard) = self.flights.lane(key).await.try_lock_owned()
            {
                self.refresh(key, seen, load, guard);
            }
            return Ok(Some(h.value));
        }
        self.load_once(key, seen, load).await
    }

    async fn load_once<E, F, Fut>(
        &self,
        key: &str,
        seen: Seen,
        load: F,
    ) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        let mut flight = Some(self.flights.board(key).await);
        if flight.as_ref().is_some_and(|f| f.waited) {
            if let Some(h) = self.held(key, self.subscription.seen()).await {
                return Ok(Some(h.value));
            }
            flight = None;
        }
        if seen.subscribed
            && let Some(v) = self.valkey_get(key).await
        {
            let v = Arc::new(v);
            self.remember(key, v.clone(), seen).await;
            return Ok(Some(v));
        }
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        if self.subscription.still(seen) {
            self.valkey_set(key, &value).await;
        }
        self.remember(key, value.clone(), seen).await;
        drop(flight);
        Ok(Some(value))
    }

    fn refresh<E, F, Fut>(&self, key: &str, seen: Seen, load: F, guard: OwnedMutexGuard<()>)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let cache = self.clone();
        let key = key.to_owned();
        tokio::spawn(async move {
            match load().await {
                Ok(Some(value)) if cache.subscription.still(seen) => {
                    let value = Arc::new(value);
                    cache.valkey_set(&key, &value).await;
                    cache.remember(&key, value, seen).await;
                }
                Ok(Some(_)) => {}
                Ok(None) => cache.drop_local(&key).await,
                Err(e) => {
                    tracing::debug!(cache = cache.name, error = %e, "cannot refresh a cache entry ahead of time; it expires as usual");
                }
            }
            drop(guard);
        });
    }

    async fn remember(&self, key: &str, value: Arc<V>, seen: Seen) {
        if self.subscription.still(seen) {
            self.memory
                .insert(
                    key.to_owned(),
                    Held {
                        value,
                        at: Instant::now(),
                    },
                )
                .await;
        } else {
            self.fallback.insert(key.to_owned(), value).await;
        }
    }

    pub async fn put(&self, key: &str, value: V) {
        let seen = self.subscription.seen();
        let value = Arc::new(value);
        self.valkey_set(key, &value).await;
        self.remember(key, value, seen).await;
    }

    pub async fn invalidate(&self, key: &str) {
        self.drop_local(key).await;
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
        self.fallback.invalidate(key).await;
    }

    fn clear_subscribed(&self) {
        self.memory.invalidate_all();
    }

    fn clear_local(&self) {
        self.memory.invalidate_all();
        self.fallback.invalidate_all();
    }

    async fn valkey_get(&self, key: &str) -> Option<V> {
        let mut conn = self.valkey.bulk();
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
        let mut conn = self.valkey.bulk();
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
    fallback: Cache<String, Arc<V>>,
    flights: Flights,
    valkey: Valkey,
    subscription: Arc<Subscription>,
}

impl<V: Send + Sync + 'static> Clone for MemoryCache<V> {
    fn clone(&self) -> Self {
        MemoryCache {
            name: self.name,
            memory: self.memory.clone(),
            fallback: self.fallback.clone(),
            flights: self.flights.clone(),
            valkey: self.valkey.clone(),
            subscription: self.subscription.clone(),
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
    fn new(name: &'static str, valkey: Valkey, subscription: Arc<Subscription>) -> Self {
        MemoryCache {
            name,
            memory: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(TTL)
                .support_invalidation_closures()
                .build(),
            fallback: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(UNSUBSCRIBED_TTL)
                .support_invalidation_closures()
                .build(),
            flights: Flights::new(10_000),
            valkey,
            subscription,
        }
    }

    async fn held(&self, key: &str, seen: Seen) -> Option<Arc<V>> {
        if seen.subscribed
            && let Some(v) = self.memory.get(key).await
        {
            return Some(v);
        }
        self.fallback.get(key).await
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
        let seen = self.subscription.seen();
        if let Some(v) = self.held(&key, seen).await {
            return Ok(Some(v));
        }
        let mut flight = Some(self.flights.board(&key).await);
        if flight.as_ref().is_some_and(|f| f.waited) {
            if let Some(v) = self.held(&key, self.subscription.seen()).await {
                return Ok(Some(v));
            }
            flight = None;
        }
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        if self.subscription.still(seen) {
            self.memory.insert(key, value.clone()).await;
        } else {
            self.fallback.insert(key, value.clone()).await;
        }
        drop(flight);
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
        for cache in [&self.memory, &self.fallback] {
            let prefix = format!("{group}:");
            if cache
                .invalidate_entries_if(move |k, _| k.starts_with(&prefix))
                .is_err()
            {
                cache.invalidate_all();
            }
        }
    }

    fn clear_subscribed(&self) {
        self.memory.invalidate_all();
    }

    fn clear_local(&self) {
        self.memory.invalidate_all();
        self.fallback.invalidate_all();
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
    subscription: Arc<Subscription>,
}

impl Caches {
    pub fn new(valkey: &Valkey) -> Caches {
        let subscription = Arc::new(Subscription::default());
        Caches {
            sessions: TieredCache::new("ses", valkey.clone(), subscription.clone()),
            api_keys: TieredCache::new("key", valkey.clone(), subscription.clone()),
            org_keys: TieredCache::new("opk", valkey.clone(), subscription.clone()),
            credentials: MemoryCache::new("cred", valkey.clone(), subscription.clone()),
            project_models: TieredCache::new("pmod", valkey.clone(), subscription.clone()),
            project_limits: TieredCache::new("plim", valkey.clone(), subscription.clone()),
            log_settings: TieredCache::new("lcfg", valkey.clone(), subscription.clone()),
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
            subscription,
        }
    }

    pub fn listen(&self, valkey: &Valkey) {
        let mut rx = valkey.subscribe_invalidations();
        let caches = self.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                caches.apply(event).await;
            }
            caches.subscription.set(false);
        });
    }

    pub fn is_subscribed(&self) -> bool {
        self.subscription.seen().subscribed
    }

    async fn apply(&self, event: Invalidation) {
        match event {
            Invalidation::Connected => {
                self.clear_subscribed();
                self.subscription.set(true);
                self.models_dev_changed.notify_one();
            }
            Invalidation::Disconnected => {
                self.subscription.set(false);
                self.clear_subscribed();
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

    fn clear_subscribed(&self) {
        self.sessions.clear_subscribed();
        self.api_keys.clear_subscribed();
        self.org_keys.clear_subscribed();
        self.credentials.clear_subscribed();
        self.project_models.clear_subscribed();
        self.project_limits.clear_subscribed();
        self.log_settings.clear_subscribed();
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use secrecy::SecretString;

    use super::*;

    fn unreachable_valkey() -> Valkey {
        Valkey::new(&SecretString::from("redis://127.0.0.1:1"), "kaeso-test:")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn cache(subscription: &Arc<Subscription>) -> TieredCache<String> {
        TieredCache::new("test", unreachable_valkey(), subscription.clone())
    }

    async fn counted(
        cache: &TieredCache<String>,
        loads: &AtomicUsize,
        value: &str,
    ) -> Option<Arc<String>> {
        cache
            .get_or_load("k", || async {
                loads.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                Ok::<_, String>(Some(value.to_owned()))
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[tokio::test]
    async fn without_the_subscription_a_load_serves_the_next_requests_for_a_moment() {
        let subscription = Arc::new(Subscription::default());
        let cache = cache(&subscription);
        let loads = AtomicUsize::new(0);
        for _ in 0..3 {
            assert_eq!(
                counted(&cache, &loads, "v1").await.as_deref(),
                Some(&"v1".to_owned())
            );
        }
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(cache.cached("k").await.is_some());
        cache.drop_local("k").await;
        assert_eq!(
            counted(&cache, &loads, "v2").await.as_deref(),
            Some(&"v2".to_owned())
        );
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn concurrent_misses_share_one_load() {
        let subscription = Arc::new(Subscription::default());
        let cache = cache(&subscription);
        let loads = AtomicUsize::new(0);
        let values = futures::future::join_all((0..32).map(|_| counted(&cache, &loads, "v"))).await;
        assert!(values.iter().all(|v| v.as_deref() == Some(&"v".to_owned())));
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_load_is_not_kept_and_waiters_load_for_themselves() {
        let subscription = Arc::new(Subscription::default());
        let cache = cache(&subscription);
        let loads = AtomicUsize::new(0);
        let failing = || async {
            loads.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Err::<Option<String>, _>("down".to_owned())
        };
        let results =
            futures::future::join_all((0..4).map(|_| cache.get_or_load("k", failing))).await;
        assert!(
            results
                .iter()
                .all(|r| r.as_ref().err().map(String::as_str) == Some("down"))
        );
        assert_eq!(loads.load(Ordering::SeqCst), 4);
        assert!(cache.cached("k").await.is_none());
        let missing = cache
            .get_or_load("k", || async { Ok::<Option<String>, String>(None) })
            .await;
        assert_eq!(missing, Ok(None));
    }

    #[tokio::test]
    async fn reconnecting_keeps_short_lived_entries_instead_of_reloading_everything() {
        let valkey = unreachable_valkey();
        let caches = Caches::new(&valkey);
        let loads = AtomicUsize::new(0);
        let load = || async {
            loads.fetch_add(1, Ordering::SeqCst);
            Ok::<_, String>(Some(Vec::new()))
        };
        assert!(!caches.is_subscribed());
        caches.project_limits.get_or_load("p", load).await.ok();
        caches.apply(Invalidation::Connected).await;
        assert!(caches.is_subscribed());
        caches.project_limits.get_or_load("p", load).await.ok();
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        caches.apply(Invalidation::Message("plim:p".into())).await;
        caches.project_limits.get_or_load("p", load).await.ok();
        assert_eq!(loads.load(Ordering::SeqCst), 2);
        caches.apply(Invalidation::Disconnected).await;
        assert!(!caches.is_subscribed());
        caches.project_limits.get_or_load("p", load).await.ok();
        assert_eq!(
            loads.load(Ordering::SeqCst),
            3,
            "subscribed entries go at once"
        );
    }

    #[tokio::test]
    async fn a_load_that_outlives_the_subscription_is_kept_only_briefly() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = cache(&subscription);
        let value = cache
            .get_or_load("k", || async {
                subscription.set(false);
                subscription.set(true);
                Ok::<_, String>(Some("v".to_owned()))
            })
            .await;
        assert_eq!(value, Ok(Some(Arc::new("v".to_owned()))));
        assert!(cache.memory.get("k").await.is_none());
        assert!(cache.fallback.get("k").await.is_some());
    }

    #[tokio::test]
    async fn an_old_entry_is_served_while_it_is_refreshed_in_the_background() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let loads = Arc::new(AtomicUsize::new(0));
        let load = |loads: Arc<AtomicUsize>| {
            move || async move {
                let n = loads.fetch_add(1, Ordering::SeqCst) + 1;
                Ok::<_, String>(Some(format!("v{n}")))
            }
        };
        let first = cache.get_or_refresh("k", load(loads.clone())).await;
        assert_eq!(first, Ok(Some(Arc::new("v1".to_owned()))));
        let served = cache.get_or_refresh("k", load(loads.clone())).await;
        assert_eq!(served, Ok(Some(Arc::new("v1".to_owned()))));
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.cached("k").await.as_deref() != Some(&"v2".to_owned()) {
            assert!(Instant::now() < deadline, "the entry was not refreshed");
            tokio::task::yield_now().await;
        }
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }
}
