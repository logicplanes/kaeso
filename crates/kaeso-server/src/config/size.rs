use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl ByteSize {
    pub fn as_usize(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }

    pub fn parse(s: &str) -> Result<ByteSize, String> {
        let s = s.trim();
        let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let (number, unit) = s.split_at(split);
        let number: u64 = number
            .parse()
            .map_err(|_| format!("`{s}` is not a size such as `32MiB` or `1048576`"))?;
        let factor: u64 = match unit.trim() {
            "" | "B" => 1,
            "KB" => 1_000,
            "MB" => 1_000_000,
            "GB" => 1_000_000_000,
            "KiB" => 1 << 10,
            "MiB" => 1 << 20,
            "GiB" => 1 << 30,
            other => {
                return Err(format!(
                    "unknown size unit `{other}`; use B, KB, MB, GB, KiB, MiB or GiB"
                ));
            }
        };
        number
            .checked_mul(factor)
            .map(ByteSize)
            .ok_or_else(|| format!("`{s}` is too large"))
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = ByteSize;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a size such as \"32MiB\" or a number of bytes")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<ByteSize, E> {
                Ok(ByteSize(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<ByteSize, E> {
                u64::try_from(v)
                    .map(ByteSize)
                    .map_err(|_| E::custom("a size cannot be negative"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<ByteSize, E> {
                ByteSize::parse(v).map_err(E::custom)
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_with_units() {
        assert_eq!(ByteSize::parse("32MiB").unwrap().0, 32 << 20);
        assert_eq!(ByteSize::parse("1 MB").unwrap().0, 1_000_000);
        assert_eq!(ByteSize::parse("1024").unwrap().0, 1024);
        assert!(ByteSize::parse("1PB").is_err());
        assert!(ByteSize::parse("MiB").is_err());
        assert!(ByteSize::parse("99999999999999999999GiB").is_err());
    }
}
