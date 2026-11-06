use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use kaeso_core::catalogue::route::Failure as Reason;
use kaeso_kpp::{ErrorKind, KppError};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
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

impl Step<'_> {
    fn key(&self, reach: Reach) -> (Uuid, Scope) {
        let scope = match reach {
            Reach::Model => Scope::Model(self.model),
            Reach::Provider => Scope::Provider(self.provider.to_owned()),
        };
        (self.project, scope)
    }
}

#[derive(Debug, Default)]
pub struct Cooldowns {
    held: Mutex<HashMap<(Uuid, Scope), Held>>,
}

impl Cooldowns {
    pub fn hold(&self, step: &Step<'_>, reach: Reach, held: Held, now: OffsetDateTime) {
        let mut map = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MOST_HELD {
            map.retain(|_, h| h.until > now);
        }
        if map.len() < MOST_HELD {
            map.insert(step.key(reach), held);
        }
    }

    pub fn held(&self, step: &Step<'_>, now: OffsetDateTime) -> Option<Held> {
        let map = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        [Reach::Provider, Reach::Model]
            .into_iter()
            .filter_map(|reach| map.get(&step.key(reach)))
            .filter(|h| h.until > now)
            .max_by_key(|h| h.until)
            .cloned()
    }

    pub fn release(&self, step: &Step<'_>) {
        let mut map = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        map.remove(&step.key(Reach::Model));
        map.remove(&step.key(Reach::Provider));
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
        let c = Cooldowns::default();
        let (reach, long) = provider(ErrorKind::Billing).unwrap();
        c.hold(&at(FABLE), reach, held(now + long), now);
        assert_eq!(c.held(&at(OPUS), now).unwrap().status, 402);
        let other = Step {
            provider: "openai",
            ..at(OPUS)
        };
        assert_eq!(c.held(&other, now), None);
        assert_eq!(c.held(&at(OPUS), now + long), None);
    }

    #[test]
    fn a_model_that_answers_is_released_at_once() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let c = Cooldowns::default();
        c.hold(&at(FABLE), Reach::Model, held(now + MODEL), now);
        assert!(c.held(&at(FABLE), now).is_some());
        assert_eq!(c.held(&at(OPUS), now), None);
        c.release(&at(FABLE));
        assert_eq!(c.held(&at(FABLE), now), None);
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
        let c = Cooldowns::default();
        for n in 0..MOST_HELD {
            let step = at(Uuid::from_u128(100 + n as u128));
            c.hold(&step, Reach::Model, held(now + MODEL), now);
        }
        let later = now + MODEL;
        c.hold(&at(FABLE), Reach::Model, held(later + MODEL), later);
        assert!(c.held(&at(FABLE), later).is_some());
    }
}
