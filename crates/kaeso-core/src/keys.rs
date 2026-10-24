use time::{Date, Duration, OffsetDateTime};

use crate::secret;

pub const PROJECT_PREFIX: &str = "sk-kso-";
pub const ORG_PREFIX: &str = "sk-kso-org-";
pub const RANDOM_LEN: usize = 48;

const BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Project,
    OrgPlatform,
}

#[derive(Debug)]
pub struct NewSecret {
    pub secret: secrecy::SecretString,
    pub hash: [u8; 32],
    pub prefix: String,
    pub last4: String,
}

pub fn generate(kind: KeyKind, mut random: impl FnMut() -> [u8; 64]) -> NewSecret {
    let mut chars = String::with_capacity(RANDOM_LEN);
    while chars.len() < RANDOM_LEN {
        for b in random() {
            if chars.len() == RANDOM_LEN {
                break;
            }
            if b < 248 {
                chars.push(char::from(
                    BASE62.get(usize::from(b % 62)).copied().unwrap_or(b'0'),
                ));
            }
        }
    }
    let prefix: String = chars.chars().take(6).collect();
    let last4: String = chars.chars().skip(RANDOM_LEN - 4).collect();
    let full = match kind {
        KeyKind::Project => format!("{PROJECT_PREFIX}{chars}"),
        KeyKind::OrgPlatform => format!("{ORG_PREFIX}{chars}"),
    };
    NewSecret {
        hash: secret::hash(&full),
        secret: secrecy::SecretString::from(full),
        prefix,
        last4,
    }
}

pub fn kind_of(presented: &str) -> Option<KeyKind> {
    let (kind, rest) = if let Some(rest) = presented.strip_prefix(ORG_PREFIX) {
        (KeyKind::OrgPlatform, rest)
    } else {
        (KeyKind::Project, presented.strip_prefix(PROJECT_PREFIX)?)
    };
    (rest.len() == RANDOM_LEN && rest.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    Days(u32),
    Date(Date),
    Never,
}

pub const MAX_LIFETIME_DAYS: std::ops::RangeInclusive<i32> = 1..=3650;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExpiryError {
    #[error("Choose an expiry of 1, 7, 30 or 90 days, a date, or never.")]
    Choice,
    #[error("The expiry date must be in the future.")]
    Past,
    #[error("Keys in this project may live at most {0} days.")]
    TooLong(i32),
    #[error("Keys in this project must expire within {0} days; \"never\" is not allowed.")]
    NeverNotAllowed(i32),
}

pub fn max_lifetime(org: Option<i32>, project: Option<i32>) -> Option<i32> {
    match (org, project) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

pub fn expires_at(
    choice: Expiry,
    now: OffsetDateTime,
    max_days: Option<i32>,
) -> Result<Option<OffsetDateTime>, ExpiryError> {
    let at = match choice {
        Expiry::Days(d) if [1, 7, 30, 90].contains(&d) => Some(now + Duration::days(i64::from(d))),
        Expiry::Days(_) => return Err(ExpiryError::Choice),
        Expiry::Date(date) => {
            let at = date.midnight().assume_utc();
            if at <= now {
                return Err(ExpiryError::Past);
            }
            Some(at)
        }
        Expiry::Never => None,
    };
    match (at, max_days) {
        (None, Some(max)) => Err(ExpiryError::NeverNotAllowed(max)),
        (Some(at), Some(max)) if at > now + Duration::days(i64::from(max)) => {
            Err(ExpiryError::TooLong(max))
        }
        _ => Ok(at),
    }
}

pub const GRACE_HOURS: [u32; 4] = [0, 1, 24, 168];

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GraceError {
    #[error("Choose a grace period of 0, 1, 24 or 168 hours.")]
    Choice,
}

pub fn grace_ends_at(hours: u32, now: OffsetDateTime) -> Result<OffsetDateTime, GraceError> {
    if !GRACE_HOURS.contains(&hours) {
        return Err(GraceError::Choice);
    }
    Ok(now + Duration::hours(i64::from(hours)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    fn random() -> [u8; 64] {
        let mut b = [0u8; 64];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as u8).wrapping_mul(37).wrapping_add(11);
        }
        b
    }

    #[test]
    fn secrets_have_the_documented_shape() {
        let k = generate(KeyKind::Project, random);
        let s = secrecy::ExposeSecret::expose_secret(&k.secret).to_owned();
        assert!(s.starts_with("sk-kso-") && s.len() == 7 + 48, "{s}");
        assert_eq!(kind_of(&s), Some(KeyKind::Project));
        assert_eq!(&s[7..13], k.prefix);
        assert_eq!(&s[s.len() - 4..], k.last4);
        assert_eq!(k.hash, secret::hash(&s));
        let o = generate(KeyKind::OrgPlatform, random);
        let os = secrecy::ExposeSecret::expose_secret(&o.secret).to_owned();
        assert!(os.starts_with("sk-kso-org-") && os.len() == 11 + 48);
        assert_eq!(kind_of(&os), Some(KeyKind::OrgPlatform));
        for bad in [
            "sk-kso-short",
            "sk-other",
            "",
            "sk-kso-org-",
            &format!("sk-kso-{}", "-".repeat(48)),
        ] {
            assert_eq!(kind_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn bytes_that_would_bias_are_skipped() {
        let mut n = 0u8;
        let k = generate(KeyKind::Project, || {
            let mut b = [250u8; 64];
            for pair in b.chunks_mut(2) {
                if let Some(x) = pair.get_mut(1) {
                    *x = n;
                    n += 1;
                }
            }
            b
        });
        let s = secrecy::ExposeSecret::expose_secret(&k.secret).to_owned();
        let expected: String = (0..48).map(|i| char::from(BASE62[i % 62])).collect();
        assert_eq!(&s[7..], expected);
    }

    #[test]
    fn expiry_choices_are_the_offered_lifetimes() {
        let now = datetime!(2026-10-11 12:00 UTC);
        assert_eq!(
            expires_at(Expiry::Days(7), now, None).unwrap(),
            Some(now + Duration::days(7))
        );
        assert_eq!(
            expires_at(Expiry::Days(5), now, None),
            Err(ExpiryError::Choice)
        );
        assert_eq!(expires_at(Expiry::Never, now, None).unwrap(), None);
        assert_eq!(
            expires_at(Expiry::Never, now, Some(90)),
            Err(ExpiryError::NeverNotAllowed(90))
        );
        assert_eq!(
            expires_at(Expiry::Days(90), now, Some(30)),
            Err(ExpiryError::TooLong(30))
        );
        assert_eq!(
            expires_at(Expiry::Date(date!(2026 - 10 - 01)), now, None),
            Err(ExpiryError::Past)
        );
        assert_eq!(
            expires_at(Expiry::Date(date!(2026 - 10 - 20)), now, Some(30)).unwrap(),
            Some(datetime!(2026-10-20 0:00 UTC))
        );
        assert_eq!(max_lifetime(Some(90), Some(30)), Some(30));
        assert_eq!(max_lifetime(None, Some(30)), Some(30));
        assert_eq!(max_lifetime(None, None), None);
    }

    #[test]
    fn grace_periods_are_the_offered_choices() {
        let now = datetime!(2026-10-11 12:00 UTC);
        assert_eq!(grace_ends_at(0, now), Ok(now));
        assert_eq!(grace_ends_at(1, now), Ok(now + Duration::hours(1)));
        assert_eq!(grace_ends_at(24, now), Ok(now + Duration::days(1)));
        assert_eq!(grace_ends_at(168, now), Ok(now + Duration::days(7)));
        for other in [2, 12, 48, 720] {
            assert_eq!(
                grace_ends_at(other, now),
                Err(GraceError::Choice),
                "{other}"
            );
        }
    }
}
