use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use kaeso_core::catalogue::route::Failure as Reason;
use kaeso_kpp::{ErrorKind, KppError};
use kaeso_store::valkey::Valkey;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

// A refused key or an empty balance lasts until someone fixes it; an overloaded or
// unreachable model usually answers again within seconds.
const ACCOUNT: Duration = Duration::minutes(5);
const MODEL: Duration = Duration::seconds(30);
const LONGEST_RETRY_AFTER: Duration = Duration::minutes(5);
const MOST_HELD: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Model,
    Provider,
}

#[derive(Clone, Copy, Debug)]
pub enum Cause<'a> {
    Unreachable,
    KeyRefused,
    Provider(&'a KppError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    #[serde(with = "time::serde::timestamp::milliseconds")]
    pub until: OffsetDateTime,
    pub reason: Reason,
    pub status: u16,
    pub error_kind: String,
    pub error_message: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Step<'a> {
    pub project: Uuid,
    pub provider: &'a str,
    pub model: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Scope {
    Model(Uuid),
    Provider(String),
}

type Key = (Uuid, Scope);

impl Step<'_> {
    fn key(&self, reach: Reach) -> Key {
        let scope = match reach {
            Reach::Model => Scope::Model(self.model),
            Reach::Provider => Scope::Provider(self.provider.to_owned()),
        };
        (self.project, scope)
    }

    fn keys(&self) -> [Key; 2] {
        [self.key(Reach::Provider), self.key(Reach::Model)]
    }
}

fn valkey_key(valkey: &Valkey, (project, scope): &Key) -> String {
    match scope {
        Scope::Model(model) => valkey.key(&format!("cooldown:{project}:m:{model}")),
        Scope::Provider(provider) => valkey.key(&format!("cooldown:{project}:p:{provider}")),
    }
}

fn longest<'a>(found: impl Iterator<Item = &'a Held>, now: OffsetDateTime) -> Option<Held> {
    found
        .filter(|h| h.until > now)
        .max_by_key(|h| h.until)
        .cloned()
}

// What one gateway process remembers itself: the answer while Valkey can't be reached.
#[derive(Debug, Default)]
pub struct Memory {
    held: Mutex<HashMap<Key, Held>>,
}

impl Memory {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Key, Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn hold(&self, step: &Step<'_>, reach: Reach, held: Held, now: OffsetDateTime) {
        let mut map = self.lock();
        if map.len() >= MOST_HELD {
            map.retain(|_, h| h.until > now);
        }
        if map.len() < MOST_HELD {
            map.insert(step.key(reach), held);
        }
    }

    pub fn held(&self, step: &Step<'_>, now: OffsetDateTime) -> Option<Held> {
        let map = self.lock();
        longest(step.keys().iter().filter_map(|k| map.get(k)), now)
    }

    pub fn release(&self, step: &Step<'_>) {
        let mut map = self.lock();
        for key in step.keys() {
            map.remove(&key);
        }
    }

    fn snapshot(&self, keys: &[Key], now: OffsetDateTime) -> HashMap<Key, Held> {
        let map = self.lock();
        keys.iter()
            .filter_map(|k| {
                map.get(k)
                    .filter(|h| h.until > now)
                    .map(|h| (k.clone(), h.clone()))
            })
            .collect()
    }
}

// What every gateway of the install shares through Valkey, so one gateway's lesson spares the
// others the same failed call.
#[derive(Debug, Default)]
pub struct Cooldowns {
    memory: Memory,
}

#[derive(Debug, Default)]
pub struct Lookup {
    found: HashMap<Key, Held>,
}

impl Lookup {
    pub fn add(&mut self, step: &Step<'_>, reach: Reach, held: Held) {
        self.found.insert(step.key(reach), held);
    }

    pub fn held(&self, step: &Step<'_>, now: OffsetDateTime) -> Option<Held> {
        longest(step.keys().iter().filter_map(|k| self.found.get(k)), now)
    }
}

impl Cooldowns {
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub async fn lookup(&self, valkey: &Valkey, steps: &[Step<'_>], now: OffsetDateTime) -> Lookup {
        let mut keys: Vec<Key> = Vec::with_capacity(steps.len() * 2);
        for step in steps {
            for key in step.keys() {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
        if keys.is_empty() {
            return Lookup::default();
        }
        let names: Vec<String> = keys.iter().map(|k| valkey_key(valkey, k)).collect();
        let mut conn = valkey.conn();
        let read: Result<Vec<Option<String>>, redis::RedisError> =
            redis::cmd("MGET").arg(&names).query_async(&mut conn).await;
        match read {
            Ok(values) => Lookup {
                found: keys
                    .into_iter()
                    .zip(values)
                    .filter_map(|(key, value)| {
                        let held: Held = serde_json::from_str(&value?).ok()?;
                        (held.until > now).then_some((key, held))
                    })
                    .collect(),
            },
            Err(e) => {
                tracing::warn!(error = %e, "cannot read route cooldowns from Valkey; using this gateway's own");
                Lookup {
                    found: self.memory.snapshot(&keys, now),
                }
            }
        }
    }

    pub async fn hold(
        &self,
        valkey: &Valkey,
        step: &Step<'_>,
        reach: Reach,
        held: Held,
        now: OffsetDateTime,
    ) {
        self.memory.hold(step, reach, held.clone(), now);
        let left = (held.until - now).whole_milliseconds();
        let Ok(px) = u64::try_from(left) else {
            return;
        };
        let Ok(value) = serde_json::to_string(&held) else {
            return;
        };
        let mut conn = valkey.conn();
        let written: Result<(), redis::RedisError> = redis::cmd("SET")
            .arg(valkey_key(valkey, &step.key(reach)))
            .arg(value)
            .arg("PX")
            .arg(px.max(1))
            .query_async(&mut conn)
            .await;
        if let Err(e) = written {
            tracing::warn!(error = %e, "cannot share a route cooldown through Valkey; only this gateway skips the step");
        }
    }

    pub async fn release(&self, valkey: &Valkey, step: &Step<'_>) {
        self.memory.release(step);
        let names: Vec<String> = step.keys().iter().map(|k| valkey_key(valkey, k)).collect();
        let mut conn = valkey.conn();
        let deleted: Result<(), redis::RedisError> =
            redis::cmd("DEL").arg(&names).query_async(&mut conn).await;
        if let Err(e) = deleted {
            tracing::warn!(error = %e, "cannot release a route cooldown in Valkey; other gateways skip the step until it lapses");
        }
    }
}

pub fn length(cause: Cause<'_>) -> Option<(Reach, Duration)> {
    match cause {
        Cause::Unreachable => Some((Reach::Model, MODEL)),
        Cause::KeyRefused => Some((Reach::Provider, ACCOUNT)),
        Cause::Provider(err) => match err.kind {
            ErrorKind::Billing => Some((Reach::Provider, ACCOUNT)),
            ErrorKind::NotFound => Some((Reach::Model, ACCOUNT)),
            ErrorKind::RateLimited => Some((Reach::Model, retry_after(err))),
            ErrorKind::Overloaded | ErrorKind::ProviderError | ErrorKind::Timeout => {
                Some((Reach::Model, MODEL))
            }
            _ => None,
        },
    }
}

fn retry_after(err: &KppError) -> Duration {
    err.retry_after
        .and_then(|d| Duration::try_from(d).ok())
        .map_or(MODEL, |d| d.clamp(Duration::SECOND, LONGEST_RETRY_AFTER))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: Uuid = Uuid::from_u128(1);
    const FABLE: Uuid = Uuid::from_u128(2);
    const OPUS: Uuid = Uuid::from_u128(3);

    fn at(model: Uuid) -> Step<'static> {
        Step {
            project: PROJECT,
            provider: "anthropic",
            model,
        }
    }

    fn held(until: OffsetDateTime) -> Held {
        Held {
            until,
            reason: Reason::Limited,
            status: 402,
            error_kind: "billing".into(),
            error_message: "Your credit balance is too low.".into(),
        }
    }

    fn provider(kind: ErrorKind) -> Option<(Reach, Duration)> {
        length(Cause::Provider(&KppError::new(kind, "x")))
    }

    #[test]
    fn an_empty_balance_holds_every_model_of_the_provider_until_it_lapses() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let m = Memory::default();
        let (reach, long) = provider(ErrorKind::Billing).unwrap();
        m.hold(&at(FABLE), reach, held(now + long), now);
        assert_eq!(m.held(&at(OPUS), now).unwrap().status, 402);
        let other = Step {
            provider: "openai",
            ..at(OPUS)
        };
        assert_eq!(m.held(&other, now), None);
        assert_eq!(m.held(&at(OPUS), now + long), None);
    }

    #[test]
    fn a_model_that_answers_is_released_at_once() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let m = Memory::default();
        m.hold(&at(FABLE), Reach::Model, held(now + MODEL), now);
        assert!(m.held(&at(FABLE), now).is_some());
        assert_eq!(m.held(&at(OPUS), now), None);
        m.release(&at(FABLE));
        assert_eq!(m.held(&at(FABLE), now), None);
    }

    #[test]
    fn lasting_failures_hold_longer_and_refusals_of_the_request_never_hold() {
        assert_eq!(length(Cause::KeyRefused), Some((Reach::Provider, ACCOUNT)));
        assert_eq!(length(Cause::Unreachable), Some((Reach::Model, MODEL)));
        assert_eq!(provider(ErrorKind::NotFound), Some((Reach::Model, ACCOUNT)));
        for kind in [
            ErrorKind::Overloaded,
            ErrorKind::ProviderError,
            ErrorKind::Timeout,
        ] {
            assert_eq!(provider(kind), Some((Reach::Model, MODEL)));
        }
        for kind in [
            ErrorKind::InvalidRequest,
            ErrorKind::ContextTooLong,
            ErrorKind::ContentFiltered,
            ErrorKind::RequestTooLarge,
            ErrorKind::unsupported("images"),
        ] {
            assert_eq!(provider(kind), None);
        }
    }

    #[test]
    fn a_rate_limit_holds_as_long_as_retry_after_says_within_bounds() {
        let limited = |after: Option<u64>| {
            let mut e = KppError::new(ErrorKind::RateLimited, "slow down");
            e.retry_after = after.map(std::time::Duration::from_secs);
            length(Cause::Provider(&e)).unwrap()
        };
        assert_eq!(limited(None), (Reach::Model, MODEL));
        assert_eq!(limited(Some(20)), (Reach::Model, Duration::seconds(20)));
        assert_eq!(limited(Some(0)), (Reach::Model, Duration::SECOND));
        assert_eq!(limited(Some(86_400)), (Reach::Model, LONGEST_RETRY_AFTER));
    }

    #[test]
    fn a_full_memory_makes_room_from_lapsed_entries() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let m = Memory::default();
        for n in 0..MOST_HELD {
            let step = at(Uuid::from_u128(100 + n as u128));
            m.hold(&step, Reach::Model, held(now + MODEL), now);
        }
        let later = now + MODEL;
        m.hold(&at(FABLE), Reach::Model, held(later + MODEL), later);
        assert!(m.held(&at(FABLE), later).is_some());
    }

    #[test]
    fn a_held_step_round_trips_through_json_with_its_time_in_milliseconds() {
        let h = held(OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(1_500));
        let text = serde_json::to_string(&h).unwrap();
        assert!(text.contains("\"until\":1500"), "{text}");
        assert_eq!(serde_json::from_str::<Held>(&text).unwrap(), h);
    }
}
