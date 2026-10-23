use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kaeso_core::id::{CredentialId, OrgId, UserId};
use kaeso_core::provider::{AccountSignIn, AuthMethod, Provider};
use kaeso_core::secret;
use kaeso_store::pg::credentials as store;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use super::credentials::{
    Added, Auth, CredentialView, add, parse_id, rewrite, validate_provider, view,
};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::platform::keys::validate_name;
use crate::platform::params::Path;
use crate::services::credentials as service;
use crate::services::provider_accounts::{
    self as accounts, AccountError, Answer, CHATGPT_PLAN_SCOPE, Callback, CallbackError,
    FIRST_CLIENT, GROK_CLIENT, Granted, Hosts, Identity, Polled, SLOW_DOWN_STEP, Tokens,
};
use crate::state::AppState;

const ATTEMPT_LIFETIME: Duration = Duration::minutes(10);
const MAX_ACCOUNT: usize = 320;
const CALLBACK_URL: Option<&str> = Some("callback_url");

const EXPIRED: &str = "This sign-in expired or was already used; start again.";
const NOT_CALLBACK: &str =
    "Paste the address of the page that didn't load; it starts with `http://127.0.0.1`.";
const REPEATED: &str = "The pasted address repeats a parameter; paste it as the browser shows it.";
const NOT_ACCOUNT: &str = "This key is an API key; replace it instead of signing in again.";

fn refuse(param: Option<&str>, message: impl Into<String>) -> ApiError {
    match param {
        Some(param) => ApiError::invalid(param, message),
        None => ApiError::bad_request(message),
    }
}

fn no_sign_in(provider: &Provider) -> ApiError {
    ApiError::invalid(
        "provider",
        format!(
            "{provider} has no account sign-in; add an API key for it.",
            provider = provider.name
        ),
    )
}

fn expired(param: Option<&str>) -> ApiError {
    refuse(param, EXPIRED)
}

fn incomplete(param: Option<&str>, product: &str) -> ApiError {
    refuse(
        param,
        format!("The answer from {product} is incomplete; start again."),
    )
}

fn denied(param: Option<&str>, product: &str) -> ApiError {
    refuse(
        param,
        format!("You declined the sign-in at {product}; start again."),
    )
}

fn refused(param: Option<&str>, product: &str) -> ApiError {
    refuse(
        param,
        format!("{product} refused the sign-in; start again."),
    )
}

fn unreachable(product: &str) -> ApiError {
    ApiError::Unavailable(format!(
        "{product} can't be reached right now; try again shortly."
    ))
}

fn unverified(param: Option<&str>, product: &str) -> ApiError {
    refuse(
        param,
        format!("The answer from {product} could not be verified; start again."),
    )
}

fn no_plan(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("The account didn't allow using its {product} plan; start again and allow it."),
    )
}

fn no_api_access(product: &str) -> ApiError {
    ApiError::bad_request(format!(
        "This {product} plan has no API access; add an API key instead."
    ))
}

fn other_account(param: Option<&str>, product: &str, account: &str) -> ApiError {
    refuse(
        param,
        format!("You signed in to another {product} account; sign in as {account}."),
    )
}

fn failed(param: Option<&str>, product: &str, e: &AccountError) -> ApiError {
    tracing::info!(error = %e, "an account sign-in failed at the provider");
    match e {
        AccountError::Unreachable(_) => unreachable(product),
        AccountError::Refused { .. } => refused(param, product),
        AccountError::Unverified(_) => unverified(param, product),
        AccountError::NoApiAccess => no_api_access(product),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A provider account to sign in to as a new provider key.")]
pub struct AccountStart {
    #[doc = "The provider: `openai` for a ChatGPT account, `xai` for a Grok account."]
    #[schema(value_type = kaeso_core::provider::ProviderId, example = "openai")]
    pub provider: String,
    #[doc = "A name to recognise the key by."]
    #[schema(example = "Ada's ChatGPT plan")]
    pub name: String,
    #[doc = "Make it the default for its provider. The first key of a provider\n\
        becomes the default either way."]
    #[serde(default)]
    pub make_default: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A sign-in waiting for the account's owner.")]
pub struct AccountAttempt {
    #[doc = "The attempt, to finish it with."]
    #[schema(example = "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5b")]
    pub attempt: String,
    #[doc = "For a ChatGPT account, the provider's sign-in page: open it in a browser\n\
        where the account's owner can sign in; `null` for a device sign-in."]
    pub authorize_url: Option<String>,
    #[doc = "For a Grok account, the code its owner confirms at the provider; `null`\n\
        for a ChatGPT account."]
    pub device: Option<DeviceSignIn>,
    #[doc = "When the attempt expires, at most ten minutes after it began."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[doc = "Seconds until the attempt expires, counted from this answer, for a\n\
        countdown that doesn't depend on the caller's clock."]
    #[schema(example = 600)]
    pub expires_in: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A code the account's owner confirms on another device.")]
pub struct DeviceSignIn {
    #[doc = "The code to show the account's owner."]
    #[schema(example = "FKFF-E5EP")]
    pub user_code: String,
    #[doc = "The provider's page where the owner signs in and enters the code."]
    #[schema(example = "https://accounts.x.ai/oauth2/device")]
    pub verification_uri: String,
    #[doc = "The same page with the code filled in, when the provider offers one."]
    #[schema(example = "https://accounts.x.ai/oauth2/device?user_code=FKFF-E5EP")]
    pub verification_uri_complete: Option<String>,
    #[doc = "Seconds to wait between calls that finish the sign-in."]
    #[schema(example = 5)]
    pub interval: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A device sign-in the account's owner hasn't confirmed yet.")]
pub struct AccountPending {
    #[doc = "Seconds to wait before asking again; it grows when the provider asks\n\
        Kaeso to slow down."]
    #[schema(example = 5)]
    pub interval: i64,
    #[doc = "When the attempt expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[doc = "Seconds until the attempt expires, counted from this answer."]
    #[schema(example = 540)]
    pub expires_in: i64,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The provider's answer, or `{}` to check a device sign-in.")]
pub struct AccountFinish {
    #[doc = "For a ChatGPT account, the address the provider sent the browser to\n\
        after the sign-in, as the address bar shows it:\n\
        `http://127.0.0.1:<port>/auth/callback?…`. The page doesn't load, because\n\
        nothing listens there. Leave it out for a Grok account."]
    #[schema(example = "http://127.0.0.1:51234/auth/callback?code=…&state=…")]
    #[serde(default)]
    pub callback_url: Option<String>,
}

impl std::fmt::Debug for AccountFinish {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountFinish").finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct SealedCode {
    ciphertext: String,
    nonce: String,
    wrapped_key: String,
    key_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "flow", rename_all = "snake_case")]
enum Flow {
    Callback {
        port: u16,
        nonce: String,
        pkce_verifier: String,
    },
    Device {
        device_code: SealedCode,
        interval: i64,
        #[serde(with = "time::serde::rfc3339")]
        poll_after: OffsetDateTime,
    },
}

#[derive(Serialize, Deserialize)]
struct Pending {
    org_id: Uuid,
    provider: String,
    name: String,
    make_default: bool,
    credential: Option<Uuid>,
    client_id: Option<String>,
    sub: Option<String>,
    account: Option<String>,
    #[serde(flatten)]
    flow: Flow,
}

struct Wanted {
    org_id: Uuid,
    provider: &'static str,
    name: String,
    make_default: bool,
    credential: Option<Uuid>,
    client_id: Option<String>,
    sub: Option<String>,
    account: Option<String>,
}

impl Wanted {
    fn pending(self, flow: Flow) -> Pending {
        Pending {
            org_id: self.org_id,
            provider: self.provider.to_owned(),
            name: self.name,
            make_default: self.make_default,
            credential: self.credential,
            client_id: self.client_id,
            sub: self.sub,
            account: self.account,
            flow,
        }
    }
}

fn sign_in_of(provider: &Provider, method: Option<AuthMethod>) -> Option<AccountSignIn> {
    provider
        .account
        .filter(|a| method.is_none_or(|m| a.method == m))
}

fn hosts(state: &AppState, sign_in: AccountSignIn) -> ApiResult<&Hosts> {
    state
        .accounts
        .hosts(sign_in.method)
        .ok_or_else(|| ApiError::internal("no sign-in hosts for an account method"))
}

fn seconds_until(ctx: &Ctx, at: OffsetDateTime) -> i64 {
    (at - ctx.now).whole_seconds().max(0)
}

fn device_aad(org: Uuid, user: UserId) -> Vec<u8> {
    format!("provider_device:{org}:{}", user.uuid()).into_bytes()
}

async fn begin(
    state: &AppState,
    ctx: &Ctx,
    user: UserId,
    sign_in: AccountSignIn,
    wanted: Wanted,
) -> ApiResult<Json<AccountAttempt>> {
    match sign_in.method {
        AuthMethod::Grok => begin_device(state, ctx, user, sign_in, wanted).await,
        AuthMethod::ChatGpt | AuthMethod::ApiKey => {
            begin_callback(state, ctx, user, sign_in, wanted).await
        }
    }
}

async fn begin_callback(
    state: &AppState,
    ctx: &Ctx,
    user: UserId,
    sign_in: AccountSignIn,
    wanted: Wanted,
) -> ApiResult<Json<AccountAttempt>> {
    let hosts = hosts(state, sign_in)?;
    let host_id = accounts::agent_host_id(state).await?;
    let begun = accounts::begin(
        hosts,
        &host_id,
        wanted.client_id.as_deref(),
        wanted.account.as_deref(),
    )
    .map_err(|e| failed(CALLBACK_URL, sign_in.product, &e))?;
    let pending = wanted.pending(Flow::Callback {
        port: begun.port,
        nonce: begun.nonce,
        pkce_verifier: begun.verifier,
    });
    let expires_at = ctx.now + ATTEMPT_LIFETIME;
    let attempt = tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::ProviderOAuth,
            hash: &secret::hash(&begun.state),
            user_id: Some(user.uuid()),
            email: None,
            data: serde_json::to_value(&pending).map_err(ApiError::internal)?,
            expires_at,
        },
    )
    .await?;
    Ok(Json(AccountAttempt {
        attempt: attempt.to_string(),
        authorize_url: Some(begun.url),
        device: None,
        expires_at,
        expires_in: seconds_until(ctx, expires_at),
    }))
}

async fn begin_device(
    state: &AppState,
    ctx: &Ctx,
    user: UserId,
    sign_in: AccountSignIn,
    wanted: Wanted,
) -> ApiResult<Json<AccountAttempt>> {
    let hosts = hosts(state, sign_in)?;
    let code = accounts::device_code(hosts)
        .await
        .map_err(|e| failed(None, sign_in.product, &e))?;
    let interval = code.interval.unwrap_or(accounts::DEFAULT_INTERVAL);
    let expires_at = ctx.now + ATTEMPT_LIFETIME.min(Duration::seconds(code.expires_in));
    let sealed = kaeso_crypto::seal(
        &state.keyring,
        code.device_code.as_bytes(),
        &device_aad(wanted.org_id, user),
    )
    .map_err(ApiError::internal)?;
    let pending = wanted.pending(Flow::Device {
        device_code: SealedCode {
            ciphertext: STANDARD.encode(&sealed.ciphertext),
            nonce: STANDARD.encode(&sealed.nonce),
            wrapped_key: STANDARD.encode(&sealed.wrapped_key),
            key_id: sealed.key_id,
        },
        interval,
        poll_after: ctx.now + Duration::seconds(interval),
    });
    let attempt = tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::ProviderDevice,
            hash: &secret::hash(&code.device_code),
            user_id: Some(user.uuid()),
            email: None,
            data: serde_json::to_value(&pending).map_err(ApiError::internal)?,
            expires_at,
        },
    )
    .await?;
    Ok(Json(AccountAttempt {
        attempt: attempt.to_string(),
        authorize_url: None,
        device: Some(DeviceSignIn {
            user_code: code.user_code,
            verification_uri: code.verification_uri,
            verification_uri_complete: code.verification_uri_complete,
            interval,
        }),
        expires_at,
        expires_in: seconds_until(ctx, expires_at),
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/oauth",
    summary = "Begins signing in to a provider account, a ChatGPT or a Grok account, to\n\
    add it as a provider key.",
    description = "For a ChatGPT account, open `authorize_url` in a browser. Once the\n\
    account's owner approves, the provider sends the browser to\n\
    `http://127.0.0.1:<port>/auth/callback`, which doesn't load; send that\n\
    address to `POST /orgs/{org}/credentials/oauth/{attempt}` within ten\n\
    minutes. For a Grok account, show `device.user_code` and\n\
    `device.verification_uri`; the owner signs in there and enters the code,\n\
    while you call `POST /orgs/{org}/credentials/oauth/{attempt}` every\n\
    `device.interval` seconds until it adds the key. Only the person who began\n\
    the sign-in can finish it, so it needs a console session.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = AccountStart,
    responses((status = 200, body = AccountAttempt))
)]
pub async fn start(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<AccountStart>,
) -> ApiResult<Json<AccountAttempt>> {
    let org = ctx.org()?.org.id;
    let user = ctx.user_id()?;
    let provider = validate_provider(&req.provider)?;
    let sign_in = sign_in_of(provider, None).ok_or_else(|| no_sign_in(provider))?;
    let name = validate_name(&req.name)?;
    let wanted = Wanted {
        org_id: org,
        provider: provider.id,
        name,
        make_default: req.make_default,
        credential: None,
        client_id: None,
        sub: None,
        account: None,
    };
    begin(&state, &ctx, user, sign_in, wanted).await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/{credential}/oauth",
    summary = "Begins signing in again to the account of a provider key, such as after\n\
    the provider ended its session.",
    description = "Works like `POST /orgs/{org}/credentials/oauth`; finish with\n\
    `POST /orgs/{org}/credentials/{credential}/oauth/{attempt}`. The same\n\
    account must sign in.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path)),
    responses((status = 200, body = AccountAttempt))
)]
pub async fn start_again(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<Json<AccountAttempt>> {
    let org = ctx.org()?.org.id;
    let user = ctx.user_id()?;
    let id = parse_id(&id)?;
    let mut db = state.pg.acquire().await?;
    let before = view(&mut db, org, id).await?;
    let provider = validate_provider(&before.provider)?;
    let sign_in = sign_in_of(provider, Some(before.method()))
        .filter(|_| before.method().is_account())
        .ok_or_else(|| ApiError::bad_request(NOT_ACCOUNT))?;
    let row = store::sealed(&mut *db, org, id)
        .await?
        .ok_or_else(|| ApiError::not_found("credential"))?;
    drop(db);
    let saved = service::tokens(&state.keyring, &row).map_err(ApiError::internal)?;
    let wanted = Wanted {
        org_id: org,
        provider: provider.id,
        name: before.name.clone(),
        make_default: false,
        credential: Some(id),
        client_id: Some(saved.client_id),
        sub: Some(saved.sub),
        account: before.account.clone(),
    };
    begin(&state, &ctx, user, sign_in, wanted).await
}

fn owned_by(
    ctx: &Ctx,
    found: &tokens::Token,
    pending: &Pending,
    attempt: Uuid,
    credential: Option<Uuid>,
) -> ApiResult<bool> {
    let org = ctx.org()?.org.id;
    let user = ctx.user_id()?;
    // A state or an attempt id proves only that someone saw the sign-in, so the attempt
    // must also be this person's, in this org, for this key, before it is redeemed.
    Ok(found.id == attempt
        && found.user_id == Some(user.uuid())
        && pending.org_id == org
        && pending.credential == credential)
}

async fn take(
    state: &AppState,
    ctx: &Ctx,
    attempt: &str,
    pasted: &str,
    credential: Option<Uuid>,
) -> ApiResult<(Pending, Callback)> {
    let callback = accounts::read_callback(pasted).map_err(|e| match e {
        CallbackError::NotCallback => ApiError::invalid("callback_url", NOT_CALLBACK),
        CallbackError::Repeated => ApiError::invalid("callback_url", REPEATED),
    })?;
    let attempt = Uuid::parse_str(attempt).map_err(|_| expired(CALLBACK_URL))?;
    let hash = secret::hash(&callback.state);
    let found = tokens::peek(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(|| expired(CALLBACK_URL))?;
    let pending: Pending =
        serde_json::from_value(found.data.clone()).map_err(|_| expired(CALLBACK_URL))?;
    if !owned_by(ctx, &found, &pending, attempt, credential)? {
        return Err(expired(CALLBACK_URL));
    }
    let Flow::Callback { port, .. } = &pending.flow else {
        return Err(expired(CALLBACK_URL));
    };
    if *port != callback.port {
        return Err(ApiError::invalid("callback_url", NOT_CALLBACK));
    }
    tokens::consume(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(|| expired(CALLBACK_URL))?;
    Ok((pending, callback))
}

struct Signed {
    tokens: Tokens,
    account: String,
    expires_at: OffsetDateTime,
}

fn account_of(
    param: Option<&str>,
    product: &str,
    pending: &Pending,
    identity: Identity,
) -> ApiResult<(String, String)> {
    if let Some(sub) = &pending.sub
        && *sub != identity.subject
    {
        return Err(other_account(
            param,
            product,
            pending.account.as_deref().unwrap_or_default(),
        ));
    }
    let account = identity
        .email
        .filter(|e| !e.is_empty() && e.chars().count() <= MAX_ACCOUNT)
        .ok_or_else(|| incomplete(param, product))?;
    Ok((identity.subject, account))
}

async fn signed_in(
    state: &AppState,
    ctx: &Ctx,
    sign_in: AccountSignIn,
    pending: &Pending,
    answer: Answer,
) -> ApiResult<Signed> {
    let product = sign_in.product;
    let Flow::Callback {
        port,
        nonce,
        pkce_verifier,
    } = &pending.flow
    else {
        return Err(expired(CALLBACK_URL));
    };
    let (code, given) = match answer {
        Answer::Code { code, client_id } => (code, client_id),
        Answer::Denied => return Err(denied(CALLBACK_URL, product)),
        Answer::Incomplete => return Err(incomplete(CALLBACK_URL, product)),
        Answer::Refused(error) => {
            tracing::info!(error = %error, "the provider refused an account sign-in");
            return Err(refused(CALLBACK_URL, product));
        }
    };
    let client_id = match (&pending.client_id, given) {
        (Some(saved), Some(given)) if *saved != given => {
            return Err(incomplete(CALLBACK_URL, product));
        }
        (Some(saved), _) => saved.clone(),
        (None, Some(given)) if given != FIRST_CLIENT => given,
        (None, _) => return Err(incomplete(CALLBACK_URL, product)),
    };
    let hosts = hosts(state, sign_in)?;
    let granted = accounts::exchange(hosts, &client_id, &code, pkce_verifier, *port)
        .await
        .map_err(|e| failed(CALLBACK_URL, product, &e))?;
    let issued = granted.refresh_token().map(|token| Revocable {
        hosts: hosts.clone(),
        method: sign_in.method,
        client_id: client_id.clone(),
        token: token.to_owned(),
        hint: "refresh_token",
    });
    let checked = checked(ctx, hosts, product, pending, nonce, &client_id, granted).await;
    if checked.is_err() {
        unsaved(issued).await;
    }
    checked
}

async fn checked(
    ctx: &Ctx,
    hosts: &Hosts,
    product: &str,
    pending: &Pending,
    nonce: &str,
    client_id: &str,
    granted: accounts::Granted,
) -> ApiResult<Signed> {
    if !granted.scopes().iter().any(|s| s == CHATGPT_PLAN_SCOPE) {
        return Err(no_plan(product));
    }
    let id_token = granted
        .id_token()
        .ok_or_else(|| incomplete(CALLBACK_URL, product))?
        .to_owned();
    let identity = accounts::verify(hosts, &id_token, client_id, nonce)
        .await
        .map_err(|e| failed(CALLBACK_URL, product, &e))?;
    let (subject, account) = account_of(CALLBACK_URL, product, pending, identity)?;
    signed(ctx, granted, client_id, &subject, account)
        .map_err(|e| failed(CALLBACK_URL, product, &e))
}

fn signed(
    ctx: &Ctx,
    granted: Granted,
    client_id: &str,
    subject: &str,
    account: String,
) -> Result<Signed, AccountError> {
    let expires_at = granted.expires_at(ctx.now);
    let tokens = granted.first_tokens(client_id, subject)?;
    Ok(Signed {
        tokens,
        account,
        expires_at,
    })
}

enum Polling {
    Waiting(AccountPending),
    Granted(Box<Pending>, Granted),
}

fn device_code(state: &AppState, pending: &Pending, user: UserId) -> ApiResult<String> {
    let Flow::Device { device_code, .. } = &pending.flow else {
        return Err(expired(None));
    };
    let bytes = |b64: &str| STANDARD.decode(b64).map_err(ApiError::internal);
    let sealed = kaeso_crypto::Sealed {
        ciphertext: bytes(&device_code.ciphertext)?,
        nonce: bytes(&device_code.nonce)?,
        wrapped_key: bytes(&device_code.wrapped_key)?,
        key_id: device_code.key_id.clone(),
    };
    let plain = kaeso_crypto::open(&state.keyring, &sealed, &device_aad(pending.org_id, user))
        .map_err(ApiError::internal)?;
    String::from_utf8(plain.expose_secret().clone()).map_err(ApiError::internal)
}

fn waiting(ctx: &Ctx, interval: i64, expires_at: OffsetDateTime) -> Polling {
    Polling::Waiting(AccountPending {
        interval,
        expires_at,
        expires_in: seconds_until(ctx, expires_at),
    })
}

struct Locked {
    pending: Pending,
    expires_at: OffsetDateTime,
}

async fn lock_device(
    tx: &mut sqlx::PgConnection,
    ctx: &Ctx,
    attempt: Uuid,
    credential: Option<Uuid>,
) -> ApiResult<Locked> {
    let found = tokens::lock_attempt(tx, attempt, ctx.now)
        .await?
        .ok_or_else(|| expired(None))?;
    let pending: Pending =
        serde_json::from_value(found.token.data.clone()).map_err(|_| expired(None))?;
    if !owned_by(ctx, &found.token, &pending, attempt, credential)? {
        return Err(expired(None));
    }
    if found.kind != TokenKind::ProviderDevice.as_str() {
        return Err(ApiError::invalid(
            "callback_url",
            "`callback_url` is required.",
        ));
    }
    Ok(Locked {
        pending,
        expires_at: found.token.expires_at,
    })
}

fn schedule(pending: &mut Pending, now: OffsetDateTime, grow: i64) -> Option<i64> {
    let Flow::Device {
        interval,
        poll_after,
        ..
    } = &mut pending.flow
    else {
        return None;
    };
    *interval += grow;
    *poll_after = now + Duration::seconds(*interval);
    Some(*interval)
}

// Each check takes the attempt's next slot before it asks xAI, and no row lock is held while
// it waits for the answer: a caller that loops, or retries after an outage, reaches the token
// endpoint at most once per interval, and a slow answer doesn't hold a pool connection.
async fn poll_device(
    state: &AppState,
    ctx: &Ctx,
    attempt: &str,
    credential: Option<Uuid>,
) -> ApiResult<Polling> {
    let user = ctx.user_id()?;
    let attempt = Uuid::parse_str(attempt).map_err(|_| expired(None))?;
    let mut tx = state.pg.begin().await?;
    let Locked {
        mut pending,
        expires_at,
    } = lock_device(&mut tx, ctx, attempt, credential).await?;
    let provider = validate_provider(&pending.provider)?;
    let sign_in = sign_in_of(provider, Some(AuthMethod::Grok)).ok_or_else(|| expired(None))?;
    let product = sign_in.product;
    let Flow::Device {
        interval,
        poll_after,
        ..
    } = &pending.flow
    else {
        return Err(expired(None));
    };
    let interval = *interval;
    if ctx.now < *poll_after {
        return Ok(waiting(ctx, interval, expires_at));
    }
    schedule(&mut pending, ctx.now, 0);
    let data = serde_json::to_value(&pending).map_err(ApiError::internal)?;
    tokens::keep_data(&mut *tx, attempt, &data).await?;
    tx.commit().await?;

    let code = device_code(state, &pending, user)?;
    let hosts = hosts(state, sign_in)?;
    let polled = match accounts::poll(hosts, &code).await {
        Ok(polled) => polled,
        Err(e @ AccountError::Unreachable(_)) => return Err(failed(None, product, &e)),
        Err(e) => {
            tokens::use_up(&state.pg, attempt, ctx.now).await?;
            return Err(failed(None, product, &e));
        }
    };
    match polled {
        Polled::Pending => Ok(waiting(ctx, interval, expires_at)),
        Polled::SlowDown => {
            let mut tx = state.pg.begin().await?;
            let Locked {
                mut pending,
                expires_at,
            } = lock_device(&mut tx, ctx, attempt, credential).await?;
            let interval =
                schedule(&mut pending, ctx.now, SLOW_DOWN_STEP).ok_or_else(|| expired(None))?;
            let data = serde_json::to_value(&pending).map_err(ApiError::internal)?;
            tokens::keep_data(&mut *tx, attempt, &data).await?;
            tx.commit().await?;
            Ok(waiting(ctx, interval, expires_at))
        }
        Polled::Denied | Polled::Expired => {
            tokens::use_up(&state.pg, attempt, ctx.now).await?;
            Err(if matches!(polled, Polled::Denied) {
                denied(None, product)
            } else {
                expired(None)
            })
        }
        Polled::Granted(granted) => {
            if tokens::use_up(&state.pg, attempt, ctx.now).await? {
                Ok(Polling::Granted(Box::new(pending), granted))
            } else {
                unsaved(Revocable::grok_grant(state, sign_in, &granted)).await;
                Err(expired(None))
            }
        }
    }
}

fn start_over(product: &str, e: &AccountError) -> ApiError {
    tracing::info!(error = %e, "a granted account sign-in could not be finished");
    match e {
        AccountError::Unreachable(_) | AccountError::Unverified(_) => unverified(None, product),
        AccountError::Refused { .. } => refused(None, product),
        AccountError::NoApiAccess => no_api_access(product),
    }
}

// xAI has issued tokens by now. Finishing in a task of its own keeps a request that goes away
// from leaving them live and unsaved, and every failure past this point ends them at xAI.
async fn settled(
    state: &AppState,
    ctx: &Ctx,
    pending: Pending,
    sign_in: AccountSignIn,
    granted: Granted,
    credential: Option<Uuid>,
) -> ApiResult<CredentialView> {
    let (done, outcome) = tokio::sync::oneshot::channel();
    let (st, cx) = (state.clone(), ctx.clone());
    state.in_flight.finish(async move {
        let issued = Revocable::grok_grant(&st, sign_in, &granted);
        let kept = keep_grant(&st, &cx, &pending, sign_in, granted, credential).await;
        if kept.is_err() {
            unsaved(issued).await;
        }
        let _ = done.send(kept);
    });
    outcome
        .await
        .map_err(|_| ApiError::internal("a granted sign-in ended without an answer"))?
}

async fn keep_grant(
    state: &AppState,
    ctx: &Ctx,
    pending: &Pending,
    sign_in: AccountSignIn,
    granted: Granted,
    credential: Option<Uuid>,
) -> ApiResult<CredentialView> {
    let product = sign_in.product;
    let hosts = hosts(state, sign_in)?;
    let identity = accounts::grok_identity(hosts, &granted)
        .await
        .map_err(|e| start_over(product, &e))?;
    let (subject, account) = account_of(None, product, pending, identity)?;
    let signed = signed(ctx, granted, GROK_CLIENT, &subject, account)
        .map_err(|e| start_over(product, &e))?;
    let provider = validate_provider(&pending.provider)?;
    match credential {
        Some(id) => save_again(state, ctx, id, provider, sign_in, &signed).await,
        None => save_new(state, ctx, pending, sign_in, &signed).await,
    }
}

fn sealed_form(tokens: &Tokens) -> ApiResult<String> {
    tokens.sealed_form().map_err(ApiError::internal)
}

pub enum Finished {
    Created(CredentialView),
    Updated(CredentialView),
    Waiting(AccountPending),
}

impl IntoResponse for Finished {
    fn into_response(self) -> Response {
        match self {
            Finished::Created(view) => (StatusCode::CREATED, Json(view)).into_response(),
            Finished::Updated(view) => (StatusCode::OK, Json(view)).into_response(),
            Finished::Waiting(pending) => (StatusCode::ACCEPTED, Json(pending)).into_response(),
        }
    }
}

enum Answered {
    Waiting(AccountPending),
    Signed(Pending, AccountSignIn, Signed),
    Granted(Pending, AccountSignIn, Granted),
}

async fn answered(
    state: &AppState,
    ctx: &Ctx,
    attempt: &str,
    callback_url: Option<&str>,
    credential: Option<Uuid>,
) -> ApiResult<Answered> {
    let Some(pasted) = callback_url else {
        return match poll_device(state, ctx, attempt, credential).await? {
            Polling::Waiting(waiting) => Ok(Answered::Waiting(waiting)),
            Polling::Granted(pending, granted) => {
                let provider = validate_provider(&pending.provider)?;
                let sign_in = sign_in_of(provider, None).ok_or_else(|| no_sign_in(provider))?;
                Ok(Answered::Granted(*pending, sign_in, granted))
            }
        };
    };
    let (pending, callback) = take(state, ctx, attempt, pasted, credential).await?;
    let provider = validate_provider(&pending.provider)?;
    let sign_in = sign_in_of(provider, None).ok_or_else(|| no_sign_in(provider))?;
    let signed = signed_in(state, ctx, sign_in, &pending, callback.answer).await?;
    Ok(Answered::Signed(pending, sign_in, signed))
}

async fn save_new(
    state: &AppState,
    ctx: &Ctx,
    pending: &Pending,
    sign_in: AccountSignIn,
    signed: &Signed,
) -> ApiResult<CredentialView> {
    let org = ctx.org()?.org.id;
    let secret = sealed_form(&signed.tokens)?;
    add(
        state,
        ctx,
        org,
        Added {
            id: CredentialId::new(),
            provider: &pending.provider,
            name: &pending.name,
            make_default: pending.make_default,
            secret: &secret,
            auth: Auth {
                method: sign_in.method,
                last4: "",
                expires_at: Some(signed.expires_at),
                account: Some(&signed.account),
            },
        },
    )
    .await
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/oauth/{attempt}",
    summary = "Finishes signing in to a provider account and adds it as a provider key.",
    description = "For a ChatGPT account, send the address the provider sent the browser\n\
    to; Kaeso checks that it answers this attempt, which is used up either way,\n\
    exchanges the code for tokens and checks the account's identity and its\n\
    permission to use the plan. For a Grok account, send `{}`: each call asks\n\
    the provider once and answers `202` while the owner hasn't confirmed the\n\
    code. The tokens are stored encrypted and never shown.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("attempt" = String, Path, description = "The attempt from `POST /orgs/{org}/credentials/oauth`.")),
    request_body = AccountFinish,
    responses(
        (status = 201, body = CredentialView),
        (status = 202, body = AccountPending, description = "The owner hasn't confirmed the code yet; ask again after `interval` seconds."),
    )
)]
pub async fn finish(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, attempt)): Path<(String, String)>,
    Body(req): Body<AccountFinish>,
) -> ApiResult<Finished> {
    let (pending, sign_in, signed) =
        match answered(&state, &ctx, &attempt, req.callback_url.as_deref(), None).await? {
            Answered::Waiting(waiting) => return Ok(Finished::Waiting(waiting)),
            Answered::Granted(pending, sign_in, granted) => {
                return Ok(Finished::Created(
                    settled(&state, &ctx, pending, sign_in, granted, None).await?,
                ));
            }
            Answered::Signed(pending, sign_in, signed) => (pending, sign_in, signed),
        };
    let added = save_new(&state, &ctx, &pending, sign_in, &signed).await;
    if added.is_err() {
        unsaved(Revocable::of(&state, sign_in, &signed.tokens)).await;
    }
    Ok(Finished::Created(added?))
}

// A session the provider opened for a key that was never saved would stay usable until it
// expires, with nobody able to see or end it from Kaeso.
async fn unsaved(issued: Option<Revocable>) {
    if let Some(issued) = issued {
        issued.revoke().await;
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/{credential}/oauth/{attempt}",
    summary = "Finishes signing in again and gives the provider key the account's new\n\
    tokens.",
    description = "Works like `POST /orgs/{org}/credentials/oauth/{attempt}`; a different\n\
    account is refused. Requests use the new tokens within seconds.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("credential" = CredentialId, Path), ("attempt" = String, Path, description = "The attempt from `POST /orgs/{org}/credentials/{credential}/oauth`.")),
    request_body = AccountFinish,
    responses(
        (status = 200, body = CredentialView),
        (status = 202, body = AccountPending, description = "The owner hasn't confirmed the code yet; ask again after `interval` seconds."),
    )
)]
pub async fn finish_again(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id, attempt)): Path<(String, String, String)>,
    Body(req): Body<AccountFinish>,
) -> ApiResult<Finished> {
    let id = parse_id(&id)?;
    let (pending, sign_in, signed) = match answered(
        &state,
        &ctx,
        &attempt,
        req.callback_url.as_deref(),
        Some(id),
    )
    .await?
    {
        Answered::Waiting(waiting) => return Ok(Finished::Waiting(waiting)),
        Answered::Granted(pending, sign_in, granted) => {
            return Ok(Finished::Updated(
                settled(&state, &ctx, pending, sign_in, granted, Some(id)).await?,
            ));
        }
        Answered::Signed(pending, sign_in, signed) => (pending, sign_in, signed),
    };
    let provider = validate_provider(&pending.provider)?;
    let saved = save_again(&state, &ctx, id, provider, sign_in, &signed).await;
    if saved.is_err() {
        unsaved(Revocable::of(&state, sign_in, &signed.tokens)).await;
    }
    Ok(Finished::Updated(saved?))
}

async fn save_again(
    state: &AppState,
    ctx: &Ctx,
    id: Uuid,
    provider: &Provider,
    sign_in: AccountSignIn,
    signed: &Signed,
) -> ApiResult<CredentialView> {
    let org = ctx.org()?.org.id;
    let secret = sealed_form(&signed.tokens)?;
    let mut tx = state.pg.begin().await?;
    // A renewal on another replica holds this lock between reading the old tokens and
    // storing their successors, so waiting here keeps it from overwriting the new sign-in.
    store::lock_provider(&mut tx, org, provider.id).await?;
    store::lock_refresh(&mut tx, id).await?;
    let before = view(&mut tx, org, id).await?;
    if before.method() != sign_in.method {
        return Err(ApiError::bad_request(NOT_ACCOUNT));
    }
    let previous = revocable(state, &mut tx, org, &before).await;
    let after = rewrite(
        state,
        ctx,
        tx,
        &before,
        &before.name,
        &secret,
        Auth {
            method: sign_in.method,
            last4: "",
            expires_at: Some(signed.expires_at),
            account: Some(&signed.account),
        },
    )
    .await?;
    if let Some(previous) = previous
        && previous.token != signed.tokens.refresh_token
    {
        previous.revoke().await;
    }
    Ok(after)
}

pub(super) struct Revocable {
    hosts: Hosts,
    method: AuthMethod,
    client_id: String,
    token: String,
    hint: &'static str,
}

impl Revocable {
    fn of(state: &AppState, sign_in: AccountSignIn, tokens: &Tokens) -> Option<Revocable> {
        Some(Revocable {
            hosts: state.accounts.hosts(sign_in.method)?.clone(),
            method: sign_in.method,
            client_id: tokens.client_id.clone(),
            token: tokens.refresh_token.clone(),
            hint: "refresh_token",
        })
    }

    fn grok_grant(
        state: &AppState,
        sign_in: AccountSignIn,
        granted: &Granted,
    ) -> Option<Revocable> {
        let (token, hint) = granted.kept_token();
        Some(Revocable {
            hosts: state.accounts.hosts(sign_in.method)?.clone(),
            method: sign_in.method,
            client_id: GROK_CLIENT.to_owned(),
            token: token.to_owned(),
            hint,
        })
    }

    pub(super) async fn revoke(self) {
        match accounts::revoke(
            &self.hosts,
            self.method,
            &self.client_id,
            &self.token,
            self.hint,
        )
        .await
        {
            Ok(()) => tracing::info!("ended an account's session at the provider"),
            Err(e) => tracing::warn!(
                error = %e,
                "cannot end an account's session at the provider; its owner can disconnect Kaeso there"
            ),
        }
    }
}

pub(super) async fn revocable(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    org: Uuid,
    before: &CredentialView,
) -> Option<Revocable> {
    let method = before.method();
    let hosts = state.accounts.hosts(method)?.clone();
    let row = match store::sealed(&mut *tx, org, before.id.uuid()).await {
        Ok(row) => row?,
        Err(e) => {
            tracing::warn!(error = %e, "cannot read an account's tokens to end its session");
            return None;
        }
    };
    match service::tokens(&state.keyring, &row) {
        Ok(tokens) => Some(Revocable {
            hosts,
            method,
            client_id: tokens.client_id,
            token: tokens.refresh_token,
            hint: "refresh_token",
        }),
        Err(e) => {
            tracing::warn!(error = %e, "cannot open an account's tokens to end its session");
            None
        }
    }
}
