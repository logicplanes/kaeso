//! Access tokens for Azure keys that are Microsoft Entra ID service principals, from the client
//! credentials grant
//! (https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-client-creds-grant-flow).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use kaeso_codecs::HeaderList;
use kaeso_core::azure::ServicePrincipal;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts, UpstreamError};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::state::AppState;

const RENEW_BEFORE: time::Duration = time::Duration::minutes(5);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ANSWER: usize = 64 * 1024;

/// A key's service principal and the organization the key belongs to.
#[derive(Debug)]
pub struct Principal {
    pub principal: ServicePrincipal,
    pub org: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Microsoft Entra ID refused the service principal ({status} {reason})")]
    Refused { status: u16, reason: String },
    #[error("Microsoft Entra ID's token endpoint answered {status}")]
    Unavailable { status: u16 },
    #[error("cannot reach Microsoft Entra ID's token endpoint")]
    Unreachable(#[source] UpstreamError),
    #[error("Microsoft Entra ID's token endpoint answered in a form Kaeso cannot read")]
    Unreadable,
}

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
pub struct AzureTokens {
    slots: Mutex<HashMap<Uuid, Arc<Slot>>>,
}

impl std::fmt::Debug for AzureTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureTokens").finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Granted {
    access_token: String,
    expires_in: i64,
}

impl AzureTokens {
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

/// The access token of a service principal key, from the cache or a new exchange. One exchange
/// per key runs at a time: requests that arrive meanwhile wait for its token.
pub async fn access_token(
    state: &AppState,
    credential: Uuid,
    revision: i64,
    key: &Principal,
) -> Result<SecretString, TokenError> {
    let principal = &key.principal;
    let slot = state.azure_tokens.slot(key.org, credential, revision);
    let mut held = slot.token.lock().await;
    let now = state.clock.now();
    if let Some(token) = held.as_ref().filter(|t| now < t.renew_at) {
        return Ok(token.secret.clone());
    }
    let granted = exchange(state, principal).await?;
    let lifetime = time::Duration::seconds(granted.expires_in.max(0));
    let token = Token {
        secret: SecretString::from(granted.access_token),
        renew_at: now + lifetime - RENEW_BEFORE,
    };
    let secret = token.secret.clone();
    *held = Some(token);
    Ok(secret)
}

async fn exchange(state: &AppState, principal: &ServicePrincipal) -> Result<Granted, TokenError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", principal.client_id())
        .append_pair("scope", principal.scope())
        .append_pair("client_secret", principal.client_secret().expose_secret())
        .append_pair("grant_type", "client_credentials")
        .finish();
    // Only Entra ID's host (or the tests' mock) is ever called: the key names its tenant, which
    // must be an id or a domain name, never a host.
    let endpoint = Url::parse(&principal.token_url(&state.accounts.azure_login))
        .map_err(|_| TokenError::Unreadable)?;
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
            sigv4: None,
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
            tracing::warn!(status, reason = %reason, "Microsoft Entra ID refused a service principal");
            Err(TokenError::Refused { status, reason })
        }
    }
}

// Entra ID's token errors are `{"error": "invalid_client", "error_description": "AADSTS7000215:
// …", "error_codes": [7000215], …}`
// (https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes). Only the
// error and its AADSTS code are kept, never the description, which quotes trace ids.
fn refusal_reason(body: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Refusal {
        error: String,
        #[serde(default)]
        error_codes: Vec<u64>,
    }
    let Some(r) = serde_json::from_slice::<Refusal>(body).ok().filter(|r| {
        r.error.len() <= 64
            && r.error
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }) else {
        return "unreadable".to_owned();
    };
    match r.error_codes.first() {
        Some(code) => format!("{} AADSTS{code}", r.error),
        None => r.error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_error_and_its_code_are_kept() {
        assert_eq!(
            refusal_reason(
                br#"{"error":"invalid_client","error_description":"AADSTS7000215: Invalid client secret provided. Trace ID: x","error_codes":[7000215]}"#
            ),
            "invalid_client AADSTS7000215"
        );
        assert_eq!(
            refusal_reason(br#"{"error":"invalid_scope"}"#),
            "invalid_scope"
        );
        assert_eq!(refusal_reason(b"<html>"), "unreadable");
        assert_eq!(refusal_reason(br#"{"error":"a b"}"#), "unreadable");
    }
}
