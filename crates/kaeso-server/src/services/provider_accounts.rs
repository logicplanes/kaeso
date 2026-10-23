use std::time::Duration;

use bytes::Bytes;
use kaeso_core::provider::AuthMethod;
use openidconnect::core::{
    CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
};
use openidconnect::{ClientId, IssuerUrl, Nonce, PkceCodeChallenge};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use url::Url;

use crate::state::AppState;

pub const CHATGPT_SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
pub const CHATGPT_PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
pub const CHATGPT_RESOURCE: &str = "https://api.openai.com/v1";
pub const FIRST_CLIENT: &str = "dynamic_agent_client";
pub const AGENT_NAME: &str = "Kaeso";
pub const CALLBACK_HOST: &str = "127.0.0.1";
pub const CALLBACK_PATH: &str = "/auth/callback";

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ANSWER: usize = 1 << 20;

// The SIWC refresh errors that mean the session is gone for good:
// https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery#refresh-errors
const SIGNED_OUT: &[&str] = &[
    "invalid_grant",
    "invalid_refresh_token",
    "token_expired",
    "refresh_token_expired",
    "refresh_token_invalidated",
    "refresh_token_reused",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hosts {
    pub auth: String,
    pub api: String,
}

impl Hosts {
    pub fn issuer(&self) -> &str {
        self.auth.trim_end_matches('/')
    }

    fn at(&self, path: &str) -> String {
        format!("{}{path}", self.issuer())
    }

    pub fn api_base(&self) -> Result<Url, AccountError> {
        Url::parse(&self.api).map_err(|e| AccountError::Unreachable(format!("api host: {e}")))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoints {
    pub chatgpt: Hosts,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints {
            chatgpt: Hosts {
                auth: "https://auth.openai.com".into(),
                api: "https://api.openai.com".into(),
            },
        }
    }
}

impl Endpoints {
    pub fn hosts(&self, method: AuthMethod) -> Option<&Hosts> {
        match method {
            AuthMethod::ChatGpt => Some(&self.chatgpt),
            AuthMethod::ApiKey => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("the sign-in service could not be reached: {0}")]
    Unreachable(String),
    #[error("the sign-in service refused with `{code}` (HTTP {status})")]
    Refused { status: u16, code: String },
    #[error("the sign-in service's answer could not be verified: {0}")]
    Unverified(String),
}

impl AccountError {
    pub fn signs_out(&self) -> bool {
        matches!(self, AccountError::Refused { code, .. } if SIGNED_OUT.contains(&code.as_str()))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub sub: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub earliest_refresh_at: Option<OffsetDateTime>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("client_id", &self.client_id)
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

impl Tokens {
    pub fn parse(json: &str) -> Option<Tokens> {
        serde_json::from_str(json).ok()
    }

    pub fn sealed_form(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Deserialize)]
pub struct Granted {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    earliest_refresh_at: Option<Value>,
}

impl std::fmt::Debug for Granted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

const DEFAULT_LIFETIME: i64 = 3600;

impl Granted {
    pub fn id_token(&self) -> Option<&str> {
        self.id_token.as_deref()
    }

    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    pub fn scopes(&self) -> Vec<String> {
        self.scope
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    pub fn expires_at(&self, now: OffsetDateTime) -> OffsetDateTime {
        now + time::Duration::seconds(self.expires_in.unwrap_or(DEFAULT_LIFETIME).max(0))
    }

    pub fn earliest_refresh_at(&self) -> Option<OffsetDateTime> {
        match self.earliest_refresh_at.as_ref()? {
            Value::Number(n) => OffsetDateTime::from_unix_timestamp(n.as_i64()?).ok(),
            Value::String(s) => {
                OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
            }
            _ => None,
        }
    }

    pub fn first_tokens(self, client_id: &str, sub: &str) -> Result<Tokens, AccountError> {
        let earliest_refresh_at = self.earliest_refresh_at();
        let scopes = self.scopes();
        let refresh_token = self
            .refresh_token
            .ok_or_else(|| AccountError::Unverified("no refresh token in the answer".into()))?;
        let id_token = self
            .id_token
            .ok_or_else(|| AccountError::Unverified("no ID token in the answer".into()))?;
        Ok(Tokens {
            access_token: self.access_token,
            refresh_token,
            id_token,
            client_id: client_id.to_owned(),
            scopes,
            sub: sub.to_owned(),
            earliest_refresh_at,
        })
    }

    pub fn renew(self, old: &Tokens) -> Tokens {
        let earliest_refresh_at = self.earliest_refresh_at();
        let scopes = self.scopes();
        Tokens {
            access_token: self.access_token,
            refresh_token: self
                .refresh_token
                .unwrap_or_else(|| old.refresh_token.clone()),
            id_token: self.id_token.unwrap_or_else(|| old.id_token.clone()),
            client_id: old.client_id.clone(),
            scopes: if scopes.is_empty() {
                old.scopes.clone()
            } else {
                scopes
            },
            sub: old.sub.clone(),
            earliest_refresh_at,
        }
    }
}

pub struct Begun {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    pub port: u16,
}

impl std::fmt::Debug for Begun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Begun")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

pub fn redirect_uri(port: u16) -> String {
    format!("http://{CALLBACK_HOST}:{port}{CALLBACK_PATH}")
}

// RFC 6335 §6: the dynamic port range, where no service is registered, so the
// browser's failed load rarely reaches a program listening on the person's computer.
fn callback_port() -> u16 {
    let [a, b] = kaeso_crypto::random_bytes::<2>();
    49152 + u16::from_le_bytes([a, b]) % 16384
}

pub async fn agent_host_id(state: &AppState) -> Result<String, kaeso_store::StoreError> {
    let fresh = uuid::Builder::from_random_bytes(kaeso_crypto::random_bytes::<16>()).into_uuid();
    kaeso_store::pg::instance::agent_host_id(&state.pg, &format!("urn:uuid:{fresh}")).await
}

pub fn begin(
    hosts: &Hosts,
    host_id: &str,
    client_id: Option<&str>,
    login_hint: Option<&str>,
) -> Result<Begun, AccountError> {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let state = kaeso_crypto::random_token();
    let nonce = kaeso_crypto::random_token();
    let port = callback_port();
    let mut url = Url::parse(&hosts.at("/api/accounts/authorize"))
        .map_err(|e| AccountError::Unreachable(format!("authorize URL: {e}")))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", client_id.unwrap_or(FIRST_CLIENT));
        if client_id.is_none() {
            q.append_pair("agent_name_hint", AGENT_NAME);
        }
        q.append_pair("ext_agent_host_id", host_id);
        if let Some(hint) = login_hint {
            q.append_pair("login_hint", hint);
        }
        q.append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri(port))
            .append_pair("scope", CHATGPT_SCOPES)
            .append_pair("resource", CHATGPT_RESOURCE)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge", challenge.as_str())
            .append_pair("code_challenge_method", "S256");
    }
    Ok(Begun {
        url: url.to_string(),
        state,
        nonce,
        verifier: verifier.secret().clone(),
        port,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub enum Answer {
    Code {
        code: String,
        client_id: Option<String>,
    },
    Denied,
    Refused(String),
    Incomplete,
}

impl std::fmt::Debug for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Answer::Code { client_id, .. } => f
                .debug_struct("Code")
                .field("client_id", client_id)
                .finish_non_exhaustive(),
            Answer::Denied => f.write_str("Denied"),
            Answer::Refused(e) => f.debug_tuple("Refused").field(e).finish(),
            Answer::Incomplete => f.write_str("Incomplete"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callback {
    pub port: u16,
    pub state: String,
    pub answer: Answer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallbackError {
    #[error("the pasted address is not a sign-in callback")]
    NotCallback,
    #[error("a parameter of the pasted address appears twice")]
    Repeated,
}

pub fn read_callback(pasted: &str) -> Result<Callback, CallbackError> {
    let url = Url::parse(pasted.trim()).map_err(|_| CallbackError::NotCallback)?;
    let port = url.port().ok_or(CallbackError::NotCallback)?;
    if url.scheme() != "http"
        || url.host_str() != Some(CALLBACK_HOST)
        || url.path() != CALLBACK_PATH
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(CallbackError::NotCallback);
    }
    let mut params = std::collections::HashMap::new();
    for (k, v) in url.query_pairs() {
        if params.insert(k, v).is_some() {
            return Err(CallbackError::Repeated);
        }
    }
    let take = |name: &str| {
        params
            .get(name)
            .filter(|v| !v.is_empty())
            .map(|v| v.clone().into_owned())
    };
    let state = take("state").ok_or(CallbackError::NotCallback)?;
    let answer = match (take("error"), take("code")) {
        (Some(e), _) if e == "access_denied" => Answer::Denied,
        (Some(e), _) => Answer::Refused(e),
        (None, Some(code)) => Answer::Code {
            code,
            client_id: take("client_id"),
        },
        (None, None) => Answer::Incomplete,
    };
    Ok(Callback {
        port,
        state,
        answer,
    })
}

fn client() -> Result<reqwest::Client, AccountError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(TIMEOUT)
        .timeout(TIMEOUT)
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AccountError::Unreachable(e.to_string()))
}

async fn read(mut response: reqwest::Response) -> Result<(u16, Bytes), AccountError> {
    let status = response.status().as_u16();
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AccountError::Unreachable(e.without_url().to_string()))?
    {
        if body.len() + chunk.len() > MAX_ANSWER {
            return Err(AccountError::Unreachable("the answer is too large".into()));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((status, Bytes::from(body)))
}

async fn post_form(url: &str, form: &[(&str, &str)]) -> Result<(u16, Bytes), AccountError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form)
        .finish();
    let response = client()?
        .post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| AccountError::Unreachable(e.without_url().to_string()))?;
    read(response).await
}

fn refusal(status: u16, body: &[u8]) -> AccountError {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let code = match value.get("error") {
        Some(Value::String(code)) => Some(code.clone()),
        Some(Value::Object(e)) => e
            .get("code")
            .or_else(|| e.get("type"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    };
    match code {
        Some(code) if status < 500 => AccountError::Refused { status, code },
        _ => AccountError::Unreachable(format!("HTTP {status}")),
    }
}

async fn token(hosts: &Hosts, form: &[(&str, &str)]) -> Result<Granted, AccountError> {
    let (status, body) = post_form(&hosts.at("/api/accounts/oauth/token"), form).await?;
    if !(200..300).contains(&status) {
        return Err(refusal(status, &body));
    }
    serde_json::from_slice(&body)
        .map_err(|_| AccountError::Unverified("the token answer is not a token response".into()))
}

pub async fn exchange(
    hosts: &Hosts,
    client_id: &str,
    code: &str,
    verifier: &str,
    port: u16,
) -> Result<Granted, AccountError> {
    token(
        hosts,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", &redirect_uri(port)),
            ("resource", CHATGPT_RESOURCE),
        ],
    )
    .await
}

pub async fn refresh(hosts: &Hosts, tokens: &Tokens) -> Result<Granted, AccountError> {
    token(
        hosts,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &tokens.client_id),
            ("refresh_token", &tokens.refresh_token),
            ("resource", CHATGPT_RESOURCE),
        ],
    )
    .await
}

pub async fn revoke(
    hosts: &Hosts,
    client_id: &str,
    refresh_token: &str,
) -> Result<(), AccountError> {
    let (status, body) = post_form(
        &hosts.at("/api/accounts/oauth/revoke"),
        &[
            ("token", refresh_token),
            ("token_type_hint", "refresh_token"),
            ("client_id", client_id),
        ],
    )
    .await?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(refusal(status, &body))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: Option<String>,
}

pub async fn verify(
    hosts: &Hosts,
    id_token: &str,
    client_id: &str,
    nonce: &str,
) -> Result<Identity, AccountError> {
    let token: CoreIdToken = id_token
        .parse()
        .map_err(|_| AccountError::Unverified("the ID token is not a JWT".into()))?;
    let response = client()?
        .get(hosts.at("/.well-known/jwks.json"))
        .send()
        .await
        .map_err(|e| AccountError::Unreachable(e.without_url().to_string()))?;
    let (status, body) = read(response).await?;
    if !(200..300).contains(&status) {
        return Err(AccountError::Unreachable(format!(
            "JWKS answered HTTP {status}"
        )));
    }
    let keys: CoreJsonWebKeySet = serde_json::from_slice(&body)
        .map_err(|e| AccountError::Unverified(format!("JWKS: {e}")))?;
    let issuer = IssuerUrl::new(hosts.issuer().to_owned())
        .map_err(|e| AccountError::Unverified(format!("issuer: {e}")))?;
    let verifier =
        CoreIdTokenVerifier::new_public_client(ClientId::new(client_id.to_owned()), issuer, keys)
            .set_allowed_algs(vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
    let claims = token
        .claims(&verifier, &Nonce::new(nonce.to_owned()))
        .map_err(|e| AccountError::Unverified(e.to_string()))?;
    let subject = claims.subject().as_str().to_owned();
    if subject.is_empty() {
        return Err(AccountError::Unverified(
            "the ID token has no subject".into(),
        ));
    }
    Ok(Identity {
        subject,
        email: claims.email().map(|e| e.as_str().to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(pasted: &str) -> Result<Callback, CallbackError> {
        read_callback(pasted)
    }

    #[test]
    fn a_callback_is_read_from_the_address_the_browser_could_not_load() {
        let cb = read(
            "  http://127.0.0.1:51234/auth/callback?code=c-1&scope=openid+email&state=s-1&client_id=oaiapp_1\n",
        )
        .unwrap();
        assert_eq!(cb.port, 51234);
        assert_eq!(cb.state, "s-1");
        assert_eq!(
            cb.answer,
            Answer::Code {
                code: "c-1".into(),
                client_id: Some("oaiapp_1".into())
            }
        );
        let again = read("http://127.0.0.1:51234/auth/callback?state=s&code=c&future=1").unwrap();
        assert_eq!(
            again.answer,
            Answer::Code {
                code: "c".into(),
                client_id: None
            },
            "an unknown parameter is ignored"
        );
        assert_eq!(
            read("http://127.0.0.1:1/auth/callback?error=access_denied&state=s")
                .unwrap()
                .answer,
            Answer::Denied
        );
        assert_eq!(
            read("http://127.0.0.1:1/auth/callback?error=server_error&state=s")
                .unwrap()
                .answer,
            Answer::Refused("server_error".into())
        );
    }

    #[test]
    fn anything_but_the_loopback_callback_is_refused() {
        for pasted in [
            "",
            "not a url",
            "127.0.0.1:51234/auth/callback?code=c&state=s",
            "https://127.0.0.1:51234/auth/callback?code=c&state=s",
            "http://localhost:51234/auth/callback?code=c&state=s",
            "http://127.0.0.2:51234/auth/callback?code=c&state=s",
            "http://[::1]:51234/auth/callback?code=c&state=s",
            "http://127.0.0.1/auth/callback?code=c&state=s",
            "http://127.0.0.1:51234/callback?code=c&state=s",
            "http://127.0.0.1:51234/auth/callback/?code=c&state=s",
            "http://127.0.0.1:51234/auth/callback#code=c&state=s",
            "http://127.0.0.1:51234/auth/callback?code=c&state=s#x",
            "http://user:pass@127.0.0.1:51234/auth/callback?code=c&state=s",
            "http://evil.example/?u=http://127.0.0.1:51234/auth/callback?code=c&state=s",
            "javascript:alert(1)",
        ] {
            assert_eq!(read(pasted), Err(CallbackError::NotCallback), "{pasted:?}");
        }
    }

    #[test]
    fn each_parameter_comes_once_and_state_and_code_are_required() {
        for pasted in [
            "http://127.0.0.1:1/auth/callback?code=a&code=b&state=s",
            "http://127.0.0.1:1/auth/callback?code=a&state=s&state=t",
            "http://127.0.0.1:1/auth/callback?code=a&state=s&client_id=x&client_id=y",
            "http://127.0.0.1:1/auth/callback?code=a&state=s&error=access_denied&error=x",
            "http://127.0.0.1:1/auth/callback?code=a&state=s&future=1&future=2",
        ] {
            assert_eq!(read(pasted), Err(CallbackError::Repeated), "{pasted}");
        }
        for pasted in [
            "http://127.0.0.1:1/auth/callback?code=a",
            "http://127.0.0.1:1/auth/callback?code=a&state=",
            "http://127.0.0.1:1/auth/callback",
        ] {
            assert_eq!(read(pasted), Err(CallbackError::NotCallback), "{pasted}");
        }
        for pasted in [
            "http://127.0.0.1:1/auth/callback?state=s",
            "http://127.0.0.1:1/auth/callback?state=s&code=",
        ] {
            assert_eq!(read(pasted).unwrap().answer, Answer::Incomplete, "{pasted}");
        }
    }

    #[test]
    fn the_authorization_asks_for_plan_usage_with_a_fresh_challenge() {
        let hosts = Endpoints::default().chatgpt;
        let first = begin(&hosts, "urn:uuid:x", None, None).unwrap();
        let url = Url::parse(&first.url).unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            "https://auth.openai.com"
        );
        assert_eq!(url.path(), "/api/accounts/authorize");
        let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "dynamic_agent_client");
        assert_eq!(q["agent_name_hint"], "Kaeso");
        assert_eq!(q["ext_agent_host_id"], "urn:uuid:x");
        assert_eq!(q["response_type"], "code");
        assert_eq!(
            q["redirect_uri"],
            format!("http://127.0.0.1:{}/auth/callback", first.port)
        );
        assert_eq!(q["scope"], CHATGPT_SCOPES);
        assert_eq!(q["resource"], "https://api.openai.com/v1");
        assert_eq!(q["state"], first.state);
        assert_eq!(q["nonce"], first.nonce);
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(first.port >= 49152);
        assert!(!q.contains_key("login_hint"));
        let again = begin(
            &hosts,
            "urn:uuid:x",
            Some("oaiapp_1"),
            Some("ada@example.com"),
        )
        .unwrap();
        let q: std::collections::HashMap<String, String> = Url::parse(&again.url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        assert_eq!(q["client_id"], "oaiapp_1");
        assert_eq!(q["login_hint"], "ada@example.com");
        assert!(
            !q.contains_key("agent_name_hint"),
            "only a first registration names the agent"
        );
        assert_ne!(again.state, first.state);
        assert_ne!(again.verifier, first.verifier);
    }

    #[test]
    fn only_a_lost_session_signs_the_account_out() {
        for code in SIGNED_OUT {
            assert!(refusal(400, format!(r#"{{"error":"{code}"}}"#).as_bytes()).signs_out());
        }
        assert!(refusal(400, br#"{"error":{"code":"refresh_token_reused"}}"#).signs_out());
        assert!(!refusal(401, br#"{"error":"invalid_client"}"#).signs_out());
        assert!(!refusal(500, br#"{"error":"invalid_grant"}"#).signs_out());
        assert!(!refusal(400, b"not json").signs_out());
    }

    #[test]
    fn the_earliest_refresh_is_read_as_seconds_or_a_timestamp() {
        let granted = |v: Value| Granted {
            access_token: "a".into(),
            refresh_token: None,
            id_token: None,
            expires_in: Some(60),
            scope: None,
            earliest_refresh_at: Some(v),
        };
        let at = OffsetDateTime::from_unix_timestamp(1_790_032_532).unwrap();
        assert_eq!(
            granted(Value::from(1_790_032_532)).earliest_refresh_at(),
            Some(at)
        );
        assert_eq!(
            granted(Value::from("2026-09-21T23:15:32Z")).earliest_refresh_at(),
            Some(at)
        );
        assert_eq!(granted(Value::Bool(true)).earliest_refresh_at(), None);
        assert_eq!(
            granted(Value::Null).expires_at(at),
            at + time::Duration::seconds(60)
        );
    }

    #[test]
    fn tokens_never_reach_a_debug_line() {
        let tokens = Tokens {
            access_token: "eyJaccess".into(),
            refresh_token: "refresh-secret".into(),
            id_token: "eyJid".into(),
            client_id: "oaiapp_1".into(),
            scopes: vec!["openid".into()],
            sub: "user-1".into(),
            earliest_refresh_at: None,
        };
        let shown = format!("{tokens:?}");
        for secret in ["eyJaccess", "refresh-secret", "eyJid"] {
            assert!(!shown.contains(secret), "{shown}");
        }
    }
}
