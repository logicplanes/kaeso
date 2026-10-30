use std::net::{IpAddr, Ipv6Addr};
use std::time::Duration;

use time::OffsetDateTime;

use super::throttle::{self, Outage, Refusal, Throttle};
use crate::state::AppState;

const NAME: &str = "key_ip";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Throttled {
    pub retry_after: u64,
}

impl Throttled {
    pub fn message(self) -> String {
        format!(
            "Too many requests with unknown API keys from your address; try again in {}.",
            throttle::wait(self.retry_after)
        )
    }

    pub fn duration(self) -> Duration {
        Duration::from_secs(self.retry_after)
    }
}

#[derive(Clone, Debug)]
pub struct Gate {
    state: AppState,
    subject: String,
}

impl Gate {
    pub fn new(state: &AppState, client: Option<IpAddr>) -> Option<Gate> {
        (state.config.key_failures.per_address > 0).then(|| Gate {
            state: state.clone(),
            subject: subject(client),
        })
    }

    pub async fn check(&self) -> Result<(), Throttled> {
        match self.state.caches.key_blocks.get(&self.subject).await {
            Some(until) => remaining(until, self.state.clock.now()).map_or(Ok(()), Err),
            None => Ok(()),
        }
    }

    pub async fn failed(&self) -> Result<(), Throttled> {
        let cfg = self.state.config.key_failures;
        let t = Throttle {
            name: NAME,
            limit: cfg.per_address,
            window: cfg.window,
            on_outage: Outage::Allow,
        };
        let Err(Refusal::Limited { retry_after }) =
            throttle::hit(&self.state, t, &self.subject).await
        else {
            return Ok(());
        };
        let blocks = &self.state.caches.key_blocks;
        let now = self.state.clock.now();
        let known = blocks
            .get(&self.subject)
            .await
            .is_some_and(|until| remaining(until, now).is_some());
        let seconds = i64::try_from(retry_after).unwrap_or(i64::MAX);
        blocks
            .insert(
                self.subject.clone(),
                now.saturating_add(time::Duration::seconds(seconds)),
            )
            .await;
        if !known {
            metrics::counter!(crate::metrics::KEY_FAILURE_BLOCKS).increment(1);
            tracing::warn!(
                client = %self.subject,
                limit = cfg.per_address,
                window_seconds = cfg.window.as_secs(),
                retry_after,
                "too many failed API key lookups from one address; keys not in memory are refused until the window ends"
            );
        }
        Err(Throttled { retry_after })
    }
}

fn remaining(until: OffsetDateTime, now: OffsetDateTime) -> Option<Throttled> {
    let left = (until - now).whole_seconds();
    (left > 0).then(|| Throttled {
        retry_after: u64::try_from(left).unwrap_or(1),
    })
}

pub fn subject(client: Option<IpAddr>) -> String {
    match client.map(|ip| ip.to_canonical()) {
        Some(IpAddr::V4(v4)) => v4.to_string(),
        Some(IpAddr::V6(v6)) => {
            let bits = u128::from(v6) & !((1u128 << 64) - 1);
            format!("{}/64", Ipv6Addr::from(bits))
        }
        None => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn addresses_are_counted_per_host() {
        assert_eq!(subject(ip("203.0.113.9")), "203.0.113.9");
        assert_eq!(
            subject(ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd")),
            "2001:db8:1:2::/64"
        );
        assert_eq!(subject(ip("2001:db8:1:2::1")), "2001:db8:1:2::/64");
        assert_ne!(subject(ip("2001:db8:1:3::1")), "2001:db8:1:2::/64");
        assert_eq!(subject(ip("::ffff:203.0.113.9")), "203.0.113.9");
        assert_eq!(subject(None), "unknown");
    }

    #[test]
    fn a_block_ends_with_its_window() {
        let until = datetime!(2026-10-13 12:01:00 UTC);
        assert_eq!(
            remaining(until, datetime!(2026-10-13 12:00:00 UTC)),
            Some(Throttled { retry_after: 60 })
        );
        assert_eq!(remaining(until, datetime!(2026-10-13 12:01:00 UTC)), None);
        assert_eq!(remaining(until, datetime!(2026-10-13 12:05:00 UTC)), None);
    }

    #[test]
    fn the_message_is_one_short_sentence() {
        let m = Throttled { retry_after: 3600 }.message();
        assert!(m.len() <= 120 && m.ends_with('.'), "{m}");
    }
}
