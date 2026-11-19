//! Linked billing: a provider key that is a link to the customer's own organization at the
//! provider, which the provider bills, instead of a secret. A provider offers it by declaring a
//! [`LinkedBilling`] here; the gateway's adapter for it does the I/O.

use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

use crate::provider::Provider;

/// Which adapter in the gateway speaks a provider's billing connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Adapter {
    /// Anthropic's linked billing on the Claude Platform: an OAuth app per install, pushed
    /// authorization requests with PKCE, connection-bound tokens.
    Anthropic,
}

impl Adapter {
    pub const fn as_str(self) -> &'static str {
        match self {
            Adapter::Anthropic => "anthropic",
        }
    }
}

/// What a provider offers for billing connections, and where its customers manage them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkedBilling {
    pub provider: &'static str,
    pub adapter: Adapter,
    /// What a person links, as the console names it: "Claude organization".
    pub organization: &'static str,
    /// Where the provider's console lives, for "set the spend limit in …".
    pub console: &'static str,
    /// Where the install's admin registers the install's OAuth app.
    pub apps_url: &'static str,
    /// Where the linked organization sets the app's spend limit and unlinks it.
    pub spend_limit_url: &'static str,
    /// The roles a person needs in their organization to approve a link.
    pub roles: &'static [&'static str],
    /// The provider's page about it, for the setup steps.
    pub docs_url: &'static str,
}

pub const ANTHROPIC: LinkedBilling = LinkedBilling {
    provider: "anthropic",
    adapter: Adapter::Anthropic,
    organization: "Claude organization",
    console: "Claude Console",
    apps_url: "https://platform.claude.com/settings/oauth-apps",
    spend_limit_url: "https://platform.claude.com/linked-platforms",
    roles: &["Admin", "Billing"],
    docs_url: "https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections",
};

/// Every provider that offers billing connections.
pub const ALL: &[LinkedBilling] = &[ANTHROPIC];

pub fn of(provider: &str) -> Option<&'static LinkedBilling> {
    ALL.iter().find(|l| l.provider == provider)
}

impl Provider {
    pub fn linked_billing(&self) -> Option<&'static LinkedBilling> {
        of(self.id)
    }
}

/// The path, under the install's public URL, the provider sends the browser back to after a
/// link is approved; the console serves it.
pub const CALLBACK_PATH: &str = "/linked-billing/callback";

/// The path, under the install's public URL, of the OpenID issuer that signs the install's
/// client assertions; its discovery document and keys are served below it.
pub const ISSUER_PATH: &str = "/platform/v1/linked-billing/oidc";

/// The subject of every client assertion the install signs for a provider.
pub fn subject(provider: &str) -> String {
    format!("kaeso:linked-billing:{provider}")
}

/// The OpenID issuer URL of an install, `https://kaeso.example.com/platform/v1/linked-billing/oidc`.
pub fn issuer(public_url: &str) -> String {
    format!("{}{ISSUER_PATH}", public_url.trim_end_matches('/'))
}

/// The redirect URI an install registers with the provider.
pub fn redirect_uri(public_url: &str) -> String {
    format!("{}{CALLBACK_PATH}", public_url.trim_end_matches('/'))
}

/// When a spend limit reached now lets requests through again at the latest: Anthropic refuses
/// until 00:00 UTC on the first day of the next month, unless the customer raises the limit.
pub fn spend_limit_resets(at: OffsetDateTime) -> OffsetDateTime {
    let at = at.to_offset(UtcOffset::UTC);
    let (year, month) = match at.month() {
        Month::December => (at.year() + 1, Month::January),
        m => (at.year(), m.next()),
    };
    Date::from_calendar_date(year, month, 1)
        .map(|d| d.with_time(Time::MIDNIGHT).assume_utc())
        .unwrap_or(at)
}

/// Whether a linked key may send a request of this kind. Anthropic bills a connection's token
/// on Messages (and its OpenAI-compatible chat) and accepts it for token counting and the
/// models listing; batches are refused and nothing else is billed to the customer
/// (https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections#call-claude-with-a-connections-access-token).
pub const fn covers(kind: kaeso_kpp::RequestKind) -> bool {
    matches!(
        kind,
        kaeso_kpp::RequestKind::Generate | kaeso_kpp::RequestKind::CountTokens
    )
}

/// Why a linked key can't make a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The linked organization reached a spend limit it set, or has no credit that can pay.
    SpendLimit,
    /// The provider can't bill the linked organization right now.
    BillingUnavailable,
    /// The link ended, or the organization can't be billed this way.
    Unlinked,
    /// The install's app for linked billing is missing, was replaced or is refused.
    NeedsSetup,
}

/// The refusal an Anthropic error code on a linked request stands for, from the error codes of
/// https://platform.claude.com/docs/en/manage-claude/oauth-apps/oauth-reference#billing-errors.
pub fn anthropic_refusal(status: u16, code: Option<&str>) -> Option<Refusal> {
    match (status, code) {
        (402, Some("linked_organization_spend_limit_reached")) => Some(Refusal::SpendLimit),
        (402, Some("linked_organization_billing_unavailable")) => Some(Refusal::BillingUnavailable),
        (403, Some("user_profile_not_billable")) => Some(Refusal::Unlinked),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn only_anthropic_offers_linked_billing() {
        let offering: Vec<&str> = crate::provider::PROVIDERS
            .iter()
            .filter(|p| p.linked_billing().is_some())
            .map(|p| p.id)
            .collect();
        assert_eq!(offering, ["anthropic"]);
        assert_eq!(of("openai"), None);
        assert_eq!(Adapter::Anthropic.as_str(), "anthropic");
    }

    #[test]
    fn the_issuer_and_redirect_uri_hang_off_the_public_url() {
        assert_eq!(
            issuer("https://kaeso.example.com/"),
            "https://kaeso.example.com/platform/v1/linked-billing/oidc"
        );
        assert_eq!(
            redirect_uri("https://kaeso.example.com"),
            "https://kaeso.example.com/linked-billing/callback"
        );
        assert_eq!(subject("anthropic"), "kaeso:linked-billing:anthropic");
    }

    #[test]
    fn a_spend_limit_lifts_at_midnight_utc_on_the_first_of_the_next_month() {
        assert_eq!(
            spend_limit_resets(datetime!(2026-11-17 21:00 UTC)),
            datetime!(2026-12-01 00:00 UTC)
        );
        assert_eq!(
            spend_limit_resets(datetime!(2026-12-31 23:59 -02:00)),
            datetime!(2027-02-01 00:00 UTC),
            "January 1st, 01:59 UTC"
        );
    }

    #[test]
    fn a_linked_key_covers_generation_and_token_counting_only() {
        use kaeso_kpp::RequestKind::*;
        let covered: Vec<_> = [
            Generate,
            CountTokens,
            Embed,
            Image,
            Speech,
            Transcription,
            Video,
            Decide,
            Rerank,
        ]
        .into_iter()
        .filter(|k| covers(*k))
        .collect();
        assert_eq!(covered, [Generate, CountTokens]);
    }

    #[test]
    fn anthropic_codes_map_to_refusals() {
        assert_eq!(
            anthropic_refusal(402, Some("linked_organization_spend_limit_reached")),
            Some(Refusal::SpendLimit)
        );
        assert_eq!(
            anthropic_refusal(402, Some("linked_organization_billing_unavailable")),
            Some(Refusal::BillingUnavailable)
        );
        assert_eq!(
            anthropic_refusal(403, Some("user_profile_not_billable")),
            Some(Refusal::Unlinked)
        );
        assert_eq!(anthropic_refusal(402, None), None);
        assert_eq!(anthropic_refusal(403, Some("other")), None);
    }
}
