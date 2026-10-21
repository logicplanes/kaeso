use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use openidconnect::core::{
    CoreClaimName, CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey, CoreSubjectIdentifierType,
};
use openidconnect::{
    AccessToken, Audience, AuthUrl, EmptyAdditionalClaims, EmptyAdditionalProviderMetadata,
    EndUserEmail, EndUserName, IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl, LocalizedClaim, Nonce,
    PrivateSigningKey, ResponseTypes, StandardClaims, SubjectIdentifier, TokenUrl,
};
use rsa::pkcs1::EncodeRsaPrivateKey;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Fault {
    #[default]
    None,
    WrongAudience,
    WrongNonce,
    Expired,
    WrongIssuer,
    UnverifiedEmail,
    NoEmailVerified,
    StaleAuthTime,
}

struct Grant {
    nonce: String,
    challenge: String,
    subject: String,
    email: String,
    name: String,
}

struct Inner {
    issuer: String,
    client_id: String,
    client_secret: String,
    key: CoreRsaPrivateSigningKey,
    grants: Mutex<HashMap<String, Grant>>,
    fault: Mutex<Fault>,
}

#[derive(Clone)]
pub struct MockIdp {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone)]
pub struct Authorized {
    pub code: String,
    pub state: String,
}

impl MockIdp {
    pub async fn start(client_id: &str, client_secret: &str) -> MockIdp {
        Self::start_on("127.0.0.1:0", client_id, client_secret).await
    }
    pub async fn start_on(addr: &str, client_id: &str, client_secret: &str) -> MockIdp {
        let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let mut rng = rsa::rand_core::OsRng;
        let private = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("RSA key");
        let pem = private
            .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .expect("PEM");
        let key = CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("test".into())))
            .expect("signing key");
        let inner = Arc::new(Inner {
            issuer: format!("http://{addr}"),
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
            key,
            grants: Mutex::new(HashMap::new()),
            fault: Mutex::new(Fault::None),
        });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .route("/authorize", get(sign_in_form).post(sign_in))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        MockIdp { inner }
    }

    pub fn issuer(&self) -> &str {
        &self.inner.issuer
    }

    pub fn set_fault(&self, fault: Fault) {
        if let Ok(mut f) = self.inner.fault.lock() {
            *f = fault;
        }
    }
    pub fn authorize(&self, authorization_url: &str, email: &str, name: &str) -> Authorized {
        self.authorize_as(authorization_url, &format!("sub-{email}"), email, name)
    }

    pub fn authorize_as(
        &self,
        authorization_url: &str,
        subject: &str,
        email: &str,
        name: &str,
    ) -> Authorized {
        let url = url::Url::parse(authorization_url).expect("authorization URL");
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        grant(&self.inner, &q, subject, email, name).expect("a valid authorization request")
    }
}

fn grant(
    inner: &Inner,
    q: &HashMap<String, String>,
    subject: &str,
    email: &str,
    name: &str,
) -> Result<Authorized, String> {
    let param = |k: &str| q.get(k).cloned().ok_or_else(|| format!("missing {k}"));
    if param("client_id")? != inner.client_id {
        return Err("unknown client_id".into());
    }
    if param("response_type")? != "code" {
        return Err("response_type must be code".into());
    }
    if param("code_challenge_method")? != "S256" {
        return Err("PKCE with S256 is required".into());
    }
    let code = format!("code-{}", uuid::Uuid::now_v7().simple());
    let grant = Grant {
        nonce: param("nonce")?,
        challenge: param("code_challenge")?,
        subject: subject.to_owned(),
        email: email.to_owned(),
        name: name.to_owned(),
    };
    if let Ok(mut grants) = inner.grants.lock() {
        grants.insert(code.clone(), grant);
    }
    Ok(Authorized {
        code,
        state: param("state")?,
    })
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn sign_in_form(
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> axum::response::Html<String> {
    let hidden: String = q
        .iter()
        .map(|(k, v)| {
            format!(
                r#"<input type="hidden" name="{}" value="{}">"#,
                escape(k),
                escape(v)
            )
        })
        .collect();
    axum::response::Html(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Mock identity provider</title></head>
<body><main><h1>Mock identity provider</h1><form method="post" action="/authorize">{hidden}
<label>Email <input name="login_email" type="email" required></label>
<label>Name <input name="login_name" required></label>
<button type="submit">Sign in</button></form></main></body></html>"#
    ))
}

async fn sign_in(
    State(inner): State<Arc<Inner>>,
    Form(mut form): Form<HashMap<String, String>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let email = form.remove("login_email").unwrap_or_default();
    let name = form.remove("login_name").unwrap_or_default();
    let Some(redirect) = form
        .get("redirect_uri")
        .and_then(|r| url::Url::parse(r).ok())
    else {
        return (StatusCode::BAD_REQUEST, "missing redirect_uri").into_response();
    };
    match grant(&inner, &form, &format!("sub-{email}"), &email, &name) {
        Ok(Authorized { code, state }) => {
            let mut to = redirect;
            to.query_pairs_mut()
                .append_pair("code", &code)
                .append_pair("state", &state);
            (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string())]).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn discovery(State(inner): State<Arc<Inner>>) -> Json<CoreProviderMetadata> {
    let issuer = &inner.issuer;
    let metadata = CoreProviderMetadata::new(
        IssuerUrl::new(issuer.clone()).expect("issuer"),
        AuthUrl::new(format!("{issuer}/authorize")).expect("auth url"),
        JsonWebKeySetUrl::new(format!("{issuer}/jwks")).expect("jwks url"),
        vec![ResponseTypes::new(vec![CoreResponseType::Code])],
        vec![CoreSubjectIdentifierType::Public],
        vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
        EmptyAdditionalProviderMetadata {},
    )
    .set_token_endpoint(Some(
        TokenUrl::new(format!("{issuer}/token")).expect("token url"),
    ))
    .set_claims_supported(Some(vec![
        CoreClaimName::new("email".into()),
        CoreClaimName::new("name".into()),
    ]));
    Json(metadata)
}

async fn jwks(State(inner): State<Arc<Inner>>) -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        inner.key.as_verification_key(),
    ]))
}

#[derive(Deserialize)]
struct TokenForm {
    grant_type: String,
    code: String,
    code_verifier: Option<String>,
}

async fn token(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> (StatusCode, Json<serde_json::Value>) {
    let error = |e: &str| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
    };
    let expected = STANDARD.encode(format!(
        "{}:{}",
        urlencode(&inner.client_id),
        urlencode(&inner.client_secret)
    ));
    let auth_ok = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Basic {expected}"));
    if !auth_ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid_client" })),
        );
    }
    if form.grant_type != "authorization_code" {
        return error("unsupported_grant_type");
    }
    let Some(grant) = inner
        .grants
        .lock()
        .ok()
        .and_then(|mut g| g.remove(&form.code))
    else {
        return error("invalid_grant");
    };
    let verifier = form.code_verifier.unwrap_or_default();
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return error("invalid_grant");
    }
    let fault = inner.fault.lock().map(|f| f.clone()).unwrap_or_default();
    let now = chrono::Utc::now();
    let (issued, expires) = if fault == Fault::Expired {
        (
            now - chrono::Duration::hours(2),
            now - chrono::Duration::hours(1),
        )
    } else {
        (now, now + chrono::Duration::minutes(5))
    };
    let issuer = if fault == Fault::WrongIssuer {
        "http://evil.example".to_owned()
    } else {
        inner.issuer.clone()
    };
    let audience = if fault == Fault::WrongAudience {
        "someone-else".to_owned()
    } else {
        inner.client_id.clone()
    };
    let nonce = if fault == Fault::WrongNonce {
        "not-the-nonce".to_owned()
    } else {
        grant.nonce.clone()
    };
    let mut name = LocalizedClaim::new();
    name.insert(None, EndUserName::new(grant.name.clone()));
    let standard = StandardClaims::new(SubjectIdentifier::new(grant.subject.clone()))
        .set_email(Some(EndUserEmail::new(grant.email.clone())))
        .set_email_verified(match fault {
            Fault::UnverifiedEmail => Some(false),
            Fault::NoEmailVerified => None,
            _ => Some(true),
        })
        .set_name(Some(name));
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(issuer).expect("issuer"),
        vec![Audience::new(audience)],
        expires,
        issued,
        standard,
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(nonce)))
    .set_auth_time(Some(if fault == Fault::StaleAuthTime {
        now - chrono::Duration::hours(1)
    } else {
        now
    }));
    let access = AccessToken::new("mock-access-token".into());
    let Ok(id_token) = CoreIdToken::new(
        claims,
        &inner.key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&access),
        None,
    ) else {
        return error("server_error");
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": "mock-access-token",
            "token_type": "Bearer",
            "expires_in": 300,
            "id_token": id_token.to_string(),
        })),
    )
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
