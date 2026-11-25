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
// A plan's usage limit lasts until the plan resets, hours or days later. A wait the provider
// states holds the account that long, up to a weekly window. OpenAI's plan limit names no
// reset and asks apps not to infer one, only to pause
// (https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery), so
// without a stated wait each hit in a row holds three times as long, from `ACCOUNT` up to
// `LONGEST_PLAN_BACKOFF`: few calls to a spent plan, and a reset plan is back within hours.
const LONGEST_PLAN_LIMIT: Duration = Duration::days(7);
const LONGEST_PLAN_BACKOFF: Duration = Duration::hours(2);
const PLAN_BACKOFF_FACTOR: i32 = 3;
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
    /// An account's plan reached its usage limit: every model of the account waits.
    PlanLimit(&'a KppError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    #[serde(with = "time::serde::timestamp::milliseconds")]
    pub until: OffsetDateTime,
    pub reason: Reason,
    pub status: u16,
    pub error_kind: String,
    pub error_message: String,
    // Set on a plan limit's hold without a stated wait. The record outlives `until` by as
    // long again, so the next hit in a row finds it and holds longer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backoff_ms: Option<u64>,
}

impl Held {
    fn kept_for(&self, now: OffsetDateTime) -> Duration {
        let grace = self
            .backoff_ms
            .and_then(|ms| i64::try_from(ms).ok())
            .map_or(Duration::ZERO, Duration::milliseconds);
        self.until - now + grace
    }
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
    /// One provider key of the project's pool.
    Key(Uuid),
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
        Scope::Key(key) => valkey.key(&format!("cooldown:{project}:k:{key}")),
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
        self.hold_at(step.key(reach), held, now);
    }

    fn hold_at(&self, key: Key, held: Held, now: OffsetDateTime) {
        let mut map = self.lock();
        if map.len() >= MOST_HELD {
            map.retain(|_, h| h.until > now);
        }
        if map.len() < MOST_HELD {
            map.insert(key, held);
        }
    }

    pub fn held(&self, step: &Step<'_>, now: OffsetDateTime) -> Option<Held> {
        let map = self.lock();
        longest(step.keys().iter().filter_map(|k| map.get(k)), now)
    }

    fn get(&self, key: &Key) -> Option<Held> {
        self.lock().get(key).cloned()
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

    // A held step is skipped only for a later one that is ready, so a route with a single
    // step it can take tries that step whatever is held, and asks Valkey nothing.
    pub async fn lookup(&self, valkey: &Valkey, steps: &[Step<'_>], now: OffsetDateTime) -> Lookup {
        if steps.len() < 2 {
            return Lookup::default();
        }
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
        let left = held.kept_for(now).whole_milliseconds();
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

    /// Holds one provider key of a project's pool, so its requests draw the pool's other keys.
    pub async fn hold_key(
        &self,
        valkey: &Valkey,
        project: Uuid,
        key: Uuid,
        held: Held,
        now: OffsetDateTime,
    ) {
        let at = (project, Scope::Key(key));
        self.memory.hold_at(at.clone(), held.clone(), now);
        let left = (held.until - now).whole_milliseconds();
        let (Ok(px), Ok(value)) = (u64::try_from(left), serde_json::to_string(&held)) else {
            return;
        };
        let mut conn = valkey.conn();
        let written: Result<(), redis::RedisError> = redis::cmd("SET")
            .arg(valkey_key(valkey, &at))
            .arg(value)
            .arg("PX")
            .arg(px.max(1))
            .query_async(&mut conn)
            .await;
        if let Err(e) = written {
            tracing::warn!(error = %e, "cannot share a held provider key through Valkey; only this gateway skips it");
        }
    }

    /// Which of a pool's keys are held, in the order given.
    pub async fn held_keys(
        &self,
        valkey: &Valkey,
        project: Uuid,
        keys: &[Uuid],
        now: OffsetDateTime,
    ) -> Vec<bool> {
        let at: Vec<Key> = keys.iter().map(|k| (project, Scope::Key(*k))).collect();
        let names: Vec<String> = at.iter().map(|k| valkey_key(valkey, k)).collect();
        let mut conn = valkey.conn();
        let read: Result<Vec<Option<String>>, redis::RedisError> =
            redis::cmd("MGET").arg(&names).query_async(&mut conn).await;
        match read {
            Ok(values) => values
                .into_iter()
                .map(|v| {
                    v.and_then(|v| serde_json::from_str::<Held>(&v).ok())
                        .is_some_and(|h| h.until > now)
                })
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "cannot read held provider keys from Valkey; using this gateway's own");
                let found = self.memory.snapshot(&at, now);
                at.iter().map(|k| found.contains_key(k)).collect()
            }
        }
    }

    /// How long a reached plan limit holds the account: the wait the provider states, or the
    /// next length of the backoff when the account's last hold was one that just lapsed.
    pub async fn plan_limit(
        &self,
        valkey: &Valkey,
        step: &Step<'_>,
        err: &KppError,
        now: OffsetDateTime,
    ) -> (Duration, Option<u64>) {
        if let Some(stated) = stated(err) {
            return (stated.clamp(ACCOUNT, LONGEST_PLAN_LIMIT), None);
        }
        let key = step.key(Reach::Provider);
        let mut conn = valkey.conn();
        let read: Result<Option<String>, redis::RedisError> = redis::cmd("GET")
            .arg(valkey_key(valkey, &key))
            .query_async(&mut conn)
            .await;
        let previous = match read {
            Ok(value) => value.and_then(|v| serde_json::from_str::<Held>(&v).ok()),
            Err(e) => {
                tracing::warn!(error = %e, "cannot read an account's last plan limit from Valkey; using this gateway's own");
                self.memory.get(&key)
            }
        };
        let length = plan_backoff(previous.as_ref(), now);
        (length, u64::try_from(length.whole_milliseconds()).ok())
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
        Cause::PlanLimit(err) => Some((
            Reach::Provider,
            stated(err).map_or(ACCOUNT, |d| d.clamp(ACCOUNT, LONGEST_PLAN_LIMIT)),
        )),
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

fn stated(err: &KppError) -> Option<Duration> {
    err.retry_after.and_then(|d| Duration::try_from(d).ok())
}

fn plan_backoff(previous: Option<&Held>, now: OffsetDateTime) -> Duration {
    match previous {
        Some(p) if p.kept_for(now) >= Duration::ZERO => p
            .backoff_ms
            .and_then(|ms| i64::try_from(ms).ok())
            .map_or(ACCOUNT, |ms| {
                (Duration::milliseconds(ms) * PLAN_BACKOFF_FACTOR)
                    .clamp(ACCOUNT, LONGEST_PLAN_BACKOFF)
            }),
        _ => ACCOUNT,
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
            backoff_ms: None,
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

    #[tokio::test]
    async fn a_route_with_one_step_asks_valkey_nothing_and_one_with_two_reads_what_is_held() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let unreachable = Valkey::new(
            &secrecy::SecretString::from("redis://127.0.0.1:1"),
            "kaeso-test:",
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let cooldowns = Cooldowns::default();
        cooldowns
            .memory()
            .hold(&at(FABLE), Reach::Model, held(now + MODEL), now);
        let alone = cooldowns.lookup(&unreachable, &[at(FABLE)], now).await;
        assert_eq!(alone.held(&at(FABLE), now), None);
        let both = cooldowns
            .lookup(&unreachable, &[at(FABLE), at(OPUS)], now)
            .await;
        assert_eq!(both.held(&at(FABLE), now).map(|h| h.status), Some(402));
        assert_eq!(both.held(&at(OPUS), now), None);
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
    fn a_reached_plan_limit_holds_the_account_at_least_as_long_as_an_empty_balance() {
        let mut limited = KppError::new(ErrorKind::RateLimited, "plan limit");
        assert_eq!(
            length(Cause::PlanLimit(&limited)),
            Some((Reach::Provider, ACCOUNT))
        );
        limited.retry_after = Some(std::time::Duration::from_secs(30));
        assert_eq!(
            length(Cause::PlanLimit(&limited)),
            Some((Reach::Provider, ACCOUNT)),
            "never shorter"
        );
        limited.retry_after = Some(std::time::Duration::from_secs(1_800));
        assert_eq!(
            length(Cause::PlanLimit(&limited)),
            Some((Reach::Provider, Duration::minutes(30)))
        );
        limited.retry_after = Some(std::time::Duration::from_secs(86_400));
        assert_eq!(
            length(Cause::PlanLimit(&limited)),
            Some((Reach::Provider, Duration::days(1))),
            "a stated reset a day away holds a day"
        );
        limited.retry_after = Some(std::time::Duration::from_secs(30 * 86_400));
        assert_eq!(
            length(Cause::PlanLimit(&limited)),
            Some((Reach::Provider, LONGEST_PLAN_LIMIT))
        );
        assert_eq!(
            length(Cause::Provider(&limited)),
            Some((Reach::Model, LONGEST_RETRY_AFTER)),
            "a passing rate limit holds only its model"
        );
    }

    #[test]
    fn a_held_step_round_trips_through_json_with_its_time_in_milliseconds() {
        let h = held(OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(1_500));
        let text = serde_json::to_string(&h).unwrap();
        assert!(text.contains("\"until\":1500"), "{text}");
        assert_eq!(serde_json::from_str::<Held>(&text).unwrap(), h);
    }

    #[test]
    fn plan_limits_in_a_row_without_a_stated_wait_hold_longer_each_time() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut lengths = Vec::new();
        let mut previous: Option<Held> = None;
        let mut at = now;
        for _ in 0..6 {
            let length = plan_backoff(previous.as_ref(), at);
            lengths.push(length);
            let mut h = held(at + length);
            h.backoff_ms = u64::try_from(length.whole_milliseconds()).ok();
            previous = Some(h);
            at += length;
        }
        assert_eq!(
            lengths,
            [
                ACCOUNT,
                Duration::minutes(15),
                Duration::minutes(45),
                LONGEST_PLAN_BACKOFF,
                LONGEST_PLAN_BACKOFF,
                LONGEST_PLAN_BACKOFF
            ]
        );
        let last = previous.unwrap();
        let long_after = last.until + LONGEST_PLAN_BACKOFF + Duration::SECOND;
        assert_eq!(
            plan_backoff(Some(&last), long_after),
            ACCOUNT,
            "a hit after a quiet spell starts over"
        );
        assert_eq!(
            plan_backoff(Some(&held(now + ACCOUNT)), now),
            ACCOUNT,
            "another cause's hold is no step of the backoff"
        );
    }

    #[test]
    fn a_backoff_record_outlives_its_hold_by_as_long_again_and_keeps_its_old_json() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut h = held(now + ACCOUNT);
        assert_eq!(h.kept_for(now), ACCOUNT);
        let text = serde_json::to_string(&h).unwrap();
        assert!(!text.contains("backoff_ms"), "{text}");
        h.backoff_ms = Some(300_000);
        assert_eq!(h.kept_for(now), ACCOUNT * 2);
        let text = serde_json::to_string(&h).unwrap();
        assert_eq!(serde_json::from_str::<Held>(&text).unwrap(), h);
    }

    #[tokio::test]
    async fn a_plan_limit_reads_the_last_hold_from_this_gateway_while_valkey_is_away() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let unreachable = Valkey::new(
            &secrecy::SecretString::from("redis://127.0.0.1:1"),
            "kaeso-test:",
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let cooldowns = Cooldowns::default();
        let limited = KppError::new(ErrorKind::RateLimited, "plan limit");
        let first = cooldowns
            .plan_limit(&unreachable, &at(FABLE), &limited, now)
            .await;
        assert_eq!(first, (ACCOUNT, Some(300_000)));
        let mut h = held(now + ACCOUNT);
        h.backoff_ms = first.1;
        cooldowns
            .hold(&unreachable, &at(FABLE), Reach::Provider, h, now)
            .await;
        let later = now + ACCOUNT;
        assert_eq!(
            cooldowns
                .plan_limit(&unreachable, &at(OPUS), &limited, later)
                .await,
            (Duration::minutes(15), Some(900_000)),
            "every model of the account shares the backoff"
        );
        let mut stated = limited.clone();
        stated.retry_after = Some(std::time::Duration::from_secs(7_200));
        assert_eq!(
            cooldowns
                .plan_limit(&unreachable, &at(OPUS), &stated, later)
                .await,
            (Duration::hours(2), None)
        );
    }
}
