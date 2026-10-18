use jiff::tz::TimeZone;

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
}
