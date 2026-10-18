use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const WINDOW: time::Duration = time::Duration::hours(1);
pub const DEFAULT_HOUR: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Manual,
    Automatic,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Manual, Mode::Automatic];

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Manual => "manual",
            Mode::Automatic => "automatic",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    Weekly,
}

impl Frequency {
    pub const ALL: [Frequency; 2] = [Frequency::Daily, Frequency::Weekly];

    pub fn as_str(self) -> &'static str {
        match self {
            Frequency::Daily => "daily",
            Frequency::Weekly => "weekly",
        }
    }

    pub fn parse(s: &str) -> Option<Frequency> {
        Frequency::ALL.into_iter().find(|f| f.as_str() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    pub fn number(self) -> i16 {
        match self {
            Weekday::Monday => 1,
            Weekday::Tuesday => 2,
            Weekday::Wednesday => 3,
            Weekday::Thursday => 4,
            Weekday::Friday => 5,
            Weekday::Saturday => 6,
            Weekday::Sunday => 7,
        }
    }

    pub fn from_number(n: i16) -> Option<Weekday> {
        Weekday::ALL.into_iter().find(|d| d.number() == n)
    }
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub frequency: Frequency,
    pub weekday: Weekday,
    pub hour: u8,
    pub zone: TimeZone,
}

fn timestamp(at: OffsetDateTime) -> Option<Timestamp> {
    Timestamp::from_second(at.unix_timestamp()).ok()
}

fn offset_date_time(at: Timestamp) -> Option<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(at.as_second()).ok()
}

impl Schedule {
    fn falls_on(&self, date: Date) -> bool {
        match self.frequency {
            Frequency::Daily => true,
            Frequency::Weekly => {
                i16::from(date.weekday().to_monday_one_offset()) == self.weekday.number()
            }
        }
    }

    fn start_on(&self, date: Date) -> Option<Timestamp> {
        let hour = i8::try_from(self.hour.min(23)).ok()?;
        Some(
            date.at(hour, 0, 0, 0)
                .to_zoned(self.zone.clone())
                .ok()?
                .timestamp(),
        )
    }

    fn starts_around(&self, at: Timestamp) -> Vec<Timestamp> {
        let today = at.to_zoned(self.zone.clone()).date();
        (-8_i64..=8)
            .filter_map(|days| today.checked_add(jiff::Span::new().days(days)).ok())
            .filter(|date| self.falls_on(*date))
            .filter_map(|date| self.start_on(date))
            .collect()
    }

    pub fn window(&self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        let now_ts = timestamp(now)?;
        let window = SignedDuration::from_secs(WINDOW.whole_seconds());
        let start = self
            .starts_around(now_ts)
            .into_iter()
            .filter(|start| *start <= now_ts)
            .max()?;
        (now_ts.duration_since(start) < window)
            .then(|| offset_date_time(start))
            .flatten()
    }

    pub fn next(&self, after: OffsetDateTime) -> Option<OffsetDateTime> {
        let after_ts = timestamp(after)?;
        self.starts_around(after_ts)
            .into_iter()
            .filter(|start| *start > after_ts)
            .min()
            .and_then(offset_date_time)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use time::macros::datetime;

    fn berlin(frequency: Frequency, weekday: Weekday, hour: u8) -> Schedule {
        Schedule {
            frequency,
            weekday,
            hour,
            zone: TimeZone::get("Europe/Berlin").unwrap(),
        }
    }

    #[test]
    fn names_and_numbers_round_trip() {
        for f in Frequency::ALL {
            assert_eq!(Frequency::parse(f.as_str()), Some(f));
        }
        for m in Mode::ALL {
            assert_eq!(Mode::parse(m.as_str()), Some(m));
        }
        assert_eq!(Mode::parse("scheduled"), None);
        for d in Weekday::ALL {
            assert_eq!(Weekday::from_number(d.number()), Some(d));
        }
        assert_eq!(Frequency::parse("hourly"), None);
        assert_eq!(Weekday::from_number(0), None);
        assert_eq!(Weekday::from_number(8), None);
    }

    #[test]
    fn a_daily_window_opens_at_the_hour_in_the_zone_and_lasts_an_hour() {
        let s = berlin(Frequency::Daily, Weekday::Monday, 3);
        assert_eq!(
            s.window(datetime!(2026-10-20 01:00 UTC)),
            Some(datetime!(2026-10-20 01:00 UTC))
        );
        assert_eq!(
            s.window(datetime!(2026-10-20 01:59:59 UTC)),
            Some(datetime!(2026-10-20 01:00 UTC))
        );
        assert_eq!(s.window(datetime!(2026-10-20 02:00 UTC)), None);
        assert_eq!(s.window(datetime!(2026-10-20 00:59 UTC)), None);
        assert_eq!(
            s.next(datetime!(2026-10-20 01:30 UTC)),
            Some(datetime!(2026-10-21 01:00 UTC))
        );
    }

    #[test]
    fn the_window_follows_daylight_saving_time() {
        let s = berlin(Frequency::Daily, Weekday::Monday, 3);
        assert_eq!(
            s.next(datetime!(2026-10-24 12:00 UTC)),
            Some(datetime!(2026-10-25 02:00 UTC)),
            "after the clocks go back, 03:00 in Berlin is 02:00 UTC"
        );
        assert_eq!(
            s.next(datetime!(2026-10-23 12:00 UTC)),
            Some(datetime!(2026-10-24 01:00 UTC))
        );
    }

    #[test]
    fn an_hour_the_clocks_skip_still_opens_a_window() {
        let s = berlin(Frequency::Daily, Weekday::Monday, 2);
        let start = s.next(datetime!(2026-03-28 12:00 UTC)).unwrap();
        assert_eq!(start, datetime!(2026-03-29 01:00 UTC));
        assert_eq!(s.window(start), Some(start));
    }

    #[test]
    fn a_weekly_window_opens_only_on_its_day() {
        let s = berlin(Frequency::Weekly, Weekday::Sunday, 4);
        assert_eq!(
            s.next(datetime!(2026-10-20 12:00 UTC)),
            Some(datetime!(2026-10-25 03:00 UTC))
        );
        assert_eq!(s.window(datetime!(2026-10-24 02:30 UTC)), None);
        assert_eq!(
            s.window(datetime!(2026-10-25 03:15 UTC)),
            Some(datetime!(2026-10-25 03:00 UTC))
        );
        assert_eq!(
            s.next(datetime!(2026-10-25 03:15 UTC)),
            Some(datetime!(2026-11-01 03:00 UTC))
        );
    }

    #[test]
    fn a_zone_far_from_utc_counts_the_day_in_that_zone() {
        let s = Schedule {
            frequency: Frequency::Weekly,
            weekday: Weekday::Monday,
            hour: 1,
            zone: TimeZone::get("Pacific/Auckland").unwrap(),
        };
        assert_eq!(
            s.next(datetime!(2026-10-20 12:00 UTC)),
            Some(datetime!(2026-10-25 12:00 UTC)),
            "Monday 01:00 in Auckland is Sunday noon in UTC"
        );
    }
}
