use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Who may create an account without an invitation.")
)]
pub enum AccountCreation {
    InvitationOnly,
    AllowedDomains,
    Everyone,
}

impl AccountCreation {
    pub fn as_str(self) -> &'static str {
        match self {
            AccountCreation::InvitationOnly => "invitation_only",
            AccountCreation::AllowedDomains => "allowed_domains",
            AccountCreation::Everyone => "everyone",
        }
    }

    pub fn from_stored(s: &str) -> AccountCreation {
        match s {
            "everyone" => AccountCreation::Everyone,
            "allowed_domains" => AccountCreation::AllowedDomains,
            _ => AccountCreation::InvitationOnly,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Who may create organizations.")
)]
pub enum OrgCreation {
    InstanceAdmins,
    AllowedDomains,
    Everyone,
}

impl OrgCreation {
    pub fn as_str(self) -> &'static str {
        match self {
            OrgCreation::InstanceAdmins => "instance_admins",
            OrgCreation::AllowedDomains => "allowed_domains",
            OrgCreation::Everyone => "everyone",
        }
    }

    pub fn from_stored(s: &str) -> OrgCreation {
        match s {
            "everyone" => OrgCreation::Everyone,
            "allowed_domains" => OrgCreation::AllowedDomains,
            _ => OrgCreation::InstanceAdmins,
        }
    }
}

pub const MAX_DOMAINS: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a domain")]
pub struct InvalidDomain(pub String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainRule {
    pub subdomains: bool,
    pub domain: String,
}

impl DomainRule {
    pub fn parse(input: &str) -> Result<DomainRule, InvalidDomain> {
        let trimmed = input.trim();
        let invalid = || InvalidDomain(trimmed.chars().take(64).collect());
        let (subdomains, rest) = match trimmed.strip_prefix("*.") {
            Some(rest) => (true, rest),
            None => (false, trimmed.strip_prefix('@').unwrap_or(trimmed)),
        };
        let domain = ascii_domain(rest).ok_or_else(invalid)?;
        Ok(DomainRule { subdomains, domain })
    }

    pub fn from_stored(s: &str) -> Option<DomainRule> {
        DomainRule::parse(s).ok()
    }

    pub fn stored(&self) -> String {
        if self.subdomains {
            format!("*.{}", self.domain)
        } else {
            self.domain.clone()
        }
    }

    pub fn display(&self) -> String {
        let domain = crate::email::display_domain(&self.domain);
        if self.subdomains {
            format!("*.{domain}")
        } else {
            domain
        }
    }

    pub fn allows(&self, domain: &str) -> bool {
        if self.subdomains {
            domain
                .strip_suffix(self.domain.as_str())
                .is_some_and(|head| head.len() > 1 && head.ends_with('.'))
        } else {
            domain == self.domain
        }
    }
}

fn ascii_domain(input: &str) -> Option<String> {
    let input = input.strip_suffix('.').unwrap_or(input);
    if input.is_empty() || input.contains(['*', '@', '/', ':']) {
        return None;
    }
    let ascii = idna::domain_to_ascii(input).ok()?;
    let labels: Vec<&str> = ascii.split('.').collect();
    let label_ok = |l: &&str| {
        (1..=63).contains(&l.len())
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    let top_ok = labels
        .last()
        .is_some_and(|t| !t.bytes().all(|b| b.is_ascii_digit()));
    (ascii.len() <= 253 && labels.len() >= 2 && labels.iter().all(label_ok) && top_ok)
        .then_some(ascii)
}

pub fn email_domain(email: &str) -> Option<String> {
    let domain = crate::email::domain(email);
    idna::domain_to_ascii(domain.strip_suffix('.').unwrap_or(domain)).ok()
}

pub fn parse_domains(inputs: &[String]) -> Result<Vec<DomainRule>, (usize, InvalidDomain)> {
    let mut rules = inputs
        .iter()
        .enumerate()
        .map(|(i, d)| DomainRule::parse(d).map_err(|e| (i, e)))
        .collect::<Result<Vec<_>, _>>()?;
    rules.sort_by(|a, b| (&a.domain, a.subdomains).cmp(&(&b.domain, b.subdomains)));
    rules.dedup();
    Ok(rules)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountPolicy {
    pub account_creation: AccountCreation,
    pub org_creation: OrgCreation,
    pub allowed_domains: Vec<DomainRule>,
}

impl AccountPolicy {
    pub fn domain_allowed(&self, email: &str) -> bool {
        email_domain(email).is_some_and(|d| self.allowed_domains.iter().any(|r| r.allows(&d)))
    }

    pub fn may_sign_up(&self, email: &str) -> bool {
        match self.account_creation {
            AccountCreation::InvitationOnly => false,
            AccountCreation::AllowedDomains => self.domain_allowed(email),
            AccountCreation::Everyone => true,
        }
    }

    pub fn sign_up_open(&self) -> bool {
        self.account_creation != AccountCreation::InvitationOnly
    }

    pub fn may_create_orgs(&self, is_instance_admin: bool, email: Option<&str>) -> bool {
        is_instance_admin
            || match self.org_creation {
                OrgCreation::InstanceAdmins => false,
                OrgCreation::AllowedDomains => email.is_some_and(|e| self.domain_allowed(e)),
                OrgCreation::Everyone => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(
        account_creation: AccountCreation,
        org_creation: OrgCreation,
        domains: &[&str],
    ) -> AccountPolicy {
        let domains: Vec<String> = domains.iter().map(|d| (*d).to_owned()).collect();
        AccountPolicy {
            account_creation,
            org_creation,
            allowed_domains: parse_domains(&domains).unwrap(),
        }
    }

    #[test]
    fn entries_are_read_as_people_type_them() {
        for (input, stored) in [
            ("example.com", "example.com"),
            ("  Example.COM. ", "example.com"),
            ("@example.com", "example.com"),
            ("*.Example.com", "*.example.com"),
            ("bücher.example", "xn--bcher-kva.example"),
            ("BÜCHER.example", "xn--bcher-kva.example"),
            ("xn--bcher-kva.example", "xn--bcher-kva.example"),
            ("*.bücher.example", "*.xn--bcher-kva.example"),
            ("例え.テスト", "xn--r8jz45g.xn--zckzah"),
        ] {
            assert_eq!(
                DomainRule::parse(input).unwrap().stored(),
                stored,
                "{input}"
            );
        }
        assert_eq!(
            DomainRule::parse("xn--bcher-kva.example")
                .unwrap()
                .display(),
            "bücher.example"
        );
        assert_eq!(
            DomainRule::parse("*.BÜCHER.example").unwrap().display(),
            "*.bücher.example"
        );
    }

    #[test]
    fn non_domains_are_refused() {
        for bad in [
            "",
            "*",
            "*.",
            "*.com.",
            "com",
            "localhost",
            "a@example.com",
            "ex ample.com",
            "example..com",
            "-example.com",
            "example-.com",
            "https://example.com",
            "example.com/path",
            "example.com:443",
            "10.0.0.1",
            "**.example.com",
            "ex*ample.com",
            &format!("{}.com", "x".repeat(64)),
        ] {
            assert!(DomainRule::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn lists_are_sorted_without_duplicates_and_name_the_bad_entry() {
        let list: Vec<String> = ["b.example", "A.example", "a.example", "*.a.example"]
            .map(String::from)
            .to_vec();
        let parsed: Vec<String> = parse_domains(&list)
            .unwrap()
            .iter()
            .map(DomainRule::stored)
            .collect();
        assert_eq!(parsed, ["a.example", "*.a.example", "b.example"]);
        let bad: Vec<String> = ["a.example", "nope"].map(String::from).to_vec();
        assert_eq!(parse_domains(&bad).unwrap_err().0, 1);
    }

    #[test]
    fn invitation_only_lets_nobody_sign_up() {
        let p = policy(
            AccountCreation::InvitationOnly,
            OrgCreation::InstanceAdmins,
            &["example.com"],
        );
        assert!(!p.may_sign_up("ada@example.com"));
        assert!(!p.sign_up_open());
    }

    #[test]
    fn everyone_lets_every_address_sign_up() {
        let p = policy(AccountCreation::Everyone, OrgCreation::InstanceAdmins, &[]);
        assert!(p.may_sign_up("ada@example.com"));
        assert!(p.may_sign_up("ada@elsewhere.example"));
        assert!(p.sign_up_open());
    }

    #[test]
    fn allowed_domains_match_exactly_or_by_subdomain() {
        let p = policy(
            AccountCreation::AllowedDomains,
            OrgCreation::InstanceAdmins,
            &["example.com", "*.corp.example"],
        );
        assert!(p.sign_up_open());
        assert!(p.may_sign_up("ada@example.com"));
        assert!(
            !p.may_sign_up("ada@eu.example.com"),
            "no subdomains without *."
        );
        assert!(!p.may_sign_up("ada@badexample.com"));
        assert!(!p.may_sign_up("ada@example.com.evil.example"));
        assert!(p.may_sign_up("ada@eu.corp.example"));
        assert!(p.may_sign_up("ada@a.b.corp.example"));
        assert!(!p.may_sign_up("ada@corp.example"), "*. is only subdomains");
        assert!(!p.may_sign_up("ada@xcorp.example"));
    }

    #[test]
    fn domains_compare_without_case_and_across_unicode_and_punycode() {
        let p = policy(
            AccountCreation::AllowedDomains,
            OrgCreation::InstanceAdmins,
            &["Example.COM", "bücher.example"],
        );
        let normalized = crate::email::normalize("Ada@EXAMPLE.com").unwrap();
        assert!(p.may_sign_up(&normalized));
        assert!(p.may_sign_up("ada@example.com."));
        assert!(p.may_sign_up("ada@xn--bcher-kva.example"));
        assert!(p.may_sign_up("ada@bücher.example"));
        assert!(p.may_sign_up("ada@BÜCHER.example"));
        assert!(!p.may_sign_up("ada@bucher.example"));
    }

    #[test]
    fn org_creation_follows_its_setting_and_admins_always_may() {
        let admins = policy(
            AccountCreation::Everyone,
            OrgCreation::InstanceAdmins,
            &["example.com"],
        );
        assert!(!admins.may_create_orgs(false, Some("ada@example.com")));
        assert!(admins.may_create_orgs(true, None));
        let everyone = policy(AccountCreation::InvitationOnly, OrgCreation::Everyone, &[]);
        assert!(everyone.may_create_orgs(false, Some("ada@elsewhere.example")));
        let domains = policy(
            AccountCreation::InvitationOnly,
            OrgCreation::AllowedDomains,
            &["*.example.com"],
        );
        assert!(domains.may_create_orgs(false, Some("ada@eu.example.com")));
        assert!(!domains.may_create_orgs(false, Some("ada@example.com")));
        assert!(
            !domains.may_create_orgs(false, None),
            "a deleted user's address"
        );
        assert!(domains.may_create_orgs(true, Some("ada@elsewhere.example")));
    }

    #[test]
    fn stored_names_round_trip_and_unknown_ones_are_closed() {
        for a in [
            AccountCreation::InvitationOnly,
            AccountCreation::AllowedDomains,
            AccountCreation::Everyone,
        ] {
            assert_eq!(AccountCreation::from_stored(a.as_str()), a);
        }
        for o in [
            OrgCreation::InstanceAdmins,
            OrgCreation::AllowedDomains,
            OrgCreation::Everyone,
        ] {
            assert_eq!(OrgCreation::from_stored(o.as_str()), o);
        }
        assert_eq!(
            AccountCreation::from_stored("public"),
            AccountCreation::InvitationOnly
        );
        assert_eq!(OrgCreation::from_stored("all"), OrgCreation::InstanceAdmins);
    }
}
