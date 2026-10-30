use jiff::tz::TimeZone;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

pub fn parse(name: &str) -> Option<TimeZone> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return None;
    }
    TimeZone::get(name).ok()
}

pub fn or_utc(name: &str) -> TimeZone {
    parse(name).unwrap_or(TimeZone::UTC)
}

/// The wall-clock time `at` reads in `zone`.
pub fn local(at: OffsetDateTime, zone: &TimeZone) -> PrimitiveDateTime {
    let offset = jiff::Timestamp::from_second(at.unix_timestamp())
        .ok()
        .and_then(|t| UtcOffset::from_whole_seconds(zone.to_offset(t).seconds()).ok())
        .unwrap_or(UtcOffset::UTC);
    let at = at.to_offset(offset);
    PrimitiveDateTime::new(at.date(), at.time())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_time_zones_are_accepted() {
        assert!(parse("Europe/Berlin").is_some());
        assert!(parse(" UTC ").is_some());
        assert!(parse("Asia/Calcutta").is_some());
        assert!(parse("Mars/Olympus").is_none());
        assert!(parse("").is_none());
        assert!(parse(&"A".repeat(65)).is_none());
        assert_eq!(or_utc("Mars/Olympus"), TimeZone::UTC);
        assert_ne!(or_utc("Asia/Tokyo"), TimeZone::UTC);
    }

    #[test]
    fn local_time_follows_the_zone_and_its_summer_time() {
        use time::macros::datetime;
        let berlin = or_utc("Europe/Berlin");
        assert_eq!(
            local(datetime!(2026-12-31 0:00 UTC), &berlin),
            datetime!(2026-12-31 1:00)
        );
        assert_eq!(
            local(datetime!(2026-07-01 22:30 UTC), &berlin),
            datetime!(2026-07-02 0:30)
        );
        assert_eq!(
            local(datetime!(2026-01-01 3:00 UTC), &or_utc("America/New_York")),
            datetime!(2025-12-31 22:00)
        );
        assert_eq!(
            local(datetime!(2026-12-31 0:00 UTC), &TimeZone::UTC),
            datetime!(2026-12-31 0:00)
        );
    }
}
