use std::time::Duration;

use bytes::Bytes;
use kaeso_core::secret;
use kaeso_crypto::{CryptoError, Sealed};
use kaeso_store::StoreError;
use kaeso_store::pg::mcp_connections::{self as store, NewConnection, SealedConnection};
use kaeso_store::pg::mcp_servers::{self, OAuthClient, ServerRow};
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_upstream::mcp::Limits;
use openidconnect::PkceCodeChallenge;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::mcp::oauth::endpoint;
use crate::state::AppState;

pub const CLIENT_DOCUMENT_PATH: &str = "oauth/client.json";
pub const CALLBACK_PATH: &str = "oauth/upstream/callback";
const STATE_LIFETIME: time::Duration = time::Duration::minutes(10);
const RENEW_BEFORE: time::Duration = time::Duration::seconds(60);
const DEFAULT_LIFETIME: i64 = 3600;
const METADATA: Limits = Limits {
    first_byte: Duration::from_secs(10),
    idle: Duration::from_secs(10),
    max_body: 64 * 1024,
};
const HEADER_FORBIDDEN: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "accept",
    "connection",
    "transfer-encoding",
    "mcp-protocol-version",
    "mcp-session-id",
    "mcp-method",
    "mcp-name",
];

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    #[error("cannot reach the sign-in server: {0}")]
    Unreachable(String),
    #[error("the server names no authorization server")]
    NoAuthServer,
    #[error("the authorization server doesn't offer PKCE with S256")]
    NoPkce,
    #[error("the authorization server takes neither client metadata documents nor registration")]
    NoRegistration,
    #[error("the authorization server refused: {0}")]
    Refused(String),
    #[error("the sign-in expired or was already used")]
    Expired,
    #[error("the sign-in answer came from another authorization server")]
    IssuerMismatch,
    #[error("the sign-in was declined: {0}")]
    Denied(String),
    #[error("the header can't carry a secret: {0}")]
    BadHeader(HeaderProblem),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HeaderProblem {
    #[error("its name must be 1 to 100 characters a header name may hold")]
    Name,
    #[error("Kaeso sets that header itself")]
    Reserved,
    #[error("its value must be printable ASCII")]
    Value,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("nobody has connected the server yet")]
    NotConnected,
    #[error("the connection was signed out and needs a new sign-in")]
    SignedOut,
    #[error("the connection's access ran out and couldn't be renewed")]
    Unrenewed,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

pub fn client_document_url(state: &AppState) -> String {
    endpoint(state, CLIENT_DOCUMENT_PATH)
}

pub fn callback_url(state: &AppState) -> String {
    endpoint(state, CALLBACK_PATH)
}

// The Client ID Metadata Document Kaeso signs in to MCP servers with: its URL is the client id.
pub fn client_document(state: &AppState) -> Value {
    json!({
        "client_id": client_document_url(state),
        "client_name": "Kaeso",
        "client_uri": endpoint(state, ""),
        "redirect_uris": [callback_url(state)],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
        "application_type": "web",
        "software_id": "kaeso",
        "software_version": env!("CARGO_PKG_VERSION"),
    })
}

fn aad(server: Uuid, user: Option<Uuid>) -> Vec<u8> {
    match user {
        Some(u) => format!("mcpconn:{server}:{u}").into_bytes(),
        None => format!("mcpconn:{server}:shared").into_bytes(),
    }
}

fn client_secret_aad(server: Uuid) -> Vec<u8> {
    format!("mcpclient:{server}").into_bytes()
}

fn last4(secret: &str) -> String {
    let count = secret.chars().count();
    secret.chars().skip(count.saturating_sub(4)).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthServer {
    pub issuer: String,
    pub authorization_endpoint: Url,
    pub token_endpoint: Url,
    pub registration_endpoint: Option<Url>,
    pub userinfo_endpoint: Option<Url>,
    pub cimd: bool,
    pub iss_parameter: bool,
    pub scopes: Vec<String>,
    pub resource: String,
}

async fn fetch(state: &AppState, url: &Url) -> Result<Option<Value>, SignInError> {
    let (status, body) = state
        .mcp
        .get_json(url, METADATA)
        .await
        .map_err(|e| SignInError::Unreachable(e.to_string()))?;
    if status != 200 {
        return Ok(None);
    }
    Ok(serde_json::from_slice::<Value>(&body)
        .ok()
        .filter(Value::is_object))
}

fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn trimmed(s: &str) -> &str {
    s.trim_end_matches('/')
}

pub fn resource_metadata_of(challenge: &str) -> Option<String> {
    let at = challenge.find("resource_metadata=\"")? + "resource_metadata=\"".len();
    let rest = challenge.get(at..)?;
    rest.find('"')
        .and_then(|end| rest.get(..end))
        .map(str::to_owned)
}

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn url_of(v: &Value, key: &str) -> Option<Url> {
    string(v, key).and_then(|s| Url::parse(&s).ok())
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub async fn discover(
    state: &AppState,
    server: &Url,
    challenge: Option<&str>,
) -> Result<AuthServer, SignInError> {
    let path = server.path().trim_end_matches('/');
    let mut candidates = Vec::new();
    if let Some(named) = challenge
        .and_then(resource_metadata_of)
        .and_then(|u| Url::parse(&u).ok())
    {
        candidates.push(named);
    }
    for suffix in [path, ""] {
        if let Ok(u) = Url::parse(&format!(
            "{}/.well-known/oauth-protected-resource{suffix}",
            origin(server)
        )) {
            candidates.push(u);
        }
    }
    let mut prm = None;
    for c in &candidates {
        if let Some(found) = fetch(state, c).await? {
            prm = Some(found);
            break;
        }
    }
    let prm = prm.ok_or(SignInError::NoAuthServer)?;
    // RFC 9728 §3.3: the metadata must be about the server Kaeso asked for, so another resource
    // can't send Kaeso's sign-in somewhere else.
    let resource = string(&prm, "resource").unwrap_or_else(|| server.to_string());
    if !trimmed(server.as_str()).starts_with(trimmed(&resource)) {
        return Err(SignInError::NoAuthServer);
    }
    let issuer = strings(&prm, "authorization_servers")
        .into_iter()
        .next()
        .ok_or(SignInError::NoAuthServer)?;
    let issuer_url = Url::parse(&issuer).map_err(|_| SignInError::NoAuthServer)?;
    let ipath = issuer_url.path().trim_end_matches('/');
    let o = origin(&issuer_url);
    let mut meta = None;
    for candidate in [
        format!("{o}/.well-known/oauth-authorization-server{ipath}"),
        format!("{o}/.well-known/openid-configuration{ipath}"),
        format!("{o}{ipath}/.well-known/openid-configuration"),
    ] {
        let Ok(u) = Url::parse(&candidate) else {
            continue;
        };
        if let Some(found) = fetch(state, &u).await?
            && string(&found, "issuer").is_some_and(|i| trimmed(&i) == trimmed(&issuer))
        {
            meta = Some(found);
            break;
        }
    }
    let meta = meta.ok_or(SignInError::NoAuthServer)?;
    if !strings(&meta, "code_challenge_methods_supported")
        .iter()
        .any(|m| m == "S256")
    {
        return Err(SignInError::NoPkce);
    }
    let scopes = strings(&prm, "scopes_supported");
    Ok(AuthServer {
        issuer,
        authorization_endpoint: url_of(&meta, "authorization_endpoint")
            .ok_or(SignInError::NoAuthServer)?,
        token_endpoint: url_of(&meta, "token_endpoint").ok_or(SignInError::NoAuthServer)?,
        registration_endpoint: url_of(&meta, "registration_endpoint"),
        userinfo_endpoint: url_of(&meta, "userinfo_endpoint"),
        cimd: meta
            .get("client_id_metadata_document_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        iss_parameter: meta
            .get("authorization_response_iss_parameter_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        scopes,
        resource,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Client {
    id: String,
    #[serde(default)]
    auth: Option<String>,
}

async fn client_for(
    state: &AppState,
    server: &ServerRow,
    auth: &AuthServer,
) -> Result<(Client, Option<SecretString>), SignInError> {
    let manual = server.oauth_registration.as_deref() == Some("manual");
    // Client credentials belong to the authorization server that issued them, so Kaeso never
    // reuses them with another one; a manual client is the admin's statement for this server.
    let same_issuer = server
        .oauth_issuer
        .as_deref()
        .is_some_and(|i| trimmed(i) == trimmed(&auth.issuer));
    if let Some(id) = server.oauth_client_id.as_deref()
        && (same_issuer || (manual && server.oauth_issuer.is_none()))
    {
        let secret = match mcp_servers::client_secret(&state.pg, server.id).await? {
            Some(s) => Some(open_secret(state, server.id, s)?),
            None => None,
        };
        if manual && server.oauth_issuer.is_none() {
            remember_client(
                state,
                server.id,
                "manual",
                &auth.issuer,
                id,
                secret.as_ref(),
            )
            .await?;
        }
        let method = if secret.is_some() {
            "client_secret_post"
        } else {
            "none"
        };
        return Ok((
            Client {
                id: id.to_owned(),
                auth: Some(method.to_owned()),
            },
            secret,
        ));
    }
    if manual {
        return Err(SignInError::NoRegistration);
    }
    if auth.cimd {
        let id = client_document_url(state);
        remember_client(state, server.id, "cimd", &auth.issuer, &id, None).await?;
        return Ok((
            Client {
                id,
                auth: Some("none".to_owned()),
            },
            None,
        ));
    }
    let Some(registration) = &auth.registration_endpoint else {
        return Err(SignInError::NoRegistration);
    };
    let mut document = client_document(state);
    if let Some(o) = document.as_object_mut() {
        o.remove("client_id");
    }
    let body = serde_json::to_vec(&document).unwrap_or_default();
    let (status, answer) = state
        .mcp
        .post_json(registration, Bytes::from(body), METADATA)
        .await
        .map_err(|e| SignInError::Unreachable(e.to_string()))?;
    let answer: Value = serde_json::from_slice(&answer).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        return Err(SignInError::Refused(refusal(&answer, status)));
    }
    let id = string(&answer, "client_id").ok_or(SignInError::NoRegistration)?;
    let secret = string(&answer, "client_secret").map(SecretString::from);
    remember_client(state, server.id, "dcr", &auth.issuer, &id, secret.as_ref()).await?;
    let method = string(&answer, "token_endpoint_auth_method").unwrap_or_else(|| {
        if secret.is_some() {
            "client_secret_basic".to_owned()
        } else {
            "none".to_owned()
        }
    });
    Ok((
        Client {
            id,
            auth: Some(method),
        },
        secret,
    ))
}

fn open_secret(
    state: &AppState,
    server: Uuid,
    s: mcp_servers::SealedSecret,
) -> Result<SecretString, CryptoError> {
    let opened = kaeso_crypto::open(
        &state.keyring,
        &Sealed {
            ciphertext: s.ciphertext,
            nonce: s.nonce,
            wrapped_key: s.wrapped_key,
            key_id: s.key_id,
        },
        &client_secret_aad(server),
    )?;
    let text =
        String::from_utf8(opened.expose_secret().clone()).map_err(|_| CryptoError::Malformed)?;
    Ok(SecretString::from(text))
}

async fn remember_client(
    state: &AppState,
    server: Uuid,
    registration: &str,
    issuer: &str,
    id: &str,
    secret: Option<&SecretString>,
) -> Result<(), SignInError> {
    let sealed = match secret {
        Some(s) => Some(kaeso_crypto::seal(
            &state.keyring,
            s.expose_secret().as_bytes(),
            &client_secret_aad(server),
        )?),
        None => None,
    };
    mcp_servers::set_oauth_client(
        &state.pg,
        server,
        &OAuthClient {
            registration,
            issuer,
            client_id: id,
            secret: sealed
                .as_ref()
                .map(|s| kaeso_store::pg::credentials::Sealed {
                    ciphertext: &s.ciphertext,
                    nonce: &s.nonce,
                    wrapped_key: &s.wrapped_key,
                    key_id: &s.key_id,
                }),
        },
    )
    .await?;
    Ok(())
}

pub async fn set_manual_client(
    state: &AppState,
    server: Uuid,
    client_id: &str,
    secret: Option<&SecretString>,
) -> Result<(), SignInError> {
    let sealed = match secret {
        Some(s) => Some(kaeso_crypto::seal(
            &state.keyring,
            s.expose_secret().as_bytes(),
            &client_secret_aad(server),
        )?),
        None => None,
    };
    let secret = sealed
        .as_ref()
        .map(|s| kaeso_store::pg::credentials::Sealed {
            ciphertext: &s.ciphertext,
            nonce: &s.nonce,
            wrapped_key: &s.wrapped_key,
            key_id: &s.key_id,
        });
    mcp_servers::set_manual_client(&state.pg, server, client_id, secret.as_ref()).await?;
    Ok(())
}

fn refusal(answer: &Value, status: u16) -> String {
    let code = string(answer, "error").unwrap_or_else(|| format!("HTTP {status}"));
    match string(answer, "error_description") {
        Some(d) => format!("{code}: {}", d.chars().take(200).collect::<String>()),
        None => code,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pending {
    org: Uuid,
    server: Uuid,
    connection_user: Option<Uuid>,
    started_by: Uuid,
    verifier: String,
    issuer: String,
    iss_parameter: bool,
    client: Client,
    redirect_uri: String,
    resource: String,
    token_endpoint: String,
    userinfo_endpoint: Option<String>,
}

#[derive(Debug)]
pub struct Begun {
    pub url: String,
}

pub async fn begin(
    state: &AppState,
    server: &ServerRow,
    connection_user: Option<Uuid>,
    started_by: Uuid,
    challenge: Option<&str>,
) -> Result<Begun, SignInError> {
    let server_url = Url::parse(&server.url).map_err(|_| SignInError::NoAuthServer)?;
    let auth = discover(state, &server_url, challenge).await?;
    let (client, _) = client_for(state, server, &auth).await?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let token = kaeso_crypto::random_token();
    let now = state.clock.now();
    let pending = Pending {
        org: server.org_id,
        server: server.id,
        connection_user,
        started_by,
        verifier: verifier.secret().clone(),
        issuer: auth.issuer.clone(),
        iss_parameter: auth.iss_parameter,
        client: client.clone(),
        redirect_uri: callback_url(state),
        resource: auth.resource.clone(),
        token_endpoint: auth.token_endpoint.to_string(),
        userinfo_endpoint: auth.userinfo_endpoint.as_ref().map(Url::to_string),
    };
    tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::McpOAuth,
            hash: &secret::hash(&token),
            user_id: Some(started_by),
            email: None,
            data: serde_json::to_value(&pending).map_err(|_| CryptoError::Malformed)?,
            expires_at: now + STATE_LIFETIME,
        },
    )
    .await?;
    let mut url = auth.authorization_endpoint.clone();
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", &client.id)
            .append_pair("redirect_uri", &pending.redirect_uri)
            .append_pair("code_challenge", challenge.as_str())
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &token)
            .append_pair("resource", &auth.resource);
        let scopes = if server.scopes.is_empty() {
            &auth.scopes
        } else {
            &server.scopes
        };
        if !scopes.is_empty() {
            q.append_pair("scope", &scopes.join(" "));
        }
    }
    Ok(Begun {
        url: url.to_string(),
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub iss: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finished {
    pub org: Uuid,
    pub server: Uuid,
    pub user: Option<Uuid>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    client_id: String,
    #[serde(default)]
    client_auth: Option<String>,
    token_endpoint: String,
    resource: String,
}

#[derive(Deserialize)]
struct Granted {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

// The person who finishes a sign-in must be the one who started it: otherwise anyone could hand
// someone a link that puts that person's upstream account on a connection they don't own.
pub async fn finish(
    state: &AppState,
    params: &CallbackParams,
    signed_in_as: Option<Uuid>,
) -> Result<Finished, (Option<Finished>, SignInError)> {
    let Some(token) = params.state.as_deref() else {
        return Err((None, SignInError::Expired));
    };
    let now = state.clock.now();
    let found = tokens::consume(&state.pg, TokenKind::McpOAuth, &secret::hash(token), now)
        .await
        .map_err(|e| (None, e.into()))?
        .ok_or((None, SignInError::Expired))?;
    let pending: Pending =
        serde_json::from_value(found.data).map_err(|_| (None, SignInError::Expired))?;
    let finished = Finished {
        org: pending.org,
        server: pending.server,
        user: pending.connection_user,
    };
    let fail = |e: SignInError| (Some(finished), e);
    if signed_in_as != Some(pending.started_by) {
        return Err(fail(SignInError::Expired));
    }
    if let Some(error) = &params.error {
        let why = params.error_description.as_deref().unwrap_or(error);
        return Err(fail(SignInError::Denied(why.chars().take(200).collect())));
    }
    // RFC 9207: an authorization response names its issuer, so an answer from another server
    // can't be redeemed here.
    match params.iss.as_deref() {
        Some(iss) if trimmed(iss) != trimmed(&pending.issuer) => {
            return Err(fail(SignInError::IssuerMismatch));
        }
        None if pending.iss_parameter => return Err(fail(SignInError::IssuerMismatch)),
        _ => {}
    }
    let Some(code) = params.code.as_deref() else {
        return Err(fail(SignInError::Denied("no code".into())));
    };
    let server = mcp_servers::get(&state.pg, pending.org, pending.server)
        .await
        .map_err(|e| fail(e.into()))?
        .ok_or_else(|| fail(SignInError::Expired))?;
    let secret = match mcp_servers::client_secret(&state.pg, server.id)
        .await
        .map_err(|e| fail(e.into()))?
    {
        Some(s) => Some(open_secret(state, server.id, s).map_err(|e| fail(e.into()))?),
        None => None,
    };
    let token_endpoint =
        Url::parse(&pending.token_endpoint).map_err(|_| fail(SignInError::NoAuthServer))?;
    let form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", pending.redirect_uri.as_str()),
        ("client_id", pending.client.id.as_str()),
        ("code_verifier", pending.verifier.as_str()),
        ("resource", pending.resource.as_str()),
    ];
    let granted = token_call(
        state,
        &token_endpoint,
        form,
        &pending.client,
        secret.as_ref(),
    )
    .await
    .map_err(fail)?;
    let label = account_label(
        state,
        pending.userinfo_endpoint.as_deref(),
        &granted.access_token,
    )
    .await;
    let stored = StoredTokens {
        access_token: granted.access_token.clone(),
        refresh_token: granted.refresh_token.clone(),
        client_id: pending.client.id.clone(),
        client_auth: pending.client.auth.clone(),
        token_endpoint: pending.token_endpoint.clone(),
        resource: pending.resource.clone(),
    };
    let expires_at =
        now + time::Duration::seconds(granted.expires_in.unwrap_or(DEFAULT_LIFETIME).max(0));
    put_connection(
        state,
        &server,
        pending.connection_user,
        "oauth",
        Some(&pending.issuer),
        None,
        &serde_json::to_string(&stored).map_err(|_| fail(CryptoError::Malformed.into()))?,
        &granted.access_token,
        Some(expires_at),
        label.as_deref(),
    )
    .await
    .map_err(fail)?;
    Ok(finished)
}

async fn token_call(
    state: &AppState,
    endpoint: &Url,
    mut form: Vec<(&str, &str)>,
    client: &Client,
    secret: Option<&SecretString>,
) -> Result<Granted, SignInError> {
    let mut basic = None;
    match (client.auth.as_deref(), secret) {
        (Some("client_secret_post"), Some(s)) => form.push(("client_secret", s.expose_secret())),
        (_, Some(s)) => basic = Some((client.id.as_str(), s)),
        _ => {}
    }
    let (status, body) = state
        .mcp
        .post_form(endpoint, &form, basic, METADATA)
        .await
        .map_err(|e| SignInError::Unreachable(e.to_string()))?;
    let answer: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    if status != 200 {
        return Err(SignInError::Refused(refusal(&answer, status)));
    }
    serde_json::from_value(answer).map_err(|_| SignInError::Refused("no access token".into()))
}

async fn account_label(state: &AppState, userinfo: Option<&str>, access: &str) -> Option<String> {
    let url = Url::parse(userinfo?).ok()?;
    let bearer = SecretString::from(format!("Bearer {access}"));
    let (status, body) = state
        .mcp
        .get_json_signed(&url, "authorization", &bearer, METADATA)
        .await
        .ok()?;
    if status != 200 {
        return None;
    }
    let info: Value = serde_json::from_slice(&body).ok()?;
    ["preferred_username", "email", "name"]
        .iter()
        .find_map(|k| string(&info, k))
        .map(|l| l.chars().take(320).collect())
        .filter(|l: &String| !l.is_empty())
}

#[allow(clippy::too_many_arguments)]
async fn put_connection(
    state: &AppState,
    server: &ServerRow,
    user: Option<Uuid>,
    kind: &str,
    issuer: Option<&str>,
    header_name: Option<&str>,
    secret_form: &str,
    shown: &str,
    expires_at: Option<OffsetDateTime>,
    label: Option<&str>,
) -> Result<Uuid, SignInError> {
    let sealed = kaeso_crypto::seal(
        &state.keyring,
        secret_form.as_bytes(),
        &aad(server.id, user),
    )?;
    let mut conn = state.pg.acquire().await.map_err(StoreError::from)?;
    let id = store::put(
        &mut conn,
        &NewConnection {
            id: Uuid::now_v7(),
            server_id: server.id,
            org_id: server.org_id,
            user_id: user,
            kind,
            issuer,
            header_name,
            sealed: kaeso_store::pg::credentials::Sealed {
                ciphertext: &sealed.ciphertext,
                nonce: &sealed.nonce,
                wrapped_key: &sealed.wrapped_key,
                key_id: &sealed.key_id,
            },
            last4: &last4(shown),
            expires_at,
            account_label: label,
        },
    )
    .await?;
    drop(conn);
    forget(state, server.id).await;
    Ok(id)
}

fn valid_header(name: &str) -> Result<(), SignInError> {
    if name.is_empty()
        || name.len() > 100
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+.^_`|~-".contains(&b))
    {
        return Err(SignInError::BadHeader(HeaderProblem::Name));
    }
    if HEADER_FORBIDDEN.contains(&name.to_ascii_lowercase().as_str())
        || name.to_ascii_lowercase().starts_with("mcp-param-")
    {
        return Err(SignInError::BadHeader(HeaderProblem::Reserved));
    }
    Ok(())
}

pub fn check_header(name: &str, value: &SecretString) -> Result<(), HeaderProblem> {
    match valid_header(name) {
        Err(SignInError::BadHeader(p)) => return Err(p),
        Err(_) => return Err(HeaderProblem::Name),
        Ok(()) => {}
    }
    let v = value.expose_secret();
    if v.is_empty() || !v.bytes().all(|b| b == b'\t' || (0x20..=0x7e).contains(&b)) {
        return Err(HeaderProblem::Value);
    }
    Ok(())
}

pub async fn set_header(
    state: &AppState,
    server: &ServerRow,
    user: Option<Uuid>,
    name: &str,
    value: &SecretString,
) -> Result<Uuid, SignInError> {
    check_header(name, value).map_err(SignInError::BadHeader)?;
    let v = value.expose_secret();
    put_connection(
        state,
        server,
        user,
        "header",
        None,
        Some(name),
        v,
        v,
        None,
        None,
    )
    .await
}

#[derive(Clone)]
pub struct Credential {
    pub header: String,
    pub value: SecretString,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

fn open_connection(state: &AppState, row: &SealedConnection) -> Result<String, CryptoError> {
    let opened = kaeso_crypto::open(
        &state.keyring,
        &Sealed {
            ciphertext: row.ciphertext.clone(),
            nonce: row.nonce.clone(),
            wrapped_key: row.wrapped_key.clone(),
            key_id: row.key_id.clone(),
        },
        &aad(row.server_id, row.user_id),
    )?;
    String::from_utf8(opened.expose_secret().clone()).map_err(|_| CryptoError::Malformed)
}

fn due(expires_at: Option<OffsetDateTime>, now: OffsetDateTime) -> bool {
    expires_at.is_some_and(|e| now >= e - RENEW_BEFORE)
}

fn credential_of(state: &AppState, row: &SealedConnection) -> Result<Credential, ConnectionError> {
    let plain = open_connection(state, row)?;
    match row.kind.as_str() {
        "header" => Ok(Credential {
            header: row
                .header_name
                .clone()
                .unwrap_or_else(|| "authorization".into()),
            value: SecretString::from(plain),
        }),
        _ => {
            let tokens: StoredTokens =
                serde_json::from_str(&plain).map_err(|_| CryptoError::Malformed)?;
            Ok(Credential {
                header: "authorization".into(),
                value: SecretString::from(format!("Bearer {}", tokens.access_token)),
            })
        }
    }
}

// The credential Kaeso sends to a server for a person, or for everyone with a shared
// connection; `None` for a server without sign-in. An OAuth token is renewed in its last minute.
pub async fn credential(
    state: &AppState,
    server: &ServerRow,
    user: Option<Uuid>,
) -> Result<Option<Credential>, ConnectionError> {
    resolve(state, server, user).await.map(|(c, _)| c)
}

#[derive(Clone)]
pub struct Held {
    credential: Option<Credential>,
    renew_at: Option<OffsetDateTime>,
}

fn owner_key(user: Option<Uuid>) -> String {
    user.map_or_else(|| "shared".to_owned(), |u| u.to_string())
}

// The portal's credentials stay in memory until a connection changes or its token is due, so a
// warm call reads no row; only the instance's own memory ever holds them opened.
pub async fn cached(
    state: &AppState,
    server: Uuid,
    user: Option<Uuid>,
) -> Result<Option<Credential>, ConnectionError> {
    let group = server.to_string();
    let rest = owner_key(user);
    for _ in 0..2 {
        let held = state
            .caches
            .mcp_credentials
            .get_or_load(&group, &rest, || async {
                let row = mcp_servers::get_by_id(&state.pg, server)
                    .await?
                    .ok_or(ConnectionError::NotConnected)?;
                let (credential, expires) = resolve(state, &row, user).await?;
                Ok::<_, ConnectionError>(Some(Held {
                    credential,
                    renew_at: expires.map(|e| e - RENEW_BEFORE),
                }))
            })
            .await?
            .ok_or(ConnectionError::NotConnected)?;
        if held.renew_at.is_none_or(|r| state.clock.now() < r) {
            return Ok(held.credential.clone());
        }
        state.caches.mcp_credentials.invalidate_group(&group).await;
    }
    Err(ConnectionError::Unrenewed)
}

pub async fn forget(state: &AppState, server: Uuid) {
    state
        .caches
        .mcp_credentials
        .invalidate_group(&server.to_string())
        .await;
}

// A server that refuses a credential Kaeso still thought good has ended the sign-in; the
// connection flips once, so the people responsible hear about it once.
pub async fn rejected(
    state: &AppState,
    org: Uuid,
    server: Uuid,
    user: Option<Uuid>,
) -> Result<(), StoreError> {
    if let Some(row) = store::sealed(&state.pg, server, user).await?
        && store::sign_out(&state.pg, row.id).await?
    {
        tracing::warn!(server = %server, connection = %row.id, "an MCP server refused Kaeso's sign-in; it needs a new one");
        if let Some(found) = mcp_servers::get(&state.pg, org, server).await? {
            crate::services::notifications::system::server_signed_out(state, &found, row.user_id)
                .await;
        }
    }
    forget(state, server).await;
    Ok(())
}

async fn resolve(
    state: &AppState,
    server: &ServerRow,
    user: Option<Uuid>,
) -> Result<(Option<Credential>, Option<OffsetDateTime>), ConnectionError> {
    if server.auth_method == "none" {
        return Ok((None, None));
    }
    let owner = if server.credential_mode == "per_user" {
        Some(user.ok_or(ConnectionError::NotConnected)?)
    } else {
        None
    };
    let row = store::sealed(&state.pg, server.id, owner)
        .await?
        .ok_or(ConnectionError::NotConnected)?;
    if row.auth_error.is_some() {
        return Err(ConnectionError::SignedOut);
    }
    let now = state.clock.now();
    if row.kind == "oauth" && due(row.expires_at, now) {
        let renewed = renew(state, server, owner, row).await?;
        if renewed.auth_error.is_some() {
            return Err(ConnectionError::SignedOut);
        }
        if renewed.expires_at.is_some_and(|e| now >= e) {
            return Err(ConnectionError::Unrenewed);
        }
        return Ok((Some(credential_of(state, &renewed)?), renewed.expires_at));
    }
    Ok((Some(credential_of(state, &row)?), row.expires_at))
}

// OAuth servers may rotate the refresh token as they answer, so the answer must be stored even
// when the request that asked is gone: the work runs as its own task, which shutdown waits for.
async fn renew(
    state: &AppState,
    server: &ServerRow,
    owner: Option<Uuid>,
    stale: SealedConnection,
) -> Result<SealedConnection, ConnectionError> {
    let (done, outcome) = tokio::sync::oneshot::channel();
    let st = state.clone();
    let server = server.clone();
    state.in_flight.finish(async move {
        let _ = done.send(renewed(&st, &server, owner, stale).await);
    });
    outcome.await.unwrap_or(Err(ConnectionError::Unrenewed))
}

// One renewal per connection at a time: the flight keeps this instance's requests from
// queueing, and the advisory lock with the re-read row keeps other replicas from spending a
// rotating refresh token twice.
async fn renewed(
    state: &AppState,
    server: &ServerRow,
    owner: Option<Uuid>,
    stale: SealedConnection,
) -> Result<SealedConnection, ConnectionError> {
    let _flight = state
        .caches
        .refreshes
        .board(&format!("mcpconn:{}", stale.id))
        .await;
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    store::lock_refresh(&mut tx, stale.id).await?;
    let Some(row) = store::sealed(&mut *tx, server.id, owner).await? else {
        return Err(ConnectionError::NotConnected);
    };
    let now = state.clock.now();
    if row.auth_error.is_some() || row.revision != stale.revision || !due(row.expires_at, now) {
        tx.commit().await.map_err(StoreError::from)?;
        return Ok(row);
    }
    let tokens: StoredTokens =
        serde_json::from_str(&open_connection(state, &row)?).map_err(|_| CryptoError::Malformed)?;
    let client = Client {
        id: tokens.client_id.clone(),
        auth: tokens.client_auth.clone(),
    };
    let secret = match mcp_servers::client_secret(&mut *tx, server.id).await? {
        Some(s) => Some(open_secret(state, server.id, s)?),
        None => None,
    };
    let Some(refresh) = tokens.refresh_token.clone() else {
        return signed_out(state, tx, server, row, "no refresh token").await;
    };
    let Ok(endpoint) = Url::parse(&tokens.token_endpoint) else {
        return signed_out(state, tx, server, row, "no token endpoint").await;
    };
    let form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.as_str()),
        ("client_id", tokens.client_id.as_str()),
        ("resource", tokens.resource.as_str()),
    ];
    match token_call(state, &endpoint, form, &client, secret.as_ref()).await {
        Ok(granted) => {
            let fresh = StoredTokens {
                access_token: granted.access_token.clone(),
                refresh_token: granted.refresh_token.or(tokens.refresh_token),
                ..tokens
            };
            let form = serde_json::to_string(&fresh).map_err(|_| CryptoError::Malformed)?;
            let sealed =
                kaeso_crypto::seal(&state.keyring, form.as_bytes(), &aad(server.id, owner))?;
            let expires_at = now
                + time::Duration::seconds(granted.expires_in.unwrap_or(DEFAULT_LIFETIME).max(0));
            store::refreshed(
                &mut *tx,
                row.id,
                row.revision,
                &kaeso_store::pg::credentials::Sealed {
                    ciphertext: &sealed.ciphertext,
                    nonce: &sealed.nonce,
                    wrapped_key: &sealed.wrapped_key,
                    key_id: &sealed.key_id,
                },
                Some(expires_at),
            )
            .await?;
            let renewed = store::sealed(&mut *tx, server.id, owner)
                .await?
                .ok_or(ConnectionError::NotConnected)?;
            tx.commit().await.map_err(StoreError::from)?;
            Ok(renewed)
        }
        Err(SignInError::Refused(why)) => signed_out(state, tx, server, row, &why).await,
        Err(e) => {
            tracing::warn!(error = %e, server = %server.id, "cannot renew an MCP connection's access token");
            drop(tx);
            Ok(row)
        }
    }
}

async fn signed_out(
    state: &AppState,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    server: &ServerRow,
    row: SealedConnection,
    why: &str,
) -> Result<SealedConnection, ConnectionError> {
    let flipped = store::sign_out(&mut *tx, row.id).await?;
    tx.commit().await.map_err(StoreError::from)?;
    if flipped {
        tracing::warn!(server = %server.id, connection = %row.id, reason = %why, "an MCP server ended a connection's sign-in; it needs a new one");
        crate::services::notifications::system::server_signed_out(state, server, row.user_id).await;
    }
    Ok(SealedConnection {
        auth_error: Some("signed_out".to_owned()),
        ..row
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_names_where_the_metadata_is() {
        assert_eq!(
            resource_metadata_of(
                r#"Bearer error="invalid_token", resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource/mcp""#
            )
            .as_deref(),
            Some("https://mcp.example.com/.well-known/oauth-protected-resource/mcp")
        );
        assert_eq!(resource_metadata_of("Bearer realm=\"x\""), None);
        assert_eq!(
            resource_metadata_of("Bearer resource_metadata=\"unclosed"),
            None
        );
    }

    #[test]
    fn tokens_renew_in_their_last_minute() {
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(20_000);
        assert!(!due(None, now));
        assert!(!due(Some(now + time::Duration::seconds(61)), now));
        assert!(due(Some(now + time::Duration::seconds(60)), now));
        assert!(due(Some(now - time::Duration::seconds(1)), now));
    }

    #[test]
    fn headers_kaeso_sets_itself_are_refused() {
        assert!(valid_header("X-Api-Key").is_ok());
        assert!(valid_header("authorization").is_ok());
        for bad in [
            "",
            "Host",
            "mcp-session-id",
            "Mcp-Param-Region",
            "x y",
            "x:y",
        ] {
            assert!(valid_header(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn last4_counts_characters() {
        assert_eq!(last4("sk-abcdef"), "cdef");
        assert_eq!(last4("ab"), "ab");
        assert_eq!(last4("ééééé"), "éééé");
    }

    #[test]
    fn the_secret_is_bound_to_its_connection() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert_ne!(aad(a, None), aad(a, Some(b)));
        assert_ne!(aad(a, None), aad(b, None));
        assert_ne!(aad(a, None), client_secret_aad(a));
    }
}
