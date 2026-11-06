use std::collections::HashMap;
use std::net::IpAddr;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_core::id::{AgentClientId, ProjectId};
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_store::StoreError;
use kaeso_store::pg::agents::{self, Client, NewRequest, NewToken, Request, TokenKind};
use kaeso_store::pg::portal_grants;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::cimd;
use crate::auth::agents::{
    ACCESS_LIFETIME, ACCESS_PREFIX, CODE_LIFETIME, CODE_PREFIX, REFRESH_PREFIX, REQUEST_LIFETIME,
    unused_too_long,
};
use crate::auth::key_failures;
use crate::auth::throttle::{
    self, AGENT_REGISTRATIONS_PER_IP, AGENT_REVOCATIONS_PER_IP, AGENT_SIGN_INS_PER_IP,
    AGENT_TOKENS_PER_IP, Throttle,
};
use crate::http::client_ip::ClientIp;
use crate::platform::params::Path;
use crate::state::AppState;

pub const SCOPE: &str = "kaeso";
pub const PORTAL_SCOPE: &str = "tools";
pub const PORTAL_CODE_PREFIX: &str = "kso_pc_";
pub const PORTAL_ACCESS_PREFIX: &str = "kso_pa_";
pub const PORTAL_REFRESH_PREFIX: &str = "kso_pr_";
const MAX_REDIRECT_URIS: usize = 10;
const MAX_URI_LEN: usize = 2000;
const MAX_STATE_LEN: usize = 2000;
const MAX_NAME: usize = 100;
const MAX_UNUSED_CLIENTS: i64 = 1000;
const MAX_REQUESTS_PER_CLIENT: i64 = 20;

pub fn issuer(state: &AppState) -> String {
    state
        .config
        .public_url
        .as_str()
        .trim_end_matches('/')
        .to_owned()
}

pub fn endpoint(state: &AppState, path: &str) -> String {
    format!("{}{path}", state.config.public_url)
}

pub fn resource(state: &AppState) -> String {
    endpoint(state, "mcp")
}

pub fn portal_resource(state: &AppState, project: Uuid) -> String {
    endpoint(state, &format!("mcp/{}", ProjectId::from_uuid(project)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Management,
    Portal(Uuid),
}

// RFC 8707: a token is for one resource. Kaeso's management server and each project's portal are
// separate resources, so a token for one is never good at another.
pub fn target_of(state: &AppState, presented: &str) -> Option<Target> {
    let presented = presented.trim_end_matches('/');
    let management = resource(state);
    if presented == management {
        return Some(Target::Management);
    }
    presented
        .strip_prefix(&format!("{management}/"))
        .and_then(|p| p.parse::<ProjectId>().ok())
        .map(|p| Target::Portal(p.uuid()))
}

fn no_store(mut r: Response) -> Response {
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    r
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    no_store(
        (
            status,
            Json(json!({ "error": error, "error_description": description })),
        )
            .into_response(),
    )
}

fn store_failed(e: &StoreError) -> Response {
    tracing::error!(error = %e, "an agent sign-in step failed");
    oauth_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "server_error",
        "Kaeso couldn't finish this step; try again shortly.",
    )
}

async fn limit(state: &AppState, ip: Option<IpAddr>, t: Throttle) -> Result<(), Response> {
    throttle::hit(state, t, &key_failures::subject(ip))
        .await
        .map_err(|refused| match refused {
            throttle::Refusal::Limited { retry_after } => {
                let mut r = oauth_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "temporarily_unavailable",
                    &format!(
                        "Too many attempts from your address; try again in {}.",
                        throttle::wait(retry_after)
                    ),
                );
                r.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from(retry_after));
                r
            }
            throttle::Refusal::Unavailable => oauth_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "Kaeso couldn't finish this step; try again shortly.",
            ),
        })
}

pub async fn protected_resource(State(state): State<AppState>) -> Response {
    no_store(
        Json(json!({
            "resource": resource(&state),
            "authorization_servers": [issuer(&state)],
            "bearer_methods_supported": ["header"],
            "scopes_supported": [SCOPE],
            "resource_name": "Kaeso",
            "resource_documentation": "https://docs.kaeso.app/api/mcp",
        }))
        .into_response(),
    )
}

pub async fn portal_resource_metadata(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Response {
    let Ok(project) = project.parse::<ProjectId>() else {
        return oauth_error(
            StatusCode::NOT_FOUND,
            "invalid_target",
            "No such MCP portal; check the project id in its address.",
        );
    };
    no_store(
        Json(json!({
            "resource": portal_resource(&state, project.uuid()),
            "authorization_servers": [issuer(&state)],
            "bearer_methods_supported": ["header"],
            "scopes_supported": [PORTAL_SCOPE],
            "resource_name": "Kaeso MCP portal",
            "resource_documentation": "https://docs.kaeso.app/concepts/mcp-portal",
        }))
        .into_response(),
    )
}

pub async fn authorization_server(State(state): State<AppState>) -> Response {
    no_store(
        Json(json!({
            "issuer": issuer(&state),
            "authorization_endpoint": endpoint(&state, "oauth/authorize"),
            "token_endpoint": endpoint(&state, "oauth/token"),
            "registration_endpoint": endpoint(&state, "oauth/register"),
            "revocation_endpoint": endpoint(&state, "oauth/revoke"),
            "response_types_supported": ["code"],
            "response_modes_supported": ["query"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "revocation_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": [SCOPE, PORTAL_SCOPE],
            "authorization_response_iss_parameter_supported": true,
            "client_id_metadata_document_supported": true,
            "service_documentation": "https://docs.kaeso.app/api/mcp",
        }))
        .into_response(),
    )
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

// RFC 8252: native apps receive the code on a loopback address or a private-use scheme; every
// other address must be HTTPS, and schemes that run code or read files never qualify.
pub fn redirect_uri_allowed(uri: &str) -> bool {
    if uri.len() > MAX_URI_LEN {
        return false;
    }
    let Ok(url) = Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match url.scheme() {
        "https" => url.host().is_some(),
        "http" => is_loopback(&url),
        "javascript" | "data" | "file" | "vbscript" | "about" | "blob" | "ftp" | "ws" | "wss"
        | "filesystem" | "chrome" | "view-source" => false,
        _ => true,
    }
}

fn receives_errors(uri: &str) -> bool {
    Url::parse(uri).is_ok_and(|u| match u.scheme() {
        "https" => false,
        "http" => is_loopback(&u),
        _ => true,
    })
}

fn redirect_matches(registered: &str, presented: &str) -> bool {
    if registered == presented {
        return true;
    }
    let (Ok(r), Ok(p)) = (Url::parse(registered), Url::parse(presented)) else {
        return false;
    };
    // RFC 8252 section 7.3: a loopback redirect may use any port, chosen when the client starts.
    r.scheme() == "http"
        && p.scheme() == "http"
        && is_loopback(&r)
        && r.host() == p.host()
        && r.path() == p.path()
        && r.query() == p.query()
}

pub fn redirects_locally(uri: &str) -> bool {
    Url::parse(uri).is_ok_and(|u| match u.scheme() {
        "http" | "https" => is_loopback(&u),
        _ => true,
    })
}

pub fn redirect_host(uri: &str) -> String {
    Url::parse(uri).map_or_else(
        |_| uri.chars().take(100).collect(),
        |u| match u.host_str() {
            Some(h) => h.to_owned(),
            None => format!("{}:", u.scheme()),
        },
    )
}

#[derive(Debug, Deserialize)]
pub struct Registration {
    #[serde(default)]
    redirect_uris: Vec<String>,
    client_name: Option<String>,
    token_endpoint_auth_method: Option<String>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
    software_id: Option<String>,
    software_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct Registered {
    client_id: String,
    client_id_issued_at: i64,
    client_name: String,
    redirect_uris: Vec<String>,
    grant_types: [&'static str; 2],
    response_types: [&'static str; 1],
    token_endpoint_auth_method: &'static str,
}

fn clean(s: Option<&str>, max: usize) -> Option<String> {
    let s = s?.trim();
    (!s.is_empty()).then(|| s.chars().filter(|c| !c.is_control()).take(max).collect())
}

fn metadata_error(description: &str) -> Response {
    oauth_error(
        StatusCode::BAD_REQUEST,
        "invalid_client_metadata",
        description,
    )
}

pub async fn register(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: Bytes,
) -> Response {
    if let Err(refused) = limit(&state, ip, AGENT_REGISTRATIONS_PER_IP).await {
        return refused;
    }
    let Ok(reg) = serde_json::from_slice::<Registration>(&body) else {
        return metadata_error("Send the client metadata as a JSON object.");
    };
    if reg.redirect_uris.is_empty() || reg.redirect_uris.len() > MAX_REDIRECT_URIS {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "Register 1 to 10 redirect URIs.",
        );
    }
    if !reg.redirect_uris.iter().all(|u| redirect_uri_allowed(u)) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "Redirect URIs must use HTTPS, a loopback address or a private-use scheme.",
        );
    }
    if reg
        .token_endpoint_auth_method
        .as_deref()
        .is_some_and(|m| m != "none")
    {
        return metadata_error("Kaeso registers public clients only; use `none`.");
    }
    if reg.grant_types.as_ref().is_some_and(|g| {
        g.iter()
            .any(|g| g != "authorization_code" && g != "refresh_token")
    }) {
        return metadata_error(
            "Only `authorization_code` and `refresh_token` grants are supported.",
        );
    }
    if reg
        .response_types
        .as_ref()
        .is_some_and(|r| r.iter().any(|r| r != "code"))
    {
        return metadata_error("Only the `code` response type is supported.");
    }
    let now = state.clock.now();
    let client = Client {
        id: AgentClientId::new().uuid(),
        name: clean(reg.client_name.as_deref(), MAX_NAME).unwrap_or_else(|| "MCP client".into()),
        redirect_uris: reg.redirect_uris,
        software_id: clean(reg.software_id.as_deref(), MAX_NAME),
        software_version: clean(reg.software_version.as_deref(), MAX_NAME),
        created_at: now,
    };
    if let Err(e) = insert_client(&state, &client).await {
        return store_failed(&e);
    }
    no_store(
        (
            StatusCode::CREATED,
            Json(Registered {
                client_id: AgentClientId::from_uuid(client.id).to_string(),
                client_id_issued_at: now.unix_timestamp(),
                client_name: client.name,
                redirect_uris: client.redirect_uris,
                grant_types: ["authorization_code", "refresh_token"],
                response_types: ["code"],
                token_endpoint_auth_method: "none",
            }),
        )
            .into_response(),
    )
}

// Registration is open, so registrations no agent signed in with are capped; the oldest go first.
async fn insert_client(state: &AppState, client: &Client) -> Result<(), StoreError> {
    agents::insert_client(&state.pg, client).await?;
    trim_clients(state, client.id).await
}

async fn trim_clients(state: &AppState, newest: Uuid) -> Result<(), StoreError> {
    let dropped = agents::keep_unused_clients(&state.pg, MAX_UNUSED_CLIENTS, newest).await?;
    if dropped > 0 {
        tracing::warn!(
            dropped,
            kept = MAX_UNUSED_CLIENTS,
            "too many agent registrations without a sign-in; the oldest were dropped"
        );
    }
    Ok(())
}

fn params(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
        out.entry(k.into_owned()).or_insert_with(|| v.into_owned());
    }
    out
}

fn console_error(state: &AppState, error: &str) -> Response {
    Redirect::to(&endpoint(state, &format!("authorize#error={error}"))).into_response()
}

pub fn redirect_with_error(
    state: &AppState,
    uri: &str,
    client_state: Option<&str>,
    error: &str,
) -> String {
    with_params(
        uri,
        &[
            ("error", Some(error)),
            ("state", client_state),
            ("iss", Some(&issuer(state))),
        ],
    )
}

fn with_params(uri: &str, add: &[(&str, Option<&str>)]) -> String {
    let Ok(mut url) = Url::parse(uri) else {
        return uri.to_owned();
    };
    {
        let mut q = url.query_pairs_mut();
        for (k, v) in add {
            if let Some(v) = v {
                q.append_pair(k, v);
            }
        }
    }
    url.to_string()
}

// Agents keep their registration across restarts, while Kaeso drops registrations that never
// signed in and loses them with a reset database. Registration is open and a loopback redirect
// only reaches the user's own machine (RFC 8252), so taking the agent's id back is as safe as
// registering it anew and saves it from a dead end it can't recover from by itself.
async fn reregister(
    state: &AppState,
    id: AgentClientId,
    redirect_uri: Option<&str>,
) -> Result<Option<Client>, StoreError> {
    let Some(redirect_uri) = redirect_uri else {
        return Ok(None);
    };
    let loopback = Url::parse(redirect_uri).is_ok_and(|u| u.scheme() == "http" && is_loopback(&u))
        && redirect_uri_allowed(redirect_uri);
    if !loopback {
        return Ok(None);
    }
    agents::insert_client_if_missing(
        &state.pg,
        &Client {
            id: id.uuid(),
            name: "MCP client".into(),
            redirect_uris: vec![redirect_uri.to_owned()],
            software_id: None,
            software_version: None,
            created_at: state.clock.now(),
        },
    )
    .await?;
    trim_clients(state, id.uuid()).await?;
    agents::get_client(&state.pg, id.uuid()).await
}

// RFC 9700 section 4.11.2: registration is open, so a redirect URI is trusted with an error only
// once someone approved an agent of its client; before that the console shows the error, and
// the user's answer on the consent page goes back to the client.
async fn refuse(
    state: &AppState,
    client: &Client,
    redirect_uri: &str,
    client_state: Option<&str>,
    error: &str,
) -> Response {
    let trusted = receives_errors(redirect_uri)
        && agents::approved(&state.pg, client.id)
            .await
            .inspect_err(|e| tracing::error!(error = %e, "an agent sign-in step failed"))
            .unwrap_or(false);
    if !trusted {
        return console_error(state, error);
    }
    Redirect::to(&redirect_with_error(
        state,
        redirect_uri,
        client_state,
        error,
    ))
    .into_response()
}

fn request_error(state: &AppState, p: &HashMap<String, String>) -> Option<&'static str> {
    let get = |k: &str| p.get(k).map(String::as_str).filter(|v| !v.is_empty());
    if get("response_type") != Some("code") {
        return Some("unsupported_response_type");
    }
    let challenge = get("code_challenge");
    let s256 = get("code_challenge_method") == Some("S256");
    if !s256 || !challenge.is_some_and(|c| c.len() == 43 && URL_SAFE_NO_PAD.decode(c).is_ok()) {
        return Some("invalid_request");
    }
    if get("state").is_some_and(|s| s.len() > MAX_STATE_LEN) {
        return Some("invalid_request");
    }
    let target = match get("resource") {
        None => Target::Management,
        Some(r) => match target_of(state, r) {
            Some(t) => t,
            None => return Some("invalid_target"),
        },
    };
    let scope = match target {
        Target::Management => SCOPE,
        Target::Portal(_) => PORTAL_SCOPE,
    };
    if get("scope").is_some_and(|s| s.split(' ').any(|s| !s.is_empty() && s != scope)) {
        return Some("invalid_scope");
    }
    None
}

async fn registered_client(
    state: &AppState,
    id: AgentClientId,
    redirect_uri: Option<&str>,
) -> Result<Option<Client>, StoreError> {
    match agents::get_client(&state.pg, id.uuid()).await? {
        Some(c) => Ok(Some(c)),
        None => reregister(state, id, redirect_uri).await,
    }
}

pub async fn authorize(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    RawQuery(query): RawQuery,
) -> Response {
    if limit(&state, ip, AGENT_SIGN_INS_PER_IP).await.is_err() {
        return console_error(&state, "temporarily_unavailable");
    }
    let p = params(query.as_deref().unwrap_or_default());
    let get = |k: &str| p.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let presented_client = get("client_id").unwrap_or_default();
    let client = if cimd::is_document_url(presented_client) {
        match cimd::client(&state, presented_client).await {
            Ok(Ok(c)) => c,
            Ok(Err(refused)) => {
                tracing::info!(?refused, "a client metadata document was refused");
                return console_error(&state, "invalid_client");
            }
            Err(e) => {
                tracing::error!(error = %e, "an agent sign-in step failed");
                return console_error(&state, "server_error");
            }
        }
    } else {
        let Ok(client_id) = presented_client.parse::<AgentClientId>() else {
            return console_error(&state, "invalid_client");
        };
        match registered_client(&state, client_id, get("redirect_uri")).await {
            Ok(Some(c)) => c,
            Ok(None) => return console_error(&state, "invalid_client"),
            Err(e) => {
                tracing::error!(error = %e, "an agent sign-in step failed");
                return console_error(&state, "server_error");
            }
        }
    };
    let redirect_uri = match get("redirect_uri") {
        Some(r) if client.redirect_uris.iter().any(|u| redirect_matches(u, r)) => r.to_owned(),
        None if client.redirect_uris.len() == 1 => client.redirect_uris.concat(),
        _ => return console_error(&state, "invalid_redirect_uri"),
    };
    let client_state = get("state").filter(|s| s.len() <= MAX_STATE_LEN);
    if let Some(error) = request_error(&state, &p) {
        return refuse(&state, &client, &redirect_uri, client_state, error).await;
    }
    let token = random_token();
    let stored = store_request(
        &state,
        &client,
        &redirect_uri,
        client_state,
        get("code_challenge").unwrap_or_default(),
        get("resource").map(|r| r.trim_end_matches('/')),
        &token,
    )
    .await;
    match stored {
        Ok(()) => no_store(
            Redirect::to(&endpoint(&state, &format!("authorize#request={token}"))).into_response(),
        ),
        Err(e) => {
            tracing::error!(error = %e, "an agent sign-in step failed");
            refuse(&state, &client, &redirect_uri, client_state, "server_error").await
        }
    }
}

// Any page can send a browser here, so each client keeps only its newest requests.
async fn store_request(
    state: &AppState,
    client: &Client,
    redirect_uri: &str,
    client_state: Option<&str>,
    challenge: &str,
    target: Option<&str>,
    token: &str,
) -> Result<(), StoreError> {
    let now = state.clock.now();
    let id = Uuid::now_v7();
    agents::insert_request(
        &state.pg,
        &NewRequest {
            id,
            token_hash: &secret::hash(token),
            client_id: client.id,
            redirect_uri,
            state: client_state,
            code_challenge: challenge,
            resource: target,
            now,
            expires_at: now + REQUEST_LIFETIME,
        },
    )
    .await?;
    agents::keep_requests(&state.pg, client.id, MAX_REQUESTS_PER_CLIENT, id).await?;
    Ok(())
}

pub async fn issue_code(
    db: &mut sqlx::PgConnection,
    agent: Uuid,
    request: &Request,
    now: OffsetDateTime,
) -> Result<String, StoreError> {
    let code = format!("{CODE_PREFIX}{}", random_token());
    agents::insert_token(
        db,
        &NewToken {
            id: Uuid::now_v7(),
            agent_id: agent,
            kind: TokenKind::Code,
            token_hash: &secret::hash(&code),
            family: Uuid::now_v7(),
            redirect_uri: Some(&request.redirect_uri),
            code_challenge: Some(&request.code_challenge),
            now,
            expires_at: Some(now + CODE_LIFETIME),
        },
    )
    .await?;
    Ok(code)
}

pub fn redirect_with_code(state: &AppState, request: &Request, code: &str) -> String {
    with_params(
        &request.redirect_uri,
        &[
            ("code", Some(code)),
            ("state", request.state.as_deref()),
            ("iss", Some(&issuer(state))),
        ],
    )
}

pub fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    // RFC 7636 section 4.1: 43 to 128 characters from the unreserved set.
    let valid = (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
    valid
        && secret::hashes_equal(
            URL_SAFE_NO_PAD
                .encode(Sha256::digest(verifier.as_bytes()))
                .as_bytes(),
            challenge.as_bytes(),
        )
}

#[derive(Debug, Serialize)]
struct Tokens {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
    scope: &'static str,
}

async fn issue_tokens(
    db: &mut sqlx::PgConnection,
    agent: Uuid,
    family: Uuid,
    now: OffsetDateTime,
) -> Result<Tokens, StoreError> {
    let access = format!("{ACCESS_PREFIX}{}", random_token());
    let refresh = format!("{REFRESH_PREFIX}{}", random_token());
    for (kind, token, expires_at) in [
        (TokenKind::Access, &access, Some(now + ACCESS_LIFETIME)),
        (TokenKind::Refresh, &refresh, None),
    ] {
        agents::insert_token(
            &mut *db,
            &NewToken {
                id: Uuid::now_v7(),
                agent_id: agent,
                kind,
                token_hash: &secret::hash(token),
                family,
                redirect_uri: None,
                code_challenge: None,
                now,
                expires_at,
            },
        )
        .await?;
    }
    Ok(Tokens {
        access_token: access,
        token_type: "Bearer",
        expires_in: ACCESS_LIFETIME.whole_seconds(),
        refresh_token: refresh,
        scope: SCOPE,
    })
}

fn invalid_grant(description: &str) -> Response {
    oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", description)
}

pub async fn token(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(refused) = limit(&state, ip, AGENT_TOKENS_PER_IP).await {
        return refused;
    }
    let form = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
    if !form {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send the request as `application/x-www-form-urlencoded`.",
        );
    }
    let p = params(&String::from_utf8_lossy(&body));
    let get = |k: &str| p.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let (kind, presented) = match get("grant_type") {
        Some("authorization_code") => (TokenKind::Code, get("code")),
        Some("refresh_token") => (TokenKind::Refresh, get("refresh_token")),
        _ => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "Use `authorization_code` or `refresh_token`.",
            );
        }
    };
    let Some(presented) = presented else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The grant is missing.",
        );
    };
    let now = state.clock.now();
    let presented_to = Presented {
        client_id: get("client_id"),
        redirect_uri: get("redirect_uri"),
        verifier: get("code_verifier"),
        resource: get("resource"),
    };
    if presented.starts_with(PORTAL_CODE_PREFIX) || presented.starts_with(PORTAL_REFRESH_PREFIX) {
        return match portal_exchange(&state, kind, presented, &presented_to, now).await {
            Ok(Ok(tokens)) => no_store(Json(tokens).into_response()),
            Ok(Err(refused)) => refused,
            Err(e) => store_failed(&e),
        };
    }
    match exchange(
        &state,
        kind,
        presented,
        presented_to.client_id,
        presented_to.redirect_uri,
        presented_to.verifier,
        now,
    )
    .await
    {
        Ok(Ok(tokens)) => no_store(Json(tokens).into_response()),
        Ok(Err(refused)) => refused,
        Err(e) => store_failed(&e),
    }
}

struct Presented<'a> {
    client_id: Option<&'a str>,
    redirect_uri: Option<&'a str>,
    verifier: Option<&'a str>,
    resource: Option<&'a str>,
}

async fn client_uuid(
    db: &mut sqlx::PgConnection,
    presented: Option<&str>,
) -> Result<Option<Uuid>, StoreError> {
    let Some(p) = presented else {
        return Ok(None);
    };
    if let Ok(id) = p.parse::<AgentClientId>() {
        return Ok(Some(id.uuid()));
    }
    if cimd::is_document_url(p) {
        return agents::client_by_metadata_url(db, p).await;
    }
    Ok(None)
}

pub async fn issue_portal_code(
    db: &mut sqlx::PgConnection,
    grant: Uuid,
    request: &Request,
    target: &str,
    now: OffsetDateTime,
) -> Result<String, StoreError> {
    let code = format!("{PORTAL_CODE_PREFIX}{}", random_token());
    portal_grants::insert_token(
        db,
        &portal_grants::NewToken {
            id: Uuid::now_v7(),
            grant_id: grant,
            kind: TokenKind::Code,
            token_hash: &secret::hash(&code),
            family: Uuid::now_v7(),
            resource: target,
            redirect_uri: Some(&request.redirect_uri),
            code_challenge: Some(&request.code_challenge),
            now,
            expires_at: Some(now + CODE_LIFETIME),
        },
    )
    .await?;
    Ok(code)
}

#[derive(Debug, Serialize)]
struct PortalTokens {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
    scope: &'static str,
}

// The same rules as an agent's sign-in, for a person's grant to one portal: codes and refresh
// tokens are single-use, PKCE and the redirect are checked, and every token keeps its resource.
async fn portal_exchange(
    state: &AppState,
    kind: TokenKind,
    presented: &str,
    p: &Presented<'_>,
    now: OffsetDateTime,
) -> Result<Result<PortalTokens, Response>, StoreError> {
    let mut tx = state.pg.begin().await?;
    let Some(spent) = portal_grants::spend(&mut tx, kind, &secret::hash(presented), now).await?
    else {
        return Ok(Err(invalid_grant(
            "The grant is invalid, expired or revoked.",
        )));
    };
    if !spent.fresh {
        portal_grants::revoke_family(&mut *tx, spent.family, now).await?;
        tx.commit().await?;
        return Ok(Err(invalid_grant(
            "The grant was already used; sign in to the portal again.",
        )));
    }
    let refused = if client_uuid(&mut tx, p.client_id).await? != Some(spent.client_id) {
        Some(oauth_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "The grant belongs to another client.",
        ))
    } else if kind == TokenKind::Code && p.redirect_uri != spent.redirect_uri.as_deref() {
        Some(invalid_grant(
            "`redirect_uri` differs from the one authorized.",
        ))
    } else if kind == TokenKind::Code
        && !p
            .verifier
            .is_some_and(|v| pkce_matches(v, spent.code_challenge.as_deref().unwrap_or_default()))
    {
        Some(invalid_grant("The PKCE code verifier doesn't match."))
    } else if p
        .resource
        .is_some_and(|r| r.trim_end_matches('/') != spent.resource)
    {
        Some(oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_target",
            "The grant is for another resource.",
        ))
    } else {
        None
    };
    if let Some(r) = refused {
        tx.commit().await?;
        return Ok(Err(r));
    }
    let access = format!("{PORTAL_ACCESS_PREFIX}{}", random_token());
    let refresh = format!("{PORTAL_REFRESH_PREFIX}{}", random_token());
    for (kind, token, expires_at) in [
        (TokenKind::Access, &access, Some(now + ACCESS_LIFETIME)),
        (TokenKind::Refresh, &refresh, None),
    ] {
        portal_grants::insert_token(
            &mut *tx,
            &portal_grants::NewToken {
                id: Uuid::now_v7(),
                grant_id: spent.grant_id,
                kind,
                token_hash: &secret::hash(token),
                family: spent.family,
                resource: &spent.resource,
                redirect_uri: None,
                code_challenge: None,
                now,
                expires_at,
            },
        )
        .await?;
    }
    portal_grants::touch(&mut *tx, spent.grant_id, now).await?;
    tx.commit().await?;
    Ok(Ok(PortalTokens {
        access_token: access,
        token_type: "Bearer",
        expires_in: ACCESS_LIFETIME.whole_seconds(),
        refresh_token: refresh,
        scope: PORTAL_SCOPE,
    }))
}

async fn exchange(
    state: &AppState,
    kind: TokenKind,
    presented: &str,
    client_id: Option<&str>,
    redirect_uri: Option<&str>,
    verifier: Option<&str>,
    now: OffsetDateTime,
) -> Result<Result<Tokens, Response>, StoreError> {
    let mut tx = state.pg.begin().await?;
    let Some(spent) = agents::spend(&mut tx, kind, &secret::hash(presented), now).await? else {
        return Ok(Err(invalid_grant(
            "The grant is invalid, expired or revoked.",
        )));
    };
    // RFC 9700 section 4.14.2: a reused refresh token or code means it leaked, so every token of
    // that sign-in ends.
    if !spent.fresh {
        agents::revoke_family(&mut *tx, spent.family, now).await?;
        tx.commit().await?;
        return Ok(Err(invalid_grant(
            "The grant was already used; the agent must sign in again.",
        )));
    }
    let checked = check_grant(
        &mut tx,
        kind,
        &spent,
        client_id,
        redirect_uri,
        verifier,
        now,
    )
    .await?;
    if let Err(refused) = checked {
        // The grant stays spent: a failed attempt must not leave a code open to more guesses.
        tx.commit().await?;
        return Ok(Err(refused));
    }
    let tokens = issue_tokens(&mut tx, spent.agent_id, spent.family, now).await?;
    agents::touch(&mut *tx, spent.agent_id, now, time::Duration::ZERO).await?;
    tx.commit().await?;
    Ok(Ok(tokens))
}

async fn check_grant(
    tx: &mut sqlx::PgConnection,
    kind: TokenKind,
    spent: &agents::UsedToken,
    client_id: Option<&str>,
    redirect_uri: Option<&str>,
    verifier: Option<&str>,
    now: OffsetDateTime,
) -> Result<Result<(), Response>, StoreError> {
    let client = client_uuid(&mut *tx, client_id).await?;
    if client.is_none() || client != spent.client_id {
        return Ok(Err(oauth_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "The grant belongs to another client.",
        )));
    }
    if kind == TokenKind::Code {
        if redirect_uri != spent.redirect_uri.as_deref() {
            return Ok(Err(invalid_grant(
                "`redirect_uri` differs from the one authorized.",
            )));
        }
        let challenge = spent.code_challenge.as_deref().unwrap_or_default();
        if !verifier.is_some_and(|v| pkce_matches(v, challenge)) {
            return Ok(Err(invalid_grant("The PKCE code verifier doesn't match.")));
        }
    }
    let Some(auth) = agents::agent_for_refresh(&mut *tx, spent.agent_id).await? else {
        return Ok(Err(invalid_grant(
            "The agent was revoked; connect it again.",
        )));
    };
    if auth.user_state != "active" || unused_too_long(&auth.agent, now) {
        return Ok(Err(invalid_grant(
            "The agent's sign-in has ended; connect it again.",
        )));
    }
    Ok(Ok(()))
}

pub async fn revoke(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: Bytes,
) -> Response {
    if let Err(refused) = limit(&state, ip, AGENT_REVOCATIONS_PER_IP).await {
        return refused;
    }
    let p = params(&String::from_utf8_lossy(&body));
    let Some(token) = p.get("token").filter(|t| !t.is_empty()) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The token is missing.",
        );
    };
    let now = state.clock.now();
    if token.starts_with(PORTAL_ACCESS_PREFIX) || token.starts_with(PORTAL_REFRESH_PREFIX) {
        let revoked = async {
            let hash = secret::hash(token);
            if let Some(family) = portal_grants::revoke_token(&state.pg, &hash, now).await? {
                portal_grants::revoke_family(&state.pg, family, now).await?;
            }
            crate::portal::auth::forget(&state, &hash).await;
            Ok::<_, StoreError>(())
        };
        if let Err(e) = revoked.await {
            return store_failed(&e);
        }
        return no_store(StatusCode::OK.into_response());
    }
    match agents::revoke_token(&state.pg, &secret::hash(token), now).await {
        Ok(Some(family)) => {
            if let Err(e) = agents::revoke_family(&state.pg, family, now).await {
                return store_failed(&e);
            }
        }
        Ok(None) => {}
        Err(e) => return store_failed(&e),
    }
    no_store(StatusCode::OK.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uris_are_https_loopback_or_private_use() {
        for ok in [
            "https://agent.example.com/callback",
            "http://localhost:33418/callback",
            "http://127.0.0.1/cb",
            "http://[::1]:8080/cb",
            "cursor://anysphere.cursor-retrieval/oauth/callback",
            "com.example.agent:/oauth",
        ] {
            assert!(redirect_uri_allowed(ok), "{ok}");
        }
        for bad in [
            "http://agent.example.com/callback",
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "https://agent.example.com/cb#fragment",
            "https://user:pass@agent.example.com/cb",
            "not a url",
        ] {
            assert!(!redirect_uri_allowed(bad), "{bad}");
        }
        assert!(!redirect_uri_allowed(&format!(
            "https://a.example/{}",
            "x".repeat(2000)
        )));
    }

    #[test]
    fn only_addresses_on_the_users_machine_receive_errors_directly() {
        for native in [
            "http://127.0.0.1:33418/callback",
            "http://localhost/cb",
            "cursor://anysphere.cursor-retrieval/oauth/callback",
            "com.example.agent:/oauth",
        ] {
            assert!(receives_errors(native), "{native}");
        }
        for web in [
            "https://agent.example/cb",
            "http://agent.example/cb",
            "not a url",
        ] {
            assert!(!receives_errors(web), "{web}");
        }
    }

    #[test]
    fn a_loopback_redirect_may_change_its_port_but_nothing_else() {
        let registered = "http://127.0.0.1:1000/callback";
        assert!(redirect_matches(
            registered,
            "http://127.0.0.1:52011/callback"
        ));
        assert!(!redirect_matches(
            registered,
            "http://127.0.0.1:52011/other"
        ));
        assert!(!redirect_matches(
            registered,
            "http://localhost:52011/callback"
        ));
        assert!(!redirect_matches(
            "https://a.example/cb",
            "https://a.example:444/cb"
        ));
        assert!(redirect_matches(
            "https://a.example/cb",
            "https://a.example/cb"
        ));
    }

    #[test]
    fn pkce_follows_rfc_7636() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(pkce_matches(verifier, challenge));
        assert!(!pkce_matches(
            verifier,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cN"
        ));
        assert!(!pkce_matches("short", challenge));
        assert!(!pkce_matches(&format!("{verifier}!"), challenge));
    }

    #[test]
    fn redirects_keep_their_query_and_name_the_host() {
        assert_eq!(
            with_params(
                "https://a.example/cb?x=1",
                &[("code", Some("c d")), ("state", None)]
            ),
            "https://a.example/cb?x=1&code=c+d"
        );
        assert_eq!(redirect_host("http://localhost:3000/cb"), "localhost");
        assert_eq!(redirect_host("cursor://anysphere/cb"), "anysphere");
        assert_eq!(
            redirect_host("com.example.agent:/oauth"),
            "com.example.agent:"
        );
    }
}
