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
    if local.is_empty() || local.len() > 64 || local.contains('@') {
        return Err(invalid());
    }
    let domain = normalize_domain(domain).ok_or_else(invalid)?;
    let address = format!("{local}@{domain}");
    if address.len() > MAX_LEN {
        return Err(invalid());
    }
    Ok(address)
}

pub fn normalize_domain(input: &str) -> Option<String> {
    if input.is_empty() || input.contains(['@', '/', ':', '*', '%']) {
        return None;
    }
    let ascii = idna::domain_to_ascii(input).ok()?;
    let label_ok = |l: &str| {
        (1..=63).contains(&l.len())
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    let ok = ascii.len() <= 253
        && ascii.split('.').all(label_ok)
        && (ascii.contains('.') || ascii == "localhost");
    ok.then_some(ascii)
}

pub fn display_domain(domain: &str) -> String {
    let (unicode, ok) = idna::domain_to_unicode(domain);
    if ok.is_ok() {
        unicode
    } else {
        domain.to_owned()
    }
}

pub fn stored_form(search: &str) -> Option<String> {
    if search.is_ascii() {
        return None;
    }
    let (local, domain) = match search.rsplit_once('@') {
        Some((l, d)) => (Some(l), d),
        None => (None, search),
    };
    let ascii = idna::domain_to_ascii(domain).ok()?;
    Some(match local {
        Some(l) => format!("{l}@{ascii}"),
        None => ascii,
    })
}

pub fn domain(email: &str) -> &str {
    email.rsplit_once('@').map_or("", |(_, d)| d)
}

const PUBLIC_MAIL: &[&str] = &[
    "126.com",
    "163.com",
    "aol.com",
    "fastmail.com",
    "foxmail.com",
    "gmail.com",
    "gmx.at",
    "gmx.ch",
    "gmx.com",
    "gmx.de",
    "gmx.net",
    "googlemail.com",
    "hey.com",
    "hotmail.com",
    "icloud.com",
    "live.com",
    "mac.com",
    "mail.com",
    "mail.ru",
    "me.com",
    "msn.com",
    "outlook.com",
    "pm.me",
    "proton.me",
    "protonmail.com",
    "qq.com",
    "sina.com",
    "t-online.de",
    "tuta.io",
    "tutanota.com",
    "web.de",
    "yahoo.com",
    "yandex.com",
    "yandex.ru",
    "ymail.com",
    "zoho.com",
];

const PUBLIC_MAIL_BRANDS: &[&str] = &["gmx", "hotmail", "live", "outlook", "yahoo", "yandex"];

pub fn is_public_mail(domain: &str) -> bool {
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    if PUBLIC_MAIL.contains(&domain.as_str()) {
        return true;
    }
    let mut labels = domain.split('.');
    let brand = labels.next().unwrap_or_default();
    let suffix: Vec<&str> = labels.collect();
    PUBLIC_MAIL_BRANDS.contains(&brand)
        && (1..=2).contains(&suffix.len())
        && suffix.iter().all(|l| (2..=3).contains(&l.len()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_mail_domains_are_known_in_every_country() {
        for public in [
            "gmail.com",
            "GMail.com.",
            "yahoo.co.uk",
            "hotmail.de",
            "outlook.com",
            "proton.me",
        ] {
            assert!(is_public_mail(public), "{public}");
        }
        for own in [
            "acme.example",
            "outlook.acme.com",
            "mail.acme.example",
            "yahoo-fans.org",
        ] {
            assert!(!is_public_mail(own), "{own}");
        }
    }

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
    fn internationalised_domains_have_one_form_however_they_are_typed() {
        for typed in [
            "ada@bücher.example",
            "ada@BÜCHER.example",
            "ada@Bu\u{308}cher.example",
            "ada@xn--bcher-kva.example",
            "ada@XN--BCHER-KVA.EXAMPLE",
            "ada@bücher\u{ff0e}example",
        ] {
            assert_eq!(
                normalize(typed).unwrap(),
                "ada@xn--bcher-kva.example",
                "{typed}"
            );
        }
        assert_eq!(
            normalize("Ada@例え.テスト").unwrap(),
            "Ada@xn--r8jz45g.xn--zckzah"
        );
        assert_eq!(
            normalize_domain("BÜCHER.example").unwrap(),
            "xn--bcher-kva.example"
        );
        assert_eq!(display_domain("xn--bcher-kva.example"), "bücher.example");
        assert_eq!(display_domain("example.com"), "example.com");
        assert_eq!(
            stored_form("ada@bücher.example").unwrap(),
            "ada@xn--bcher-kva.example"
        );
        assert_eq!(stored_form("BÜCHER").unwrap(), "xn--bcher-kva");
        assert_eq!(stored_form("ada@example"), None);
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
            "a@example.com.",
            "a@exa_mple.com",
            "a@xn--zz.example",
            "a@example.com/x",
            &format!("{}@example.com", "x".repeat(65)),
            &format!("a@{}.com", "x".repeat(64)),
            &format!("{}@{}.com", "x".repeat(60), "ü".repeat(60)),
        ] {
            assert!(normalize(bad).is_err(), "{bad}");
        }
    }
}
