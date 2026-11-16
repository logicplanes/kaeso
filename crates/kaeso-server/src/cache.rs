use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};

use futures::FutureExt;
use futures::future::BoxFuture;
use moka::future::Cache;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, OwnedMutexGuard};

use kaeso_crypto::{KeyRing, MacKey};
use kaeso_store::valkey::{Invalidation, Valkey};

pub const TTL: Duration = Duration::from_secs(30);

pub const UNSUBSCRIBED_TTL: Duration = Duration::from_secs(2);

pub const REFRESH_AFTER: Duration = Duration::from_secs(20);

// An entry a caller asked for within this long is loaded again before it runs out, even when
// nobody asks for it meanwhile: an app that pauses between requests, as a person reading an
// agent's answer does, then finds its key and its project's settings in memory. An entry left
// alone for longer runs out as before, and every entry is still never older than `TTL`.
pub const KEEP_WARM: Duration = Duration::from_secs(15 * 60);

// How often the entries are looked over for a load ahead of time. Shorter than what is left of
// an entry when it becomes due, `TTL - REFRESH_AFTER`, so none runs out between two looks.
pub const WARM_EVERY: Duration = Duration::from_secs(5);

// The most loads ahead of time one cache runs at once, so they never take all of the database
// connections that requests wait for.
const WARM_AT_ONCE: usize = 4;

const KEY_BLOCK_TTL: Duration = Duration::from_secs(3600);
// A player asks for a finished video in many ranges while it plays and seeks; the job's files
// are taken from the provider's answer to the first of them for this long.
const FINISHED_VIDEO_TTL: Duration = Duration::from_secs(60);

const FLIGHT_IDLE: Duration = Duration::from_secs(60);

pub const TOUCH_EVERY: Duration = Duration::from_secs(60);
// An upstream MCP server's version and, for older ones, its session; a session the server has
// ended is rebuilt on its 404 before this runs out.
const HANDSHAKE_TTL: Duration = Duration::from_secs(600);

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
pub struct Flights(Cache<String, Arc<Mutex<()>>>);

impl std::fmt::Debug for Flights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flights").finish_non_exhaustive()
    }
}

pub struct Flight {
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

    pub async fn board(&self, key: &str) -> Flight {
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

type Reload<V> = Arc<dyn Fn() -> BoxFuture<'static, Result<Option<V>, String>> + Send + Sync>;

// A day before the first use, so a use can be dated back, as the tests do.
static EPOCH: LazyLock<Instant> = LazyLock::new(|| {
    let now = Instant::now();
    now.checked_sub(Duration::from_secs(86_400)).unwrap_or(now)
});

fn millis_since_epoch(at: Instant) -> u64 {
    u64::try_from(at.saturating_duration_since(*EPOCH).as_millis()).unwrap_or(u64::MAX)
}

// Shared by every copy of one entry and kept across its reloads: when a caller last asked for
// it, and how to load it again ahead of time.
struct Warm<V> {
    used: AtomicU64,
    reload: OnceLock<Reload<V>>,
}

impl<V: Send + Sync + 'static> Warm<V> {
    fn new() -> Arc<Warm<V>> {
        Arc::new(Warm {
            used: AtomicU64::new(millis_since_epoch(Instant::now())),
            reload: OnceLock::new(),
        })
    }

    fn used(&self) {
        self.used
            .store(millis_since_epoch(Instant::now()), Ordering::Relaxed);
    }

    fn wanted(&self, now: Instant) -> bool {
        let idle = millis_since_epoch(now).saturating_sub(self.used.load(Ordering::Relaxed));
        u128::from(idle) <= KEEP_WARM.as_millis()
    }

    // The first loader a caller hands over is kept; every caller of one key loads the same row.
    fn reload_with<E, F, Fut>(&self, load: &F)
    where
        F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        if self.reload.get().is_some() {
            return;
        }
        let load = load.clone();
        let _ = self.reload.set(Arc::new(move || {
            let load = load.clone();
            async move { load().await.map_err(|e| e.to_string()) }.boxed()
        }));
    }

    fn reload(&self) -> Option<Reload<V>> {
        self.reload.get().cloned()
    }
}

#[derive(Clone)]
struct Held<V> {
    value: Arc<V>,
    until: Instant,
    warm: Arc<Warm<V>>,
}

const VERSION_TTL: Duration = Duration::from_secs(600);

const READ: &str = r"
return {redis.call('GET', KEYS[1]) or false, redis.call('PTTL', KEYS[1]), redis.call('GET', KEYS[2]) or ''}
";

const FILL: &str = r"
if (redis.call('GET', KEYS[2]) or '') ~= ARGV[1] then return 0 end
redis.call('SET', KEYS[1], ARGV[2], 'PX', ARGV[3])
return 1
";

const DROP: &str = r"
redis.call('DEL', KEYS[1])
redis.call('INCR', KEYS[2])
redis.call('PEXPIRE', KEYS[2], ARGV[1])
return 1
";

static READ_SCRIPT: std::sync::LazyLock<redis::Script> =
    std::sync::LazyLock::new(|| redis::Script::new(READ));
static FILL_SCRIPT: std::sync::LazyLock<redis::Script> =
    std::sync::LazyLock::new(|| redis::Script::new(FILL));
static DROP_SCRIPT: std::sync::LazyLock<redis::Script> =
    std::sync::LazyLock::new(|| redis::Script::new(DROP));

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

enum Shared<V> {
    Found(V, Duration),
    Missing(String),
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
    dropped: Arc<AtomicU64>,
    mac: Arc<MacKey>,
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
    fn new(
        name: &'static str,
        valkey: Valkey,
        subscription: Arc<Subscription>,
        mac: Arc<MacKey>,
    ) -> Self {
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
            dropped: Arc::new(AtomicU64::new(0)),
            mac,
        }
    }

    fn valkey_key(&self, key: &str) -> String {
        self.valkey.key(&format!("cache:{}:{key}", self.name))
    }

    fn version_key(&self, key: &str) -> String {
        self.valkey.key(&format!("cache:{}:{key}:v", self.name))
    }

    async fn held(&self, key: &str, seen: Seen) -> Option<Held<V>> {
        if seen.subscribed
            && let Some(h) = self.memory.get(key).await
        {
            if h.until > Instant::now() {
                h.warm.used();
                return Some(h);
            }
            self.memory.invalidate(key).await;
        }
        self.fallback.get(key).await.map(|value| {
            let now = Instant::now();
            Held {
                value,
                until: now + UNSUBSCRIBED_TTL,
                warm: Warm::new(),
            }
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
        Box::pin(self.load_once(key, seen, load)).await
    }

    // Like `get_or_load`, and the entry is loaded again ahead of time: in the background once a
    // caller finds it in its last third, and by `keep_warm` while callers keep asking for it.
    pub async fn get_or_refresh<E, F, Fut>(&self, key: &str, load: F) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let seen = self.subscription.seen();
        if let Some(h) = self.held(key, seen).await {
            h.warm.reload_with(&load);
            self.refresh_if_due(key, seen, &h, load).await;
            return Ok(Some(h.value));
        }
        let reload = load.clone();
        let loaded = Box::pin(self.load_once(key, seen, load)).await?;
        self.keep_warm_with(key, &reload).await;
        Ok(loaded)
    }

    // Like `cached`, for a caller that loads a missing entry its own way but keeps a hot one fresh.
    pub async fn cached_or_refresh<E, F, Fut>(&self, key: &str, load: F) -> Option<Arc<V>>
    where
        F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let seen = self.subscription.seen();
        let h = self.held(key, seen).await?;
        h.warm.reload_with(&load);
        self.refresh_if_due(key, seen, &h, load).await;
        Some(h.value)
    }

    // Hands an entry loaded its own way, such as with a check before the load, the loader that
    // keeps it warm.
    pub async fn keep_warm_with<E, F, Fut>(&self, key: &str, load: &F)
    where
        F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        if let Some(h) = self.memory.get(key).await {
            h.warm.reload_with(load);
        }
    }

    // Loads again, before they run out, the entries callers asked for within `KEEP_WARM`.
    // Returns how many it loaded.
    async fn keep_warm(&self) -> usize {
        let seen = self.subscription.seen();
        if !seen.subscribed {
            return 0;
        }
        let now = Instant::now();
        let due: Vec<(String, Arc<Warm<V>>, Reload<V>)> = self
            .memory
            .iter()
            .filter(|(_, h)| {
                h.until > now
                    && h.until.saturating_duration_since(now) <= self.fresh_enough()
                    && h.warm.wanted(now)
            })
            .filter_map(|(key, h)| {
                let reload = h.warm.reload()?;
                Some((key.as_ref().clone(), h.warm, reload))
            })
            .collect();
        let mut loaded = 0;
        let mut due = due.into_iter();
        loop {
            let mut running = Vec::with_capacity(WARM_AT_ONCE);
            for (key, warm, reload) in due.by_ref().take(WARM_AT_ONCE) {
                running.push(self.warm_one(key, seen, warm, reload));
            }
            if running.is_empty() {
                return loaded;
            }
            loaded += futures::future::join_all(running)
                .await
                .into_iter()
                .sum::<usize>();
        }
    }

    async fn warm_one(
        &self,
        key: String,
        seen: Seen,
        warm: Arc<Warm<V>>,
        reload: Reload<V>,
    ) -> usize {
        let Ok(guard) = self.flights.lane(&key).await.try_lock_owned() else {
            return 0;
        };
        self.reload(key, seen, || reload(), guard, warm).await;
        1
    }

    fn fresh_enough(&self) -> Duration {
        TTL.saturating_sub(self.refresh_after)
    }

    // Due by what is left of the entry, not its age: one taken from Valkey keeps only Valkey's
    // remaining time and would otherwise expire before it is ever refreshed.
    async fn refresh_if_due<E, F, Fut>(&self, key: &str, seen: Seen, h: &Held<V>, load: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        if seen.subscribed
            && h.until.saturating_duration_since(Instant::now()) <= self.fresh_enough()
            && let Ok(guard) = self.flights.lane(key).await.try_lock_owned()
        {
            let cache = self.clone();
            let (key, warm) = (key.to_owned(), h.warm.clone());
            tokio::spawn(async move { cache.reload(key, seen, load, guard, warm).await });
        }
    }

    // Boxed at its callers: a hit, the hot path, then carries none of a load's state in its future.
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
        let dropped = self.dropped.load(Ordering::Acquire);
        let mut version = None;
        if seen.subscribed {
            match self.valkey_get(key).await {
                Some(Shared::Found(v, left)) => {
                    let v = Arc::new(v);
                    self.remember(key, v.clone(), seen, left).await;
                    return Ok(Some(v));
                }
                Some(Shared::Missing(v)) => version = Some(v),
                None => {}
            }
        }
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        if self.dropped.load(Ordering::Acquire) != dropped {
            return Ok(Some(value));
        }
        let kept = match &version {
            Some(v) if self.subscription.still(seen) => self.valkey_fill(key, &value, v).await,
            _ => true,
        };
        if kept {
            self.remember(key, value.clone(), seen, TTL).await;
        }
        drop(flight);
        Ok(Some(value))
    }

    // The entry keeps its `Warm`, so a reload neither counts as a use nor loses its loader.
    async fn reload<E, F, Fut>(
        &self,
        key: String,
        seen: Seen,
        load: F,
        guard: OwnedMutexGuard<()>,
        warm: Arc<Warm<V>>,
    ) where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
        E: std::fmt::Display,
    {
        let dropped = self.dropped.load(Ordering::Acquire);
        // Another instance may have refreshed the shared entry already; then it is enough.
        let version = match self.valkey_get(&key).await {
            Some(Shared::Found(value, left)) if left > self.fresh_enough() => {
                if self.dropped.load(Ordering::Acquire) == dropped {
                    self.keep(&key, Arc::new(value), seen, left, warm).await;
                }
                drop(guard);
                return;
            }
            Some(Shared::Missing(version)) => Some(version),
            Some(Shared::Found(..)) => self.version(&key).await,
            None => None,
        };
        let loaded = load().await;
        let undisturbed =
            self.subscription.still(seen) && self.dropped.load(Ordering::Acquire) == dropped;
        match loaded {
            Ok(Some(value)) if undisturbed => {
                let value = Arc::new(value);
                let kept = match version {
                    Some(v) => self.valkey_fill(&key, &value, &v).await,
                    None => true,
                };
                if kept {
                    self.keep(&key, value, seen, TTL, warm).await;
                }
            }
            Ok(Some(_)) => {}
            Ok(None) if undisturbed => self.drop_local(&key).await,
            Ok(None) => {}
            Err(e) => {
                tracing::debug!(cache = self.name, error = %e, "cannot refresh a cache entry ahead of time; it expires as usual");
            }
        }
        drop(guard);
    }

    async fn remember(&self, key: &str, value: Arc<V>, seen: Seen, left: Duration) {
        self.keep(key, value, seen, left, Warm::new()).await;
    }

    async fn keep(&self, key: &str, value: Arc<V>, seen: Seen, left: Duration, warm: Arc<Warm<V>>) {
        if self.subscription.still(seen) {
            let now = Instant::now();
            self.memory
                .insert(
                    key.to_owned(),
                    Held {
                        value,
                        until: now + left.min(TTL),
                        warm,
                    },
                )
                .await;
        } else {
            self.fallback.insert(key.to_owned(), value).await;
        }
    }

    pub async fn invalidate(&self, key: &str) {
        self.drop_local(key).await;
        let mut conn = self.valkey.conn();
        let deleted: Result<i64, _> = DROP_SCRIPT
            .key(self.valkey_key(key))
            .key(self.version_key(key))
            .arg(millis(VERSION_TTL))
            .invoke_async(&mut conn)
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
        self.dropped.fetch_add(1, Ordering::AcqRel);
        self.memory.invalidate(key).await;
        self.fallback.invalidate(key).await;
    }

    fn clear_subscribed(&self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
        self.memory.invalidate_all();
    }

    fn clear_local(&self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
        self.memory.invalidate_all();
        self.fallback.invalidate_all();
    }

    fn sealed(&self, key: &str, value: &V) -> Option<Vec<u8>> {
        let json = serde_json::to_vec(value).ok()?;
        let tag = self.mac.tag(&[self.valkey_key(key).as_bytes(), &json]);
        let mut bytes = Vec::with_capacity(tag.len() + json.len());
        bytes.extend_from_slice(&tag);
        bytes.extend_from_slice(&json);
        Some(bytes)
    }

    fn opened(&self, key: &str, bytes: &[u8]) -> Option<V> {
        let (tag, json) = bytes.split_at_checked(MacKey::TAG_LEN)?;
        if !self
            .mac
            .verify(&[self.valkey_key(key).as_bytes(), json], tag)
        {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "cache_mac")
                .increment(1);
            tracing::warn!(
                cache = self.name,
                "a shared cache entry is not signed by this install's master key; loading it from PostgreSQL"
            );
            return None;
        }
        serde_json::from_slice(json)
            .inspect_err(|e| {
                tracing::warn!(cache = self.name, error = %e, "a shared cache entry does not parse; loading it from PostgreSQL");
            })
            .ok()
    }

    async fn valkey_get(&self, key: &str) -> Option<Shared<V>> {
        let mut conn = self.valkey.bulk();
        let (raw, left, version): (Option<Vec<u8>>, i64, String) = READ_SCRIPT
            .key(self.valkey_key(key))
            .key(self.version_key(key))
            .invoke_async(&mut conn)
            .await
            .inspect_err(|e| {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "cache_get")
                    .increment(1);
                tracing::debug!(cache = self.name, error = %e, "cannot read a shared cache entry; loading it from PostgreSQL");
            })
            .ok()?;
        let left = u64::try_from(left).ok().map(Duration::from_millis);
        Some(
            match (raw.and_then(|bytes| self.opened(key, &bytes)), left) {
                (Some(value), Some(left)) => Shared::Found(value, left),
                _ => Shared::Missing(version),
            },
        )
    }

    async fn version(&self, key: &str) -> Option<String> {
        let mut conn = self.valkey.bulk();
        let version: Result<Option<String>, _> = redis::cmd("GET")
            .arg(self.version_key(key))
            .query_async(&mut conn)
            .await;
        version.ok().map(Option::unwrap_or_default)
    }

    async fn valkey_fill(&self, key: &str, value: &V, version: &str) -> bool {
        let Some(bytes) = self.sealed(key, value) else {
            return false;
        };
        let mut conn = self.valkey.bulk();
        let filled: Result<i64, _> = FILL_SCRIPT
            .key(self.valkey_key(key))
            .key(self.version_key(key))
            .arg(version)
            .arg(bytes)
            .arg(millis(TTL))
            .invoke_async(&mut conn)
            .await;
        match filled {
            Ok(n) => n == 1,
            Err(_) => {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "cache_set")
                    .increment(1);
                false
            }
        }
    }
}

pub struct MemoryCache<V: Send + Sync + 'static> {
    name: &'static str,
    memory: Cache<String, Loaded<V>>,
    fallback: Cache<String, Arc<V>>,
    flights: Flights,
    valkey: Valkey,
    subscription: Arc<Subscription>,
    refresh_after: Duration,
    dropped: Arc<AtomicU64>,
}

struct Loaded<V> {
    value: Arc<V>,
    at: Instant,
    warm: Arc<Warm<V>>,
}

impl<V> Clone for Loaded<V> {
    fn clone(&self) -> Self {
        Loaded {
            value: self.value.clone(),
            at: self.at,
            warm: self.warm.clone(),
        }
    }
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
            refresh_after: self.refresh_after,
            dropped: self.dropped.clone(),
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
            refresh_after: REFRESH_AFTER,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn loaded(&self, key: &str, seen: Seen) -> Option<Loaded<V>> {
        if !seen.subscribed {
            return None;
        }
        let l = self.memory.get(key).await?;
        l.warm.used();
        Some(l)
    }

    async fn held(&self, key: &str, seen: Seen) -> Option<Arc<V>> {
        match self.loaded(key, seen).await {
            Some(l) => Some(l.value),
            None => self.fallback.get(key).await,
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
        let seen = self.subscription.seen();
        if let Some(v) = self.held(&key, seen).await {
            return Ok(Some(v));
        }
        Box::pin(self.load_once(key, seen, load)).await
    }

    // Like `get_or_load`, and an entry in its last third is loaded again in the background,
    // so a caller that keeps using it never waits for the load.
    pub async fn get_or_refresh<E, F, Fut>(
        &self,
        group: &str,
        rest: &str,
        load: F,
    ) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<V>, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let key = format!("{group}:{rest}");
        let seen = self.subscription.seen();
        if let Some(l) = self.loaded(&key, seen).await {
            l.warm.reload_with(&load);
            if l.at.elapsed() >= self.refresh_after
                && let Ok(guard) = self.flights.lane(&key).await.try_lock_owned()
            {
                let cache = self.clone();
                tokio::spawn(async move { cache.reload(key, seen, load, guard, l.warm).await });
            }
            return Ok(Some(l.value));
        }
        if let Some(v) = self.fallback.get(&key).await {
            return Ok(Some(v));
        }
        let reload = load.clone();
        let loaded = Box::pin(self.load_once(key.clone(), seen, load)).await?;
        if let Some(l) = self.memory.get(&key).await {
            l.warm.reload_with(&reload);
        }
        Ok(loaded)
    }

    // The entry keeps its `Warm`, so a reload neither counts as a use nor loses its loader.
    async fn reload<E, F, Fut>(
        &self,
        key: String,
        seen: Seen,
        load: F,
        guard: OwnedMutexGuard<()>,
        warm: Arc<Warm<V>>,
    ) where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
        E: std::fmt::Display,
    {
        let dropped = self.dropped.load(Ordering::Acquire);
        let loaded = load().await;
        let undisturbed =
            self.subscription.still(seen) && self.dropped.load(Ordering::Acquire) == dropped;
        match loaded {
            Ok(Some(value)) if undisturbed => self.keep(key, Arc::new(value), warm).await,
            Ok(None) if undisturbed => self.memory.invalidate(&key).await,
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(cache = self.name, error = %e, "cannot refresh a cache entry ahead of time; it expires as usual");
            }
        }
        drop(guard);
    }

    // Loads again, before they run out, the entries callers asked for within `KEEP_WARM`.
    // Returns how many it loaded.
    async fn keep_warm(&self) -> usize {
        let seen = self.subscription.seen();
        if !seen.subscribed {
            return 0;
        }
        let now = Instant::now();
        let due: Vec<(String, Arc<Warm<V>>, Reload<V>)> = self
            .memory
            .iter()
            .filter(|(_, l)| {
                now.saturating_duration_since(l.at) >= self.refresh_after && l.warm.wanted(now)
            })
            .filter_map(|(key, l)| {
                let reload = l.warm.reload()?;
                Some((key.as_ref().clone(), l.warm, reload))
            })
            .collect();
        let mut loaded = 0;
        let mut due = due.into_iter();
        loop {
            let mut running = Vec::with_capacity(WARM_AT_ONCE);
            for (key, warm, reload) in due.by_ref().take(WARM_AT_ONCE) {
                running.push(self.warm_one(key, seen, warm, reload));
            }
            if running.is_empty() {
                return loaded;
            }
            loaded += futures::future::join_all(running)
                .await
                .into_iter()
                .sum::<usize>();
        }
    }

    async fn warm_one(
        &self,
        key: String,
        seen: Seen,
        warm: Arc<Warm<V>>,
        reload: Reload<V>,
    ) -> usize {
        let Ok(guard) = self.flights.lane(&key).await.try_lock_owned() else {
            return 0;
        };
        self.reload(key, seen, || reload(), guard, warm).await;
        1
    }

    async fn remember(&self, key: String, value: Arc<V>) {
        self.keep(key, value, Warm::new()).await;
    }

    async fn keep(&self, key: String, value: Arc<V>, warm: Arc<Warm<V>>) {
        let at = Instant::now();
        self.memory.insert(key, Loaded { value, at, warm }).await;
    }

    async fn load_once<E, F, Fut>(
        &self,
        key: String,
        seen: Seen,
        load: F,
    ) -> Result<Option<Arc<V>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        let mut flight = Some(self.flights.board(&key).await);
        if flight.as_ref().is_some_and(|f| f.waited) {
            if let Some(v) = self.held(&key, self.subscription.seen()).await {
                return Ok(Some(v));
            }
            flight = None;
        }
        let dropped = self.dropped.load(Ordering::Acquire);
        let Some(value) = load().await? else {
            return Ok(None);
        };
        let value = Arc::new(value);
        // An invalidation during the load may have dropped what it read, such as a
        // credential replaced meanwhile: serve it to this caller only.
        if self.dropped.load(Ordering::Acquire) != dropped {
            return Ok(Some(value));
        }
        if self.subscription.still(seen) {
            self.remember(key, value.clone()).await;
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
        self.dropped.fetch_add(1, Ordering::AcqRel);
        let prefix = format!("{group}:");
        let own = prefix.clone();
        if self
            .memory
            .invalidate_entries_if(move |k, _| k.starts_with(&own))
            .is_err()
        {
            self.memory.invalidate_all();
        }
        if self
            .fallback
            .invalidate_entries_if(move |k, _| k.starts_with(&prefix))
            .is_err()
        {
            self.fallback.invalidate_all();
        }
    }

    fn clear_subscribed(&self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
        self.memory.invalidate_all();
    }

    fn clear_local(&self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
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
    pub response_cache: TieredCache<crate::services::response_cache::Cached>,
    pub portal: TieredCache<crate::portal::snapshot::PortalSnapshot>,
    pub mcp_credentials: MemoryCache<crate::services::mcp_connections::Held>,
    pub guardrails: MemoryCache<crate::services::guardrails::Armed>,
    pub mcp_handshakes: Cache<String, Arc<kaeso_upstream::mcp::Handshake>>,
    pub client_documents: Cache<String, Arc<kaeso_store::pg::agents::Client>>,
    pub portal_tokens: TieredCache<crate::portal::auth::TokenAuth>,
    pub kpmr_changed: Arc<tokio::sync::Notify>,
    pub touched: Cache<uuid::Uuid, ()>,
    pub unknown_keys: Cache<String, ()>,
    pub key_blocks: Cache<String, time::OffsetDateTime>,
    pub time_zone: Cache<(), Arc<str>>,
    pub finished_videos: Cache<uuid::Uuid, Arc<kaeso_kpp::VideoJob>>,
    pub refreshes: Flights,
    subscription: Arc<Subscription>,
}

impl Caches {
    pub fn new(valkey: &Valkey, keyring: &KeyRing) -> Caches {
        let subscription = Arc::new(Subscription::default());
        let mac = Arc::new(MacKey::derive(keyring, "cache"));
        Caches {
            sessions: TieredCache::new("ses", valkey.clone(), subscription.clone(), mac.clone()),
            api_keys: TieredCache::new("key", valkey.clone(), subscription.clone(), mac.clone()),
            org_keys: TieredCache::new("opk", valkey.clone(), subscription.clone(), mac.clone()),
            credentials: MemoryCache::new("cred", valkey.clone(), subscription.clone()),
            project_models: TieredCache::new(
                "pmod",
                valkey.clone(),
                subscription.clone(),
                mac.clone(),
            ),
            project_limits: TieredCache::new(
                "plim",
                valkey.clone(),
                subscription.clone(),
                mac.clone(),
            ),
            log_settings: TieredCache::new(
                "lcfg",
                valkey.clone(),
                subscription.clone(),
                mac.clone(),
            ),
            response_cache: TieredCache::new(
                "rcfg",
                valkey.clone(),
                subscription.clone(),
                mac.clone(),
            ),
            portal: TieredCache::new("ptl", valkey.clone(), subscription.clone(), mac.clone()),
            mcp_credentials: MemoryCache::new("mcred", valkey.clone(), subscription.clone()),
            guardrails: MemoryCache::new("grd", valkey.clone(), subscription.clone()),
            mcp_handshakes: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(HANDSHAKE_TTL)
                .build(),
            client_documents: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(HANDSHAKE_TTL)
                .build(),
            portal_tokens: TieredCache::new(
                "ptok",
                valkey.clone(),
                subscription.clone(),
                mac.clone(),
            ),
            kpmr_changed: Arc::new(tokio::sync::Notify::new()),
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
            finished_videos: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(FINISHED_VIDEO_TTL)
                .build(),
            refreshes: Flights::new(10_000),
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

    // One look over every cache that loads its entries ahead of time; see `KEEP_WARM`.
    pub async fn keep_warm(&self) -> usize {
        let (a, b, c, d, e, f, g, h, i, j, k, l) = tokio::join!(
            self.api_keys.keep_warm(),
            self.org_keys.keep_warm(),
            self.credentials.keep_warm(),
            self.project_models.keep_warm(),
            self.project_limits.keep_warm(),
            self.log_settings.keep_warm(),
            self.response_cache.keep_warm(),
            self.sessions.keep_warm(),
            self.portal.keep_warm(),
            self.portal_tokens.keep_warm(),
            self.mcp_credentials.keep_warm(),
            self.guardrails.keep_warm(),
        );
        a + b + c + d + e + f + g + h + i + j + k + l
    }

    pub fn spawn_keep_warm(&self, shutdown: tokio_util::sync::CancellationToken) {
        let caches = self.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(WARM_EVERY);
            every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = shutdown.cancelled() => return,
                    _ = every.tick() => {}
                }
                let loaded = caches.keep_warm().await;
                if loaded > 0 {
                    tracing::debug!(loaded, "loaded cache entries again before they ran out");
                }
            }
        });
    }

    async fn apply(&self, event: Invalidation) {
        match event {
            Invalidation::Connected => {
                self.clear_subscribed();
                self.subscription.set(true);
                self.kpmr_changed.notify_one();
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
                Some(("rcfg", key)) => self.response_cache.drop_local(key).await,
                Some(("ptl", key)) => self.portal.drop_local(key).await,
                Some(("ptok", key)) => self.portal_tokens.drop_local(key).await,
                Some(("mcred", group)) => self.mcp_credentials.drop_group(group),
                Some(("grd", group)) => self.guardrails.drop_group(group),
                Some(("kpmr", _)) => self.kpmr_changed.notify_one(),
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
        self.response_cache.clear_subscribed();
        self.portal.clear_subscribed();
        self.portal_tokens.clear_subscribed();
        self.mcp_credentials.clear_subscribed();
        self.guardrails.clear_subscribed();
        self.mcp_handshakes.invalidate_all();
    }

    fn clear_local(&self) {
        self.sessions.clear_local();
        self.api_keys.clear_local();
        self.org_keys.clear_local();
        self.credentials.clear_local();
        self.project_models.clear_local();
        self.project_limits.clear_local();
        self.log_settings.clear_local();
        self.response_cache.clear_local();
        self.portal.clear_local();
        self.portal_tokens.clear_local();
        self.mcp_credentials.clear_local();
        self.guardrails.clear_local();
        self.mcp_handshakes.invalidate_all();
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
        Valkey::new(
            &SecretString::from("redis://127.0.0.1:1"),
            "kaeso-test:",
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"))
    }

    fn ring() -> KeyRing {
        KeyRing::parse(&kaeso_crypto::generate_master_key()).unwrap_or_else(|e| panic!("{e}"))
    }

    fn cache(subscription: &Arc<Subscription>) -> TieredCache<String> {
        TieredCache::new(
            "test",
            unreachable_valkey(),
            subscription.clone(),
            Arc::new(MacKey::derive(&ring(), "cache")),
        )
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
        let caches = Caches::new(&valkey, &ring());
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

    #[tokio::test]
    async fn an_entry_with_little_time_left_is_refreshed_before_it_expires() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = cache(&subscription);
        let seen = subscription.seen();
        let shared = Arc::new("read from valkey".to_owned());
        cache
            .remember("k", shared, seen, Duration::from_secs(5))
            .await;
        let served = cache
            .get_or_refresh("k", || async { Ok::<_, String>(Some("v1".to_owned())) })
            .await;
        assert_eq!(served, Ok(Some(Arc::new("read from valkey".to_owned()))));
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.cached("k").await.as_deref() != Some(&"v1".to_owned()) {
            assert!(Instant::now() < deadline, "the entry was not refreshed");
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn a_refresh_takes_the_entry_another_instance_already_refreshed() {
        let services = kaeso_testkit::services::TestServices::start().await;
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mac = Arc::new(MacKey::derive(&ring(), "cache"));
        let here = TieredCache::<String>::new(
            "test",
            services.valkey.clone(),
            subscription.clone(),
            mac.clone(),
        );
        let there =
            TieredCache::<String>::new("test", services.valkey.clone(), subscription.clone(), mac);
        let fresh = there
            .get_or_load("k", || async { Ok::<_, String>(Some("fresh".to_owned())) })
            .await;
        assert_eq!(fresh, Ok(Some(Arc::new("fresh".to_owned()))));
        let seen = subscription.seen();
        here.remember(
            "k",
            Arc::new("old".to_owned()),
            seen,
            Duration::from_secs(1),
        )
        .await;
        let loads = Arc::new(AtomicUsize::new(0));
        let load = {
            let loads = loads.clone();
            move || async move {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok::<_, String>(Some("from postgresql".to_owned()))
            }
        };
        let served = here.get_or_refresh("k", load).await;
        assert_eq!(served, Ok(Some(Arc::new("old".to_owned()))));
        let deadline = Instant::now() + Duration::from_secs(10);
        while here.cached("k").await.as_deref() != Some(&"fresh".to_owned()) {
            assert!(Instant::now() < deadline, "the shared entry was not taken");
            tokio::task::yield_now().await;
        }
        assert_eq!(
            loads.load(Ordering::SeqCst),
            0,
            "nothing was read from PostgreSQL"
        );
    }

    #[tokio::test]
    async fn a_load_that_overlaps_a_group_invalidation_is_not_kept() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache: MemoryCache<String> =
            MemoryCache::new("test", unreachable_valkey(), subscription.clone());
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let loading = {
            let (cache, started, release) = (cache.clone(), started.clone(), release.clone());
            tokio::spawn(async move {
                cache
                    .get_or_load("org", "p:openai", || async move {
                        started.notify_one();
                        release.notified().await;
                        Ok::<_, String>(Some("read before the change".to_owned()))
                    })
                    .await
            })
        };
        started.notified().await;
        cache.drop_group("org");
        release.notify_one();
        let served = loading.await.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            served,
            Ok(Some(Arc::new("read before the change".to_owned())))
        );
        let next = cache
            .get_or_load("org", "p:openai", || async {
                Ok::<_, String>(Some("current".to_owned()))
            })
            .await;
        assert_eq!(next, Ok(Some(Arc::new("current".to_owned()))));
    }

    fn memory_cache(subscription: &Arc<Subscription>) -> MemoryCache<String> {
        let mut cache = MemoryCache::new("test", unreachable_valkey(), subscription.clone());
        cache.refresh_after = Duration::ZERO;
        cache
    }

    async fn current(cache: &MemoryCache<String>) -> Option<Arc<String>> {
        cache.held("org:p:openai", cache.subscription.seen()).await
    }

    #[tokio::test]
    async fn an_old_memory_entry_is_served_while_it_is_loaded_again_in_the_background() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = memory_cache(&subscription);
        let loads = Arc::new(AtomicUsize::new(0));
        let load = |loads: Arc<AtomicUsize>| {
            move || async move {
                let n = loads.fetch_add(1, Ordering::SeqCst) + 1;
                Ok::<_, String>(Some(format!("v{n}")))
            }
        };
        let first = cache
            .get_or_refresh("org", "p:openai", load(loads.clone()))
            .await;
        assert_eq!(first, Ok(Some(Arc::new("v1".to_owned()))));
        let served = cache
            .get_or_refresh("org", "p:openai", load(loads.clone()))
            .await;
        assert_eq!(served, Ok(Some(Arc::new("v1".to_owned()))));
        let deadline = Instant::now() + Duration::from_secs(10);
        while current(&cache).await.as_deref() != Some(&"v2".to_owned()) {
            assert!(Instant::now() < deadline, "the entry was not loaded again");
            tokio::task::yield_now().await;
        }
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_fresh_memory_entry_is_not_loaded_again() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache: MemoryCache<String> =
            MemoryCache::new("test", unreachable_valkey(), subscription.clone());
        let loads = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let loads = loads.clone();
            let served = cache
                .get_or_refresh("org", "p:openai", move || async move {
                    loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, String>(Some("v1".to_owned()))
                })
                .await;
            assert_eq!(served, Ok(Some(Arc::new("v1".to_owned()))));
        }
        assert!(cache.flights.lane("org:p:openai").await.try_lock().is_ok());
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_memory_refresh_that_overlaps_a_group_invalidation_is_dropped() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = memory_cache(&subscription);
        let first = cache
            .get_or_refresh("org", "p:openai", || async {
                Ok::<_, String>(Some("old".to_owned()))
            })
            .await;
        assert_eq!(first, Ok(Some(Arc::new("old".to_owned()))));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let refreshing = {
            let (started, release) = (started.clone(), release.clone());
            move || async move {
                started.notify_one();
                release.notified().await;
                Ok::<_, String>(Some("read before the change".to_owned()))
            }
        };
        let served = cache.get_or_refresh("org", "p:openai", refreshing).await;
        assert_eq!(served, Ok(Some(Arc::new("old".to_owned()))));
        started.notified().await;
        cache.drop_group("org");
        release.notify_one();
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.flights.lane("org:p:openai").await.try_lock().is_err() {
            assert!(Instant::now() < deadline, "the refresh did not finish");
            tokio::task::yield_now().await;
        }
        cache.memory.run_pending_tasks().await;
        assert_eq!(current(&cache).await, None);
    }

    #[tokio::test]
    async fn a_refresh_that_overlaps_an_invalidation_is_dropped() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let first = cache
            .get_or_refresh("k", || async { Ok::<_, String>(Some("old".to_owned())) })
            .await;
        assert_eq!(first, Ok(Some(Arc::new("old".to_owned()))));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let refreshing = {
            let (started, release) = (started.clone(), release.clone());
            move || async move {
                started.notify_one();
                release.notified().await;
                Ok::<_, String>(Some("read before the change".to_owned()))
            }
        };
        let served = cache.get_or_refresh("k", refreshing).await;
        assert_eq!(served, Ok(Some(Arc::new("old".to_owned()))));
        started.notified().await;
        cache.drop_local("k").await;
        release.notify_one();
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.flights.lane("k").await.try_lock().is_err() {
            assert!(Instant::now() < deadline, "the refresh did not finish");
            tokio::task::yield_now().await;
        }
        assert!(cache.cached("k").await.is_none());
    }

    fn counting(
        loads: &Arc<AtomicUsize>,
    ) -> impl FnOnce() -> futures::future::Ready<Result<Option<String>, String>>
    + Clone
    + Send
    + Sync
    + 'static {
        let loads = loads.clone();
        move || {
            let n = loads.fetch_add(1, Ordering::SeqCst) + 1;
            futures::future::ready(Ok(Some(format!("v{n}"))))
        }
    }

    fn idle_since(warm: &Warm<String>, ago: Duration) {
        let at = Instant::now().checked_sub(ago).unwrap_or_else(Instant::now);
        warm.used.store(millis_since_epoch(at), Ordering::Relaxed);
    }

    #[tokio::test]
    async fn an_entry_asked_for_recently_is_loaded_again_before_it_runs_out() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let loads = Arc::new(AtomicUsize::new(0));
        let first = cache.get_or_refresh("k", counting(&loads)).await;
        assert_eq!(first, Ok(Some(Arc::new("v1".to_owned()))));
        assert_eq!(cache.keep_warm().await, 1);
        assert_eq!(cache.cached("k").await.as_deref(), Some(&"v2".to_owned()));
        assert_eq!(
            cache.keep_warm().await,
            1,
            "the loader stays with the entry"
        );
        assert_eq!(cache.cached("k").await.as_deref(), Some(&"v3".to_owned()));
        assert_eq!(loads.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn an_entry_nobody_asked_for_lately_is_left_to_run_out() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let loads = Arc::new(AtomicUsize::new(0));
        let _ = cache.get_or_refresh("k", counting(&loads)).await;
        let held = cache
            .memory
            .get("k")
            .await
            .unwrap_or_else(|| panic!("kept"));
        idle_since(&held.warm, KEEP_WARM + Duration::from_secs(1));
        assert_eq!(cache.keep_warm().await, 0);
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_entry_with_time_left_is_not_loaded_ahead_of_time() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = cache(&subscription);
        let loads = Arc::new(AtomicUsize::new(0));
        let _ = cache.get_or_refresh("k", counting(&loads)).await;
        assert_eq!(cache.keep_warm().await, 0);
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_entry_loaded_without_a_loader_is_kept_warm_once_it_is_handed_one() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let loads = Arc::new(AtomicUsize::new(0));
        let load = counting(&loads);
        let _ = cache.get_or_load("k", load.clone()).await;
        assert_eq!(cache.keep_warm().await, 0, "no loader yet");
        cache.keep_warm_with("k", &load).await;
        assert_eq!(cache.keep_warm().await, 1);
        assert_eq!(cache.cached("k").await.as_deref(), Some(&"v2".to_owned()));
    }

    #[tokio::test]
    async fn nothing_is_loaded_ahead_of_time_without_the_subscription() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let mut cache = cache(&subscription);
        cache.refresh_after = Duration::ZERO;
        let loads = Arc::new(AtomicUsize::new(0));
        let _ = cache.get_or_refresh("k", counting(&loads)).await;
        subscription.set(false);
        assert_eq!(cache.keep_warm().await, 0);
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_memory_entry_asked_for_recently_is_loaded_again_before_it_runs_out() {
        let subscription = Arc::new(Subscription::default());
        subscription.set(true);
        let cache = memory_cache(&subscription);
        let loads = Arc::new(AtomicUsize::new(0));
        let first = cache
            .get_or_refresh("org", "p:openai", counting(&loads))
            .await;
        assert_eq!(first, Ok(Some(Arc::new("v1".to_owned()))));
        assert_eq!(cache.keep_warm().await, 1);
        assert_eq!(current(&cache).await.as_deref(), Some(&"v2".to_owned()));
        let loaded = cache
            .memory
            .get("org:p:openai")
            .await
            .unwrap_or_else(|| panic!("kept"));
        idle_since(&loaded.warm, KEEP_WARM + Duration::from_secs(1));
        assert_eq!(cache.keep_warm().await, 0);
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }
}
