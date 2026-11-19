//! Linked billing: provider keys that are links to a customer's own organization at the
//! provider, which the provider bills. The console, the Platform API, notifications and usage
//! only talk to this module; the provider's own protocol is in its adapter below.
//!
//! Anthropic's adapter follows https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections
//! and https://platform.claude.com/docs/en/manage-claude/oauth-apps/oauth-reference: the install
//! is the "platform", with an OAuth app of its own. Anthropic issues no client secrets, so the
//! install signs its client assertions (RFC 7523 `private_key_jwt`) with a P-256 key it keeps,
//! and publishes the public key as an OpenID issuer the app's client authentication trusts.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::Bytes;
use kaeso_core::linked_billing::{self as core, LinkedBilling};
use kaeso_crypto::{CryptoError, EcSigner, ExposeSecret, SecretString};
use kaeso_store::StoreError;
use kaeso_store::pg::credentials::Sealed as SealedParts;
use kaeso_store::pg::linked_billing::{self as store, AppRow};
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

pub const CLIENT_ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";
const WIF_GRANT: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

/// What a link asks for: billing, and a refresh token so Kaeso can renew the connection's
/// access token without the customer approving again.
pub const SCOPE: &str = "billing:attribution offline_access";
const CONNECTION_TYPE: &str = "organization_shared";
const AUTH_TARGET: &str = "platform";

/// The `anthropic-version` of the user profile requests, as the linked billing guide sends it.
pub const API_VERSION: &str = "2026-10-21";

/// A client assertion is valid this long; Anthropic allows at most an hour and asks for less.
const ASSERTION_LIFETIME: time::Duration = time::Duration::minutes(5);

// The guide's own client allows 10 seconds for an answer and 2 to connect: a refresh whose
// answer times out loses its tokens, so the answer gets time, and a host that can't be reached
// fails fast.
const TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_ANSWER: usize = 1 << 20;

/// A refresh whose answer was lost may be sent once more, and only this soon after the first
/// try. Anthropic allows 30 seconds; the guide keeps it under the read timeout, so a request
/// that timed out is never sent again.
const SECOND_REFRESH_WITHIN: Duration = Duration::from_secs(4);

/// The state of a linked key that keeps it from paying.
pub const UNLINKED: &str = "unlinked";
pub const NEEDS_SETUP: &str = "needs_setup";
/// A refresh is under way, or ended without knowing whether its refresh token was used.
pub const ON_HOLD: &str = "on_hold";

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("the provider can't be reached: {0}")]
    Unreachable(String),
    #[error("the provider's answer timed out")]
    TimedOut,
    #[error("the provider refused with `{code}` (HTTP {status})")]
    Refused { status: u16, code: String },
    #[error("the provider's answer could not be read: {0}")]
    Unreadable(String),
    #[error("linked billing is not set up for this provider")]
    NoApp,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the install's signing key: {0}")]
    Crypto(#[from] CryptoError),
}

impl LinkError {
    fn refused(status: u16, body: &[u8]) -> LinkError {
        LinkError::Refused {
            status,
            code: oauth_error(body).unwrap_or_else(|| format!("http_{status}")),
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            LinkError::Refused { code, .. } => Some(code),
            _ => None,
        }
    }

    /// Whether the install's app is set up wrong: Anthropic doesn't recognise the client, its
    /// assertion, its scopes or its service account.
    pub fn is_setup(&self) -> bool {
        matches!(
            self.code(),
            Some("invalid_client" | "unauthorized_client" | "invalid_scope")
        ) || matches!(self, LinkError::Refused { status: 401, .. })
    }
}

// The OAuth error of an answer, `{"error": "…"}`; the Claude API's own errors put an object
// there, whose `type` is read instead.
fn oauth_error(body: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    match value.get("error")? {
        Value::String(code) => Some(code.clone()),
        Value::Object(o) => o.get("type").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

// --- The install's signing key -------------------------------------------------------------

pub struct Signer {
    pub kid: String,
    key: EcSigner,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer").field("kid", &self.kid).finish()
    }
}

#[derive(Debug, Default)]
pub struct Signers {
    signer: tokio::sync::OnceCell<Arc<Signer>>,
}

fn key_aad(kid: &str) -> Vec<u8> {
    format!("linked_billing_key:{kid}").into_bytes()
}

/// The install's signing key, made and stored the first time it's needed.
pub async fn signer(state: &AppState) -> Result<Arc<Signer>, LinkError> {
    state
        .linked_billing
        .signer
        .get_or_try_init(|| async { load_or_create(state).await.map(Arc::new) })
        .await
        .cloned()
}

async fn load_or_create(state: &AppState) -> Result<Signer, LinkError> {
    if let Some(found) = store::signing_key(&state.pg).await? {
        return open_key(state, &found);
    }
    let (_, pkcs8) = EcSigner::generate()?;
    let kid = URL_SAFE_NO_PAD.encode(kaeso_crypto::random_bytes::<9>());
    let sealed = kaeso_crypto::seal(&state.keyring, pkcs8.expose_secret(), &key_aad(&kid))?;
    store::insert_signing_key(
        &state.pg,
        &kid,
        &SealedParts {
            ciphertext: &sealed.ciphertext,
            nonce: &sealed.nonce,
            wrapped_key: &sealed.wrapped_key,
            key_id: &sealed.key_id,
        },
    )
    .await?;
    // Another replica may have stored its key first; every replica signs with the stored one.
    let stored = store::signing_key(&state.pg)
        .await?
        .ok_or_else(|| LinkError::Unreadable("the signing key wasn't stored".into()))?;
    open_key(state, &stored)
}

fn open_key(state: &AppState, row: &store::SigningKey) -> Result<Signer, LinkError> {
    let sealed = kaeso_crypto::Sealed {
        ciphertext: row.ciphertext.clone(),
        nonce: row.nonce.clone(),
        wrapped_key: row.wrapped_key.clone(),
        key_id: row.key_id.clone(),
    };
    let der = kaeso_crypto::open(&state.keyring, &sealed, &key_aad(&row.kid))?;
    Ok(Signer {
        kid: row.kid.clone(),
        key: EcSigner::from_pkcs8(der.expose_secret())?,
    })
}

impl Signer {
    pub fn jwk(&self) -> Result<Value, LinkError> {
        let public = self.key.public_key()?;
        Ok(json!({
            "kty": "EC",
            "crv": "P-256",
            "use": "sig",
            "alg": "ES256",
            "kid": self.kid,
            "x": public.x,
            "y": public.y,
        }))
    }

    /// A client assertion: a JWT the install signs, which names the app's audience, valid for
    /// five minutes and used once.
    pub fn assertion(
        &self,
        issuer: &str,
        subject: &str,
        audience: &str,
        now: OffsetDateTime,
    ) -> Result<SecretString, LinkError> {
        let header = json!({ "alg": "ES256", "typ": "JWT", "kid": self.kid });
        let iat = now.unix_timestamp();
        let claims = json!({
            "iss": issuer,
            "sub": subject,
            "aud": audience,
            "iat": iat,
            "exp": iat + ASSERTION_LIFETIME.whole_seconds(),
            "jti": Uuid::now_v7().to_string(),
        });
        let signed = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = self.key.sign(signed.as_bytes())?;
        Ok(SecretString::from(format!(
            "{signed}.{}",
            URL_SAFE_NO_PAD.encode(signature)
        )))
    }
}

pub fn issuer(state: &AppState) -> String {
    core::issuer(state.config.public_url.as_str())
}

pub fn redirect_uri(state: &AppState) -> String {
    core::redirect_uri(state.config.public_url.as_str())
}

/// The install's OpenID discovery document, served at the issuer for Anthropic to find its keys.
pub fn discovery(state: &AppState) -> Value {
    let issuer = issuer(state);
    json!({
        "issuer": issuer,
        "jwks_uri": format!("{issuer}/jwks.json"),
        "id_token_signing_alg_values_supported": ["ES256"],
        "response_types_supported": ["id_token"],
        "subject_types_supported": ["public"],
    })
}

pub async fn jwks(state: &AppState) -> Result<Value, LinkError> {
    Ok(json!({ "keys": [signer(state).await?.jwk()?] }))
}

// --- The install's app -----------------------------------------------------------------------

pub fn offered(provider: &str) -> Result<&'static LinkedBilling, LinkError> {
    core::of(provider).ok_or(LinkError::NoApp)
}

pub async fn app(state: &AppState, provider: &str) -> Result<AppRow, LinkError> {
    store::app(&state.pg, provider)
        .await?
        .ok_or(LinkError::NoApp)
}

async fn assertion(state: &AppState, app: &AppRow) -> Result<SecretString, LinkError> {
    signer(state).await?.assertion(
        &issuer(state),
        &core::subject(&app.provider),
        &app.audience,
        state.clock.now(),
    )
}

fn hosts<'a>(
    state: &'a AppState,
    app: &AppRow,
) -> Result<&'a crate::services::provider_accounts::Hosts, LinkError> {
    let offered = offered(&app.provider)?;
    Ok(state.accounts.billing(offered.adapter))
}

// --- HTTP ------------------------------------------------------------------------------------

enum Transport {
    /// No connection was made, so nothing was sent.
    Connect(String),
    /// The answer didn't come in time; the request may still run.
    Timeout,
    /// The connection broke after the request went out.
    Dropped(String),
}

struct Answer {
    status: u16,
    retry_after: bool,
    body: Bytes,
}

fn client() -> Result<reqwest::Client, LinkError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TIMEOUT)
        .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| LinkError::Unreachable(e.to_string()))
}

async fn post(
    url: &str,
    body: &Value,
    bearer: Option<&SecretString>,
    headers: &[(&str, &str)],
) -> Result<Answer, Transport> {
    let client = client().map_err(|e| Transport::Connect(e.to_string()))?;
    let mut request = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(body.to_string());
    if let Some(token) = bearer {
        let mut value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token.expose_secret()))
                .map_err(|_| Transport::Connect("the token can't be sent".into()))?;
        value.set_sensitive(true);
        request = request.header(reqwest::header::AUTHORIZATION, value);
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request.send().await.map_err(|e| {
        if e.is_connect() {
            Transport::Connect(e.without_url().to_string())
        } else if e.is_timeout() {
            Transport::Timeout
        } else {
            Transport::Dropped(e.without_url().to_string())
        }
    })?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .contains_key(reqwest::header::RETRY_AFTER);
    let mut read = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if read.len() + chunk.len() > MAX_ANSWER {
                    return Err(Transport::Dropped("the answer is too large".into()));
                }
                read.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) if e.is_timeout() => return Err(Transport::Timeout),
            Err(e) => return Err(Transport::Dropped(e.without_url().to_string())),
        }
    }
    Ok(Answer {
        status,
        retry_after,
        body: Bytes::from(read),
    })
}

async fn call(
    url: &str,
    body: &Value,
    bearer: Option<&SecretString>,
    headers: &[(&str, &str)],
) -> Result<Answer, LinkError> {
    post(url, body, bearer, headers).await.map_err(|t| match t {
        Transport::Connect(e) | Transport::Dropped(e) => LinkError::Unreachable(e),
        Transport::Timeout => LinkError::TimedOut,
    })
}

fn read_ok<T: for<'de> Deserialize<'de>>(answer: &Answer, ok: u16) -> Result<T, LinkError> {
    if answer.status != ok && !(ok == 200 && answer.status == 201) {
        return Err(LinkError::refused(answer.status, &answer.body));
    }
    serde_json::from_slice(&answer.body).map_err(|e| LinkError::Unreadable(e.to_string()))
}

fn with_client(app: &AppRow, assertion: &SecretString, mut body: Value) -> Value {
    if let Value::Object(map) = &mut body {
        map.insert("client_id".into(), Value::from(app.client_id.clone()));
        map.insert(
            "client_assertion_type".into(),
            Value::from(CLIENT_ASSERTION_TYPE),
        );
        map.insert(
            "client_assertion".into(),
            Value::from(assertion.expose_secret().to_owned()),
        );
    }
    body
}

// --- Anthropic: the operator's service account and user profiles ----------------------------

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
}

/// A token for the app's operator service account, from the install's own assertion through
/// Workload Identity Federation: it creates the user profiles that links bill.
pub async fn operator_token(state: &AppState, app: &AppRow) -> Result<SecretString, LinkError> {
    let hosts = hosts(state, app)?;
    let mut body = json!({
        "grant_type": WIF_GRANT,
        "assertion": assertion(state, app).await?.expose_secret(),
        "federation_rule_id": app.federation_rule_id,
        "organization_id": app.organization_id,
    });
    if let Value::Object(map) = &mut body {
        if let Some(id) = &app.service_account_id {
            map.insert("service_account_id".into(), Value::from(id.clone()));
        }
        if let Some(id) = &app.workspace_id {
            map.insert("workspace_id".into(), Value::from(id.clone()));
        }
    }
    let answer = call(
        &format!("{}/v1/oauth/token", hosts.api.trim_end_matches('/')),
        &body,
        None,
        &[],
    )
    .await?;
    let token: TokenAnswer = read_ok(&answer, 200)?;
    Ok(SecretString::from(token.access_token))
}

#[derive(Deserialize)]
struct ProfileAnswer {
    id: String,
}

/// Creates a user profile in the install's Claude organization for one Kaeso organization.
pub async fn create_profile(
    state: &AppState,
    app: &AppRow,
    token: &SecretString,
    name: &str,
    reference: &str,
) -> Result<String, LinkError> {
    let hosts = hosts(state, app)?;
    let body = json!({
        "name": name,
        "external_user_details": { "reference_id": reference },
    });
    let answer = call(
        &format!("{}/v1/user_profiles", hosts.api.trim_end_matches('/')),
        &body,
        Some(token),
        &[("anthropic-version", API_VERSION)],
    )
    .await?;
    let profile: ProfileAnswer = read_ok(&answer, 200)?;
    if profile.id.is_empty() || profile.id.len() > 100 {
        return Err(LinkError::Unreadable("no user profile id".into()));
    }
    Ok(profile.id)
}

/// The organization's user profile for the app, made the first time it links. Anthropic makes a
/// new profile for every create request, so the one kept is reused for every later link.
pub async fn profile_of(
    state: &AppState,
    app: &AppRow,
    org: Uuid,
    org_name: &str,
) -> Result<String, LinkError> {
    if let Some(kept) = store::profile(&state.pg, org, &app.provider, &app.client_id).await? {
        return Ok(kept);
    }
    let token = operator_token(state, app).await?;
    let made = create_profile(state, app, &token, org_name, &org.to_string()).await?;
    Ok(store::keep_profile(&state.pg, org, &app.provider, &app.client_id, &made).await?)
}

// --- Anthropic: linking ----------------------------------------------------------------------

#[derive(Deserialize)]
struct Pushed {
    request_uri: String,
}

pub struct Begun {
    pub authorize_url: String,
}

/// A pushed authorization request for a link (RFC 9126), with PKCE: returns the page the person
/// approves on.
pub async fn push(
    state: &AppState,
    app: &AppRow,
    user_profile_id: &str,
    state_value: &str,
    challenge: &str,
) -> Result<Begun, LinkError> {
    let hosts = hosts(state, app)?;
    let assertion = assertion(state, app).await?;
    let body = with_client(
        app,
        &assertion,
        json!({
            "response_type": "code",
            "redirect_uri": redirect_uri(state),
            "scope": SCOPE,
            "connection_type": CONNECTION_TYPE,
            "connection_auth_target": AUTH_TARGET,
            "user_profile_id": user_profile_id,
            "state": state_value,
            "code_challenge": challenge,
            "code_challenge_method": "S256",
        }),
    );
    let auth = hosts.auth.trim_end_matches('/');
    let answer = call(&format!("{auth}/v1/oauth/par"), &body, None, &[]).await?;
    let pushed: Pushed = read_ok(&answer, 201)?;
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &app.client_id)
        .append_pair("request_uri", &pushed.request_uri)
        .finish();
    Ok(Begun {
        authorize_url: format!("{auth}/oauth/authorize?{query}"),
    })
}

#[derive(Deserialize)]
pub struct Granted {
    pub access_token: String,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub refresh_token_expires_in: Option<i64>,
}

impl std::fmt::Debug for Granted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted")
            .field("connection_id", &self.connection_id)
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

// Anthropic issues access tokens for an hour at most.
const DEFAULT_LIFETIME: i64 = 3600;

impl Granted {
    pub fn expires_at(&self, now: OffsetDateTime) -> OffsetDateTime {
        now + time::Duration::seconds(
            self.expires_in
                .unwrap_or(DEFAULT_LIFETIME)
                .clamp(60, DEFAULT_LIFETIME),
        )
    }

    pub fn refresh_expires_at(&self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        self.refresh_token_expires_in
            .map(|s| now + time::Duration::seconds(s.max(0)))
    }
}

/// Exchanges the code a link's approval returned for the connection and its tokens.
pub async fn exchange(
    state: &AppState,
    app: &AppRow,
    code: &str,
    verifier: &str,
) -> Result<Granted, LinkError> {
    let hosts = hosts(state, app)?;
    let assertion = assertion(state, app).await?;
    let body = with_client(
        app,
        &assertion,
        json!({
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": verifier,
            "redirect_uri": redirect_uri(state),
        }),
    );
    let answer = call(
        &format!("{}/v1/oauth/token", hosts.auth.trim_end_matches('/')),
        &body,
        None,
        &[],
    )
    .await?;
    let granted: Granted = read_ok(&answer, 200)?;
    if granted.connection_id.as_deref().is_none_or(str::is_empty) {
        return Err(LinkError::Unreadable("no connection id".into()));
    }
    Ok(granted)
}

#[derive(Deserialize)]
struct Revoked {
    #[serde(default)]
    revoked_connection_ids: Vec<String>,
}

/// What to end at the provider.
pub enum Target<'a> {
    Connection(&'a str),
    Profile(&'a str),
}

/// Ends a connection, or every connection of a user profile, at the provider; returns the
/// connections it ended, empty when they had ended already.
pub async fn revoke(
    state: &AppState,
    app: &AppRow,
    target: Target<'_>,
) -> Result<Vec<String>, LinkError> {
    let hosts = hosts(state, app)?;
    let assertion = assertion(state, app).await?;
    let body = with_client(
        app,
        &assertion,
        match target {
            Target::Connection(id) => json!({ "connection_id": id }),
            Target::Profile(id) => json!({ "user_profile_id": id }),
        },
    );
    let answer = call(
        &format!("{}/v1/oauth/revoke", hosts.auth.trim_end_matches('/')),
        &body,
        None,
        &[],
    )
    .await?;
    let revoked: Revoked = read_ok(&answer, 200)?;
    Ok(revoked.revoked_connection_ids)
}

// --- Anthropic: renewing a connection's tokens ------------------------------------------------

/// How a refresh ended.
#[derive(Debug)]
pub enum Refreshed {
    Renewed(Granted),
    /// The refresh token wasn't used: try again later with the same one.
    Later(String),
    /// The install's app is set up wrong; nothing was revoked.
    Setup(String),
    /// The connection ended, or the refresh token may have been used without Kaeso getting its
    /// successor: the customer has to link again.
    Ended(String),
}

fn lost(answer: &Answer) -> bool {
    if answer.status < 500 {
        return false;
    }
    // A 503 in the Claude API's own format with `retry-after` is a refusal that didn't use the
    // refresh token; every other 5xx may have followed a refresh that took effect.
    let paused = answer.status == 503
        && answer.retry_after
        && serde_json::from_slice::<Value>(&answer.body)
            .ok()
            .and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_owned))
            .as_deref()
            == Some("error");
    !paused
}

/// Renews a connection's tokens, following Anthropic's rules for a refresh token that rotates
/// and ends the connection when it is sent again late
/// (https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections#refresh-the-tokens).
/// The caller holds the key before calling, so a crash leaves it held, not reused.
pub async fn refresh(state: &AppState, app: &AppRow, refresh_token: &str) -> Refreshed {
    let hosts = match hosts(state, app) {
        Ok(h) => h,
        Err(e) => return Refreshed::Setup(e.to_string()),
    };
    let assertion = match assertion(state, app).await {
        Ok(a) => a,
        Err(e) => return Refreshed::Later(e.to_string()),
    };
    let url = format!("{}/v1/oauth/token", hosts.auth.trim_end_matches('/'));
    let body = with_client(
        app,
        &assertion,
        json!({ "grant_type": "refresh_token", "refresh_token": refresh_token }),
    );
    let started = Instant::now();
    let answer = match post(&url, &body, None, &[]).await {
        Ok(answer) if !lost(&answer) => answer,
        Ok(_) | Err(Transport::Dropped(_)) => {
            if started.elapsed() > SECOND_REFRESH_WITHIN {
                return Refreshed::Ended("a refresh got no usable answer in time".into());
            }
            match post(&url, &body, None, &[]).await {
                Ok(answer) if answer.status == 200 => answer,
                _ => return Refreshed::Ended("a refresh and its one repeat both failed".into()),
            }
        }
        Err(Transport::Connect(e)) => return Refreshed::Later(e),
        Err(Transport::Timeout) => return Refreshed::Ended("a refresh timed out".into()),
    };
    let code = oauth_error(&answer.body);
    if code.as_deref() == Some("invalid_grant") {
        return Refreshed::Ended("invalid_grant".into());
    }
    if answer.status == 429 || (answer.status == 503 && answer.retry_after) {
        return Refreshed::Later(format!("HTTP {}", answer.status));
    }
    if matches!(
        code.as_deref(),
        Some("invalid_client" | "unauthorized_client" | "invalid_scope")
    ) {
        return Refreshed::Setup(code.unwrap_or_default());
    }
    if answer.status != 200 {
        return Refreshed::Later(format!(
            "HTTP {} {}",
            answer.status,
            code.unwrap_or_default()
        ));
    }
    match serde_json::from_slice::<Granted>(&answer.body) {
        Ok(granted) if granted.refresh_token.is_some() => Refreshed::Renewed(granted),
        // The refresh took effect, and its new refresh token is lost with this answer.
        _ => Refreshed::Ended("a refresh answered without new tokens".into()),
    }
}

// --- PKCE ---------------------------------------------------------------------------------

/// A PKCE verifier of 86 characters (43 to 128 allowed) and its S256 challenge.
pub fn pkce() -> (String, String) {
    use sha2::Digest as _;
    let verifier = format!(
        "{}{}",
        kaeso_crypto::random_token(),
        kaeso_crypto::random_token()
    );
    let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_follows_rfc_7636() {
        use sha2::Digest as _;
        let (verifier, challenge) = pkce();
        assert!((43..=128).contains(&verifier.len()), "{}", verifier.len());
        assert_eq!(
            challenge,
            URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
        );
    }

    #[test]
    fn an_assertion_is_an_es256_jwt_for_the_app() {
        let (key, _) = EcSigner::generate().unwrap();
        let signer = Signer {
            kid: "k1".into(),
            key,
        };
        let now = time::macros::datetime!(2026-11-17 12:00 UTC);
        let jwt = signer
            .assertion(
                "https://kaeso.example.com/i",
                "kaeso:linked-billing:anthropic",
                "aud-1",
                now,
            )
            .unwrap();
        let parts: Vec<&str> = jwt.expose_secret().split('.').collect();
        assert_eq!(parts.len(), 3);
        let decode = |p: &str| -> Value {
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(p).unwrap()).unwrap()
        };
        assert_eq!(
            decode(parts[0]),
            json!({"alg": "ES256", "typ": "JWT", "kid": "k1"})
        );
        let claims = decode(parts[1]);
        assert_eq!(claims["iss"], "https://kaeso.example.com/i");
        assert_eq!(claims["sub"], "kaeso:linked-billing:anthropic");
        assert_eq!(claims["aud"], "aud-1");
        assert_eq!(
            claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap(),
            300
        );
        assert!(claims["jti"].as_str().is_some_and(|j| !j.is_empty()));
        let jwk = signer.jwk().unwrap();
        assert_eq!(
            (jwk["kty"].as_str(), jwk["kid"].as_str()),
            (Some("EC"), Some("k1"))
        );
        assert!(!format!("{signer:?}").contains('x'), "{signer:?}");
    }

    #[test]
    fn oauth_errors_and_api_errors_name_their_code() {
        assert_eq!(
            oauth_error(br#"{"error":"invalid_grant","error_description":"x"}"#).as_deref(),
            Some("invalid_grant")
        );
        assert_eq!(
            oauth_error(br#"{"type":"error","error":{"type":"authentication_error","message":"Authentication failed"}}"#).as_deref(),
            Some("authentication_error")
        );
        assert_eq!(oauth_error(b"<html>"), None);
        assert!(
            LinkError::Refused {
                status: 400,
                code: "invalid_client".into()
            }
            .is_setup()
        );
        assert!(
            LinkError::Refused {
                status: 401,
                code: "authentication_error".into()
            }
            .is_setup()
        );
        assert!(
            !LinkError::Refused {
                status: 400,
                code: "invalid_request".into()
            }
            .is_setup()
        );
    }

    #[test]
    fn only_a_paused_503_keeps_a_refresh_token_unused() {
        let answer = |status, retry_after, body: &str| Answer {
            status,
            retry_after,
            body: Bytes::from(body.to_owned()),
        };
        assert!(!lost(&answer(400, false, "{}")));
        assert!(lost(&answer(500, false, "{}")));
        assert!(lost(&answer(503, false, r#"{"type":"error"}"#)));
        assert!(lost(&answer(503, true, "{}")));
        assert!(!lost(&answer(503, true, r#"{"type":"error","error":{}}"#)));
    }
}
