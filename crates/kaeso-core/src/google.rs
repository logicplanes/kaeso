use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

pub const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

pub const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

pub const JWT_BEARER_GRANT: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

pub const MAX_KEY_LEN: usize = 16 * 1024;

pub const AGENT_PLATFORM: &str = "gemini-agent-platform";

pub const GLOBAL: &str = "global";

pub const EXPRESS_MODELS_PATH: &str = "/v1/publishers/google/models";

pub fn is_location(location: &str) -> bool {
    if matches!(location, GLOBAL | "us" | "eu") {
        return true;
    }
    let Some((area, rest)) = location.split_once('-') else {
        return false;
    };
    let digits = rest.trim_start_matches(|c: char| c.is_ascii_lowercase());
    !area.is_empty()
        && area.bytes().all(|b| b.is_ascii_lowercase())
        && digits.len() < rest.len()
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

pub fn host(location: &str) -> String {
    match location {
        GLOBAL => "aiplatform.googleapis.com".to_owned(),
        "us" | "eu" => format!("aiplatform.{location}.rep.googleapis.com"),
        region => format!("{region}-aiplatform.googleapis.com"),
    }
}

pub fn openapi_path(project: &str, location: &str) -> String {
    format!("/v1/projects/{project}/locations/{location}/endpoints/openapi")
}

pub fn publisher_path(project: &str, location: &str, publisher: &str) -> String {
    format!("/v1/projects/{project}/locations/{location}/publishers/{publisher}/models")
}

const UNIVERSE: &str = "googleapis.com";

pub struct ServiceAccountKey {
    project_id: String,
    client_email: String,
    private_key_id: String,
    private_key: SecretString,
}

impl fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("project_id", &self.project_id)
            .field("client_email", &self.client_email)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyProblem {
    #[error("not a JSON object")]
    NotJson,
    #[error("too long")]
    TooLong,
    #[error("not a service account key")]
    NotServiceAccount,
    #[error("`{0}` is missing or not a string")]
    Missing(&'static str),
    #[error("`{0}` is malformed")]
    Malformed(&'static str),
    #[error("`token_uri` names another host")]
    TokenUri,
    #[error("`universe_domain` names another universe")]
    Universe,
}

impl KeyProblem {
    pub fn field(self) -> &'static str {
        match self {
            KeyProblem::NotJson | KeyProblem::TooLong => "secret",
            KeyProblem::NotServiceAccount => "type",
            KeyProblem::Missing(f) | KeyProblem::Malformed(f) => f,
            KeyProblem::TokenUri => "token_uri",
            KeyProblem::Universe => "universe_domain",
        }
    }
}

#[derive(Deserialize)]
struct Fields {
    #[serde(rename = "type")]
    kind: Option<serde_json::Value>,
    project_id: Option<serde_json::Value>,
    private_key_id: Option<serde_json::Value>,
    private_key: Option<serde_json::Value>,
    client_email: Option<serde_json::Value>,
    token_uri: Option<serde_json::Value>,
    universe_domain: Option<serde_json::Value>,
}

fn text(v: Option<serde_json::Value>, name: &'static str) -> Result<String, KeyProblem> {
    match v {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Ok(s),
        _ => Err(KeyProblem::Missing(name)),
    }
}

// Project ids: 6 to 30 lowercase letters, digits and hyphens, from a letter, not ending in a
// hyphen; older projects carry a domain prefix such as `example.com:` (Resource Manager,
// "Creating and managing projects").
pub fn is_project_id(id: &str) -> bool {
    let (domain, name) = match id.rsplit_once(':') {
        Some((d, n)) => (Some(d), n),
        None => (None, id),
    };
    let domain_ok = domain.is_none_or(|d| {
        !d.is_empty()
            && d.len() <= 63
            && d.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
    });
    let bytes = name.as_bytes();
    domain_ok
        && (6..=30).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last().is_some_and(|b| *b != b'-')
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn is_client_email(email: &str) -> bool {
    email.len() <= 320
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.contains('@')
        })
        && email
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'@' | b'.' | b'-' | b'_'))
}

impl ServiceAccountKey {
    pub fn parse(json: &str) -> Result<ServiceAccountKey, KeyProblem> {
        if json.len() > MAX_KEY_LEN {
            return Err(KeyProblem::TooLong);
        }
        let fields: Fields = serde_json::from_str(json.trim()).map_err(|_| KeyProblem::NotJson)?;
        if fields.kind.as_ref().and_then(serde_json::Value::as_str) != Some("service_account") {
            return Err(KeyProblem::NotServiceAccount);
        }
        let project_id = text(fields.project_id, "project_id")?;
        if !is_project_id(&project_id) {
            return Err(KeyProblem::Malformed("project_id"));
        }
        let client_email = text(fields.client_email, "client_email")?;
        if !is_client_email(&client_email) {
            return Err(KeyProblem::Malformed("client_email"));
        }
        let private_key_id = text(fields.private_key_id, "private_key_id")?;
        if private_key_id.len() > 128 || !private_key_id.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(KeyProblem::Malformed("private_key_id"));
        }
        let private_key = SecretString::from(text(fields.private_key, "private_key")?);
        // The JWT is signed for this audience and Kaeso sends it only to Google's token
        // endpoint, so a key naming another host can't move the signed assertion elsewhere.
        if text(fields.token_uri, "token_uri")? != TOKEN_URI {
            return Err(KeyProblem::TokenUri);
        }
        match fields.universe_domain {
            None => {}
            Some(serde_json::Value::String(u)) if u == UNIVERSE => {}
            Some(_) => return Err(KeyProblem::Universe),
        }
        Ok(ServiceAccountKey {
            project_id,
            client_email,
            private_key_id,
            private_key,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn client_email(&self) -> &str {
        &self.client_email
    }

    pub fn private_key_id(&self) -> &str {
        &self.private_key_id
    }

    pub fn private_key_pem(&self) -> &str {
        self.private_key.expose_secret()
    }

    pub fn last4(&self) -> String {
        let n = self.private_key_id.chars().count();
        self.private_key_id
            .chars()
            .skip(n.saturating_sub(4))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>)) -> String {
        let mut v = serde_json::json!({
            "type": "service_account",
            "project_id": "kaeso-example",
            "private_key_id": "0123456789abcdef0123456789abcdef01234567",
            "private_key": "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
            "client_email": "kaeso@kaeso-example.iam.gserviceaccount.com",
            "client_id": "100000000000000000000",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "universe_domain": "googleapis.com"
        });
        if let Some(map) = v.as_object_mut() {
            edit(map);
        }
        v.to_string()
    }

    #[test]
    fn reads_a_service_account_key() {
        let k = ServiceAccountKey::parse(&key(|_| {})).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(k.project_id(), "kaeso-example");
        assert_eq!(
            k.client_email(),
            "kaeso@kaeso-example.iam.gserviceaccount.com"
        );
        assert_eq!(k.last4(), "4567");
        assert!(
            k.private_key_pem()
                .starts_with("-----BEGIN PRIVATE KEY-----")
        );
        let shown = format!("{k:?}");
        assert!(!shown.contains("PRIVATE"));
        assert!(!shown.contains("0123456789abcdef"));
        assert!(
            ServiceAccountKey::parse(&key(|m| {
                m.remove("universe_domain");
            }))
            .is_ok()
        );
    }

    #[test]
    fn refuses_keys_that_are_not_service_account_keys() {
        let cases: Vec<(String, KeyProblem)> = vec![
            ("sk-not-json".into(), KeyProblem::NotJson),
            ("[]".into(), KeyProblem::NotJson),
            (
                key(|m| {
                    m.insert("type".into(), "authorized_user".into());
                }),
                KeyProblem::NotServiceAccount,
            ),
            (
                key(|m| {
                    m.remove("private_key");
                }),
                KeyProblem::Missing("private_key"),
            ),
            (
                key(|m| {
                    m.insert("client_email".into(), 7.into());
                }),
                KeyProblem::Missing("client_email"),
            ),
            (
                key(|m| {
                    m.insert("client_email".into(), "no at sign".into());
                }),
                KeyProblem::Malformed("client_email"),
            ),
            (
                key(|m| {
                    m.insert("project_id".into(), "Bad_Project".into());
                }),
                KeyProblem::Malformed("project_id"),
            ),
            (
                key(|m| {
                    m.insert("private_key_id".into(), "a/b".into());
                }),
                KeyProblem::Malformed("private_key_id"),
            ),
            (
                key(|m| {
                    m.insert(
                        "token_uri".into(),
                        "https://attacker.example.com/token".into(),
                    );
                }),
                KeyProblem::TokenUri,
            ),
            (
                key(|m| {
                    m.insert(
                        "token_uri".into(),
                        "https://oauth2.googleapis.com/token?x".into(),
                    );
                }),
                KeyProblem::TokenUri,
            ),
            (
                key(|m| {
                    m.insert("universe_domain".into(), "example.com".into());
                }),
                KeyProblem::Universe,
            ),
            ("x".repeat(MAX_KEY_LEN + 1), KeyProblem::TooLong),
        ];
        for (json, problem) in cases {
            assert_eq!(
                ServiceAccountKey::parse(&json).err(),
                Some(problem),
                "{json}"
            );
        }
    }

    #[test]
    fn locations_name_their_host() {
        for (location, host) in [
            ("global", "aiplatform.googleapis.com"),
            ("us-central1", "us-central1-aiplatform.googleapis.com"),
            ("europe-west4", "europe-west4-aiplatform.googleapis.com"),
            ("us", "aiplatform.us.rep.googleapis.com"),
            ("eu", "aiplatform.eu.rep.googleapis.com"),
        ] {
            assert!(is_location(location), "{location}");
            assert_eq!(super::host(location), host);
        }
        for bad in [
            "",
            "Global",
            "us-central",
            "-central1",
            "us_central1",
            "us-central1.example.com",
            "us-1",
            "europe-west4/x",
        ] {
            assert!(!is_location(bad), "{bad}");
        }
        assert_eq!(
            publisher_path("kaeso-example", "global", "google"),
            "/v1/projects/kaeso-example/locations/global/publishers/google/models"
        );
    }

    #[test]
    fn project_ids_follow_googles_rule() {
        for ok in ["kaeso-example", "abcdef", "example.com:kaeso-1"] {
            assert!(is_project_id(ok), "{ok}");
        }
        for bad in [
            "short",
            "1abcdef",
            "abcdef-",
            "ABCDEFG",
            "a".repeat(31).as_str(),
            ":abcdef",
            "abc def",
        ] {
            assert!(!is_project_id(bad), "{bad}");
        }
    }
}
