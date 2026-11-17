//! Durations in the configuration, such as `30s`, `15m`, `1h 30m` or `250ms`: one or more numbers,
//! each followed by its unit, read as humantime read them before (its units and their spellings).

use std::time::Duration;

use serde::Deserialize;

const UNITS: &[(&[&str], u64)] = &[
    (&["ns", "nsec", "nanos"], 1),
    (&["us", "µs", "usec", "micros"], 1_000),
    (&["ms", "msec", "millis"], 1_000_000),
    (&["s", "sec", "secs", "second", "seconds"], 1_000_000_000),
    (&["m", "min", "mins", "minute", "minutes"], 60_000_000_000),
    (&["h", "hr", "hrs", "hour", "hours"], 3_600_000_000_000),
    (&["d", "day", "days"], 86_400_000_000_000),
    (&["w", "week", "weeks"], 604_800_000_000_000),
    // A month and a year as humantime counts them: 30.44 and 365.25 days.
    (&["M", "month", "months"], 2_630_016_000_000_000),
    (&["y", "year", "years"], 31_557_600_000_000_000),
];

/// A duration such as `30s` or `1h 30m`; a bare `0` is no time.
pub(crate) fn parse(text: &str) -> Result<Duration, String> {
    let text = text.trim();
    let wrong = || format!("`{text}` is not a duration such as 30s, 15m or 1h");
    if text == "0" {
        return Ok(Duration::ZERO);
    }
    let mut nanos: u128 = 0;
    let mut rest = text;
    if rest.is_empty() {
        return Err(wrong());
    }
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at(digits);
        let number: u128 = number.parse().map_err(|_| wrong())?;
        let after = after.trim_start();
        let unit_len = after
            .find(|c: char| c.is_ascii_digit() || c.is_whitespace())
            .unwrap_or(after.len());
        let (unit, after) = after.split_at(unit_len);
        let scale = UNITS
            .iter()
            .find(|(names, _)| names.contains(&unit))
            .map(|(_, scale)| *scale)
            .ok_or_else(wrong)?;
        nanos = number
            .checked_mul(u128::from(scale))
            .and_then(|n| nanos.checked_add(n))
            .ok_or_else(wrong)?;
        rest = after.trim_start();
    }
    let secs = u64::try_from(nanos / 1_000_000_000).map_err(|_| wrong())?;
    let sub = u32::try_from(nanos % 1_000_000_000).map_err(|_| wrong())?;
    Ok(Duration::new(secs, sub))
}

/// For `#[serde(deserialize_with = "duration::deserialize")]`.
pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
    let text = String::deserialize(d)?;
    parse(&text).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_as_they_always_have() {
        for (text, want) in [
            ("30s", Duration::from_secs(30)),
            ("15m", Duration::from_secs(900)),
            ("1h", Duration::from_secs(3600)),
            ("1h 30m", Duration::from_secs(5400)),
            ("1h30m", Duration::from_secs(5400)),
            ("250ms", Duration::from_millis(250)),
            ("0s", Duration::ZERO),
            ("0", Duration::ZERO),
            ("2d", Duration::from_secs(172_800)),
            ("1w", Duration::from_secs(604_800)),
            (" 600s ", Duration::from_secs(600)),
            ("5 min", Duration::from_secs(300)),
            ("10us", Duration::from_micros(10)),
            ("7ns", Duration::from_nanos(7)),
            ("1M", Duration::from_secs(2_630_016)),
            ("2years", Duration::from_secs(63_115_200)),
        ] {
            assert_eq!(parse(text), Ok(want), "{text}");
        }
        for bad in [
            "",
            "soon",
            "5",
            "5x",
            "s",
            "1.5s",
            "-1s",
            "99999999999999999999999w",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
