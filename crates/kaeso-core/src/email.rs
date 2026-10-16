#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a valid email address")]
pub struct InvalidEmail(pub String);

pub const MAX_LEN: usize = 254;

pub fn normalize(input: &str) -> Result<String, InvalidEmail> {
    let s = input.trim();
    let invalid = || InvalidEmail(s.chars().take(64).collect());
    if s.is_empty() || s.len() > MAX_LEN || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid());
    }
    let (local, domain) = s.rsplit_once('@').ok_or_else(invalid)?;
    if local.is_empty() || local.len() > 64 || local.contains('@') || domain.is_empty() {
        return Err(invalid());
    }
    let domain = domain.to_ascii_lowercase();
    let labels_ok = domain.split('.').all(|l| {
        !l.is_empty()
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_alphanumeric() || c == '-')
    });
    if !labels_ok || !(domain.contains('.') || domain == "localhost") {
        return Err(invalid());
    }
    Ok(format!("{local}@{domain}"))
}

pub fn domain(email: &str) -> &str {
    email.rsplit_once('@').map_or("", |(_, d)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_trimmed_and_domains_lowercased() {
        assert_eq!(
            normalize("  Ada.Lovelace@Example.COM ").unwrap(),
            "Ada.Lovelace@example.com"
        );
        assert_eq!(normalize("owner@localhost").unwrap(), "owner@localhost");
        assert_eq!(domain("a@example.com"), "example.com");
    }

    #[test]
    fn malformed_addresses_are_refused() {
        for bad in [
            "",
            "plain",
            "@example.com",
            "a@",
            "a b@example.com",
            "a@exa mple.com",
            "a@example",
            "a@-x.com",
            "a@x..com",
            &format!("{}@example.com", "x".repeat(65)),
        ] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
    }
}
