use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::http::{HeaderMap, HeaderValue, header};
use kaeso_codecs::EncodedRequest;
use kaeso_crypto::{MacKey, Sealed};
use kaeso_kpp::{DialectId, Reply, RequestKind, Usage};
use kaeso_store::pg::response_cache::{self as store, CacheSettings};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::cache::hex;
use crate::state::AppState;

pub const HEADER: &str = "kaeso-cache";

// Bump when `Stored` changes shape: older entries are then never looked up and expire by TTL.
const ENTRY_VERSION: &str = "v1";

const MEMORY_EVERY: Duration = Duration::from_secs(10);

const CLEAR_BATCH: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Hit,
    Miss,
    Bypass,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Hit => "hit",
            Verdict::Miss => "miss",
            Verdict::Bypass => "bypass",
        }
    }

    pub fn header(self) -> HeaderValue {
        HeaderValue::from_static(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directive {
    pub read: bool,
    pub write: bool,
}

pub fn directive(headers: &HeaderMap) -> Directive {
    let mut d = Directive {
        read: true,
        write: true,
    };
    for value in headers.get_all(header::CACHE_CONTROL) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for token in value.split(',') {
            let name = token.split('=').next().unwrap_or_default().trim();
            if name.eq_ignore_ascii_case("no-store") {
                d.read = false;
                d.write = false;
            } else if name.eq_ignore_ascii_case("no-cache") {
                d.read = false;
            }
        }
    }
    d
}

#[derive(Debug)]
pub struct Shared {
    mac: MacKey,
    room: AtomicBool,
}

impl Shared {
    pub fn new(keyring: &kaeso_crypto::KeyRing) -> Shared {
        Shared {
            mac: MacKey::derive(keyring, "response-cache"),
            room: AtomicBool::new(true),
        }
    }

    pub fn has_room(&self) -> bool {
        self.room.load(Ordering::Acquire)
    }

    pub fn set_room(&self, room: bool) {
        self.room.store(room, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Cached {
    org_allows: bool,
    enabled: bool,
    ttl_seconds: i32,
    generation: i64,
}

impl From<CacheSettings> for Cached {
    fn from(s: CacheSettings) -> Self {
        Cached {
            org_allows: s.org_allows,
            enabled: s.enabled,
            ttl_seconds: s.ttl_seconds,
            generation: s.generation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Active {
    pub ttl: Duration,
    pub generation: i64,
}

pub async fn active(state: &AppState, project: Uuid) -> Option<Active> {
    if !state.config.response_cache.enabled {
        return None;
    }
    let pg = state.pg.clone();
    let found = state
        .caches
        .response_cache
        .get_or_load(&project.to_string(), || async move {
            store::settings(&pg, project)
                .await
                .map(|s| s.map(Cached::from))
        })
        .await;
    let c = match found {
        Ok(Some(c)) => *c,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!(error = %e, "cannot load the response cache settings; the request skips the cache");
            return None;
        }
    };
    (c.org_allows && c.enabled).then(|| Active {
        ttl: Duration::from_secs(u64::try_from(c.ttl_seconds).unwrap_or(0)),
        generation: c.generation,
    })
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .response_cache
        .invalidate(&project.to_string())
        .await;
}

pub struct Request<'a> {
    pub org: Uuid,
    pub project: Uuid,
    pub kind: RequestKind,
    pub dialect: DialectId,
    pub provider: &'a str,
    pub entry: Uuid,
    pub model: &'a str,
    pub encoded: &'a EncodedRequest,
}

pub fn cacheable(kind: RequestKind, stream: bool) -> bool {
    !stream && matches!(kind, RequestKind::Generate | RequestKind::Embed)
}

fn kind_name(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::Generate => "generate",
        RequestKind::Embed => "embed",
        RequestKind::CountTokens => "count_tokens",
        _ => "other",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    key: String,
    ttl: Duration,
}

fn prefix(state: &AppState, project: Uuid) -> String {
    state.valkey.key(&format!("rc:{ENTRY_VERSION}:{project}:"))
}

pub fn slot(state: &AppState, active: Active, r: &Request<'_>) -> Slot {
    let mut headers: Vec<(String, &str)> = r
        .encoded
        .headers
        .iter()
        .map(|(n, v)| (n.to_ascii_lowercase(), v))
        .collect();
    headers.sort_unstable();
    let headers = headers
        .iter()
        .map(|(n, v)| format!("{n}:{v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let generation = active.generation.to_be_bytes();
    let tag = state.response_cache.mac.tag(&[
        r.org.as_bytes(),
        r.project.as_bytes(),
        &generation,
        kind_name(r.kind).as_bytes(),
        r.dialect.as_str().as_bytes(),
        r.provider.as_bytes(),
        r.entry.as_bytes(),
        r.model.as_bytes(),
        r.encoded.method.as_bytes(),
        r.encoded.path.as_bytes(),
        headers.as_bytes(),
        &r.encoded.body,
    ]);
    Slot {
        key: format!("{}{}", prefix(state, r.project), hex(&tag)),
        ttl: active.ttl,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub reply: Reply,
    pub usage: Usage,
    pub usage_estimated: bool,
    pub stored_at_ms: i64,
}

impl Stored {
    pub fn age(&self, now: time::OffsetDateTime) -> Duration {
        let now_ms = i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX);
        Duration::from_millis(u64::try_from(now_ms.saturating_sub(self.stored_at_ms)).unwrap_or(0))
    }
}

pub fn unix_ms(at: time::OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}

fn count_write(outcome: &'static str) {
    metrics::counter!(crate::metrics::RESPONSE_CACHE_WRITES, "outcome" => outcome).increment(1);
}

pub fn count_lookup(verdict: Verdict) {
    metrics::counter!(crate::metrics::RESPONSE_CACHE_LOOKUPS, "result" => verdict.as_str())
        .increment(1);
}

pub async fn get(state: &AppState, slot: &Slot) -> Option<Stored> {
    let mut conn = state.valkey.bulk();
    let fields: HashMap<String, Vec<u8>> = match redis::cmd("HGETALL")
        .arg(&slot.key)
        .query_async(&mut conn)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "response_cache_get")
                .increment(1);
            tracing::warn!(error = %e, "cannot look up the response cache; the request goes to the provider");
            return None;
        }
    };
    if fields.is_empty() {
        return None;
    }
    let stored = open(state, slot, fields);
    if stored.is_none() {
        tracing::warn!("a response cache entry cannot be read; the request goes to the provider");
    }
    stored
}

fn open(state: &AppState, slot: &Slot, mut fields: HashMap<String, Vec<u8>>) -> Option<Stored> {
    let sealed = Sealed {
        key_id: String::from_utf8(fields.remove("kid")?).ok()?,
        nonce: fields.remove("nonce")?,
        wrapped_key: fields.remove("wrapped")?,
        ciphertext: fields.remove("ct")?,
    };
    let plain = kaeso_crypto::open(&state.keyring, &sealed, slot.key.as_bytes()).ok()?;
    serde_json::from_slice(secrecy::ExposeSecret::expose_secret(&plain)).ok()
}

pub fn put(state: &AppState, slot: Slot, stored: Stored) {
    if !state.response_cache.has_room() {
        count_write("full");
        return;
    }
    let task = state.clone();
    state.in_flight.finish(async move {
        let outcome = write(&task, &slot, &stored).await;
        count_write(outcome);
    });
}

async fn write(state: &AppState, slot: &Slot, stored: &Stored) -> &'static str {
    let Ok(plain) = serde_json::to_vec(stored) else {
        return "failed";
    };
    if plain.len() > state.config.response_cache.max_entry_size.as_usize() {
        return "too_large";
    }
    let sealed = match kaeso_crypto::seal(&state.keyring, &plain, slot.key.as_bytes()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "cannot encrypt a reply for the response cache");
            return "failed";
        }
    };
    let ttl = u64::try_from(slot.ttl.as_millis()).unwrap_or(u64::MAX);
    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("HSET")
        .arg(&slot.key)
        .arg("kid")
        .arg(sealed.key_id.as_bytes())
        .arg("nonce")
        .arg(sealed.nonce)
        .arg("wrapped")
        .arg(sealed.wrapped_key)
        .arg("ct")
        .arg(sealed.ciphertext)
        .ignore()
        .cmd("PEXPIRE")
        .arg(&slot.key)
        .arg(ttl)
        .ignore();
    let mut conn = state.valkey.bulk();
    let done: Result<(), _> = pipe.query_async(&mut conn).await;
    match done {
        Ok(()) => "stored",
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "response_cache_put")
                .increment(1);
            tracing::warn!(error = %e, "cannot store a reply in the response cache");
            "failed"
        }
    }
}

pub fn sweep(state: &AppState, projects: Vec<Uuid>) {
    let task = state.clone();
    tokio::spawn(async move {
        for project in projects {
            if let Err(e) = unlink(&task, project).await {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "response_cache_clear")
                    .increment(1);
                tracing::warn!(error = %e, "cannot remove cleared response cache entries; they expire with their TTL");
            }
        }
    });
}

async fn unlink(state: &AppState, project: Uuid) -> Result<(), redis::RedisError> {
    let pattern = format!("{}*", glob_escape(&prefix(state, project)));
    let mut conn = state.valkey.bulk();
    let mut cursor: u64 = 0;
    loop {
        let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(CLEAR_BATCH)
            .query_async(&mut conn)
            .await?;
        if !keys.is_empty() {
            let _: i64 = redis::cmd("UNLINK")
                .arg(&keys)
                .query_async(&mut conn)
                .await?;
        }
        if next == 0 {
            return Ok(());
        }
        cursor = next;
    }
}

fn glob_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '?' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub used: u64,
    pub max: u64,
}

pub fn memory(info: &str) -> Memory {
    let mut m = Memory::default();
    for line in info.lines() {
        let Some((name, value)) = line.trim().split_once(':') else {
            continue;
        };
        let Ok(n) = value.trim().parse::<u64>() else {
            continue;
        };
        match name {
            "used_memory" => m.used = n,
            "maxmemory" => m.max = n,
            _ => {}
        }
    }
    m
}

pub fn has_room(m: Memory, share: u8) -> bool {
    m.max == 0 || u128::from(m.used) * 100 < u128::from(m.max) * u128::from(share)
}

pub async fn check_memory(state: &AppState) {
    let mut conn = state.valkey.conn();
    let info: Result<String, _> = redis::cmd("INFO")
        .arg("memory")
        .query_async(&mut conn)
        .await;
    match info {
        Ok(info) => {
            let room = has_room(memory(&info), state.config.response_cache.memory_share);
            if room != state.response_cache.has_room() {
                if room {
                    tracing::info!("Valkey has room again; the response cache stores new replies");
                } else {
                    tracing::warn!(
                        "Valkey memory passed response_cache.memory_share; the response cache stops storing new replies"
                    );
                }
            }
            state.response_cache.set_room(room);
        }
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "response_cache_memory")
                .increment(1);
            tracing::warn!(error = %e, "cannot read Valkey's memory use for the response cache");
        }
    }
}

pub fn spawn_memory_watch(state: AppState, shutdown: CancellationToken) {
    if !state.config.response_cache.enabled {
        return;
    }
    tokio::spawn(async move {
        loop {
            check_memory(&state).await;
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(MEMORY_EVERY) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(values: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(header::CACHE_CONTROL, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn cache_control_chooses_whether_the_cache_is_read_and_written() {
        let both = Directive {
            read: true,
            write: true,
        };
        assert_eq!(directive(&HeaderMap::new()), both);
        assert_eq!(directive(&headers(&["max-age=60"])), both);
        assert_eq!(
            directive(&headers(&["no-cache"])),
            Directive {
                read: false,
                write: true
            }
        );
        let neither = Directive {
            read: false,
            write: false,
        };
        assert_eq!(directive(&headers(&["No-Store"])), neither);
        assert_eq!(directive(&headers(&["max-age=0, no-store"])), neither);
        assert_eq!(directive(&headers(&["no-cache", "no-store"])), neither);
    }

    #[test]
    fn only_non_streamed_generations_and_embeddings_are_cacheable() {
        assert!(cacheable(RequestKind::Generate, false));
        assert!(cacheable(RequestKind::Embed, false));
        assert!(!cacheable(RequestKind::Generate, true));
        assert!(!cacheable(RequestKind::CountTokens, false));
    }

    #[test]
    fn valkey_memory_is_read_from_info() {
        let info = "# Memory\r\nused_memory:1048576\r\nused_memory_human:1.00M\r\nmaxmemory:4194304\r\nmaxmemory_policy:noeviction\r\n";
        assert_eq!(
            memory(info),
            Memory {
                used: 1_048_576,
                max: 4_194_304
            }
        );
        assert_eq!(memory(""), Memory::default());
    }

    #[test]
    fn the_cache_stops_storing_above_its_share_of_maxmemory() {
        let at = |used| Memory { used, max: 1000 };
        assert!(has_room(at(499), 50));
        assert!(!has_room(at(500), 50));
        assert!(!has_room(at(2000), 90));
        assert!(has_room(Memory { used: 10, max: 0 }, 1));
        assert!(has_room(
            Memory {
                used: u64::MAX - 1,
                max: u64::MAX
            },
            100
        ));
    }

    #[test]
    fn glob_characters_in_the_prefix_match_only_themselves() {
        assert_eq!(glob_escape("kaeso:rc:"), "kaeso:rc:");
        assert_eq!(glob_escape("a*b?[c]\\"), "a\\*b\\?\\[c\\]\\\\");
    }

    #[test]
    fn an_entrys_age_counts_from_when_it_was_stored() {
        let at = time::macros::datetime!(2026-10-31 12:00:00 UTC);
        let stored = Stored {
            reply: Reply::Generate(kaeso_kpp::Response::new("id", "m", vec![])),
            usage: Usage::new(1, 1),
            usage_estimated: false,
            stored_at_ms: unix_ms(at),
        };
        assert_eq!(
            stored.age(at + time::Duration::seconds(42)),
            Duration::from_secs(42)
        );
        assert_eq!(stored.age(at - time::Duration::seconds(5)), Duration::ZERO);
    }
}
