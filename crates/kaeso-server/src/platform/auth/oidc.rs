use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, UserId};
use kaeso_core::secret;
use kaeso_crypto::{ExposeSecret, Sealed, random_token};
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{invites, members, oidc, orgs, sso_identities, users};
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
const LINKED_PAGE: &str = "settings/account/security";
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

#[derive(Default)]
struct Back {
    org: Option<OrgId>,
    next: Option<String>,
    link: bool,
}

pub(crate) struct Linking {
    pub authorization_url: String,
    pub cookie: HeaderValue,
}

pub(crate) async fn begin_link(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    user: UserId,
    next: Option<String>,
) -> ApiResult<Linking> {
    sso_on(state)?;
    throttle_ip(state, ctx, TOKENS_PER_IP).await?;
    let begun = begin(state, ctx, org, false, next, Some(user)).await?;
    Ok(Linking {
        authorization_url: begun.authorization_url,
        cookie: begun.cookie,
    })
}

async fn begin(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    named: bool,
    next: Option<String>,
    link: Option<UserId>,
) -> ApiResult<Begun> {
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
                "sign_in_org": named.then(|| org.to_string()),
                "link_user": link.map(|u| u.to_string()),
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
    let (org, named) = match (req.org, req.email) {
        (Some(org), None) => (org, true),
        (None, Some(email)) => {
            let email = kaeso_core::email::normalize(&email)
                .map_err(|e| ApiError::invalid("email", format!("{e}.")))?;
            let org = oidc::org_for_domain(&state.pg, kaeso_core::email::domain(&email))
                .await?
                .map(OrgId::from_uuid)
                .ok_or_else(no_sso)?;
            (org, false)
        }
        _ => return Err(ApiError::bad_request("Send either `org` or `email`.")),
    };
    let begun = begin(&state, &ctx, org, named, next, None).await?;
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
        Err(e) => return refused(&state, &e, &Back::default(), false),
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
        begin(&state, &ctx, org, link.org.is_some(), next, None).await
    };
    match begun.await {
        Ok(begun) => {
            let mut response = redirect(&begun.authorization_url);
            response
                .headers_mut()
                .append(header::SET_COOKIE, begun.cookie);
            response
        }
        Err(e) => {
            let back = Back {
                org: link.org,
                next: link.next,
                link: false,
            };
            refused(&state, &e, &back, false)
        }
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

fn failed_location(state: &AppState, message: &str, back: &Back) -> String {
    let reason = percent_encoding::utf8_percent_encode(message, FRAGMENT).to_string();
    if back.link {
        let mut page = back
            .next
            .as_deref()
            .and_then(|n| next::target(&state.config.public_url, n))
            .or_else(|| state.config.public_url.join(LINKED_PAGE).ok())
            .unwrap_or_else(|| state.config.public_url.clone());
        page.set_fragment(Some(&format!("sso_error={reason}")));
        return page.to_string();
    }
    let next = back
        .next
        .as_deref()
        .filter(|n| next::target(&state.config.public_url, n).is_some())
        .map(|n| {
            format!(
                "next={}",
                percent_encoding::utf8_percent_encode(n, FRAGMENT)
            )
        });
    let org = back.org.map(|o| format!("org={o}"));
    let query = [next, org].into_iter().flatten().collect::<Vec<_>>();
    let query = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    format!(
        "{}sign-in{query}#sso_error={reason}",
        state.config.public_url
    )
}

fn refused(state: &AppState, e: &ApiError, back: &Back, clear_binding: bool) -> Response {
    metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "oidc", "reason" => e.kind())
        .increment(1);
    tracing::info!(error = %e, "SSO sign-in failed");
    let message = match e {
        ApiError::Internal(_) => "Something went wrong; try again.".to_owned(),
        other => other.to_string(),
    };
    let mut response = redirect(&failed_location(state, &message, back));
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
    let (back, outcome) = match cb {
        Ok(Query(cb)) => answer(&state, &ctx, &headers, cb).await,
        Err(e) => (Back::default(), Err(e)),
    };
    outcome.unwrap_or_else(|e| refused(&state, &e, &back, true))
}

async fn answer(
    state: &AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    cb: Callback,
) -> (Back, ApiResult<Response>) {
    if let Err(e) = sso_on(state) {
        return (Back::default(), Err(e));
    }
    let flow = match cb.state.as_deref() {
        Some(flow_state) => take_flow(state, ctx, headers, flow_state).await,
        None => Err(incomplete()),
    };
    if let Some(error) = cb.error {
        tracing::info!(
            error = %error,
            description = cb.error_description.as_deref().unwrap_or_default(),
            "identity provider refused the sign-in"
        );
        let back = flow
            .map_err(|e| tracing::info!(error = %e, "refused SSO answer matched no flow"))
            .map(|f| f.back)
            .unwrap_or_default();
        return (
            back,
            Err(ApiError::Authentication(
                "The identity provider refused the sign-in.".into(),
            )),
        );
    }
    let flow = match flow {
        Ok(flow) => flow,
        Err(e) => return (Back::default(), Err(e)),
    };
    let outcome = match cb.code.as_deref() {
        Some(code) => complete(state, ctx, &flow, code).await,
        None => Err(incomplete()),
    };
    (flow.back, outcome)
}

fn incomplete() -> ApiError {
    ApiError::bad_request("The identity provider's answer is incomplete.")
}

fn no_access() -> ApiError {
    ApiError::PermissionDenied(
        "You have no access to this organization yet; ask an admin to invite you.".into(),
    )
}

struct Flow {
    org: OrgId,
    nonce: String,
    verifier: String,
    link_user: Option<UserId>,
    back: Back,
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
    flow_state: &str,
) -> ApiResult<Flow> {
    let state_hash = secret::hash(flow_state);
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
    let link_user = match field("link_user") {
        Some(u) => Some(u.parse::<UserId>().map_err(|_| expired())?),
        None => None,
    };
    Ok(Flow {
        org,
        nonce,
        verifier,
        link_user,
        back: Back {
            org: field("sign_in_org").and_then(|o| o.parse().ok()),
            next: field("next"),
            link: link_user.is_some(),
        },
    })
}

fn link_required() -> ApiError {
    ApiError::PermissionDenied(
        "Your account isn't linked to this SSO; sign in another way and link it under Account."
            .into(),
    )
}

fn linked_elsewhere() -> ApiError {
    ApiError::Conflict("This SSO identity is already linked to another Kaeso account.".into())
}

fn deactivated() -> ApiError {
    ApiError::PermissionDenied("This account is deactivated.".into())
}

fn check_domain(connection: &oidc::Connection, email: &str) -> ApiResult<()> {
    let domain = kaeso_core::email::domain(email);
    let verified = &connection.verified_domains;
    if verified.iter().any(|d| d.eq_ignore_ascii_case(domain)) {
        return Ok(());
    }
    if verified.is_empty() {
        return Err(ApiError::PermissionDenied(
            "This organization's SSO has no verified email domain yet; ask an admin to verify one."
                .into(),
        ));
    }
    Err(ApiError::PermissionDenied(format!(
        "This organization's SSO only signs in addresses of {}.",
        verified
            .iter()
            .map(|d| kaeso_core::email::display_domain(d))
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

async fn complete(state: &AppState, ctx: &Ctx, flow: &Flow, code: &str) -> ApiResult<Response> {
    let org = flow.org;
    let connection = oidc::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let identity = provider(state, org, &connection)
        .await?
        .finish(code, &flow.verifier, &flow.nonce)
        .await
        .map_err(|e| {
            tracing::info!(error = %e, "SSO answer refused");
            ApiError::Authentication(e.message().to_owned())
        })?;
    let email = kaeso_core::email::normalize(&identity.email).map_err(|_| {
        ApiError::Authentication("The identity provider sent an invalid email address.".into())
    })?;
    check_domain(&connection, &email)?;
    if let Some(user) = flow.link_user {
        return link(state, ctx, flow, &connection, &identity, user).await;
    }
    let org_row = orgs::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let scheduled = org_row.state != "active";

    let mut tx = state.pg.begin().await?;
    let linked =
        sso_identities::user_of(&mut *tx, org.uuid(), &connection.issuer, &identity.subject)
            .await?;
    let (user, new_account) = match linked {
        Some(id) => (
            users::get(&mut *tx, id)
                .await?
                .ok_or_else(|| ApiError::internal("linked user vanished"))?,
            false,
        ),
        None => match users::get_by_email(&mut *tx, &email).await? {
            Some(u) if u.state != "active" => return Err(deactivated()),
            Some(u) => {
                if !sso_identities::bind_pending(
                    &mut *tx,
                    org.uuid(),
                    u.id,
                    &connection.issuer,
                    &identity.subject,
                    ctx.now,
                )
                .await?
                {
                    return Err(link_required());
                }
                ctx.audit(Some(org), "user.sso_link")
                    .by(UserId::from_uuid(u.id))
                    .target("user", UserId::from_uuid(u.id))
                    .after(serde_json::json!({ "issuer": connection.issuer, "email": email }))
                    .write(&mut tx)
                    .await?;
                (u, false)
            }
            None if scheduled => return Err(ApiError::OrgScheduledForDeletion),
            None => {
                let user = create_account(&mut tx, ctx, org, &email, &identity).await?;
                sso_identities::link(
                    &mut *tx,
                    org.uuid(),
                    user.id,
                    &connection.issuer,
                    &identity.subject,
                    ctx.now,
                )
                .await?;
                (user, true)
            }
        },
    };
    if user.state != "active" {
        return Err(deactivated());
    }
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
    if user.email.as_deref() == Some(email.as_str()) {
        users::mark_verified(&mut *tx, user.id, ctx.now).await?;
    }
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
    let mut response = redirect(&signed_in_location(state, flow.back.next.as_deref()));
    if let Ok(v) = binding_cookie(state, "", 0) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    let cookie =
        cookies::set(state, &created.token).ok_or_else(|| ApiError::internal("session cookie"))?;
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}

async fn create_account(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &Ctx,
    org: OrgId,
    email: &str,
    identity: &crate::auth::oidc::Identity,
) -> ApiResult<users::User> {
    let id = UserId::new();
    let name = identity
        .name
        .as_deref()
        .and_then(crate::platform::validate::display_name)
        .unwrap_or_else(|| email.split('@').next().unwrap_or(email).to_owned());
    users::insert(
        &mut **tx,
        &users::NewUser {
            id: id.uuid(),
            email,
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
        .write(tx)
        .await?;
    users::get(&mut **tx, id.uuid())
        .await?
        .ok_or_else(|| ApiError::internal("new user vanished"))
}

async fn link(
    state: &AppState,
    ctx: &Ctx,
    flow: &Flow,
    connection: &oidc::Connection,
    identity: &crate::auth::oidc::Identity,
    user: UserId,
) -> ApiResult<Response> {
    let org = flow.org;
    let mut tx = state.pg.begin().await?;
    let account = users::get(&mut *tx, user.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(deactivated)?;
    if members::get_org(&mut *tx, org.uuid(), account.id)
        .await?
        .is_none()
    {
        return Err(no_access());
    }
    match sso_identities::user_of(&mut *tx, org.uuid(), &connection.issuer, &identity.subject)
        .await?
    {
        Some(other) if other != account.id => return Err(linked_elsewhere()),
        _ => {}
    }
    sso_identities::link(
        &mut *tx,
        org.uuid(),
        account.id,
        &connection.issuer,
        &identity.subject,
        ctx.now,
    )
    .await?;
    ctx.audit(Some(org), "user.sso_link")
        .by(user)
        .target("user", user)
        .after(serde_json::json!({ "issuer": connection.issuer, "email": identity.email }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let to = flow
        .back
        .next
        .as_deref()
        .and_then(|n| next::target(&state.config.public_url, n))
        .or_else(|| state.config.public_url.join(LINKED_PAGE).ok())
        .map_or_else(|| state.config.public_url.to_string(), |u| u.to_string());
    let mut response = redirect(&to);
    if let Ok(v) = binding_cookie(state, "", 0) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    Ok(response)
}
