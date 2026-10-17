use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::secret;
use kaeso_crypto::{ExposeSecret, Sealed, random_token};
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{invites, members, oidc, orgs, users};
use serde::{Deserialize, Serialize};
use time::Duration;
use utoipa::{IntoParams, ToSchema};

use super::{next, throttle_ip};
use crate::auth::oidc::Provider;
use crate::auth::sessions::{self, Method, SignIn};
use crate::auth::throttle::TOKENS_PER_IP;
use crate::platform::access::Ctx;
use crate::platform::cookies;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Query;
use crate::services::{invites as invite_service, orgs as org_service};
use crate::state::AppState;

const FLOW_LIFETIME: Duration = Duration::minutes(10);
const BINDING_COOKIE: &str = "__Host-kaeso_oidc";
const DEV_BINDING_COOKIE: &str = "kaeso_oidc";

const FRAGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub fn secret_aad(org: OrgId) -> Vec<u8> {
    format!("oidc:{org}").into_bytes()
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Which org to sign in to, and where to go afterwards.")]
pub struct StartRequest {
    #[doc = "The org: from its sign-in link (`/sign-in?org=…`, the `sign_in_url` of\n\
        its SSO connection), from `provider` in `auth/methods`, or from the\n\
        console page that asks for its SSO."]
    pub org: Option<OrgId>,
    #[doc = "Or the user's email address, whose domain names the org."]
    #[schema(example = "ada@acme.example")]
    pub email: Option<String>,
    #[doc = "The console path to open once signed in, starting with one `/`; never\n\
        another site, the Platform API or the model API. Without it, the\n\
        console's start page."]
    #[schema(example = "/settings/account")]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StartLink {
    #[doc = "The org whose identity provider signs in. Without it, the install's\n\
        only SSO connection."]
    pub org: Option<OrgId>,
    #[doc = "The console path to open once signed in, as in `POST auth/oidc/start`."]
    pub next: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Where to send the browser.")]
pub struct StartResponse {
    #[doc = "The identity provider's authorization URL."]
    pub authorization_url: String,
}

fn binding_cookie_name(state: &AppState) -> &'static str {
    if state.config.is_local_development() {
        DEV_BINDING_COOKIE
    } else {
        BINDING_COOKIE
    }
}

fn binding_cookie(
    state: &AppState,
    value: &str,
    max_age: i64,
) -> Result<HeaderValue, axum::http::header::InvalidHeaderValue> {
    let secure = if state.config.is_local_development() {
        ""
    } else {
        "; Secure"
    };
    HeaderValue::from_str(&format!(
        "{}={value}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age={max_age}",
        binding_cookie_name(state)
    ))
}

fn sso_on(state: &AppState) -> ApiResult<()> {
    if state.config.auth.oidc {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "SSO sign-in is turned off on this install.".into(),
        ))
    }
}

fn no_sso() -> ApiError {
    ApiError::NotFound(
        "No organization uses SSO for this sign-in; use another way to sign in.".into(),
    )
}

struct Begun {
    authorization_url: String,
    cookie: HeaderValue,
}

async fn begin(state: &AppState, ctx: &Ctx, org: OrgId, next: Option<String>) -> ApiResult<Begun> {
    let connection = oidc::get(&state.pg, org.uuid()).await?.ok_or_else(no_sso)?;
    let provider = provider(state, org, &connection).await?;
    let started = provider.start();
    let binding = random_token();
    tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::OidcState,
            hash: &secret::hash(&started.state),
            user_id: None,
            email: None,
            data: serde_json::json!({
                "org_id": org.to_string(),
                "nonce": started.nonce,
                "pkce_verifier": started.pkce_verifier,
                "binding": crate::cache::hex(&secret::hash(&binding)),
                "next": next,
            }),
            expires_at: ctx.now + FLOW_LIFETIME,
        },
    )
    .await?;
    let cookie = binding_cookie(state, &binding, FLOW_LIFETIME.whole_seconds())
        .map_err(ApiError::internal)?;
    Ok(Begun {
        authorization_url: started.url,
        cookie,
    })
}

#[utoipa::path(
    post,
    path = "/auth/oidc/start",
    summary = "Starts SSO for an org: answers with the provider's URL and sets a cookie\n\
    that binds the flow to this browser.",
    tag = "auth",
    request_body = StartRequest,
    responses((status = 200, body = StartResponse))
)]
pub async fn start(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<StartRequest>,
) -> ApiResult<Response> {
    sso_on(&state)?;
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let next = next::validate(&state.config.public_url, req.next.as_deref())?;
    let org = match (req.org, req.email) {
        (Some(org), None) => org,
        (None, Some(email)) => {
            let email = kaeso_core::email::normalize(&email)
                .map_err(|e| ApiError::invalid("email", format!("{e}.")))?;
            oidc::org_for_domain(&state.pg, kaeso_core::email::domain(&email))
                .await?
                .map(OrgId::from_uuid)
                .ok_or_else(no_sso)?
        }
        _ => return Err(ApiError::bad_request("Send either `org` or `email`.")),
    };
    let begun = begin(&state, &ctx, org, next).await?;
    Ok((
        [(header::SET_COOKIE, begun.cookie)],
        axum::Json(StartResponse {
            authorization_url: begun.authorization_url,
        }),
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/auth/oidc/start",
    summary = "A link that starts SSO: sends the browser straight to the identity\n\
    provider, with the flow bound to this browser as `POST auth/oidc/start`\n\
    binds it. On failure, to the sign-in page with the reason in `#sso_error=`.",
    description = "For links from an intranet page or a bookmark. It only begins a sign-in \
    in the browser that follows it, and whoever signs in at the provider is who is signed in \
    afterwards, so it needs no `Origin` header.",
    tag = "auth",
    params(StartLink),
    responses((status = 303, description = "Redirect to the identity provider, or to the sign-in page with `#sso_error=`."))
)]
pub async fn start_link(
    State(state): State<AppState>,
    ctx: Ctx,
    link: Result<Query<StartLink>, ApiError>,
) -> Response {
    let link = match link {
        Ok(Query(link)) => link,
        Err(e) => return refused(&state, &e, None, false),
    };
    let begun = async {
        sso_on(&state)?;
        throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
        let next = next::validate(&state.config.public_url, link.next.as_deref())?;
        let org = match link.org {
            Some(org) => org,
            None => match oidc::connected(&state.pg).await? {
                oidc::Connected::Only(org) => OrgId::from_uuid(org),
                oidc::Connected::Nowhere => return Err(no_sso()),
                oidc::Connected::Several => {
                    return Err(ApiError::invalid(
                        "org",
                        "Name the organization with `org`; several organizations use SSO here.",
                    ));
                }
            },
        };
        begin(&state, &ctx, org, next).await
    };
    match begun.await {
        Ok(begun) => {
            let mut response = redirect(&begun.authorization_url);
            response
                .headers_mut()
                .append(header::SET_COOKIE, begun.cookie);
            response
        }
        Err(e) => refused(&state, &e, link.next.as_deref(), false),
    }
}

async fn provider(state: &AppState, org: OrgId, c: &oidc::Connection) -> ApiResult<Provider> {
    let sealed = Sealed {
        ciphertext: c.secret_ciphertext.clone(),
        nonce: c.secret_nonce.clone(),
        wrapped_key: c.wrapped_key.clone(),
        key_id: c.key_id.clone(),
    };
    let secret = kaeso_crypto::open(&state.keyring, &sealed, &secret_aad(org))
        .map_err(ApiError::internal)?;
    let secret = String::from_utf8(secret.expose_secret().clone()).map_err(ApiError::internal)?;
    Provider::discover(state, &c.issuer, &c.client_id, secret)
        .await
        .map_err(|e| {
            tracing::info!(error = %e, "SSO provider unusable");
            ApiError::Unavailable(e.message().to_owned())
        })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Callback {
    #[doc = "The authorization code."]
    pub code: Option<String>,
    #[doc = "The state from `auth/oidc/start`."]
    pub state: Option<String>,
    #[doc = "An error from the provider."]
    pub error: Option<String>,
    #[doc = "The provider's description of the error."]
    pub error_description: Option<String>,
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(location) {
        headers.insert(header::LOCATION, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn signed_in_location(state: &AppState, next: Option<&str>) -> String {
    next.and_then(|n| next::target(&state.config.public_url, n))
        .map_or_else(|| state.config.public_url.to_string(), |u| u.to_string())
}

fn failed_location(state: &AppState, message: &str, next: Option<&str>) -> String {
    let next = next
        .filter(|n| next::target(&state.config.public_url, n).is_some())
        .map(|n| {
            format!(
                "?next={}",
                percent_encoding::utf8_percent_encode(n, FRAGMENT)
            )
        })
        .unwrap_or_default();
    format!(
        "{}sign-in{next}#sso_error={}",
        state.config.public_url,
        percent_encoding::utf8_percent_encode(message, FRAGMENT)
    )
}

fn refused(state: &AppState, e: &ApiError, next: Option<&str>, clear_binding: bool) -> Response {
    metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "oidc", "reason" => e.kind())
        .increment(1);
    tracing::info!(error = %e, "SSO sign-in failed");
    let message = match e {
        ApiError::Internal(_) => "Something went wrong; try again.".to_owned(),
        other => other.to_string(),
    };
    let mut response = redirect(&failed_location(state, &message, next));
    if clear_binding && let Ok(v) = binding_cookie(state, "", 0) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    response
}

fn binding_from(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let wanted = binding_cookie_name(state);
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == wanted)
        .map(|(_, v)| v.to_owned())
}

#[utoipa::path(
    get,
    path = "/auth/oidc/callback",
    summary = "The identity provider returns here. On success the session cookie is\n\
    set and the browser goes to the console; on failure to the sign-in page\n\
    with the reason in `#sso_error=`.",
    tag = "auth",
    params(Callback),
    responses((status = 303, description = "Redirect to the console (signed in) or to the sign-in page with `#sso_error=`."))
)]
pub async fn callback(
    State(state): State<AppState>,
    ctx: Ctx,
    headers: HeaderMap,
    cb: Result<Query<Callback>, ApiError>,
) -> Response {
    let flow = match cb {
        Ok(Query(cb)) => take_flow(&state, &ctx, &headers, cb).await,
        Err(e) => Err(e),
    };
    let flow = match flow {
        Ok(flow) => flow,
        Err(e) => return refused(&state, &e, None, true),
    };
    match complete(&state, &ctx, &flow).await {
        Ok(response) => response,
        Err(e) => refused(&state, &e, flow.next.as_deref(), true),
    }
}

fn no_access() -> ApiError {
    ApiError::PermissionDenied(
        "You have no access to this organization yet; ask an admin to invite you.".into(),
    )
}

struct Flow {
    org: OrgId,
    code: String,
    nonce: String,
    verifier: String,
    next: Option<String>,
}

fn expired() -> ApiError {
    ApiError::Authentication(
        "The SSO sign-in expired or began in another browser; start again.".into(),
    )
}

async fn take_flow(
    state: &AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    cb: Callback,
) -> ApiResult<Flow> {
    sso_on(state)?;
    if let Some(error) = cb.error {
        tracing::info!(
            error = %error,
            description = cb.error_description.as_deref().unwrap_or_default(),
            "identity provider refused the sign-in"
        );
        return Err(ApiError::Authentication(
            "The identity provider refused the sign-in.".into(),
        ));
    }
    let (Some(code), Some(flow_state)) = (cb.code, cb.state) else {
        return Err(ApiError::bad_request(
            "The identity provider's answer is incomplete.",
        ));
    };
    let state_hash = secret::hash(&flow_state);
    let binding = binding_from(state, headers).ok_or_else(expired)?;
    let bound = |data: &serde_json::Value| {
        data.get("binding").and_then(|v| v.as_str())
            == Some(crate::cache::hex(&secret::hash(&binding)).as_str())
    };
    let pending = tokens::peek(&state.pg, TokenKind::OidcState, &state_hash, ctx.now)
        .await?
        .ok_or_else(expired)?;
    if !bound(&pending.data) {
        return Err(expired());
    }
    let flow = tokens::consume(&state.pg, TokenKind::OidcState, &state_hash, ctx.now)
        .await?
        .filter(|f| bound(&f.data))
        .ok_or_else(expired)?;
    let field = |k: &str| flow.data.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    let org: OrgId = field("org_id")
        .and_then(|o| o.parse().ok())
        .ok_or_else(expired)?;
    let (Some(nonce), Some(verifier)) = (field("nonce"), field("pkce_verifier")) else {
        return Err(expired());
    };
    Ok(Flow {
        org,
        code,
        nonce,
        verifier,
        next: field("next"),
    })
}

async fn complete(state: &AppState, ctx: &Ctx, flow: &Flow) -> ApiResult<Response> {
    let org = flow.org;
    let connection = oidc::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let identity = provider(state, org, &connection)
        .await?
        .finish(&flow.code, &flow.verifier, &flow.nonce)
        .await
        .map_err(|e| {
            tracing::info!(error = %e, "SSO answer refused");
            ApiError::Authentication(e.message().to_owned())
        })?;
    let email = kaeso_core::email::normalize(&identity.email).map_err(|_| {
        ApiError::Authentication("The identity provider sent an invalid email address.".into())
    })?;
    let domain = kaeso_core::email::domain(&email).to_owned();
    if !connection
        .domains
        .iter()
        .any(|d| d.eq_ignore_ascii_case(&domain))
    {
        return Err(ApiError::PermissionDenied(format!(
            "This organization's SSO only signs in addresses of {}.",
            connection
                .domains
                .iter()
                .map(|d| kaeso_core::email::display_domain(d))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let org_row = orgs::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let scheduled = org_row.state != "active";

    let mut tx = state.pg.begin().await?;
    let (user, new_account) = match users::get_by_email(&mut *tx, &email).await? {
        Some(u) if u.state != "active" => {
            return Err(ApiError::PermissionDenied(
                "This account is deactivated.".into(),
            ));
        }
        Some(u) => (u, false),
        None if scheduled => return Err(ApiError::OrgScheduledForDeletion),
        None => {
            let id = UserId::new();
            let name = identity
                .name
                .as_deref()
                .and_then(crate::platform::validate::display_name)
                .unwrap_or_else(|| email.split('@').next().unwrap_or(&email).to_owned());
            users::insert(
                &mut *tx,
                &users::NewUser {
                    id: id.uuid(),
                    email: &email,
                    name: &name,
                    password_hash: None,
                    email_verified_at: Some(ctx.now),
                    is_instance_admin: false,
                },
            )
            .await?;
            ctx.audit(None, "user.create_by_sso")
                .by(id)
                .target("user", id)
                .after(serde_json::json!({ "email": email, "org": org.to_string() }))
                .write(&mut tx)
                .await?;
            let user = users::get(&mut *tx, id.uuid())
                .await?
                .ok_or_else(|| ApiError::internal("new user vanished"))?;
            (user, true)
        }
    };
    let user_id = UserId::from_uuid(user.id);
    if members::get_org(&mut *tx, org.uuid(), user.id)
        .await?
        .is_none()
    {
        if scheduled {
            return Err(ApiError::OrgScheduledForDeletion);
        }
        let invite = invites::pending_for(&mut *tx, org.uuid(), &email, ctx.now).await?;
        if !new_account {
            return Err(match invite {
                Some(_) => ApiError::PermissionDenied(format!(
                    "An account with {email} exists; sign in to accept the invitation."
                )),
                None => no_access(),
            });
        }
        if let Some(invite) = invite {
            invite_service::accept(&mut tx, &invite, user_id, ctx.now).await?;
            ctx.audit(Some(org), "invite.accept")
                .by(user_id)
                .target("invite", invite.id)
                .write(&mut tx)
                .await?;
        } else if let Some(preset) = org_row.sso_default_preset.clone() {
            let grant = kaeso_store::pg::members::Grant {
                preset,
                role_id: org_row.sso_default_role_id,
                ..Default::default()
            };
            org_service::add_member(&mut tx, org, user_id, &grant).await?;
            ctx.audit(Some(org), "member.join_by_sso")
                .by(user_id)
                .target("user", user_id)
                .after(serde_json::json!({ "preset": grant.preset, "role_id": grant.role_id.map(|r| kaeso_core::id::RoleId::from_uuid(r).to_string()) }))
                .write(&mut tx)
                .await?;
        } else {
            return Err(no_access());
        }
    }
    users::mark_verified(&mut *tx, user.id, ctx.now).await?;
    let created = sessions::create(
        &mut tx,
        ctx.now,
        &SignIn {
            user_id,
            method: Method::Oidc,
            second_factor: false,
            oidc_org: Some(org.uuid()),
            signed_in_at: ctx.now,
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await?;
    tx.commit().await?;
    let mut response = redirect(&signed_in_location(state, flow.next.as_deref()));
    if let Ok(v) = binding_cookie(state, "", 0) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    let cookie =
        cookies::set(state, &created.token).ok_or_else(|| ApiError::internal("session cookie"))?;
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}
