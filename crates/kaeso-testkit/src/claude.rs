//! A mock of Anthropic's linked billing: `auth.claude.com`'s pushed authorization request,
//! token and revocation endpoints, and the Claude API's service account token exchange and user
//! profiles, on one address. It checks what the linked billing guide asks of a platform: JSON
//! bodies, PKCE with S256, the exact redirect URI, the scopes, and an ES256 client assertion
//! signed with the key set a test gives it.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use serde_json::{Value, json};
use sha2::Digest;

pub const CLIENT_ID: &str = "oaapp_01MOCK";
pub const AUDIENCE: &str = "https://auth.claude.com/clients/oaapp_01MOCK";
pub const ORGANIZATION: &str = "00000000-0000-4000-8000-000000000001";
pub const RULE: &str = "fdrl_01MOCK";
pub const SCOPE: &str = "billing:attribution offline_access";
pub const LIFETIME: i64 = 3600;
pub const REFRESH_LIFETIME: i64 = 365 * 24 * 3600;

/// What the mock does wrong on purpose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    /// Every endpoint refuses the client: `invalid_client`.
    InvalidClient,
    /// The exchange answers `user_profile_already_linked`.
    AlreadyLinked,
    /// The exchange answers without a refresh token.
    NoRefreshToken,
    /// A refresh answers `invalid_grant`, as for a connection that ended.
    Ended,
    /// A refresh answers 500 once, then works, as a lost answer that a second try within 30
    /// seconds repeats.
    RefreshLostOnce,
    /// A refresh answers 503 with `retry-after` in the Claude API's format: the refresh token
    /// wasn't used.
    RefreshPaused,
    /// The service account token exchange is refused.
    FederationRefused,
    /// Revocation answers 503.
    RevokeDown,
}

struct Pushed {
    client_id: String,
    redirect_uri: String,
    user_profile_id: String,
    state: String,
    challenge: String,
}

struct Code {
    pushed: Pushed,
}

#[derive(Clone)]
struct Connection {
    id: String,
    user_profile_id: String,
    refresh: String,
    used_refresh: Vec<String>,
    revoked: bool,
}

#[derive(Default)]
struct Inner {
    redirect_uri: Mutex<String>,
    issuer: Mutex<String>,
    jwks: Mutex<Option<Value>>,
    fault: Mutex<Fault>,
    pushed: Mutex<HashMap<String, Pushed>>,
    codes: Mutex<HashMap<String, Code>>,
    connections: Mutex<Vec<Connection>>,
    profiles: Mutex<Vec<(String, String)>>,
    revoked: Mutex<Vec<Value>>,
    asked: Mutex<Vec<(String, Value)>>,
    counter: Mutex<u64>,
    lost_once: Mutex<bool>,
}

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone)]
pub struct MockClaude {
    inner: Arc<Inner>,
    pub base_url: String,
}

impl MockClaude {
    pub async fn start() -> MockClaude {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner::default());
        let app = Router::new()
            .route("/v1/oauth/par", post(par))
            .route("/v1/oauth/token", post(token))
            .route("/v1/oauth/revoke", post(revoke))
            .route("/v1/user_profiles", post(profile))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockClaude {
            inner,
            base_url: format!("http://{addr}"),
        }
    }

    /// The app as registered: its redirect URI, the issuer it trusts and that issuer's keys.
    pub fn register(&self, redirect_uri: &str, issuer: &str, jwks: Value) {
        *locked(&self.inner.redirect_uri) = redirect_uri.to_owned();
        *locked(&self.inner.issuer) = issuer.to_owned();
        *locked(&self.inner.jwks) = Some(jwks);
    }

    pub fn fail(&self, fault: Fault) {
        *locked(&self.inner.fault) = fault;
        *locked(&self.inner.lost_once) = false;
    }

    /// The person approves the request behind an approval page's address: the address the
    /// browser is sent back to, with a code and the state.
    pub fn approve(&self, authorize_url: &str) -> String {
        let (request_uri, client_id) = query_of(authorize_url);
        let pushed = locked(&self.inner.pushed)
            .remove(&request_uri)
            .expect("an unknown or used request_uri");
        assert_eq!(pushed.client_id, client_id, "the approval page's client_id");
        let code = format!("code-{}", self.next());
        let back = format!(
            "{}?code={code}&state={}",
            pushed.redirect_uri,
            urlencode(&pushed.state)
        );
        locked(&self.inner.codes).insert(code, Code { pushed });
        back
    }

    /// The person doesn't approve: the address the browser is sent back to with `error`.
    pub fn refuse(&self, authorize_url: &str, error: &str) -> String {
        let (request_uri, _) = query_of(authorize_url);
        let pushed = locked(&self.inner.pushed)
            .remove(&request_uri)
            .expect("an unknown or used request_uri");
        format!(
            "{}?error={error}&error_description=the+user+declined+the+request&state={}",
            pushed.redirect_uri,
            urlencode(&pushed.state)
        )
    }

    /// Every request the mock got, as `(path, body)`, oldest first.
    pub fn asked(&self) -> Vec<(String, Value)> {
        locked(&self.inner.asked).clone()
    }

    /// The bodies of the revocations, oldest first.
    pub fn revoked(&self) -> Vec<Value> {
        locked(&self.inner.revoked).clone()
    }

    /// The user profiles made, as `(id, name)`.
    pub fn profiles(&self) -> Vec<(String, String)> {
        locked(&self.inner.profiles).clone()
    }

    /// Ends a connection at the provider, as its organization unlinking in its console does.
    pub fn unlink(&self, connection_id: &str) {
        for c in locked(&self.inner.connections).iter_mut() {
            if c.id == connection_id {
                c.revoked = true;
            }
        }
    }

    /// How many refreshes the mock answered with new tokens.
    pub fn refreshes(&self) -> usize {
        self.asked()
            .iter()
            .filter(|(p, b)| p == "/v1/oauth/token" && b["grant_type"] == "refresh_token")
            .count()
    }

    fn next(&self) -> u64 {
        let mut n = locked(&self.inner.counter);
        *n += 1;
        *n
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn query_of(url: &str) -> (String, String) {
    let parsed = url::Url::parse(url).expect("an approval page address");
    let mut request_uri = String::new();
    let mut client_id = String::new();
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "request_uri" => request_uri = v.into_owned(),
            "client_id" => client_id = v.into_owned(),
            other => panic!("the approval page takes only client_id and request_uri, not {other}"),
        }
    }
    (request_uri, client_id)
}

fn oauth(status: StatusCode, error: &str) -> Response {
    (
        status,
        Json(json!({ "error": error, "error_description": format!("mock: {error}") })),
    )
        .into_response()
}

fn api_error(status: StatusCode, ty: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "type": "error", "error": { "type": ty, "message": message } })),
    )
        .into_response()
}

fn decode(part: &str) -> Option<Value> {
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).ok()?).ok()
}

// Checks a client assertion as the guide describes it: ES256 with a `kid` of the registered
// key set, the registered issuer, exactly one audience, and at most an hour of life.
fn assertion_ok(inner: &Inner, jwt: &str) -> bool {
    let parts: Vec<&str> = jwt.split('.').collect();
    let [header, claims, signature] = parts.as_slice() else {
        return false;
    };
    let (Some(h), Some(c)) = (decode(header), decode(claims)) else {
        return false;
    };
    if h["alg"] != "ES256" || c["iss"] != *locked(&inner.issuer) || c["aud"] != AUDIENCE {
        return false;
    }
    let (Some(iat), Some(exp)) = (c["iat"].as_i64(), c["exp"].as_i64()) else {
        return false;
    };
    if exp <= iat || exp - iat > 3600 || c["sub"].as_str().is_none_or(str::is_empty) {
        return false;
    }
    let jwks = locked(&inner.jwks).clone();
    let Some(key) = jwks.as_ref().and_then(|j| {
        j["keys"]
            .as_array()?
            .iter()
            .find(|k| k["kid"] == h["kid"])
            .cloned()
    }) else {
        return false;
    };
    let coordinate = |name: &str| {
        key[name]
            .as_str()
            .and_then(|s| URL_SAFE_NO_PAD.decode(s).ok())
            .unwrap_or_default()
    };
    let mut point = vec![4_u8];
    point.extend(coordinate("x"));
    point.extend(coordinate("y"));
    let Ok(verifying) = VerifyingKey::from_sec1_bytes(&point) else {
        return false;
    };
    let Some(bytes) = URL_SAFE_NO_PAD.decode(signature).ok() else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(&bytes) else {
        return false;
    };
    verifying
        .verify(format!("{header}.{claims}").as_bytes(), &sig)
        .is_ok()
}

fn client_ok(inner: &Inner, body: &Value) -> bool {
    *locked(&inner.fault) != Fault::InvalidClient
        && body["client_id"] == CLIENT_ID
        && body["client_assertion_type"] == "urn:ietf:params:oauth:client-assertion-type:jwt-bearer"
        && body["client_assertion"]
            .as_str()
            .is_some_and(|a| assertion_ok(inner, a))
}

fn read(inner: &Inner, path: &str, body: &Bytes) -> Option<Value> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let mut shown = value.clone();
    if let Some(o) = shown.as_object_mut() {
        for secret in [
            "client_assertion",
            "assertion",
            "refresh_token",
            "code_verifier",
        ] {
            if o.contains_key(secret) {
                o.insert(secret.into(), json!("…"));
            }
        }
    }
    locked(&inner.asked).push((path.to_owned(), shown));
    Some(value)
}

async fn par(State(inner): State<Arc<Inner>>, body: Bytes) -> Response {
    let Some(b) = read(&inner, "/v1/oauth/par", &body) else {
        return oauth(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if !client_ok(&inner, &b) {
        return oauth(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    if b["scope"] != "billing:attribution" && b["scope"] != SCOPE {
        return oauth(StatusCode::BAD_REQUEST, "invalid_scope");
    }
    let known_profile = locked(&inner.profiles)
        .iter()
        .any(|(id, _)| b["user_profile_id"] == id.as_str());
    if b["response_type"] != "code"
        || b["connection_type"] != "organization_shared"
        || b["connection_auth_target"] != "platform"
        || b["code_challenge_method"] != "S256"
        || b["redirect_uri"] != *locked(&inner.redirect_uri)
        || !known_profile
        || b["state"].as_str().is_none_or(str::is_empty)
    {
        return oauth(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let request_uri = format!(
        "urn:ietf:params:oauth:request_uri:{}",
        locked(&inner.pushed).len() + 1
    );
    locked(&inner.pushed).insert(
        request_uri.clone(),
        Pushed {
            client_id: CLIENT_ID.to_owned(),
            redirect_uri: b["redirect_uri"].as_str().unwrap_or_default().to_owned(),
            user_profile_id: b["user_profile_id"].as_str().unwrap_or_default().to_owned(),
            state: b["state"].as_str().unwrap_or_default().to_owned(),
            challenge: b["code_challenge"].as_str().unwrap_or_default().to_owned(),
        },
    );
    (
        StatusCode::CREATED,
        Json(json!({ "request_uri": request_uri, "expires_in": 600 })),
    )
        .into_response()
}

fn next(inner: &Inner) -> u64 {
    let mut n = locked(&inner.counter);
    *n += 1;
    *n
}

async fn token(State(inner): State<Arc<Inner>>, body: Bytes) -> Response {
    let Some(b) = read(&inner, "/v1/oauth/token", &body) else {
        return oauth(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let fault = *locked(&inner.fault);
    match b["grant_type"].as_str() {
        Some("urn:ietf:params:oauth:grant-type:jwt-bearer") => {
            let assertion = b["assertion"].as_str().unwrap_or_default();
            if fault == Fault::FederationRefused
                || b["federation_rule_id"] != RULE
                || b["organization_id"] != ORGANIZATION
                || !assertion_ok(&inner, assertion)
            {
                return api_error(
                    StatusCode::UNAUTHORIZED,
                    "authentication_error",
                    "Authentication failed",
                );
            }
            Json(json!({
                "access_token": format!("sk-ant-oat01-operator-{}", next(&inner)),
                "token_type": "Bearer",
                "expires_in": 600,
                "scope": "workspace:developer",
            }))
            .into_response()
        }
        Some("authorization_code") => exchange(&inner, &b, fault),
        Some("refresh_token") => refresh(&inner, &b, fault),
        _ => (StatusCode::BAD_REQUEST, "unsupported grant").into_response(),
    }
}

fn exchange(inner: &Inner, b: &Value, fault: Fault) -> Response {
    if !client_ok(inner, b) {
        return oauth(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let Some(code) = b["code"]
        .as_str()
        .and_then(|c| locked(&inner.codes).remove(c))
    else {
        return oauth(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let verifier = b["code_verifier"].as_str().unwrap_or_default();
    let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    if challenge != code.pushed.challenge || b["redirect_uri"] != code.pushed.redirect_uri.as_str()
    {
        return oauth(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    if fault == Fault::AlreadyLinked {
        return oauth(StatusCode::BAD_REQUEST, "user_profile_already_linked");
    }
    let n = next(inner);
    let connection = Connection {
        id: format!("ocon_{n:04}"),
        user_profile_id: code.pushed.user_profile_id.clone(),
        refresh: format!("lbref-{n}"),
        used_refresh: Vec::new(),
        revoked: false,
    };
    let mut answer = json!({
        "access_token": format!("lbtok-{n}"),
        "token_type": "Bearer",
        "expires_in": LIFETIME,
        "scope": "workspace:inference",
        "connection_id": connection.id,
    });
    if fault != Fault::NoRefreshToken {
        answer["refresh_token"] = json!(connection.refresh);
        answer["refresh_token_expires_in"] = json!(REFRESH_LIFETIME);
    }
    locked(&inner.connections).push(connection);
    Json(answer).into_response()
}

fn refresh(inner: &Inner, b: &Value, fault: Fault) -> Response {
    if !client_ok(inner, b) {
        return oauth(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    if b.get("scope").is_some() {
        return oauth(StatusCode::BAD_REQUEST, "invalid_scope");
    }
    if fault == Fault::RefreshPaused {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "30")],
            Json(json!({ "type": "error", "error": { "type": "overloaded_error", "message": "Overloaded" } })),
        )
            .into_response();
    }
    let sent = b["refresh_token"].as_str().unwrap_or_default().to_owned();
    let mut connections = locked(&inner.connections);
    let Some(c) = connections
        .iter_mut()
        .find(|c| c.refresh == sent || c.used_refresh.contains(&sent))
    else {
        return oauth(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    if fault == Fault::Ended || c.revoked || c.used_refresh.contains(&sent) {
        return oauth(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    if fault == Fault::RefreshLostOnce && !*locked(&inner.lost_once) {
        *locked(&inner.lost_once) = true;
        // The refresh took effect, and its answer is lost: the same refresh token, sent once
        // more at once, gets another new pair.
        return (StatusCode::INTERNAL_SERVER_ERROR, "lost").into_response();
    }
    let n = next(inner);
    c.used_refresh.push(sent);
    c.refresh = format!("lbref-{n}");
    Json(json!({
        "access_token": format!("lbtok-{n}"),
        "token_type": "Bearer",
        "expires_in": LIFETIME,
        "scope": "workspace:inference",
        "refresh_token": c.refresh,
        "refresh_token_expires_in": REFRESH_LIFETIME,
    }))
    .into_response()
}

async fn revoke(State(inner): State<Arc<Inner>>, body: Bytes) -> Response {
    let Some(b) = read(&inner, "/v1/oauth/revoke", &body) else {
        return oauth(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if !client_ok(&inner, &b) {
        return oauth(StatusCode::BAD_REQUEST, "invalid_client");
    }
    if *locked(&inner.fault) == Fault::RevokeDown {
        return (StatusCode::SERVICE_UNAVAILABLE, "down").into_response();
    }
    locked(&inner.revoked).push(b.clone());
    let mut ended = Vec::new();
    for c in locked(&inner.connections).iter_mut() {
        let named = b["connection_id"] == c.id.as_str()
            || b["user_profile_id"] == c.user_profile_id.as_str();
        if named && !c.revoked {
            c.revoked = true;
            ended.push(c.id.clone());
        }
    }
    Json(json!({ "revoked_connection_ids": ended })).into_response()
}

async fn profile(
    State(inner): State<Arc<Inner>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let Some(b) = read(&inner, "/v1/user_profiles", &body) else {
        return api_error(StatusCode::BAD_REQUEST, "invalid_request_error", "bad body");
    };
    let operator = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer sk-ant-oat01-operator-"));
    if !operator || headers.get("anthropic-version").is_none() {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "invalid bearer token",
        );
    }
    let id = format!("uprof_{:04}", next(&inner));
    locked(&inner.profiles).push((
        id.clone(),
        b["name"].as_str().unwrap_or_default().to_owned(),
    ));
    Json(json!({ "id": id, "type": "user_profile", "name": b["name"] })).into_response()
}
