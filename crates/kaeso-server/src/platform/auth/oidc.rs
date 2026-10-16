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

use super::throttle_ip;
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
#[schema(description = "Which org to sign in to.")]
pub struct StartRequest {
    #[doc = "The org, from its sign-in link."]
    pub org: Option<OrgId>,
    #[doc = "Or the user's email address, whose domain names the org."]
    #[schema(example = "ada@acme.example")]
    pub email: Option<String>,
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
    if !state.config.auth.oidc {
        return Err(ApiError::PermissionDenied(
            "SSO sign-in is turned off on this install.".into(),
        ));
    }
    throttle_ip(&state, &ctx, TOKENS_PER_IP).await?;
    let no_sso = || {
        ApiError::NotFound(
            "No organization uses SSO for this sign-in; use another way to sign in.".into(),
        )
    };
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
    let connection = oidc::get(&state.pg, org.uuid()).await?.ok_or_else(no_sso)?;
    let provider = provider(&state, org, &connection).await?;
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
            }),
            expires_at: ctx.now + FLOW_LIFETIME,
        },
    )
    .await?;
    let cookie = binding_cookie(&state, &binding, FLOW_LIFETIME.whole_seconds())
        .map_err(ApiError::internal)?;
    Ok((
        [(header::SET_COOKIE, cookie)],
        axum::Json(StartResponse {
            authorization_url: started.url,
        }),
    )
        .into_response())
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

fn finish_redirect(state: &AppState, error: Option<&str>, clear_binding: bool) -> Response {
    let location = match error {
        None => state.config.public_url.to_string(),
        Some(message) => format!(
            "{}sign-in#sso_error={}",
            state.config.public_url,
            percent_encoding::utf8_percent_encode(message, FRAGMENT)
        ),
    };
    let mut response = (StatusCode::SEE_OTHER, [(header::LOCATION, location)]).into_response();
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
    let outcome = match cb {
        Ok(Query(cb)) => complete(&state, &ctx, &headers, cb).await,
        Err(e) => Err(e),
    };
    match outcome {
        Ok(response) => response,
        Err(e) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "oidc", "reason" => e.kind()).increment(1);
            tracing::info!(error = %e, "SSO sign-in failed");
            let message = match &e {
                ApiError::Internal(_) => "Something went wrong; try again.".to_owned(),
                other => other.to_string(),
            };
            finish_redirect(&state, Some(&message), true)
        }
    }
}

fn no_access() -> ApiError {
    ApiError::PermissionDenied(
        "You have no access to this organization yet; ask an admin to invite you.".into(),
    )
}

async fn complete(
    state: &AppState,
    ctx: &Ctx,
    headers: &HeaderMap,
    cb: Callback,
) -> ApiResult<Response> {
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
    let expired = || {
        ApiError::Authentication(
            "The SSO sign-in expired or began in another browser; start again.".into(),
        )
    };
    let flow = tokens::consume(
        &state.pg,
        TokenKind::OidcState,
        &secret::hash(&flow_state),
        ctx.now,
    )
    .await?
    .ok_or_else(expired)?;
    let binding = binding_from(state, headers).ok_or_else(expired)?;
    let field = |k: &str| flow.data.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    if field("binding").as_deref() != Some(crate::cache::hex(&secret::hash(&binding)).as_str()) {
        return Err(expired());
    }
    let org: OrgId = field("org_id")
        .and_then(|o| o.parse().ok())
        .ok_or_else(expired)?;
    let (Some(nonce), Some(verifier)) = (field("nonce"), field("pkce_verifier")) else {
        return Err(expired());
    };
    let connection = oidc::get(&state.pg, org.uuid())
        .await?
        .ok_or_else(expired)?;
    let identity = provider(state, org, &connection)
        .await?
        .finish(&code, &verifier, &nonce)
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
            connection.domains.join(", ")
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
                .clone()
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
            ip: ctx.ip,
            user_agent: ctx.user_agent.clone(),
        },
    )
    .await?;
    tx.commit().await?;
    let mut response = finish_redirect(state, None, true);
    let cookie =
        cookies::set(state, &created.token).ok_or_else(|| ApiError::internal("session cookie"))?;
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(response)
}
