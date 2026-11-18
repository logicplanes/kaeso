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
    CoreClaimName, CoreGenderClaim, CoreJsonWebKeySet, CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm, CoreProviderMetadata, CoreResponseType, CoreRsaPrivateSigningKey,
    CoreSubjectIdentifierType,
};
use openidconnect::{
    AccessToken, AdditionalClaims, Audience, AuthUrl, EmptyAdditionalProviderMetadata,
    EndUserEmail, EndUserName, IdToken, IdTokenClaims, IssuerUrl, JsonWebKeyId, JsonWebKeySetUrl,
    LocalizedClaim, Nonce, PrivateSigningKey, ResponseTypes, StandardClaims, SubjectIdentifier,
    TokenUrl,
};
use rsa::pkcs1::EncodeRsaPrivateKey;
use serde::{Deserialize, Serialize};

/// Claims an identity provider adds on request, such as `groups` or `roles`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Extra {
    #[serde(flatten)]
    claims: serde_json::Map<String, serde_json::Value>,
}

impl AdditionalClaims for Extra {}
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
    NoAuthTime,
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
    // The extra claims of each address's ID tokens; none unless set.
    claims: Mutex<HashMap<String, serde_json::Map<String, serde_json::Value>>>,
    // What Microsoft Graph's `/me/transitiveMemberOf` lists for each address.
    memberships: Mutex<HashMap<String, Vec<serde_json::Value>>>,
    // Each access token handed out, and whose it is.
    access: Mutex<HashMap<String, String>>,
    graph_refuses: Mutex<bool>,
    fault: Mutex<Fault>,
    hold: Mutex<Option<Hold>>,
}

struct Hold {
    reached: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

pub struct HeldToken {
    reached: tokio::sync::oneshot::Receiver<()>,
    release: tokio::sync::oneshot::Sender<()>,
}

impl HeldToken {
    pub async fn reached(&mut self) {
        (&mut self.reached)
            .await
            .expect("the token request arrives");
    }

    pub fn release(self) {
        let _ = self.release.send(());
    }
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
    /// The `groups` claim the ID tokens for `email` carry from now on; `None` leaves it out.
    pub fn set_groups(&self, email: &str, groups: Option<&[&str]>) {
        self.set_claim(email, "groups", groups);
    }

    /// A claim the ID tokens for `email` carry from now on, as a list; `None` leaves it out.
    pub fn set_claim(&self, email: &str, claim: &str, values: Option<&[&str]>) {
        if let Ok(mut all) = self.inner.claims.lock() {
            let mine = all.entry(email.to_owned()).or_default();
            match values {
                Some(v) => {
                    mine.insert(claim.to_owned(), serde_json::json!(v));
                }
                None => {
                    mine.remove(claim);
                }
            }
        }
    }

    /// Makes `email` a member of more groups than fit into a token, as Microsoft Entra ID does:
    /// the ID tokens name a source instead of the `groups` claim, and the mock Graph lists the
    /// groups, a page of two at a time, with a directory role among them.
    pub fn set_groups_overage(&self, email: &str, groups: &[&str]) {
        if let Ok(mut all) = self.inner.claims.lock() {
            let mine = all.entry(email.to_owned()).or_default();
            mine.remove("groups");
            mine.insert(
                "_claim_names".into(),
                serde_json::json!({ "groups": "src1" }),
            );
            mine.insert(
                "_claim_sources".into(),
                serde_json::json!({ "src1": { "endpoint": "https://graph.example/users/0/getMemberObjects" } }),
            );
        }
        let mut listed: Vec<serde_json::Value> = groups
            .iter()
            .map(|g| serde_json::json!({ "@odata.type": "#microsoft.graph.group", "id": g }))
            .collect();
        listed.insert(
            1,
            serde_json::json!({ "@odata.type": "#microsoft.graph.directoryRole", "id": "a-directory-role" }),
        );
        if let Ok(mut all) = self.inner.memberships.lock() {
            all.insert(email.to_owned(), listed);
        }
    }

    /// Whether the mock Graph refuses every request, as it does without the `User.Read` scope.
    pub fn set_graph_refuses(&self, refuses: bool) {
        if let Ok(mut r) = self.inner.graph_refuses.lock() {
            *r = refuses;
        }
    }

    /// The mock Microsoft Graph, for an Entra cloud whose sign-in host is the issuer's.
    pub fn graph(&self) -> String {
        format!("{}/graph", self.inner.issuer)
    }

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
            claims: Mutex::new(HashMap::new()),
            memberships: Mutex::new(HashMap::new()),
            access: Mutex::new(HashMap::new()),
            graph_refuses: Mutex::new(false),
            fault: Mutex::new(Fault::None),
            hold: Mutex::new(None),
        });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .route("/authorize", get(sign_in_form).post(sign_in))
            .route("/graph/v1.0/me/transitiveMemberOf", get(member_of))
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
    pub fn hold_next_token(&self) -> HeldToken {
        let (reached_tx, reached) = tokio::sync::oneshot::channel();
        let (release, release_rx) = tokio::sync::oneshot::channel();
        if let Ok(mut hold) = self.inner.hold.lock() {
            *hold = Some(Hold {
                reached: reached_tx,
                release: release_rx,
            });
        }
        HeldToken { reached, release }
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
    let hold = inner.hold.lock().ok().and_then(|mut h| h.take());
    if let Some(hold) = hold {
        let _ = hold.reached.send(());
        let _ = hold.release.await;
    }
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
    let extra = inner
        .claims
        .lock()
        .ok()
        .and_then(|c| c.get(&grant.email).cloned())
        .unwrap_or_default();
    let claims = IdTokenClaims::<Extra, CoreGenderClaim>::new(
        IssuerUrl::new(issuer).expect("issuer"),
        vec![Audience::new(audience)],
        expires,
        issued,
        standard,
        Extra { claims: extra },
    )
    .set_nonce(Some(Nonce::new(nonce)))
    .set_auth_time(match fault {
        Fault::StaleAuthTime => Some(now - chrono::Duration::hours(1)),
        Fault::NoAuthTime => None,
        _ => Some(now),
    });
    let access_token = format!("access-{}", uuid::Uuid::now_v7().simple());
    if let Ok(mut all) = inner.access.lock() {
        all.insert(access_token.clone(), grant.email.clone());
    }
    let access = AccessToken::new(access_token.clone());
    let Ok(id_token) = IdToken::<
        Extra,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >::new(
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
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": 300,
            "id_token": id_token.to_string(),
        })),
    )
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

const GRAPH_PAGE: usize = 2;

async fn member_of(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let refused = (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": { "code": "Authorization_RequestDenied" } })),
    );
    if inner.graph_refuses.lock().map(|r| *r).unwrap_or(true) {
        return refused;
    }
    let email = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|t| inner.access.lock().ok()?.get(t).cloned());
    let Some(email) = email else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": { "code": "InvalidAuthenticationToken" } })),
        );
    };
    let all = inner
        .memberships
        .lock()
        .ok()
        .and_then(|m| m.get(&email).cloned())
        .unwrap_or_default();
    let from: usize = q
        .get("$skiptoken")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    let page: Vec<_> = all.iter().skip(from).take(GRAPH_PAGE).cloned().collect();
    let mut body = serde_json::json!({ "value": page });
    if from + GRAPH_PAGE < all.len() {
        body["@odata.nextLink"] = serde_json::json!(format!(
            "{}/graph/v1.0/me/transitiveMemberOf?$skiptoken={}",
            inner.issuer,
            from + GRAPH_PAGE
        ));
    }
    (StatusCode::OK, Json(body))
}
