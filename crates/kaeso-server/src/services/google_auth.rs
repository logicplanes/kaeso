use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_codecs::HeaderList;
use kaeso_core::google::{JWT_BEARER_GRANT, SCOPE, ServiceAccountKey, TOKEN_URI};
use kaeso_crypto::RsaSigner;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts, UpstreamError};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::state::AppState;

// google-auth signs assertions for an hour, the longest the token endpoint accepts
// (https://developers.google.com/identity/protocols/oauth2/service-account#authorizingrequests).
const ASSERTION_LIFETIME: time::Duration = time::Duration::hours(1);
const RENEW_BEFORE: time::Duration = time::Duration::minutes(5);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ANSWER: usize = 64 * 1024;

pub struct ServiceAccount {
    pub key: ServiceAccountKey,
    pub org: Uuid,
    pub location: String,
    signer: RsaSigner,
}

impl std::fmt::Debug for ServiceAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAccount")
            .field("client_email", &self.key.client_email())
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}

impl ServiceAccount {
    pub fn open(json: &str, org: Uuid, location: String) -> Option<ServiceAccount> {
        let key = ServiceAccountKey::parse(json).ok()?;
        let signer = RsaSigner::from_pkcs8_pem(key.private_key_pem()).ok()?;
        Some(ServiceAccount {
            key,
            org,
            location,
            signer,
        })
    }

    pub fn assertion(
        &self,
        now: OffsetDateTime,
    ) -> Result<SecretString, kaeso_crypto::CryptoError> {
        let header = serde_json::json!({
            "alg": "RS256",
            "typ": "JWT",
            "kid": self.key.private_key_id(),
        });
        let iat = now.unix_timestamp();
        let claims = serde_json::json!({
            "iss": self.key.client_email(),
            "aud": TOKEN_URI,
            "scope": SCOPE,
            "iat": iat,
            "exp": iat + ASSERTION_LIFETIME.whole_seconds(),
        });
        let signed = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = self.signer.sign(signed.as_bytes())?;
        Ok(SecretString::from(format!(
            "{signed}.{}",
            URL_SAFE_NO_PAD.encode(signature)
        )))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Google refused the service account key ({status} {reason})")]
    Refused { status: u16, reason: String },
    #[error("Google's token endpoint answered {status}")]
    Unavailable { status: u16 },
    #[error("cannot reach Google's token endpoint")]
    Unreachable(#[source] UpstreamError),
    #[error("Google's token endpoint answered in a form Kaeso cannot read")]
    Unreadable,
    #[error("the service account key cannot sign")]
    Sign,
}

#[derive(Clone)]
struct Token {
    secret: SecretString,
    renew_at: OffsetDateTime,
}

struct Slot {
    org: Uuid,
    revision: i64,
    token: tokio::sync::Mutex<Option<Token>>,
}

#[derive(Default)]
pub struct GoogleTokens {
    slots: Mutex<HashMap<Uuid, Arc<Slot>>>,
}

impl std::fmt::Debug for GoogleTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleTokens").finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Granted {
    access_token: String,
    expires_in: i64,
}

impl GoogleTokens {
    fn slot(&self, org: Uuid, credential: Uuid, revision: i64) -> Arc<Slot> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        match slots.get(&credential) {
            Some(slot) if slot.revision == revision => Arc::clone(slot),
            _ => {
                let slot = Arc::new(Slot {
                    org,
                    revision,
                    token: tokio::sync::Mutex::new(None),
                });
                slots.insert(credential, Arc::clone(&slot));
                slot
            }
        }
    }

    pub fn forget_org(&self, org: Uuid) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, slot| slot.org != org);
    }

    pub fn holds(&self, credential: Uuid) -> bool {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(&credential)
    }
}

// One exchange per credential at a time: the requests that arrive while it runs wait on the
// slot's lock and take the token it stored instead of signing assertions of their own.
pub async fn access_token(
    state: &AppState,
    credential: Uuid,
    revision: i64,
    account: &ServiceAccount,
) -> Result<SecretString, TokenError> {
    let slot = state.google_tokens.slot(account.org, credential, revision);
    let mut held = slot.token.lock().await;
    let now = state.clock.now();
    if let Some(token) = held.as_ref().filter(|t| now < t.renew_at) {
        return Ok(token.secret.clone());
    }
    let granted = exchange(state, account, now).await?;
    let lifetime = time::Duration::seconds(granted.expires_in.max(0));
    let token = Token {
        secret: SecretString::from(granted.access_token),
        renew_at: now + lifetime - RENEW_BEFORE,
    };
    let secret = token.secret.clone();
    *held = Some(token);
    Ok(secret)
}

async fn exchange(
    state: &AppState,
    account: &ServiceAccount,
    now: OffsetDateTime,
) -> Result<Granted, TokenError> {
    let assertion = account.assertion(now).map_err(|_| TokenError::Sign)?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", JWT_BEARER_GRANT)
        .append_pair("assertion", assertion.expose_secret())
        .finish();
    // Only Google's endpoint (or the tests' mock) is ever called: keys naming another
    // `token_uri` are refused when added.
    let endpoint = Url::parse(&state.accounts.google_token).map_err(|_| TokenError::Unreadable)?;
    let request = Outgoing {
        method: "POST",
        path: String::new(),
        headers: HeaderList::new()
            .with("content-type", "application/x-www-form-urlencoded")
            .with("accept", "application/json"),
        body: bytes::Bytes::from(body.into_bytes()),
    };
    let none = SecretString::from(String::new());
    let resp = state
        .upstream
        .send(Call {
            base_url: &endpoint,
            auth: None,
            guard: kaeso_upstream::Guard::Install,
            credential: &none,
            request,
            stream: false,
            timeouts: Timeouts {
                request: EXCHANGE_TIMEOUT,
                first_byte: EXCHANGE_TIMEOUT,
                idle: EXCHANGE_TIMEOUT,
            },
            max_body: MAX_ANSWER,
        })
        .await
        .map_err(TokenError::Unreachable)?;
    let ResponseBody::Full(body) = resp.body else {
        return Err(TokenError::Unreadable);
    };
    match resp.status {
        200..=299 => {
            let granted: Granted =
                serde_json::from_slice(&body).map_err(|_| TokenError::Unreadable)?;
            if granted.access_token.is_empty() || granted.expires_in <= 0 {
                return Err(TokenError::Unreadable);
            }
            Ok(granted)
        }
        status @ (429 | 500..=599) => Err(TokenError::Unavailable { status }),
        status => {
            let reason = refusal_reason(&body);
            tracing::warn!(
                status,
                reason = %reason,
                "Google refused a service account key's assertion"
            );
            Err(TokenError::Refused { status, reason })
        }
    }
}

// Google's token errors are `{"error": "invalid_grant", "error_description": "…"}` (RFC 6749
// section 5.2); only the error code is logged, never the description, which may quote the
// assertion's claims.
fn refusal_reason(body: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Refusal {
        error: String,
    }
    serde_json::from_slice::<Refusal>(body)
        .ok()
        .map(|r| r.error)
        .filter(|e| e.len() <= 64 && e.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .unwrap_or_else(|| "unreadable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_error_code_of_a_refusal_is_kept() {
        assert_eq!(
            refusal_reason(
                br#"{"error":"invalid_grant","error_description":"Invalid JWT Signature."}"#
            ),
            "invalid_grant"
        );
        assert_eq!(refusal_reason(b"<html>"), "unreadable");
        assert_eq!(refusal_reason(br#"{"error":"a b"}"#), "unreadable");
    }
}
