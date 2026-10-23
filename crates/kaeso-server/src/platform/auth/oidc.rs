use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, SessionId, UserId};
use kaeso_core::secret;
use kaeso_crypto::{ExposeSecret, Sealed, random_token};
use kaeso_store::pg::sessions as sessions_store;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use kaeso_store::pg::{invites, members, oidc, orgs, sso_identities, users};
use serde::{Deserialize, Serialize};
use time::Duration;
use utoipa::{IntoParams, ToSchema};

use super::{next, throttle_ip};
use crate::auth::oidc::Provider;
use crate::auth::sessions::{self, ABSOLUTE_LIFETIME, Method, RECENT_SIGN_IN, SignIn};
use crate::auth::throttle::TOKENS_PER_IP;
use crate::platform::access::Ctx;
use crate::platform::cookies;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::params::Query;
use crate::services::{invites as invite_service, orgs as org_service};
use crate::state::AppState;

const FLOW_LIFETIME: Duration = Duration::minutes(10);
const CLOCK_SKEW: Duration = Duration::minutes(1);
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
    #[doc = "The org, only from a signed-in member of it, such as on the page that\n\
        says the org requires single sign-on; otherwise send `email`."]
    pub org: Option<OrgId>,
    #[doc = "Or the user's email address, whose verified domain names the org."]
    #[schema(example = "ada@acme.example")]
    pub email: Option<String>,
    #[doc = "The console path to open once signed in, starting with one `/`; never\n\
        another site, the Platform API or the model API. Without it, the\n\
        console's start page."]
    #[schema(example = "/settings/account")]
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

const NO_SSO: &str = "No organization uses SSO for this sign-in; use another way to sign in.";
const ENTER_EMAIL: &str = "Enter your email to sign in with single sign-on.";
const PROVIDER_REFUSED: &str = "The identity provider refused the sign-in.";
const INCOMPLETE: &str = "The identity provider's answer is incomplete.";
const INVALID_EMAIL: &str = "The identity provider sent an invalid email address.";
const NO_ACCESS: &str = "You have no access to this organization yet; ask an admin to invite you.";
const EXPIRED: &str = "The SSO sign-in expired or began in another browser; start again.";
const LINK_REQUIRED: &str =
    "Your account isn't linked to this SSO; sign in another way and link it under Account.";
const LINKED_ELSEWHERE: &str = "This SSO identity is already linked to another Kaeso account.";
const OTHER_ACCOUNT: &str =
    "You signed in at the identity provider as someone else; sign in there as yourself.";
const CANNOT_CONFIRM: &str =
    "The identity provider can't confirm a recent sign-in; use your password or an email link.";
const DEACTIVATED: &str = "This account is deactivated.";
const SIGNED_IN_AS_OTHER: &str =
    "This browser is signed in to another account; sign out first, then sign in again.";
const NO_VERIFIED_DOMAIN: &str =
    "This organization's SSO has no verified email domain yet; ask an admin to verify one.";
const OTHER_DOMAIN: &str = "This organization's SSO only signs in addresses of ";
const ACCEPT_INVITE: &str = "; sign in to accept the invitation.";

const CODES: &[(&str, &str)] = &[
    (NO_SSO, "no_sso"),
    (PROVIDER_REFUSED, "provider_refused"),
    (INCOMPLETE, "incomplete"),
    (INVALID_EMAIL, "invalid_email"),
    (NO_ACCESS, "no_access"),
    (EXPIRED, "expired"),
    (LINK_REQUIRED, "link_required"),
    (LINKED_ELSEWHERE, "linked_elsewhere"),
    (OTHER_ACCOUNT, "other_account"),
    (CANNOT_CONFIRM, "cannot_confirm"),
    (DEACTIVATED, "deactivated"),
    (SIGNED_IN_AS_OTHER, "signed_in_as_other"),
    (NO_VERIFIED_DOMAIN, "no_verified_domain"),
];

pub fn failure_code(e: &ApiError) -> &'static str {
    let message = e.to_string();
    if let Some((_, code)) = CODES.iter().find(|(m, _)| *m == message) {
        return code;
    }
    if message.starts_with(OTHER_DOMAIN) {
        return "domain_not_allowed";
    }
    if message.ends_with(ACCEPT_INVITE) {
        return "accept_invite";
    }
    if let Some(code) = crate::auth::oidc::OidcError::code_of_message(&message) {
        return code;
    }
    match e {
        ApiError::RateLimited { .. } => "throttled",
        ApiError::OrgScheduledForDeletion => "org_scheduled",
        ApiError::InvalidRequest { .. } | ApiError::NotFound(_) => "invalid_link",
        _ => "failed",
    }
}

fn no_sso() -> ApiError {
    ApiError::NotFound(NO_SSO.into())
}

struct Begun {
    authorization_url: String,
    cookie: HeaderValue,
}

#[derive(Default)]
struct Back {
    next: Option<String>,
    to_page: bool,
}

#[derive(Clone, Copy)]
enum Purpose {
    SignIn,
    Link(UserId),
    Confirm { user: UserId, session: SessionId },
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
    throttle_ip(state, ctx, TOKENS_PER_IP).await?;
    let begun = begin(state, ctx, org, Purpose::Link(user), next).await?;
    Ok(Linking {
        authorization_url: begun.authorization_url,
        cookie: begun.cookie,
    })
}

pub(crate) async fn begin_confirm(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    user: UserId,
    session: SessionId,
    next: Option<String>,
) -> ApiResult<Linking> {
    throttle_ip(state, ctx, TOKENS_PER_IP).await?;
    let begun = begin(state, ctx, org, Purpose::Confirm { user, session }, next).await?;
    Ok(Linking {
        authorization_url: begun.authorization_url,
        cookie: begun.cookie,
    })
}

async fn begin(
    state: &AppState,
    ctx: &Ctx,
    org: OrgId,
    purpose: Purpose,
    next: Option<String>,
) -> ApiResult<Begun> {
    let connection = oidc::get(&state.pg, org.uuid()).await?.ok_or_else(no_sso)?;
    let provider = provider(state, org, &connection).await?;
    let (link, confirm) = match purpose {
        Purpose::SignIn => (None, None),
        Purpose::Link(user) => (Some(user), None),
        Purpose::Confirm { user, session } => (None, Some((user, session))),
    };
    let started = provider.start(confirm.is_some());
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
                "link_user": link.map(|u| u.to_string()),
                "confirm_user": confirm.map(|(u, _)| u.to_string()),
                "confirm_session": confirm.map(|(_, s)| s.to_string()),
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
    description = "With `email`, the org whose verified domain the address is at. With `org`, \
    only an org the signed-in user is a member of; without a session, or for another org, it \
    answers 400 and asks for `email`.",
    tag = "auth",
    request_body = StartRequest,
    responses((status = 200, body = StartResponse))
)]
pub async fn start(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<StartRequest>,
) -> ApiResult<Response> {
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let next = next::validate(&state.config.public_url, req.next.as_deref())?;
    let org = match (req.org, req.email) {
        (Some(org), None) => {
            member_org(&state, &ctx, org).await?;
            org
        }
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
    let begun = begin(&state, &ctx, org, Purpose::SignIn, next).await?;
    Ok((
        [(header::SET_COOKIE, begun.cookie)],
        axum::Json(StartResponse {
            authorization_url: begun.authorization_url,
        }),
    )
        .into_response())
}

// Single sign-on belongs to the org of the person's verified email domain. A named org would
// let a link from another site sign a signed-out browser in to that org's account, so only a
// signed-in member may name its own org.
async fn member_org(state: &AppState, ctx: &Ctx, org: OrgId) -> ApiResult<()> {
    let member = match ctx.session() {
        Ok(current) => members::get_org(&state.pg, org.uuid(), current.user_id)
            .await?
            .is_some(),
        Err(_) => false,
    };
    if !member {
        return Err(ApiError::invalid("email", ENTER_EMAIL));
    }
    oidc::get(&state.pg, org.uuid()).await?.ok_or_else(no_sso)?;
    Ok(())
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

fn failed_location(state: &AppState, reason: &str, back: &Back) -> String {
    if back.to_page {
        let mut page = back
            .next
            .as_deref()
            .and_then(|n| next::target(&state.config.public_url, n))
            .or_else(|| state.config.public_url.join(LINKED_PAGE).ok())
            .unwrap_or_else(|| state.config.public_url.clone());
        page.set_fragment(Some(&format!("sso_error={reason}")));
        return page.to_string();
    }
    format!(
        "{}#sso_error={reason}",
        sign_in_location(state, back.next.as_deref())
    )
}

fn sign_in_location(state: &AppState, next: Option<&str>) -> String {
    let query = next
        .filter(|n| next::target(&state.config.public_url, n).is_some())
        .map(|n| {
            format!(
                "?next={}",
                percent_encoding::utf8_percent_encode(n, FRAGMENT)
            )
        })
        .unwrap_or_default();
    format!("{}sign-in{query}", state.config.public_url)
}

fn refused(state: &AppState, e: &ApiError, back: &Back) -> Response {
    metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "oidc", "reason" => e.kind())
        .increment(1);
    tracing::info!(error = %e, "SSO sign-in failed");
    let mut response = redirect(&failed_location(state, failure_code(e), back));
    if let Ok(v) = binding_cookie(state, "", 0) {
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
    with an error code in `#sso_error=`.",
    description = "The codes: `no_sso`, `invalid_link`, `expired`, \
    `incomplete`, `provider_refused`, `provider_unreachable`, `answer_unverified`, \
    `provider_incompatible`, `email_unverified`, `invalid_email`, `domain_not_allowed`, \
    `no_verified_domain`, `deactivated`, `no_access`, `accept_invite`, `link_required`, \
    `linked_elsewhere`, `signed_in_as_other`, `org_scheduled`, `throttled` and `failed`. \
    `signed_in_as_other` means the browser is signed in to another account, which stays signed \
    in; an answer for the same account replaces its session. Show your own message for each; \
    new codes may appear, so show a general one for a code you don't know.",
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
    outcome.unwrap_or_else(|e| refused(&state, &e, &back))
}

async fn answer(
    state: &AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    cb: Callback,
) -> (Back, ApiResult<Response>) {
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
        return (back, Err(ApiError::Authentication(PROVIDER_REFUSED.into())));
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
    ApiError::Authentication(INCOMPLETE.into())
}

fn no_access() -> ApiError {
    ApiError::PermissionDenied(NO_ACCESS.into())
}

struct Flow {
    org: OrgId,
    nonce: String,
    verifier: String,
    link_user: Option<UserId>,
    confirm: Option<(UserId, SessionId)>,
    back: Back,
}

fn expired() -> ApiError {
    ApiError::Authentication(EXPIRED.into())
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
    let confirm = match (field("confirm_user"), field("confirm_session")) {
        (Some(u), Some(s)) => Some((
            u.parse::<UserId>().map_err(|_| expired())?,
            s.parse::<SessionId>().map_err(|_| expired())?,
        )),
        _ => None,
    };
    Ok(Flow {
        org,
        nonce,
        verifier,
        link_user,
        confirm,
        back: Back {
            next: field("next"),
            to_page: link_user.is_some() || confirm.is_some(),
        },
    })
}

fn link_required() -> ApiError {
    ApiError::PermissionDenied(LINK_REQUIRED.into())
}

fn linked_elsewhere() -> ApiError {
    ApiError::Conflict(LINKED_ELSEWHERE.into())
}

fn other_account() -> ApiError {
    ApiError::PermissionDenied(OTHER_ACCOUNT.into())
}

fn deactivated() -> ApiError {
    ApiError::PermissionDenied(DEACTIVATED.into())
}

fn check_domain(connection: &oidc::Connection, email: &str) -> ApiResult<()> {
    let domain = kaeso_core::email::domain(email);
    let verified = &connection.verified_domains;
    if verified.iter().any(|d| d.eq_ignore_ascii_case(domain)) {
        return Ok(());
    }
    if verified.is_empty() {
        return Err(ApiError::PermissionDenied(NO_VERIFIED_DOMAIN.into()));
    }
    Err(ApiError::PermissionDenied(format!(
        "{OTHER_DOMAIN}{}.",
        verified
            .iter()
            .map(|d| kaeso_core::email::display_domain(d))
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

// Removing or repointing the connection takes this row's lock before it ends the org's SSO
// sessions, so a callback that read the connection before the change either sees the change
// here or holds the lock until its session is committed and then ended with the others.
async fn still_connected(
    tx: &mut sqlx::PgConnection,
    connection: &oidc::Connection,
) -> ApiResult<()> {
    match oidc::lock_pointer(&mut *tx, connection.org_id).await? {
        Some(now) if now.issuer == connection.issuer && now.client_id == connection.client_id => {
            Ok(())
        }
        _ => Err(expired()),
    }
}

async fn complete(state: &AppState, ctx: &Ctx, flow: &Flow, code: &str) -> ApiResult<Response> {
    let org = flow.org;
    let connection = oidc::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let identity = provider(state, org, &connection)
        .await?
        .finish(
            code,
            &flow.verifier,
            &flow.nonce,
            connection.require_email_verified,
        )
        .await
        .map_err(|e| {
            tracing::info!(error = %e, "SSO answer refused");
            ApiError::Authentication(e.message().to_owned())
        })?;
    let email = kaeso_core::email::normalize(&identity.email)
        .map_err(|_| ApiError::Authentication(INVALID_EMAIL.into()))?;
    check_domain(&connection, &email)?;
    if let Some(user) = flow.link_user {
        return link(state, ctx, flow, &connection, &identity, user).await;
    }
    if let Some((user, session)) = flow.confirm {
        return confirm(
            state,
            ctx,
            flow,
            &connection,
            &identity,
            &email,
            user,
            session,
        )
        .await;
    }
    let org_row = orgs::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let scheduled = org_row.state != "active";

    let mut tx = state.pg.begin().await?;
    still_connected(&mut tx, &connection).await?;
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
    // A link from another site can carry a browser through someone else's identity provider,
    // so an SSO answer never replaces the session of another account in this browser.
    if let Ok(current) = ctx.session()
        && current.user_id != user.id
    {
        return Err(ApiError::PermissionDenied(SIGNED_IN_AS_OTHER.into()));
    }
    let user_id = UserId::from_uuid(user.id);
    if linked.is_some() {
        follow_address(&mut tx, ctx, org, &connection.issuer, &user, &email).await?;
    }
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
                    "An account with {email} exists{ACCEPT_INVITE}"
                )),
                None => no_access(),
            });
        }
        if let Some(invite) = invite {
            let invite_service::Accepted::Joined { revoked_others, .. } =
                invite_service::accept(&mut tx, &invite, user_id, ctx.now).await?
            else {
                return Err(no_access());
            };
            ctx.audit(Some(org), "invite.accept")
                .by(user_id)
                .target("invite", invite.id)
                .after(serde_json::json!({ "revoked_invites": revoked_others }))
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
    if user
        .email
        .as_deref()
        .is_some_and(|stored| kaeso_core::email::same_address(stored, &email))
    {
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
            signed_in_at: signed_in_at(&identity, ctx.now),
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await?;
    tx.commit().await?;
    super::end_previous(state, ctx, user_id).await?;
    let mut response = redirect(&signed_in_location(state, flow.back.next.as_deref()));
    if let Ok(v) = binding_cookie(state, "", 0) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    let cookie =
        cookies::set(state, &created.token).ok_or_else(|| ApiError::internal("session cookie"))?;
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}

// The issuer and subject link the account, not the address, so the provider of the org that
// manages the address may move it within that org's verified domains; another org's provider,
// or an address another account holds, never changes it.
async fn follow_address(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &Ctx,
    org: OrgId,
    issuer: &str,
    user: &users::User,
    email: &str,
) -> ApiResult<()> {
    let Some(stored) = user.email.as_deref() else {
        return Ok(());
    };
    if kaeso_core::email::same_address(stored, email) {
        return Ok(());
    }
    let domain = crate::services::managed::domain_of(stored);
    let managed_here = oidc::manager_of_domain(&mut **tx, &domain)
        .await?
        .is_some_and(|m| m.org_id == org.uuid());
    if !managed_here {
        return Ok(());
    }
    if !users::set_email(&mut **tx, user.id, email, ctx.now).await? {
        tracing::warn!(user = %user.id, "the identity provider sent an address another account holds");
        return Ok(());
    }
    let id = UserId::from_uuid(user.id);
    ctx.audit(Some(org), "user.email_change")
        .by(id)
        .target("user", id)
        .before(serde_json::json!({ "email": stored }))
        .after(serde_json::json!({ "email": email, "by": "identity_provider", "issuer": issuer }))
        .write(tx)
        .await?;
    Ok(())
}

// The provider may reuse its own older sign-in, so the age comes from its auth_time against the
// token's iat (one clock); without auth_time the age is unknown and must not count as recent.
fn signed_in_at(
    identity: &crate::auth::oidc::Identity,
    now: time::OffsetDateTime,
) -> time::OffsetDateTime {
    match (identity.auth_time, identity.issued_at) {
        (Some(signed_in), Some(issued)) => {
            now - (issued - signed_in).clamp(Duration::ZERO, ABSOLUTE_LIFETIME)
        }
        _ => now - RECENT_SIGN_IN - Duration::SECOND,
    }
}

// max_age=0 makes auth_time required in the answer (OpenID Connect Core 1.0 §2 and §3.1.2.1);
// a provider that leaves it out, or ignores max_age and reuses its own older sign-in, can't
// confirm. Both times come from the provider's clock, so a clock difference between it and Kaeso
// can't make a fresh sign-in look stale.
fn signed_in_afresh(identity: &crate::auth::oidc::Identity) -> bool {
    matches!(
        (identity.auth_time, identity.issued_at),
        (Some(signed_in), Some(issued)) if issued - signed_in <= FLOW_LIFETIME + CLOCK_SKEW
    )
}

// A confirmation proves the signed-in person is at the keyboard, so it must come back as the
// same account and from a fresh sign-in at the provider; it never creates or switches accounts.
#[allow(clippy::too_many_arguments)]
async fn confirm(
    state: &AppState,
    ctx: &Ctx,
    flow: &Flow,
    connection: &oidc::Connection,
    identity: &crate::auth::oidc::Identity,
    email: &str,
    user: UserId,
    session: SessionId,
) -> ApiResult<Response> {
    let afresh = signed_in_afresh(identity);
    if connection.confirms_sign_in != Some(afresh) {
        oidc::record_confirms_sign_in(&state.pg, connection.org_id, &connection.issuer, afresh)
            .await?;
    }
    if !afresh {
        return Err(ApiError::PermissionDenied(CANNOT_CONFIRM.into()));
    }
    let org = flow.org;
    let mut tx = state.pg.begin().await?;
    still_connected(&mut tx, connection).await?;
    let account = users::get(&mut *tx, user.uuid())
        .await?
        .filter(|u| u.state == "active")
        .ok_or_else(deactivated)?;
    match sso_identities::user_of(&mut *tx, org.uuid(), &connection.issuer, &identity.subject)
        .await?
    {
        Some(id) if id == account.id => {}
        Some(_) => return Err(other_account()),
        None if !account
            .email
            .as_deref()
            .is_some_and(|stored| kaeso_core::email::same_address(stored, email)) =>
        {
            return Err(other_account());
        }
        None => {
            if !sso_identities::bind_pending(
                &mut *tx,
                org.uuid(),
                account.id,
                &connection.issuer,
                &identity.subject,
                ctx.now,
            )
            .await?
            {
                return Err(link_required());
            }
            ctx.audit(Some(org), "user.sso_link")
                .by(user)
                .target("user", user)
                .after(serde_json::json!({ "issuer": connection.issuer, "email": email }))
                .write(&mut tx)
                .await?;
        }
    }
    if members::get_org(&mut *tx, org.uuid(), account.id)
        .await?
        .is_none()
    {
        return Err(no_access());
    }
    if !sessions_store::is_live(&mut *tx, session.uuid(), ctx.now).await? {
        return Err(expired());
    }
    let created = sessions::create(
        &mut tx,
        ctx.now,
        &SignIn {
            user_id: user,
            method: Method::Oidc,
            second_factor: false,
            oidc_org: Some(org.uuid()),
            signed_in_at: signed_in_at(identity, ctx.now),
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await?;
    ctx.audit(Some(org), "user.sign_in_confirm")
        .by(user)
        .target("user", user)
        .after(serde_json::json!({ "method": "oidc", "issuer": connection.issuer }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    sessions::replace(state, user, session).await?;
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
    let created = users::insert(
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
    if !created {
        return Err(link_required());
    }
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
    still_connected(&mut tx, connection).await?;
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

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn identity(auth_time: Option<time::OffsetDateTime>) -> crate::auth::oidc::Identity {
        crate::auth::oidc::Identity {
            subject: "sub".into(),
            email: "ada@acme.example".into(),
            name: None,
            auth_time,
            issued_at: Some(datetime!(2026-10-21 12:00 UTC)),
        }
    }

    fn signed_in(auth_time: Option<time::OffsetDateTime>) -> time::OffsetDateTime {
        signed_in_at(&identity(auth_time), datetime!(2026-10-21 18:30 UTC))
    }

    #[test]
    fn only_a_sign_in_during_the_confirmation_confirms() {
        assert!(signed_in_afresh(&identity(Some(
            datetime!(2026-10-21 11:49 UTC)
        ))));
        assert!(
            !signed_in_afresh(&identity(Some(datetime!(2026-10-21 11:48:59 UTC)))),
            "a sign-in from before the confirmation began is the provider's own, reused"
        );
        assert!(!signed_in_afresh(&identity(None)));
        let mut unissued = identity(Some(datetime!(2026-10-21 12:00 UTC)));
        unissued.issued_at = None;
        assert!(!signed_in_afresh(&unissued));
    }

    #[test]
    fn an_sso_session_dates_from_the_providers_sign_in() {
        assert_eq!(
            signed_in(Some(datetime!(2026-10-21 11:00 UTC))),
            datetime!(2026-10-21 17:30 UTC),
            "an hour before the token on the provider's clock is an hour before now"
        );
        assert_eq!(
            signed_in(Some(datetime!(2026-10-21 12:05 UTC))),
            datetime!(2026-10-21 18:30 UTC),
            "a sign-in after the token can't be in the future"
        );
        assert_eq!(
            signed_in(Some(datetime!(2026-07-01 12:00 UTC))),
            datetime!(2026-09-21 18:30 UTC),
            "no older than a session lives"
        );
        assert_eq!(
            signed_in(None),
            datetime!(2026-10-21 18:14:59 UTC),
            "without auth_time it isn't recent"
        );
    }
}
