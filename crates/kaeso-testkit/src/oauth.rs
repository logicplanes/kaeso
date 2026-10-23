use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::{Body, Bytes};
use axum::extract::{Form, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreRsaPrivateSigningKey,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserEmail, IssuerUrl, JsonWebKeyId, Nonce,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use rsa::pkcs1::EncodeRsaPrivateKey;
use sha2::{Digest, Sha256};

use crate::mock_provider::{Received, kpp_fixtures};

pub const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
pub const RESOURCE: &str = "https://api.openai.com/v1";
pub const FIRST_CLIENT: &str = "dynamic_agent_client";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    WithoutPlan,
    WrongNonce,
    WrongAudience,
    WrongIssuer,
    NoEmail,
    SignedOut,
    Down,
    RevokeDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reply {
    #[default]
    Text,
    UsageLimit,
    UsageLimitInStream,
    ModelNotFound,
    Trickle,
}

struct Grant {
    nonce: String,
    challenge: String,
    redirect_uri: String,
    client_id: String,
    subject: String,
    email: String,
}

struct Session {
    client_id: String,
    access: String,
    refresh: String,
}

pub(crate) struct Hold {
    reached: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

impl Hold {
    pub(crate) async fn wait(self) {
        let _ = self.reached.send(());
        let _ = self.release.await;
    }
}

pub(crate) fn hold() -> (Hold, Held) {
    let (reached_tx, reached) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    (
        Hold {
            reached: reached_tx,
            release: release_rx,
        },
        Held { reached, release },
    )
}

pub struct Held {
    reached: tokio::sync::oneshot::Receiver<()>,
    release: tokio::sync::oneshot::Sender<()>,
}

impl Held {
    pub async fn reached(&mut self) {
        let _ = (&mut self.reached).await;
    }

    pub fn release(self) {
        let _ = self.release.send(());
    }
}

struct Inner {
    issuer: String,
    key: CoreRsaPrivateSigningKey,
    clients: Mutex<HashMap<String, String>>,
    grants: Mutex<HashMap<String, Grant>>,
    sessions: Mutex<Vec<Session>>,
    issued: Mutex<Vec<String>>,
    authorizations: Mutex<Vec<HashMap<String, String>>>,
    refreshes: AtomicUsize,
    revoked: Mutex<Vec<(String, String)>>,
    received: Mutex<Vec<Received>>,
    fault: Mutex<Fault>,
    reply: Mutex<Reply>,
    refresh_wait: Mutex<i64>,
    hold: Mutex<Option<Hold>>,
}

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone)]
pub struct MockOAuth {
    inner: Arc<Inner>,
}

impl MockOAuth {
    pub async fn start() -> MockOAuth {
        MockOAuth::start_on("127.0.0.1:0").await
    }

    pub async fn start_on(addr: &str) -> MockOAuth {
        let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let mut rng = rsa::rand_core::OsRng;
        let private = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("RSA key");
        let pem = private
            .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .expect("PEM");
        let key = CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("mock".into())))
            .expect("signing key");
        let inner = Arc::new(Inner {
            issuer: format!("http://{addr}"),
            key,
            clients: Mutex::default(),
            grants: Mutex::default(),
            sessions: Mutex::default(),
            issued: Mutex::default(),
            authorizations: Mutex::default(),
            refreshes: AtomicUsize::new(0),
            revoked: Mutex::default(),
            received: Mutex::default(),
            fault: Mutex::default(),
            reply: Mutex::default(),
            refresh_wait: Mutex::new(0),
            hold: Mutex::default(),
        });
        let app = Router::new()
            .route("/.well-known/jwks.json", get(jwks))
            .route("/api/accounts/oauth/token", post(token))
            .route("/api/accounts/oauth/revoke", post(revoke))
            .route("/v1/responses", post(api))
            .route("/v1/models", get(api))
            .fallback(api)
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockOAuth { inner }
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

    pub fn refresh_not_before(&self, seconds: i64) {
        *locked(&self.inner.refresh_wait) = seconds;
    }

    pub fn hold_next_refresh(&self) -> Held {
        let (hold, held) = hold();
        *locked(&self.inner.hold) = Some(hold);
        held
    }

    pub fn refreshes(&self) -> usize {
        self.inner.refreshes.load(Ordering::SeqCst)
    }

    pub fn revoked(&self) -> Vec<(String, String)> {
        locked(&self.inner.revoked).clone()
    }

    pub fn issued(&self) -> Vec<String> {
        locked(&self.inner.issued).clone()
    }

    pub fn received(&self) -> Vec<Received> {
        locked(&self.inner.received).clone()
    }

    pub fn authorizations(&self) -> Vec<HashMap<String, String>> {
        locked(&self.inner.authorizations).clone()
    }

    pub fn end_sessions(&self) {
        locked(&self.inner.sessions).clear();
    }

    pub fn approve(&self, authorize_url: &str, subject: &str, email: &str) -> String {
        let (q, redirect) = self.authorized(authorize_url);
        let requested = &q["client_id"];
        let client_id = if requested == FIRST_CLIENT {
            let mut clients = locked(&self.inner.clients);
            let issued = clients.len();
            clients
                .entry(subject.to_owned())
                .or_insert_with(|| format!("oaiapp_mock{issued}"))
                .clone()
        } else {
            requested.clone()
        };
        let code = format!("code-{}", uuid::Uuid::now_v7().simple());
        locked(&self.inner.grants).insert(
            code.clone(),
            Grant {
                nonce: q["nonce"].clone(),
                challenge: q["code_challenge"].clone(),
                redirect_uri: q["redirect_uri"].clone(),
                client_id: client_id.clone(),
                subject: subject.to_owned(),
                email: email.to_owned(),
            },
        );
        let mut to = redirect;
        {
            let mut pairs = to.query_pairs_mut();
            pairs
                .append_pair("code", &code)
                .append_pair("scope", &q["scope"])
                .append_pair("state", &q["state"]);
            if requested == FIRST_CLIENT {
                pairs.append_pair("client_id", &client_id);
            }
        }
        to.to_string()
    }

    pub fn deny(&self, authorize_url: &str) -> String {
        let (q, mut redirect) = self.authorized(authorize_url);
        redirect
            .query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &q["state"]);
        redirect.to_string()
    }

    fn authorized(&self, authorize_url: &str) -> (HashMap<String, String>, url::Url) {
        let url = url::Url::parse(authorize_url).expect("authorize URL");
        assert_eq!(
            url.as_str()
                .split('?')
                .next()
                .expect("authorize URL")
                .trim_end_matches('/'),
            format!("{}/api/accounts/authorize", self.inner.issuer)
        );
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["resource"], RESOURCE);
        assert!(q["scope"].split(' ').any(|s| s == PLAN_SCOPE));
        assert!(q["ext_agent_host_id"].starts_with("urn:uuid:"));
        let redirect = url::Url::parse(&q["redirect_uri"]).expect("redirect_uri");
        assert_eq!(redirect.host_str(), Some("127.0.0.1"));
        assert_eq!(redirect.path(), "/auth/callback");
        locked(&self.inner.authorizations).push(q.clone());
        (q, redirect)
    }
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

fn jwt(
    inner: &Inner,
    audience: &str,
    subject: &str,
    email: Option<&str>,
    nonce: Option<&str>,
) -> String {
    let fault = *locked(&inner.fault);
    let issued = now();
    let issuer = if fault == Fault::WrongIssuer {
        "http://evil.example".to_owned()
    } else {
        inner.issuer.clone()
    };
    let mut standard = StandardClaims::new(SubjectIdentifier::new(subject.to_owned()));
    if let Some(email) = email {
        standard = standard.set_email(Some(EndUserEmail::new(email.to_owned())));
    }
    let mut claims = CoreIdTokenClaims::new(
        IssuerUrl::new(issuer).expect("issuer"),
        vec![Audience::new(audience.to_owned())],
        issued + chrono::Duration::hours(1),
        issued,
        standard,
        EmptyAdditionalClaims {},
    );
    let unique = uuid::Uuid::now_v7().simple().to_string();
    claims = claims.set_nonce(Some(Nonce::new(nonce.unwrap_or(&unique).to_owned())));
    let token = CoreIdToken::new(
        claims,
        &inner.key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .expect("signed JWT")
    .to_string();
    locked(&inner.issued).push(token.clone());
    token
}

fn refresh_token(inner: &Inner) -> String {
    let token = format!("rt_{}", uuid::Uuid::now_v7().simple());
    locked(&inner.issued).push(token.clone());
    token
}

fn oauth_error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "error": code }))).into_response()
}

fn granted(
    inner: &Inner,
    access: String,
    refresh: String,
    id_token: Option<String>,
    scope: &str,
) -> Response {
    let wait = *locked(&inner.refresh_wait);
    let mut body = serde_json::json!({
        "access_token": access,
        "refresh_token": refresh,
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": scope,
        "earliest_refresh_at": now().timestamp() + wait,
    });
    if let Some(id_token) = id_token {
        body["id_token"] = serde_json::Value::String(id_token);
    }
    (StatusCode::OK, Json(body)).into_response()
}

async fn jwks(State(inner): State<Arc<Inner>>) -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        inner.key.as_verification_key(),
    ]))
}

async fn token(
    State(inner): State<Arc<Inner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    if field("resource") != RESOURCE {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_target");
    }
    match field("grant_type").as_str() {
        "authorization_code" => code_grant(&inner, &form),
        "refresh_token" => {
            let hold = locked(&inner.hold).take();
            if let Some(hold) = hold {
                hold.wait().await;
            }
            inner.refreshes.fetch_add(1, Ordering::SeqCst);
            refresh_grant(&inner, &field("client_id"), &field("refresh_token"))
        }
        _ => oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    }
}

fn code_grant(inner: &Inner, form: &HashMap<String, String>) -> Response {
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    let fault = *locked(&inner.fault);
    if fault == Fault::Down {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    let Some(grant) = locked(&inner.grants).remove(&field("code")) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let verifier = field("code_verifier");
    if field("client_id") != grant.client_id
        || field("redirect_uri") != grant.redirect_uri
        || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let audience = if fault == Fault::WrongAudience {
        "someone-else".to_owned()
    } else {
        grant.client_id.clone()
    };
    let nonce = if fault == Fault::WrongNonce {
        "not-the-nonce".to_owned()
    } else {
        grant.nonce.clone()
    };
    let email = (fault != Fault::NoEmail).then_some(grant.email.as_str());
    let id_token = jwt(inner, &audience, &grant.subject, email, Some(&nonce));
    let access = jwt(inner, RESOURCE, &grant.subject, None, None);
    let refresh = refresh_token(inner);
    locked(&inner.sessions).push(Session {
        client_id: grant.client_id.clone(),
        access: access.clone(),
        refresh: refresh.clone(),
    });
    let scope = if fault == Fault::WithoutPlan {
        "openid profile email offline_access"
    } else {
        "chatgpt.tokens.use.direct email offline_access openid profile resource.invoke"
    };
    granted(inner, access, refresh, Some(id_token), scope)
}

fn refresh_grant(inner: &Inner, client_id: &str, refresh: &str) -> Response {
    match *locked(&inner.fault) {
        Fault::SignedOut => return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant"),
        Fault::Down => {
            return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
        }
        _ => {}
    }
    let mut sessions = locked(&inner.sessions);
    let Some(session) = sessions
        .iter_mut()
        .find(|s| s.refresh == refresh && s.client_id == client_id)
    else {
        return oauth_error(StatusCode::BAD_REQUEST, "refresh_token_reused");
    };
    let subject = "refreshed";
    session.access = jwt(inner, RESOURCE, subject, None, None);
    session.refresh = refresh_token(inner);
    let (access, refresh) = (session.access.clone(), session.refresh.clone());
    drop(sessions);
    granted(
        inner,
        access,
        refresh,
        None,
        "chatgpt.tokens.use.direct email offline_access openid profile resource.invoke",
    )
}

async fn revoke(
    State(inner): State<Arc<Inner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let field = |k: &str| form.get(k).cloned().unwrap_or_default();
    if matches!(*locked(&inner.fault), Fault::Down | Fault::RevokeDown) {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    }
    if field("token_type_hint") != "refresh_token" {
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_token_type");
    }
    let token = field("token");
    locked(&inner.revoked).push((token.clone(), field("client_id")));
    locked(&inner.sessions).retain(|s| s.refresh != token);
    StatusCode::OK.into_response()
}

const MODELS: &str = r#"{"models":[{"slug":"gpt-5","display_name":"GPT-5","visibility":"list"},{"slug":"gpt-5-mini","display_name":"GPT-5 mini","visibility":"hide"}]}"#;

const MODEL_NOT_FOUND: &str = r#"{"error":{"message":"The requested model does not exist.","type":"invalid_request_error","param":"model","code":"model_not_found"}}"#;

const USAGE_LIMIT: &str = r#"{"error":{"message":"You have reached the usage limit of your ChatGPT plan.","type":"usage_limit_reached","param":null,"code":"subscription_sharing_usage_limit_exceeded"}}"#;

const FAILED_STREAM: &str = concat!(
    "event: response.created\n",
    r#"data: {"type":"response.created","sequence_number":0,"response":{"id":"resp_mock","object":"response","created_at":1791720589,"status":"in_progress","error":null,"model":"gpt-6.1-sol","output":[],"usage":null}}"#,
    "\n\nevent: response.failed\n",
    r#"data: {"type":"response.failed","sequence_number":1,"response":{"id":"resp_mock","object":"response","created_at":1791720589,"status":"failed","error":{"code":"subscription_sharing_usage_limit_exceeded","message":"You have reached the usage limit of your ChatGPT plan."},"model":"gpt-6.1-sol","output":[],"usage":null}}"#,
    "\n\n"
);

fn trickle() -> Response {
    let created = FAILED_STREAM
        .split_inclusive("\n\n")
        .next()
        .unwrap_or_default();
    let chunks = futures::stream::unfold(Some(created), |first| async move {
        let chunk = match first {
            Some(created) => created,
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                ": still working\n\n"
            }
        };
        Some((
            Ok::<_, std::io::Error>(Bytes::from_static(chunk.as_bytes())),
            None,
        ))
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(chunks))
        .unwrap_or_default()
}

async fn api(
    State(inner): State<Arc<Inner>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !locked(&inner.sessions).iter().any(|s| s.access == bearer) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": { "message": "The access token is not valid.", "type": "invalid_request_error", "param": null, "code": "invalid_api_key" } })),
        )
            .into_response();
    }
    match (method, uri.path()) {
        (Method::GET, "/v1/models") => {
            ([(header::CONTENT_TYPE, "application/json")], MODELS).into_response()
        }
        (Method::POST, "/v1/responses") => {
            let stream = |body: String| {
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from(body))
                    .unwrap_or_default()
            };
            match *locked(&inner.reply) {
                Reply::Text => {
                    let path = kpp_fixtures().join("openai_responses/streams/text-basic-stream.sse");
                    stream(std::fs::read_to_string(path).expect("the text stream fixture"))
                }
                Reply::UsageLimit => (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::CONTENT_TYPE, "application/json")],
                    USAGE_LIMIT,
                )
                    .into_response(),
                Reply::UsageLimitInStream => stream(FAILED_STREAM.to_owned()),
                Reply::Trickle => trickle(),
                Reply::ModelNotFound => (
                    StatusCode::NOT_FOUND,
                    [(header::CONTENT_TYPE, "application/json")],
                    MODEL_NOT_FOUND,
                )
                    .into_response(),
            }
        }
        _ => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": { "message": "This route is not supported.", "type": "invalid_request_error", "param": null, "code": "subscription_sharing_route_not_supported" } })),
        )
            .into_response(),
    }
}
