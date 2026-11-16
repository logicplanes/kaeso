//! Signing in to Azure with a Microsoft Entra ID service principal: a tenant, an application
//! (client) id and a client secret, exchanged for access tokens with the client credentials grant
//! (https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-client-creds-grant-flow).

use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

pub const AZURE: &str = "azure";

/// Microsoft Entra ID's token service in Azure's global cloud.
pub const LOGIN: &str = "https://login.microsoftonline.com";

/// The scope Microsoft's examples for Azure OpenAI's v1 API ask for
/// (https://learn.microsoft.com/en-us/azure/ai-foundry/openai/api-version-lifecycle).
pub const COGNITIVE_SERVICES_SCOPE: &str = "https://cognitiveservices.azure.com/.default";

/// The scope some Microsoft Foundry pages ask for instead.
pub const FOUNDRY_SCOPE: &str = "https://ai.azure.com/.default";

/// The scopes a key may name; never a free value, so a key can only ask for tokens of Azure's
/// AI services.
pub const SCOPES: [&str; 2] = [COGNITIVE_SERVICES_SCOPE, FOUNDRY_SCOPE];

/// The role a service principal needs on the resource to call its models.
pub const ROLE: &str = "Cognitive Services OpenAI User";

const MAX_SECRET_LEN: usize = 1024;

/// A service principal as a key keeps it: one JSON object.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    tenant_id: String,
    client_id: String,
    client_secret: String,
    #[serde(default)]
    scope: Option<String>,
}

pub struct ServicePrincipal {
    tenant_id: String,
    client_id: String,
    client_secret: SecretString,
    scope: &'static str,
}

impl fmt::Debug for ServicePrincipal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServicePrincipal")
            .field("tenant_id", &self.tenant_id)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    NotJson,
    Missing(&'static str),
    Malformed(&'static str),
}

fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && parts
            .iter()
            .all(|p| p.bytes().all(|b| b.is_ascii_hexdigit()))
}

// A tenant is its id or one of its domains, such as `contoso.onmicrosoft.com`.
fn is_tenant(s: &str) -> bool {
    is_guid(s)
        || (s.len() <= 253
            && s.contains('.')
            && s.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            }))
}

impl ServicePrincipal {
    pub fn parse(json: &str) -> Result<ServicePrincipal, Problem> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|_| Problem::NotJson)?;
        if !v.is_object() {
            return Err(Problem::NotJson);
        }
        for field in ["tenant_id", "client_id", "client_secret"] {
            if v.get(field)
                .and_then(serde_json::Value::as_str)
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(Problem::Missing(field));
            }
        }
        let s: Stored = serde_json::from_value(v).map_err(|_| Problem::NotJson)?;
        let tenant_id = s.tenant_id.trim().to_ascii_lowercase();
        if !is_tenant(&tenant_id) {
            return Err(Problem::Malformed("tenant_id"));
        }
        let client_id = s.client_id.trim().to_ascii_lowercase();
        if !is_guid(&client_id) {
            return Err(Problem::Malformed("client_id"));
        }
        let secret = s.client_secret.trim();
        if secret.len() > MAX_SECRET_LEN || secret.contains(char::is_whitespace) {
            return Err(Problem::Malformed("client_secret"));
        }
        let scope = match s.scope.as_deref().map(str::trim) {
            None | Some("") => COGNITIVE_SERVICES_SCOPE,
            Some(wanted) => SCOPES
                .into_iter()
                .find(|s| *s == wanted)
                .ok_or(Problem::Malformed("scope"))?,
        };
        Ok(ServicePrincipal {
            tenant_id,
            client_id,
            client_secret: SecretString::from(secret.to_owned()),
            scope,
        })
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn scope(&self) -> &'static str {
        self.scope
    }

    pub fn client_secret(&self) -> &SecretString {
        &self.client_secret
    }

    /// The last four characters of the client id, which the list shows.
    pub fn last4(&self) -> String {
        let id = &self.client_id;
        id[id.len().saturating_sub(4)..].to_owned()
    }

    /// The form the key is stored in, normalized.
    pub fn sealed_form(&self) -> String {
        serde_json::json!({
            "tenant_id": self.tenant_id,
            "client_id": self.client_id,
            "client_secret": self.client_secret.expose_secret(),
            "scope": self.scope,
        })
        .to_string()
    }

    /// The token endpoint of the tenant at `login`, Entra ID's host.
    pub fn token_url(&self, login: &str) -> String {
        format!(
            "{}/{}/oauth2/v2.0/token",
            login.trim_end_matches('/'),
            self.tenant_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENANT: &str = "aaaabbbb-0000-cccc-1111-dddd2222eeee";
    const CLIENT: &str = "00001111-aaaa-2222-bbbb-3333cccc4444";

    fn json(tenant: &str, client: &str, secret: &str, scope: Option<&str>) -> String {
        let mut v = serde_json::json!({ "tenant_id": tenant, "client_id": client, "client_secret": secret });
        if let Some(s) = scope {
            v["scope"] = s.into();
        }
        v.to_string()
    }

    #[test]
    fn a_service_principal_is_read_and_normalized() {
        let sp = ServicePrincipal::parse(&json(
            &TENANT.to_uppercase(),
            CLIENT,
            " A1bC2dE3fH4iJ5kL6mN7oP8qR9sT0u ",
            None,
        ))
        .unwrap();
        assert_eq!((sp.tenant_id(), sp.client_id()), (TENANT, CLIENT));
        assert_eq!(sp.scope(), COGNITIVE_SERVICES_SCOPE);
        assert_eq!(sp.last4(), "4444");
        assert_eq!(
            sp.client_secret().expose_secret(),
            "A1bC2dE3fH4iJ5kL6mN7oP8qR9sT0u"
        );
        assert_eq!(
            sp.token_url("https://login.microsoftonline.com/"),
            format!("https://login.microsoftonline.com/{TENANT}/oauth2/v2.0/token")
        );
        let again = ServicePrincipal::parse(&sp.sealed_form()).unwrap();
        assert_eq!(again.scope(), COGNITIVE_SERVICES_SCOPE);
        assert!(!format!("{sp:?}").contains("A1bC"));
        let domain = ServicePrincipal::parse(&json(
            "contoso.onmicrosoft.com",
            CLIENT,
            "s",
            Some(FOUNDRY_SCOPE),
        ))
        .unwrap();
        assert_eq!(
            (domain.tenant_id(), domain.scope()),
            ("contoso.onmicrosoft.com", FOUNDRY_SCOPE)
        );
    }

    #[test]
    fn what_isnt_a_service_principal_is_refused_by_its_field() {
        for (input, problem) in [
            ("nope".to_owned(), Problem::NotJson),
            ("[]".to_owned(), Problem::NotJson),
            (json(TENANT, CLIENT, " ", None), Problem::Missing("client_secret")),
            (json("contoso", CLIENT, "s", None), Problem::Malformed("tenant_id")),
            (json("-a.com", CLIENT, "s", None), Problem::Malformed("tenant_id")),
            (json(TENANT, "client", "s", None), Problem::Malformed("client_id")),
            (json(TENANT, CLIENT, "a b", None), Problem::Malformed("client_secret")),
            (json(TENANT, CLIENT, "s", Some("https://graph.microsoft.com/.default")), Problem::Malformed("scope")),
            (
                serde_json::json!({ "tenant_id": TENANT, "client_id": CLIENT, "client_secret": "s", "extra": 1 }).to_string(),
                Problem::NotJson,
            ),
        ] {
            assert_eq!(ServicePrincipal::parse(&input).err(), Some(problem), "{input}");
        }
        assert!(!is_tenant(&"a.".repeat(200)));
    }
}
