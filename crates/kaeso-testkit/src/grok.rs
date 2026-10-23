use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::Generate;
use serde_json::{Value, json};

use crate::mock_provider::{Received, kpp_fixtures};
use crate::oauth::{Held, Hold, hold};

pub const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const SCOPES: &str = "openid profile email offline_access grok-cli:access api:access";
pub const REFERRER: &str = "kaeso";
pub const LIFETIME: i64 = 1800;
const KEY_ID: &str = "mock-es256";
const INVALID_KEY: &str =
    "Incorrect API key provided. You can obtain an API key from https://console.x.ai.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    NoApiAccess,
    NoIdToken,
    IdTokenWithoutEmail,
    WrongIssuer,
    WrongAudience,
    OtherUserInfo,
    UserInfoDown,
    NoRefreshToken,
    Challenge,
    SignedOut,
    Down,
    RevokeDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reply {
    #[default]
    Text,
    RateLimited,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Decision {
    Waiting,
    Approved { subject: String, email: String },
    Denied,
    Expired,
}

struct Device {
    user_code: String,
    decision: Decision,
}

struct Session {
    access: String,
    refresh: String,
    subject: String,
    email: String,
}

struct Inner {
    issuer: String,
    key: SigningKey,
    devices: Mutex<HashMap<String, Device>>,
    sessions: Mutex<Vec<Session>>,
    asked: Mutex<Vec<HashMap<String, String>>>,
    polls: AtomicUsize,
    slow_downs: AtomicUsize,
    refreshes: AtomicUsize,
    user_infos: AtomicUsize,
    issued: Mutex<Vec<String>>,
    revoked: Mutex<Vec<(String, String)>>,
    received: Mutex<Vec<Received>>,
    fault: Mutex<Fault>,
    reply: Mutex<Reply>,
    hold: Mutex<Option<Hold>>,
    info_hold: Mutex<Option<Hold>>,
}

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone)]
pub struct MockGrok {
    inner: Arc<Inner>,
}

impl MockGrok {
    pub async fn start() -> MockGrok {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let inner = Arc::new(Inner {
            issuer: format!("http://{addr}"),
            key: SigningKey::generate(),
            devices: Mutex::default(),
            sessions: Mutex::default(),
            asked: Mutex::default(),
            polls: AtomicUsize::new(0),
            slow_downs: AtomicUsize::new(0),
            refreshes: AtomicUsize::new(0),
            user_infos: AtomicUsize::new(0),
            issued: Mutex::default(),
            revoked: Mutex::default(),
            received: Mutex::default(),
            fault: Mutex::default(),
            reply: Mutex::default(),
            hold: Mutex::default(),
            info_hold: Mutex::default(),
        });
        let app = Router::new()
            .route("/.well-known/jwks.json", get(jwks))
            .route("/oauth2/device/code", post(device_code))
            .route("/oauth2/token", post(token))
            .route("/oauth2/revoke", post(revoke))
            .route("/oauth2/userinfo", get(user_info))
            .route("/v1/models/{id}", get(api_model))
            .fallback(api)
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockGrok { inner }
    }

    pub fn base_url(&self) -> &str {
        &self.inner.issuer
    }

    pub fn set_fault(&self, fault: Fault) {
        *locked(&self.inner.fault) = fault;
    }

    pub fn reply_with(&self, reply: Reply) {
        *locked(&self.inner.reply) = reply;
    }

    pub fn slow_down_next(&self) {
        self.inner.slow_downs.fetch_add(1, Ordering::SeqCst);
    }

    fn decide(&self, user_code: &str, decision: Decision) {
        let mut devices = locked(&self.inner.devices);
        let device = devices
            .values_mut()
            .find(|d| d.user_code == user_code)
            .expect("a device code with this user code");
        device.decision = decision;
    }

    pub fn approve(&self, user_code: &str, subject: &str, email: &str) {
        self.decide(
            user_code,
            Decision::Approved {
                subject: subject.to_owned(),
                email: email.to_owned(),
            },
        );
    }

    pub fn deny(&self, user_code: &str) {
        self.decide(user_code, Decision::Denied);
    }

    pub fn expire(&self, user_code: &str) {
        self.decide(user_code, Decision::Expired);
    }

    pub fn device_codes(&self) -> Vec<String> {
        locked(&self.inner.devices).keys().cloned().collect()
    }

    pub fn asked(&self) -> Vec<HashMap<String, String>> {
        locked(&self.inner.asked).clone()
    }

    pub fn polls(&self) -> usize {
        self.inner.polls.load(Ordering::SeqCst)
    }

    pub fn refreshes(&self) -> usize {
        self.inner.refreshes.load(Ordering::SeqCst)
    }

    pub fn user_infos(&self) -> usize {
        self.inner.user_infos.load(Ordering::SeqCst)
    }

    pub fn hold_next_refresh(&self) -> Held {
        let (hold, held) = hold();
        *locked(&self.inner.hold) = Some(hold);
        held
    }

    pub fn hold_next_user_info(&self) -> Held {
        let (hold, held) = hold();
        *locked(&self.inner.info_hold) = Some(hold);
        held
    }

    pub fn issued(&self) -> Vec<String> {
        locked(&self.inner.issued).clone()
    }

    pub fn revoked(&self) -> Vec<(String, String)> {
        locked(&self.inner.revoked).clone()
    }

    pub fn received(&self) -> Vec<Received> {
        locked(&self.inner.received).clone()
    }

    pub fn end_sessions(&self) {
        locked(&self.inner.sessions).clear();
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn jwt(inner: &Inner, claims: &Value) -> String {
    let header = b64(json!({ "alg": "ES256", "typ": "JWT", "kid": KEY_ID })
        .to_string()
        .as_bytes());
    let body = b64(claims.to_string().as_bytes());
    let input = format!("{header}.{body}");
    let signature: Signature = inner.key.sign(input.as_bytes());
    let token = format!("{input}.{}", b64(&signature.to_bytes()));
    locked(&inner.issued).push(token.clone());
    token
}

fn opaque(inner: &Inner, prefix: &str) -> String {
    let token = format!("{prefix}{}", uuid::Uuid::now_v7().simple());
    locked(&inner.issued).push(token.clone());
    token
}

fn oauth_error(status: StatusCode, code: &str) -> Response {
    (
        status,
        Json(json!({ "error": code, "error_description": "mock" })),
    )
        .into_response()
}

fn form_of(headers: &HeaderMap, body: &Bytes) -> Option<HashMap<String, String>> {
    let form = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
    form.then(|| url::form_urlencoded::parse(body).into_owned().collect())
}

fn not_a_form() -> Response {
    (
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Form requests must have `Content-Type: application/x-www-form-urlencoded`",
    )
        .into_response()
}

async fn jwks(State(inner): State<Arc<Inner>>) -> Json<Value> {
    let point = inner.key.verifying_key().to_sec1_point(false);
    let coordinate = |c: Option<&p256::FieldBytes>| b64(&c.map(|c| c.to_vec()).unwrap_or_default());
    Json(json!({ "keys": [{
        "kty": "EC",
        "crv": "P-256",
        "use": "sig",
        "alg": "ES256",
        "kid": KEY_ID,
        "x": coordinate(point.x()),
        "y": coordinate(point.y()),
    }] }))
}

async fn device_code(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(form) = form_of(&headers, &body) else {
        return not_a_form();
    };
    locked(&inner.asked).push(form.clone());
    match *locked(&inner.fault) {
        Fault::Down => {
            return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
        }
        Fault::NoApiAccess => return oauth_error(StatusCode::FORBIDDEN, "unauthorized_client"),
        _ => {}
    }
    if form.get("client_id").map(String::as_str) != Some(CLIENT_ID) {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_client");
    }
    let device_code = uuid::Uuid::now_v7().simple().to_string();
    let raw = uuid::Uuid::now_v7().simple().to_string().to_uppercase();
    let user_code = format!("{}-{}", &raw[24..28], &raw[28..32]);
    locked(&inner.devices).insert(
        device_code.clone(),
        Device {
            user_code: user_code.clone(),
            decision: Decision::Waiting,
        },
    );
    let page = format!("{}/oauth2/device", inner.issuer);
    Json(json!({
        "device_code": device_code,
        "user_code": user_code,
        "verification_uri": page,
        "verification_uri_complete": format!("{page}?user_code={user_code}"),
        "expires_in": 1800,
        "interval": 5,
    }))
    .into_response()
}

fn granted(inner: &Inner, subject: &str, email: &str, with_id_token: bool) -> Response {
    let fault = *locked(&inner.fault);
    let issued = now();
    let access = jwt(
        inner,
        &json!({
            "iss": inner.issuer,
            "aud": "https://api.x.ai",
            "sub": subject,
            "iat": issued,
            "exp": issued + LIFETIME,
            "jti": uuid::Uuid::now_v7().simple().to_string(),
        }),
    );
    let refresh = opaque(inner, "xai-refresh-");
    locked(&inner.sessions).push(Session {
        access: access.clone(),
        refresh: refresh.clone(),
        subject: subject.to_owned(),
        email: email.to_owned(),
    });
    let mut body = json!({
        "access_token": access,
        "refresh_token": refresh,
        "token_type": "Bearer",
        "expires_in": LIFETIME,
        "scope": SCOPES,
    });
    if fault == Fault::NoRefreshToken
        && let Some(fields) = body.as_object_mut()
    {
        fields.remove("refresh_token");
    }
    if with_id_token && fault != Fault::NoIdToken {
        let issuer = if fault == Fault::WrongIssuer {
            "https://evil.example".to_owned()
        } else {
            inner.issuer.clone()
        };
        let audience = if fault == Fault::WrongAudience {
            "someone-else"
        } else {
            CLIENT_ID
        };
        let mut claims = json!({ "iss": issuer, "aud": audience, "sub": subject, "iat": issued, "exp": issued + 3600 });
        if !matches!(
            fault,
            Fault::IdTokenWithoutEmail | Fault::OtherUserInfo | Fault::UserInfoDown
        ) {
            claims["email"] = json!(email);
            claims["email_verified"] = json!(true);
        }
        body["id_token"] = json!(jwt(inner, &claims));
    }
    (StatusCode::OK, Json(body)).into_response()
}

async fn token(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(form) = form_of(&headers, &body) else {
        return not_a_form();
    };
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    let fault = *locked(&inner.fault);
    if field("grant_type") == "urn:ietf:params:oauth:grant-type:device_code" {
        inner.polls.fetch_add(1, Ordering::SeqCst);
    }
    if fault == Fault::Down {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    if field("client_id") != CLIENT_ID {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_client");
    }
    if form.contains_key("resource") {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_target");
    }
    match field("grant_type").as_str() {
        "urn:ietf:params:oauth:grant-type:device_code" => {
            device_grant(&inner, &field("device_code"))
        }
        "refresh_token" => {
            let hold = locked(&inner.hold).take();
            if let Some(hold) = hold {
                hold.wait().await;
            }
            inner.refreshes.fetch_add(1, Ordering::SeqCst);
            refresh_grant(&inner, &field("refresh_token"))
        }
        _ => oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    }
}

fn device_grant(inner: &Inner, device_code: &str) -> Response {
    let slowed = inner
        .slow_downs
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
        .is_ok();
    if slowed {
        return oauth_error(StatusCode::BAD_REQUEST, "slow_down");
    }
    let decision = match locked(&inner.devices).get(device_code) {
        Some(device) => device.decision.clone(),
        None => return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"),
    };
    match decision {
        Decision::Waiting => oauth_error(StatusCode::BAD_REQUEST, "authorization_pending"),
        Decision::Denied => oauth_error(StatusCode::BAD_REQUEST, "access_denied"),
        Decision::Expired => oauth_error(StatusCode::BAD_REQUEST, "expired_token"),
        Decision::Approved { .. } if *locked(&inner.fault) == Fault::NoApiAccess => {
            oauth_error(StatusCode::FORBIDDEN, "unauthorized_client")
        }
        Decision::Approved { subject, email } => {
            locked(&inner.devices).remove(device_code);
            granted(inner, &subject, &email, true)
        }
    }
}

fn refresh_grant(inner: &Inner, refresh: &str) -> Response {
    match *locked(&inner.fault) {
        Fault::SignedOut => return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"),
        Fault::NoApiAccess => return oauth_error(StatusCode::FORBIDDEN, "unauthorized_client"),
        Fault::Challenge => {
            return (
                StatusCode::FORBIDDEN,
                [(header::CONTENT_TYPE, "text/html")],
                "<html><title>Just a moment...</title></html>",
            )
                .into_response();
        }
        _ => {}
    }
    let mut sessions = locked(&inner.sessions);
    let Some(at) = sessions.iter().position(|s| s.refresh == refresh) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let old = sessions.remove(at);
    drop(sessions);
    granted(inner, &old.subject, &old.email, false)
}

async fn revoke(State(inner): State<Arc<Inner>>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(form) = form_of(&headers, &body) else {
        return not_a_form();
    };
    if matches!(*locked(&inner.fault), Fault::Down | Fault::RevokeDown) {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    let token = field("token");
    locked(&inner.revoked).push((token.clone(), field("client_id")));
    locked(&inner.sessions).retain(|s| s.refresh != token);
    StatusCode::OK.into_response()
}

fn bearer(headers: &HeaderMap) -> String {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_owned()
}

async fn user_info(State(inner): State<Arc<Inner>>, headers: HeaderMap) -> Response {
    let hold = locked(&inner.info_hold).take();
    if let Some(hold) = hold {
        hold.wait().await;
    }
    inner.user_infos.fetch_add(1, Ordering::SeqCst);
    if *locked(&inner.fault) == Fault::UserInfoDown {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    let access = bearer(&headers);
    let sessions = locked(&inner.sessions);
    let Some(session) = sessions.iter().find(|s| s.access == access) else {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_token");
    };
    let subject = if *locked(&inner.fault) == Fault::OtherUserInfo {
        "someone-else".to_owned()
    } else {
        session.subject.clone()
    };
    Json(json!({ "sub": subject, "email": session.email, "email_verified": true })).into_response()
}

fn xai_error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "code": code, "error": message }))).into_response()
}

fn signed_in(inner: &Inner, headers: &HeaderMap) -> bool {
    let access = bearer(headers);
    locked(&inner.sessions).iter().any(|s| s.access == access)
}

fn record(inner: &Inner, method: &Method, uri: &Uri, headers: &HeaderMap, body: Bytes) {
    locked(&inner.received).push(Received {
        method: method.to_string(),
        path: uri.path().to_owned(),
        query: uri.query().map(str::to_owned),
        headers: headers
            .iter()
            .filter_map(|(n, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (n.as_str().to_owned(), v.to_owned()))
            })
            .collect(),
        body,
    });
}

async fn api_model(
    State(inner): State<Arc<Inner>>,
    Path(id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    record(&inner, &method, &uri, &headers, Bytes::new());
    if !signed_in(&inner, &headers) {
        return xai_error(StatusCode::BAD_REQUEST, "invalid-argument", INVALID_KEY);
    }
    if id != "grok-4.3" {
        return xai_error(
            StatusCode::NOT_FOUND,
            "not-found",
            "The model does not exist or your team does not have access to it.",
        );
    }
    Json(json!({ "id": id, "object": "model", "created": 1_790_000_000, "owned_by": "xai" }))
        .into_response()
}

fn fixture(path: &str) -> String {
    std::fs::read_to_string(
        kpp_fixtures()
            .join("openai_responses/providers/xai")
            .join(path),
    )
    .expect("an xAI fixture")
}

async fn api(
    State(inner): State<Arc<Inner>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    record(&inner, &method, &uri, &headers, body.clone());
    if !signed_in(&inner, &headers) {
        return xai_error(StatusCode::BAD_REQUEST, "invalid-argument", INVALID_KEY);
    }
    match (method, uri.path()) {
        (Method::GET, "/v1/models") => Json(json!({ "object": "list", "data": [
            { "id": "grok-4.3", "object": "model", "created": 1_790_000_000, "owned_by": "xai" }
        ] }))
        .into_response(),
        (Method::POST, "/v1/responses") => {
            if *locked(&inner.reply) == Reply::RateLimited {
                return xai_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "resource-exhausted",
                    "Your team has reached its monthly spending limit.",
                );
            }
            let streams = serde_json::from_slice::<Value>(&body)
                .ok()
                .and_then(|b| b["stream"].as_bool())
                .unwrap_or(false);
            if streams {
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from(fixture(
                        "streams/recorded-text-basic-stream.sse",
                    )))
                    .unwrap_or_default()
            } else {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    fixture("responses/recorded-text-input-string.json"),
                )
                    .into_response()
            }
        }
        _ => xai_error(StatusCode::NOT_FOUND, "not-found", "No such route."),
    }
}
