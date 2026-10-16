use kaeso_kpp::{DialectId, Extras, KppError, RawJson};
use serde_json::{Map, Value};

use crate::wire::{Maybe, Short};

pub(crate) const FORM_NONE: &str = "none";
pub(crate) const FORM_SPELLING: &str = "spelling";
pub(crate) const QUERY_PREFIX: &str = "query.";

const ABSENT: &str = "absent";
const NULL: &str = "null";

pub(crate) fn query_pairs(query: &str) -> Result<Vec<(String, String)>, KppError> {
    let query = query.strip_prefix('?').unwrap_or(query);
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            Ok((percent_decode(name)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(text: &str) -> Result<String, KppError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                let byte = bytes
                    .get(i + 1..i + 3)
                    .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                    .ok_or_else(|| {
                        KppError::invalid_request(format!(
                            "The query string is not valid percent-encoding: it has a `%` not followed by two hex digits: `{}`.",
                            Short(text, 100)
                        ))
                    })?;
                out.push(byte);
                i += 3;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| {
        KppError::invalid_request(format!(
            "The query string is not valid UTF-8 after percent-decoding `{}`.",
            Short(text, 100)
        ))
    })
}

pub(crate) fn add_query_extra(extras: &mut Extras, name: &str, value: &str) {
    let key = format!("{QUERY_PREFIX}{name}");
    let next = match extras.fields.get(&key).and_then(|v| v.to_value().ok()) {
        Some(Value::Array(mut items)) => {
            items.push(Value::from(value));
            Value::Array(items)
        }
        Some(previous) => Value::Array(vec![previous, Value::from(value)]),
        None => Value::from(value),
    };
    if let Ok(raw) = RawJson::from_serialize(&next) {
        extras.fields.insert(key, raw);
    }
}

const SECONDS_PER_DAY: i64 = 86_400;

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn digits(bytes: &[u8], from: usize, to: usize) -> Option<i64> {
    let part = bytes.get(from..to)?;
    if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
        return None;
    }
    part.iter().try_fold(0_i64, |n, d| {
        n.checked_mul(10)?.checked_add(i64::from(d - b'0'))
    })
}

fn full_date(bytes: &[u8]) -> Option<i64> {
    let year = digits(bytes, 0, 4)?;
    let month = digits(bytes, 5, 7)?;
    let day = digits(bytes, 8, 10)?;
    let ok = bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day);
    ok.then(|| days_from_civil(year, month, day))
}

pub(crate) fn parse_date(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 {
        return None;
    }
    u64::try_from(full_date(bytes)?.checked_mul(SECONDS_PER_DAY)?).ok()
}

pub(crate) fn parse_rfc3339(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let days = full_date(bytes)?;
    let hour = digits(bytes, 11, 13)?;
    let minute = digits(bytes, 14, 16)?;
    let second = digits(bytes, 17, 19)?;
    let shape = matches!(bytes.get(10), Some(b'T' | b't'))
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
        && hour < 24
        && minute < 60
        && second <= 60;
    if !shape {
        return None;
    }
    let mut rest = bytes.get(19..)?;
    if let Some((b'.', frac)) = rest.split_first() {
        let n = frac.iter().take_while(|b| b.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        rest = frac.get(n..)?;
    }
    let offset = match rest {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), ..] if rest.len() == 6 && rest.get(3) == Some(&b':') => {
            let oh = digits(rest, 1, 3)?;
            let om = digits(rest, 4, 6)?;
            if oh >= 24 || om >= 60 {
                return None;
            }
            let minutes = oh * 60 + om;
            if *sign == b'+' { minutes } else { -minutes }
        }
        _ => return None,
    };
    let total = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(hour * 3600 + minute * 60 + second - offset * 60)?;
    u64::try_from(total).ok()
}

pub(crate) const MAX_RFC3339_SECS: u64 = 253_402_300_799;

fn civil(secs: u64) -> Option<(i64, i64, i64, u64)> {
    if secs > MAX_RFC3339_SECS {
        return None;
    }
    let days = i64::try_from(secs / 86_400).ok()?;
    let (y, m, d) = civil_from_days(days);
    Some((y, m, d, secs % 86_400))
}

pub(crate) fn format_rfc3339(secs: u64) -> Option<String> {
    let (y, m, d, s) = civil(secs)?;
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s / 60 % 60,
        s % 60
    ))
}

pub(crate) fn format_date(secs: u64) -> Option<String> {
    let (y, m, d, _) = civil(secs)?;
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

pub(crate) const EPOCH_RFC3339: &str = "1970-01-01T00:00:00Z";

fn form_object<'a>(extras: &'a mut Extras, marker: &str) -> Option<&'a mut Map<String, Value>> {
    extras
        .form
        .entry(marker.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
}

pub(crate) fn record_none<T>(
    extras: &mut Extras,
    key: &str,
    value: &Maybe<T>,
    default_is_null: bool,
) {
    let spelling = match value {
        Maybe::Absent => ABSENT,
        Maybe::Null if !default_is_null => NULL,
        Maybe::Null | Maybe::Value(_) => return,
    };
    if let Some(map) = form_object(extras, FORM_NONE) {
        map.insert(key.to_owned(), Value::from(spelling));
    }
}

pub(crate) fn none_or<T>(
    extras: &Extras,
    dialect: DialectId,
    key: &str,
    default: Maybe<T>,
) -> Maybe<T> {
    let recorded = extras
        .form_value(dialect, FORM_NONE)
        .and_then(|v| v.get(key))
        .and_then(Value::as_str);
    match recorded {
        Some(ABSENT) => Maybe::Absent,
        Some(NULL) => Maybe::Null,
        _ => default,
    }
}

pub(crate) fn decode_date(
    extras: &mut Extras,
    key: &str,
    text: &str,
    parse: fn(&str) -> Option<u64>,
    format: fn(u64) -> Option<String>,
) -> Option<u64> {
    let secs = parse(text)?;
    if format(secs).as_deref() != Some(text)
        && let Some(map) = form_object(extras, FORM_SPELLING)
    {
        map.insert(key.to_owned(), Value::from(text));
    }
    Some(secs)
}

pub(crate) fn encode_date(
    extras: &Extras,
    dialect: DialectId,
    key: &str,
    secs: u64,
    parse: fn(&str) -> Option<u64>,
    format: fn(u64) -> Option<String>,
) -> Option<String> {
    extras
        .form_value(dialect, FORM_SPELLING)
        .and_then(|v| v.get(key))
        .and_then(Value::as_str)
        .filter(|s| parse(s) == Some(secs))
        .map_or_else(|| format(secs), |s| Some(s.to_owned()))
}

pub(crate) fn bad_listing(dialect: DialectId, problem: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a model listing that is not a valid {} response: {problem}.",
        dialect.display_name()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::Origin;

    #[test]
    fn query_pairs_percent_decode_names_and_values() {
        let pairs = query_pairs("?lifecycle%5B%5D=active&a+b=c%2Fd&&flag&x=%E2%9C%93").unwrap();
        assert_eq!(
            pairs,
            [
                ("lifecycle[]".to_owned(), "active".to_owned()),
                ("a b".to_owned(), "c/d".to_owned()),
                ("flag".to_owned(), String::new()),
                ("x".to_owned(), "✓".to_owned()),
            ]
        );
        assert!(query_pairs("").unwrap().is_empty());
        for bad in ["a=%", "a=%4", "a=%zz", "a=%+F", "%FF=1"] {
            let e = query_pairs(bad).unwrap_err();
            assert_eq!(e.kind, kaeso_kpp::ErrorKind::InvalidRequest, "{bad}");
        }
    }

    #[test]
    fn repeated_unknown_query_params_become_arrays() {
        let mut e = Extras::new(Origin::new(DialectId::OpenAiChat));
        add_query_extra(&mut e, "x", "1");
        add_query_extra(&mut e, "x", "2");
        add_query_extra(&mut e, "x", "3");
        add_query_extra(&mut e, "y", "z");
        let get = |k: &str| e.fields.get(k).unwrap().to_value().unwrap();
        assert_eq!(get("query.x"), serde_json::json!(["1", "2", "3"]));
        assert_eq!(get("query.y"), serde_json::json!("z"));
    }

    #[test]
    fn dates_convert_both_ways() {
        let cases = [
            ("1970-01-01T00:00:00Z", 0),
            ("2023-06-15T12:34:56Z", 1_686_832_496),
            ("2000-02-29T23:59:59Z", 951_868_799),
            ("2026-10-23T00:00:00Z", 1_792_713_600),
        ];
        for (text, secs) in cases {
            assert_eq!(parse_rfc3339(text), Some(secs), "{text}");
            assert_eq!(format_rfc3339(secs).as_deref(), Some(text));
        }
        assert_eq!(parse_date("2026-10-23"), Some(1_792_713_600));
        assert_eq!(
            format_date(1_792_713_600 + 3600).as_deref(),
            Some("2026-10-23")
        );
        for day in 0..157_000_u64 {
            let s = day * 86_400 + 43_210;
            assert_eq!(parse_rfc3339(&format_rfc3339(s).unwrap()), Some(s));
            assert_eq!(parse_date(&format_date(s).unwrap()), Some(day * 86_400));
        }
        let last = "9999-12-31T23:59:59Z";
        assert_eq!(parse_rfc3339(last), Some(MAX_RFC3339_SECS));
        assert_eq!(format_rfc3339(MAX_RFC3339_SECS).as_deref(), Some(last));
        assert_eq!(format_date(MAX_RFC3339_SECS).as_deref(), Some("9999-12-31"));
        for secs in [MAX_RFC3339_SECS + 1, 1_700_000_000_000, u64::MAX] {
            assert_eq!(format_rfc3339(secs), None, "{secs}");
            assert_eq!(format_date(secs), None, "{secs}");
        }
    }

    #[test]
    fn rfc3339_variants_and_rejections() {
        let base = parse_rfc3339("2025-08-05T16:30:00Z").unwrap();
        assert_eq!(parse_rfc3339("2025-08-05T09:30:00.250-07:00"), Some(base));
        assert_eq!(parse_rfc3339("2025-08-05t18:30:00+02:00"), Some(base));
        assert_eq!(parse_rfc3339("2025-08-05T16:30:00.999999z"), Some(base));
        assert_eq!(
            parse_rfc3339("2016-12-31T23:59:60Z"),
            parse_rfc3339("2017-01-01T00:00:00Z")
        );
        for bad in [
            "",
            "2025-08-05",
            "2025-08-05 16:30:00Z",
            "2025-08-05T16:30:00",
            "2025-08-05T16:30:00.Z",
            "2025-08-05T24:00:00Z",
            "2025-08-05T16:60:00Z",
            "2025-08-05T16:30:61Z",
            "2025-13-05T16:30:00Z",
            "2025-02-29T16:30:00Z",
            "2025-08-00T16:30:00Z",
            "2025-08-05T16:30:00+24:00",
            "2025-08-05T16:30:00+02:60",
            "2025-08-05T16:30:00+0200",
            "2025-08-05T16:30:00ZZ",
            "1969-12-31T23:59:59Z",
            "1970-01-01T00:30:00+01:00",
            "+025-08-05T16:30:00Z",
            "2025/08/05T16:30:00Z",
        ] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
        for bad in [
            "2026-10-2",
            "2026-10-233",
            "2026-02-30",
            "1969-12-31",
            "2026-10-23T00:00:00Z",
        ] {
            assert_eq!(parse_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn form_markers_keep_spellings_for_their_dialect_only() {
        let d = DialectId::AnthropicMessages;
        let mut e = Extras::new(Origin::new(d));
        let text = "2025-08-05T09:30:00.250-07:00";
        let secs = decode_date(&mut e, "created_at", text, parse_rfc3339, format_rfc3339).unwrap();
        assert_eq!(
            decode_date(
                &mut e,
                "x",
                "1970-01-01T00:00:00Z",
                parse_rfc3339,
                format_rfc3339
            ),
            Some(0)
        );
        assert_eq!(
            decode_date(&mut e, "y", "bad", parse_rfc3339, format_rfc3339),
            None
        );
        let enc = |d, secs| encode_date(&e, d, "created_at", secs, parse_rfc3339, format_rfc3339);
        assert_eq!(enc(d, secs).as_deref(), Some(text));
        assert_eq!(
            enc(d, secs + 1).as_deref(),
            Some("2025-08-05T16:30:01Z"),
            "a changed value wins over a stale spelling"
        );
        assert_eq!(
            enc(DialectId::OpenAiChat, secs).as_deref(),
            Some("2025-08-05T16:30:00Z")
        );
        assert_eq!(enc(d, u64::MAX), None, "past year 9999");
        assert!(e.form.get(FORM_SPELLING).unwrap().get("x").is_none());

        record_none::<u64>(&mut e, "a", &Maybe::Absent, true);
        record_none::<u64>(&mut e, "b", &Maybe::Null, false);
        record_none::<u64>(&mut e, "c", &Maybe::Null, true);
        record_none(&mut e, "d", &Maybe::Value(1), false);
        let none = |k| none_or(&e, d, k, Maybe::Value(9_u64));
        assert_eq!(none("a"), Maybe::Absent);
        assert_eq!(none("b"), Maybe::Null);
        assert_eq!(none("c"), Maybe::Value(9));
        assert_eq!(none("d"), Maybe::Value(9));
        assert_eq!(
            none_or(&e, DialectId::OpenAiChat, "a", Maybe::<u64>::Null),
            Maybe::Null
        );
    }
}
