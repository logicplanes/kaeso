use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserializer;

pub const DECIMAL_PATTERN: &str = r"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$";

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

fn matches_pattern(text: &str) -> bool {
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(exponent)),
        None => (text, None),
    };
    let mantissa_is_valid = match mantissa.split_once('.') {
        Some((whole, fraction)) => digits(fraction) && (whole.is_empty() || digits(whole)),
        None => digits(mantissa),
    };
    mantissa_is_valid
        && exponent
            .is_none_or(|exponent| digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent)))
}

pub(crate) fn parse_non_negative(text: &str) -> Result<Decimal, String> {
    if !matches_pattern(text) {
        return Err(format!("`{text}` is not a decimal number"));
    }
    Decimal::from_str(text)
        .or_else(|_| Decimal::from_scientific(text))
        .map(|decimal| decimal.normalize())
        .map_err(|_| format!("`{text}` is not a decimal number"))
}

struct DecimalVisitor(fn(&str) -> Result<Decimal, String>);

impl<'de> serde::de::Visitor<'de> for DecimalVisitor {
    type Value = Option<Decimal>;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a decimal string or a whole number")
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
        (self.0)(s).map(Some).map_err(E::custom)
    }

    fn visit_u64<E: serde::de::Error>(self, n: u64) -> Result<Self::Value, E> {
        (self.0)(&n.to_string()).map(Some).map_err(E::custom)
    }

    fn visit_i64<E: serde::de::Error>(self, n: i64) -> Result<Self::Value, E> {
        (self.0)(&n.to_string()).map(Some).map_err(E::custom)
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
        Err(E::custom(
            "a fraction must be sent as a decimal string, such as \"0.25\"",
        ))
    }
}

pub(crate) fn deserialize_with<'de, D: Deserializer<'de>>(
    d: D,
    parse: fn(&str) -> Result<Decimal, String>,
) -> Result<Option<Decimal>, D::Error> {
    d.deserialize_option(DecimalVisitor(parse))
}

#[cfg(test)]
mod tests {
    use regex::Regex;
    use rust_decimal_macros::dec;

    use super::*;

    const ACCEPTED: [&str; 22] = [
        "0",
        "5",
        "007",
        "12.50",
        "0.000001",
        "1000000000",
        ".5",
        ".50",
        "1e3",
        "1E3",
        "2.5E-1",
        "1.5e+2",
        "1.5e02",
        ".5e1",
        ".5E-1",
        "0e0",
        "00.10",
        "123456789.123456789",
        "1e-9",
        "9e+0",
        "0.0",
        "10e0",
    ];

    const REFUSED: [&str; 38] = [
        "", " ", ".", "e", "e5", ".e5", "+5", "-5", "-0", "+.5", "5.", "5.e3", "1_000", "1_0.5",
        "_1", "1_", " 5", "5 ", "\t5", "5\n", "\n5", "5\u{a0}", "1,5", "0x10", "1e", "1e+", "1e-",
        "1e3.5", "1.2.3", "1e2e3", "1 e3", "1e 3", "Infinity", "NaN", "٣", "５", "1/2", "five",
    ];

    #[test]
    fn the_parser_accepts_exactly_what_the_published_pattern_describes() {
        let pattern = Regex::new(DECIMAL_PATTERN).unwrap();
        for text in ACCEPTED {
            assert!(pattern.is_match(text), "{text:?} should match the pattern");
            assert!(matches_pattern(text), "{text:?} should be accepted");
            assert!(parse_non_negative(text).is_ok(), "{text:?} should parse");
        }
        for text in REFUSED {
            assert!(
                !pattern.is_match(text),
                "{text:?} should not match the pattern"
            );
            assert!(!matches_pattern(text), "{text:?} should be refused");
            assert!(
                parse_non_negative(text).is_err(),
                "{text:?} should not parse"
            );
        }
    }

    #[test]
    fn every_short_string_over_the_pattern_alphabet_agrees() {
        let pattern = Regex::new(DECIMAL_PATTERN).unwrap();
        let alphabet = ['0', '7', '.', 'e', 'E', '+', '-', '_', ' '];
        let mut texts = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..6 {
            frontier = frontier
                .iter()
                .flat_map(|prefix| alphabet.iter().map(move |c| format!("{prefix}{c}")))
                .collect();
            texts.extend(frontier.iter().cloned());
        }
        for text in &texts {
            assert_eq!(
                matches_pattern(text),
                pattern.is_match(text),
                "{text:?} disagrees with the pattern"
            );
            if !pattern.is_match(text) {
                assert!(
                    parse_non_negative(text).is_err(),
                    "{text:?} should not parse"
                );
            }
        }
    }

    #[test]
    fn parsed_values_keep_every_digit() {
        for (text, value) in [
            ("12.50", dec!(12.5)),
            (".5", dec!(0.5)),
            (".5e1", dec!(5)),
            ("1E3", dec!(1000)),
            ("2.5E-1", dec!(0.25)),
            ("1.5e+2", dec!(150)),
            ("007", dec!(7)),
            ("0.30000000000000004441", dec!(0.30000000000000004441)),
        ] {
            assert_eq!(parse_non_negative(text), Ok(value), "{text}");
        }
    }

    #[test]
    fn values_beyond_the_decimal_range_are_refused() {
        for text in ["1e999999999999", "79228162514264337593543950336", "1e-99"] {
            assert!(parse_non_negative(text).is_err(), "{text}");
        }
    }
}
