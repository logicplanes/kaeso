//! Requests of a provider whose API path names the account the key belongs to, such as
//! Cloudflare AI Gateway's `/client/v4/accounts/{account_id}/ai/v1`: the key's account becomes the
//! dialects' base path, and its gateway, when it names one, a header.

use kaeso_codecs::{EgressCtx, HeaderList};
use kaeso_core::provider::AccountScope;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::KppError;

use crate::services::credentials::Resolved;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scoped {
    pub base_path: String,
    gateway: Option<(&'static str, String)>,
}

impl Scoped {
    pub fn apply(&self, ctx: EgressCtx) -> EgressCtx {
        ctx.with_api_root(self.base_path.clone())
    }

    pub fn add_headers(&self, headers: &mut HeaderList) {
        if let Some((name, value)) = &self.gateway {
            headers.push(*name, value.clone());
        }
    }
}

/// The base path and gateway header of the key's account, `None` for a provider whose API names
/// no account. A key without an account, which the database refuses for such a provider, is an
/// error rather than a request to another account's path.
pub fn of(provider: &ProviderRef, credential: &Resolved) -> Result<Option<Scoped>, KppError> {
    let Some(scope) = provider.built_in().and_then(|p| p.account_scope) else {
        return Ok(None);
    };
    let Some(key) = &credential.scope else {
        return Err(KppError::provider(format!(
            "The {} key names no account; edit it and give its account id.",
            provider.name()
        )));
    };
    Ok(Some(scoped(
        scope,
        &key.account_id,
        key.gateway_id.as_deref(),
    )))
}

fn scoped(scope: AccountScope, account: &str, gateway: Option<&str>) -> Scoped {
    Scoped {
        base_path: scope.base_path(account),
        gateway: gateway.map(|g| (scope.gateway_header, g.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use kaeso_core::provider::provider;

    use super::*;

    #[test]
    fn a_cloudflare_key_calls_its_account_and_names_its_gateway() {
        let scope = provider("cloudflare-ai-gateway")
            .unwrap()
            .account_scope
            .unwrap();
        let s = scoped(
            scope,
            "0123456789abcdef0123456789abcdef",
            Some("production"),
        );
        assert_eq!(
            s.base_path,
            "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1"
        );
        let mut headers = HeaderList::new();
        s.add_headers(&mut headers);
        assert_eq!(headers.get("cf-aig-gateway-id"), Some("production"));
        let mut none = HeaderList::new();
        scoped(scope, "0123456789abcdef0123456789abcdef", None).add_headers(&mut none);
        assert_eq!(none.iter().count(), 0);
    }
}
