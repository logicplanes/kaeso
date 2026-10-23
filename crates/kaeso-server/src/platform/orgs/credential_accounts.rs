use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{CredentialId, OrgId, UserId};
use kaeso_core::provider::{AccountSignIn, AuthMethod, Provider};
use kaeso_core::secret;
use kaeso_store::pg::credentials as store;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
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
    FIRST_CLIENT, Hosts, Tokens,
};
use crate::state::AppState;

const ATTEMPT_LIFETIME: Duration = Duration::minutes(10);
const MAX_ACCOUNT: usize = 320;

const EXPIRED: &str = "This sign-in expired or was already used; start again.";
const NOT_CALLBACK: &str =
    "Paste the address of the page that didn't load; it starts with `http://127.0.0.1`.";
const REPEATED: &str = "The pasted address repeats a parameter; paste it as the browser shows it.";
const NOT_ACCOUNT: &str = "This key is an API key; replace it instead of signing in again.";

fn no_sign_in(provider: &Provider) -> ApiError {
    ApiError::invalid(
        "provider",
        format!(
            "{provider} has no account sign-in; add an API key for it.",
            provider = provider.name
        ),
    )
}

fn expired() -> ApiError {
    ApiError::invalid("callback_url", EXPIRED)
}

fn incomplete(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("The answer from {product} is incomplete; start again."),
    )
}

fn denied(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("You declined the sign-in at {product}; start again."),
    )
}

fn refused(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("{product} refused the sign-in; start again."),
    )
}

fn unreachable(product: &str) -> ApiError {
    ApiError::Unavailable(format!(
        "{product} can't be reached right now; try again shortly."
    ))
}

fn unverified(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("The answer from {product} could not be verified; start again."),
    )
}

fn no_plan(product: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("The account didn't allow using its {product} plan; start again and allow it."),
    )
}

fn other_account(product: &str, account: &str) -> ApiError {
    ApiError::invalid(
        "callback_url",
        format!("You signed in to another {product} account; sign in as {account}."),
    )
}

fn failed(product: &str, e: &AccountError) -> ApiError {
    tracing::info!(error = %e, "an account sign-in failed at the provider");
    match e {
        AccountError::Unreachable(_) => unreachable(product),
        AccountError::Refused { .. } => refused(product),
        AccountError::Unverified(_) => unverified(product),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A provider account to sign in to as a new provider key.")]
pub struct AccountStart {
    #[doc = "The provider: `openai`, whose accounts are ChatGPT accounts."]
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
#[schema(description = "A sign-in waiting for the provider's answer.")]
pub struct AccountAttempt {
    #[doc = "The attempt, to finish it with."]
    #[schema(example = "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5b")]
    pub attempt: String,
    #[doc = "The provider's sign-in page; open it in a browser where the account's\n\
        owner can sign in."]
    pub authorize_url: String,
    #[doc = "When the attempt expires, ten minutes after it began."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The provider's answer.")]
pub struct AccountFinish {
    #[doc = "The address the provider sent the browser to after the sign-in, as the\n\
        address bar shows it: `http://127.0.0.1:<port>/auth/callback?…`. The page\n\
        doesn't load, because nothing listens there."]
    #[schema(example = "http://127.0.0.1:51234/auth/callback?code=…&state=…")]
    pub callback_url: String,
}

impl std::fmt::Debug for AccountFinish {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountFinish").finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct Pending {
    org_id: Uuid,
    provider: String,
    name: String,
    make_default: bool,
    credential: Option<Uuid>,
    port: u16,
    nonce: String,
    pkce_verifier: String,
    client_id: Option<String>,
    sub: Option<String>,
    account: Option<String>,
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

async fn begin(
    state: &AppState,
    ctx: &Ctx,
    user: UserId,
    sign_in: AccountSignIn,
    mut pending: Pending,
) -> ApiResult<Json<AccountAttempt>> {
    let hosts = hosts(state, sign_in)?;
    let host_id = accounts::agent_host_id(state).await?;
    let begun = accounts::begin(
        hosts,
        &host_id,
        pending.client_id.as_deref(),
        pending.account.as_deref(),
    )
    .map_err(|e| failed(sign_in.product, &e))?;
    pending.port = begun.port;
    pending.nonce = begun.nonce;
    pending.pkce_verifier = begun.verifier;
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
        authorize_url: begun.url,
        expires_at,
    }))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/oauth",
    summary = "Begins signing in to a provider account, such as a ChatGPT account, to\n\
    add it as a provider key.",
    description = "Open `authorize_url` in a browser. Once the account's owner approves,\n\
    the provider sends the browser to `http://127.0.0.1:<port>/auth/callback`,\n\
    which doesn't load; send that address to\n\
    `POST /orgs/{org}/credentials/oauth/{attempt}` within ten minutes. Only\n\
    the person who began the sign-in can finish it, so it needs a console\n\
    session.",
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
    let pending = Pending {
        org_id: org,
        provider: provider.id.to_owned(),
        name,
        make_default: req.make_default,
        credential: None,
        port: 0,
        nonce: String::new(),
        pkce_verifier: String::new(),
        client_id: None,
        sub: None,
        account: None,
    };
    begin(&state, &ctx, user, sign_in, pending).await
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
    let pending = Pending {
        org_id: org,
        provider: provider.id.to_owned(),
        name: before.name.clone(),
        make_default: false,
        credential: Some(id),
        port: 0,
        nonce: String::new(),
        pkce_verifier: String::new(),
        client_id: Some(saved.client_id),
        sub: Some(saved.sub),
        account: before.account.clone(),
    };
    begin(&state, &ctx, user, sign_in, pending).await
}

async fn take(
    state: &AppState,
    ctx: &Ctx,
    attempt: &str,
    pasted: &str,
    credential: Option<Uuid>,
) -> ApiResult<(Pending, Callback)> {
    let org = ctx.org()?.org.id;
    let user = ctx.user_id()?;
    let callback = accounts::read_callback(pasted).map_err(|e| match e {
        CallbackError::NotCallback => ApiError::invalid("callback_url", NOT_CALLBACK),
        CallbackError::Repeated => ApiError::invalid("callback_url", REPEATED),
    })?;
    let attempt = Uuid::parse_str(attempt).map_err(|_| expired())?;
    let hash = secret::hash(&callback.state);
    let found = tokens::peek(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(expired)?;
    let pending: Pending = serde_json::from_value(found.data).map_err(|_| expired())?;
    // The state alone proves only that someone saw the authorization URL, so the attempt
    // must also be this person's, in this org, for this key, before its code is redeemed.
    if found.id != attempt
        || found.user_id != Some(user.uuid())
        || pending.org_id != org
        || pending.credential != credential
    {
        return Err(expired());
    }
    if pending.port != callback.port {
        return Err(ApiError::invalid("callback_url", NOT_CALLBACK));
    }
    tokens::consume(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(expired)?;
    Ok((pending, callback))
}

struct Signed {
    tokens: Tokens,
    account: String,
    expires_at: OffsetDateTime,
}

async fn signed_in(
    state: &AppState,
    ctx: &Ctx,
    sign_in: AccountSignIn,
    pending: &Pending,
    answer: Answer,
) -> ApiResult<Signed> {
    let product = sign_in.product;
    let (code, given) = match answer {
        Answer::Code { code, client_id } => (code, client_id),
        Answer::Denied => return Err(denied(product)),
        Answer::Incomplete => return Err(incomplete(product)),
        Answer::Refused(error) => {
            tracing::info!(error = %error, "the provider refused an account sign-in");
            return Err(refused(product));
        }
    };
    let client_id = match (&pending.client_id, given) {
        (Some(saved), Some(given)) if *saved != given => return Err(incomplete(product)),
        (Some(saved), _) => saved.clone(),
        (None, Some(given)) if given != FIRST_CLIENT => given,
        (None, _) => return Err(incomplete(product)),
    };
    let hosts = hosts(state, sign_in)?;
    let granted = accounts::exchange(
        hosts,
        &client_id,
        &code,
        &pending.pkce_verifier,
        pending.port,
    )
    .await
    .map_err(|e| failed(product, &e))?;
    if !granted.scopes().iter().any(|s| s == CHATGPT_PLAN_SCOPE) {
        return Err(no_plan(product));
    }
    let id_token = granted
        .id_token()
        .ok_or_else(|| incomplete(product))?
        .to_owned();
    let identity = accounts::verify(hosts, &id_token, &client_id, &pending.nonce)
        .await
        .map_err(|e| failed(product, &e))?;
    if let Some(sub) = &pending.sub
        && *sub != identity.subject
    {
        return Err(other_account(
            product,
            pending.account.as_deref().unwrap_or_default(),
        ));
    }
    let account = identity
        .email
        .filter(|e| !e.is_empty() && e.chars().count() <= MAX_ACCOUNT)
        .ok_or_else(|| incomplete(product))?;
    let expires_at = granted.expires_at(ctx.now);
    let tokens = granted
        .first_tokens(&client_id, &identity.subject)
        .map_err(|e| failed(product, &e))?;
    Ok(Signed {
        tokens,
        account,
        expires_at,
    })
}

fn sealed_form(tokens: &Tokens) -> ApiResult<String> {
    tokens.sealed_form().map_err(ApiError::internal)
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/credentials/oauth/{attempt}",
    summary = "Finishes signing in to a provider account and adds it as a provider key.",
    description = "Send the address the provider sent the browser to. Kaeso checks that\n\
    it answers this attempt, which is used up either way, exchanges the code\n\
    for tokens and checks the account's identity and its permission to use\n\
    the plan. The tokens are stored encrypted and never shown.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("attempt" = String, Path, description = "The attempt from `POST /orgs/{org}/credentials/oauth`.")),
    request_body = AccountFinish,
    responses((status = 201, body = CredentialView))
)]
pub async fn finish(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, attempt)): Path<(String, String)>,
    Body(req): Body<AccountFinish>,
) -> ApiResult<(StatusCode, Json<CredentialView>)> {
    let org = ctx.org()?.org.id;
    let (pending, callback) = take(&state, &ctx, &attempt, &req.callback_url, None).await?;
    let provider = validate_provider(&pending.provider)?;
    let sign_in = sign_in_of(provider, None).ok_or_else(|| no_sign_in(provider))?;
    let signed = signed_in(&state, &ctx, sign_in, &pending, callback.answer).await?;
    let secret = sealed_form(&signed.tokens)?;
    let view = add(
        &state,
        &ctx,
        org,
        Added {
            id: CredentialId::new(),
            provider: provider.id,
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
    .await?;
    Ok((StatusCode::CREATED, Json(view)))
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
    responses((status = 200, body = CredentialView))
)]
pub async fn finish_again(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id, attempt)): Path<(String, String, String)>,
    Body(req): Body<AccountFinish>,
) -> ApiResult<Json<CredentialView>> {
    let org = ctx.org()?.org.id;
    let id = parse_id(&id)?;
    let (pending, callback) = take(&state, &ctx, &attempt, &req.callback_url, Some(id)).await?;
    let provider = validate_provider(&pending.provider)?;
    let sign_in = sign_in_of(provider, None).ok_or_else(|| no_sign_in(provider))?;
    let signed = signed_in(&state, &ctx, sign_in, &pending, callback.answer).await?;
    let secret = sealed_form(&signed.tokens)?;
    let mut tx = state.pg.begin().await?;
    let before = view(&mut tx, org, id).await?;
    if before.method() != sign_in.method {
        return Err(ApiError::bad_request(NOT_ACCOUNT));
    }
    let after = rewrite(
        &state,
        &ctx,
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
    Ok(Json(after))
}

pub(super) struct Revocable {
    hosts: Hosts,
    tokens: Tokens,
}

impl Revocable {
    pub(super) async fn revoke(self) {
        match accounts::revoke(&self.hosts, &self.tokens).await {
            Ok(()) => tracing::info!("ended a deleted account's session at the provider"),
            Err(e) => tracing::warn!(
                error = %e,
                "cannot end a deleted account's session at the provider; its owner can disconnect Kaeso there"
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
    let hosts = state.accounts.hosts(before.method())?.clone();
    let row = match store::sealed(&mut *tx, org, before.id.uuid()).await {
        Ok(row) => row?,
        Err(e) => {
            tracing::warn!(error = %e, "cannot read a deleted account's tokens to end its session");
            return None;
        }
    };
    match service::tokens(&state.keyring, &row) {
        Ok(tokens) => Some(Revocable { hosts, tokens }),
        Err(e) => {
            tracing::warn!(error = %e, "cannot open a deleted account's tokens to end its session");
            None
        }
    }
}
