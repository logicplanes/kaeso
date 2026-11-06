use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Version {
    V2025_03_26,
    V2025_06_18,
    V2025_11_25,
    V2026_07_28,
}

impl Version {
    pub const ALL: [Self; 4] = [
        Self::V2026_07_28,
        Self::V2025_11_25,
        Self::V2025_06_18,
        Self::V2025_03_26,
    ];
    pub const LATEST: Self = Self::V2026_07_28;
    pub const LATEST_LEGACY: Self = Self::V2025_11_25;
    // Streamable HTTP lets a server read a request without `MCP-Protocol-Version` as the first
    // version that had the transport, since 2025-03-26 clients don't send the header.
    pub const WITHOUT_HEADER: Self = Self::V2025_03_26;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2025_03_26 => "2025-03-26",
            Self::V2025_06_18 => "2025-06-18",
            Self::V2025_11_25 => "2025-11-25",
            Self::V2026_07_28 => "2026-07-28",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == value)
    }

    pub fn names() -> [&'static str; 4] {
        Self::ALL.map(Self::as_str)
    }

    pub const fn is_stateless(self) -> bool {
        matches!(self, Self::V2026_07_28)
    }

    pub fn for_initialize(asked: Option<&str>) -> Self {
        asked
            .and_then(Self::parse)
            .filter(|v| !v.is_stateless())
            .unwrap_or(Self::LATEST_LEGACY)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_version_parses_back() {
        for v in Version::ALL {
            assert_eq!(Version::parse(v.as_str()), Some(v));
            assert_eq!(v.to_string(), v.as_str());
        }
        assert_eq!(Version::parse("2024-11-05"), None);
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn newest_comes_first() {
        assert_eq!(
            Version::names(),
            ["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"]
        );
        assert_eq!(Version::ALL.first(), Some(&Version::LATEST));
    }

    #[test]
    fn only_the_newest_is_stateless() {
        assert!(Version::V2026_07_28.is_stateless());
        assert!(!Version::V2025_11_25.is_stateless());
        assert!(!Version::V2025_03_26.is_stateless());
    }

    #[test]
    fn initialize_agrees_on_a_version_that_has_initialize() {
        assert_eq!(
            Version::for_initialize(Some("2025-06-18")),
            Version::V2025_06_18
        );
        assert_eq!(
            Version::for_initialize(Some("2026-07-28")),
            Version::V2025_11_25
        );
        assert_eq!(
            Version::for_initialize(Some("1900-01-01")),
            Version::V2025_11_25
        );
        assert_eq!(Version::for_initialize(None), Version::V2025_11_25);
    }
}
