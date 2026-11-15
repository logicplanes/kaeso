//! Requests of a provider whose keys each belong to one resource with an address of its own,
//! such as an Azure resource's `https://{resource}.openai.azure.com`: the key's endpoint, with the
//! provider's path, takes the place of the provider's base URL, unless the install sends every
//! request of the provider elsewhere by setting that base URL.

use kaeso_core::providers::ProviderRef;
use kaeso_kpp::KppError;
use url::Url;

use crate::services::credentials::Resolved;
use crate::state::AppState;

/// The base URL of the key's requests, `None` for a provider whose keys carry no endpoint. A key
/// without one, which the Platform API refuses for such a provider, is an error rather than a
/// request to the default address.
pub fn base(
    state: &AppState,
    provider: &ProviderRef,
    credential: &Resolved,
) -> Result<Option<Url>, KppError> {
    let configured = crate::model_api::pipeline::base_url(state, provider).map(Url::as_str);
    of(provider, configured, credential.endpoint.as_deref())
}

fn of(
    provider: &ProviderRef,
    configured: Option<&str>,
    endpoint: Option<&str>,
) -> Result<Option<Url>, KppError> {
    let Some(p) = provider.built_in() else {
        return Ok(None);
    };
    let Some(rule) = p.key_endpoint else {
        return Ok(None);
    };
    let root = match configured.map(|c| c.trim_end_matches('/')) {
        Some(c) if c != p.default_base_url => c,
        _ => endpoint.ok_or_else(|| {
            KppError::provider(format!(
                "The {} key names no endpoint; edit it and give its resource's endpoint.",
                p.name
            ))
        })?,
    };
    Url::parse(&rule.base(root)).map(Some).map_err(|_| {
        KppError::provider(format!(
            "The {} key's endpoint isn't a URL; edit it and give its resource's endpoint.",
            p.name
        ))
    })
}

#[cfg(test)]
mod tests {
    use kaeso_core::provider::provider;

    use super::*;

    fn azure() -> ProviderRef {
        ProviderRef::from(provider("azure").unwrap())
    }

    #[test]
    fn an_azure_key_calls_its_resource_unless_the_install_sends_azure_elsewhere() {
        let endpoint = Some("https://example.openai.azure.com");
        let base = |configured| of(&azure(), configured, endpoint).unwrap().unwrap();
        assert_eq!(
            base(Some("https://openai.azure.com/")).as_str(),
            "https://example.openai.azure.com/openai"
        );
        assert_eq!(
            base(None).as_str(),
            "https://example.openai.azure.com/openai"
        );
        assert_eq!(
            base(Some("https://proxy.example.com/azure")).as_str(),
            "https://proxy.example.com/azure/openai"
        );
        let missing = of(&azure(), Some("https://openai.azure.com/"), None).unwrap_err();
        assert!(missing.message.contains("names no endpoint"), "{missing:?}");
    }

    #[test]
    fn other_providers_keep_their_base_url() {
        let openai = ProviderRef::from(provider("openai").unwrap());
        assert_eq!(
            of(&openai, Some("https://api.openai.com/"), None).unwrap(),
            None
        );
    }
}
