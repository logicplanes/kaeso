use std::sync::Arc;
use std::sync::Mutex;

use time::OffsetDateTime;

pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Clone, Debug)]
pub struct ManualClock(Arc<Mutex<OffsetDateTime>>);

impl ManualClock {
    pub fn new(at: OffsetDateTime) -> Self {
        ManualClock(Arc::new(Mutex::new(at)))
    }

    pub fn set(&self, at: OffsetDateTime) {
        if let Ok(mut now) = self.0.lock() {
            *now = at;
        }
    }

    pub fn advance(&self, by: time::Duration) {
        if let Ok(mut now) = self.0.lock() {
            *now += by;
        }
    }
}

impl Clock for ManualClock {
    fn now(&self) -> OffsetDateTime {
        self.0
            .lock()
            .map(|now| *now)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }
}

pub type SharedClock = Arc<dyn Clock>;

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn manual_clock_moves_only_when_told() {
        let clock = ManualClock::new(datetime!(2026-10-11 23:59:59 UTC));
        assert_eq!(clock.now(), datetime!(2026-10-11 23:59:59 UTC));
        clock.advance(time::Duration::seconds(1));
        assert_eq!(clock.now(), datetime!(2026-10-12 00:00:00 UTC));
    }
}
