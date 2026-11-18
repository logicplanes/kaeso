//! Linking an organization's own account at a provider as a provider key (linked billing), through
//! the same attempt endpoints as account sign-ins: `POST /orgs/{org}/credentials/oauth` with
//! `linked_billing`, then the address the provider sent the browser back to.

use kaeso_core::id::CredentialId;
use kaeso_core::linked_billing::LinkedBilling;
use kaeso_core::provider::{AuthMethod, Provider};
use kaeso_core::secret;
use kaeso_store::pg::credentials as store;
use kaeso_store::pg::linked_billing as links;
use kaeso_store::pg::tokens::{self, NewToken, TokenKind};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use url::Url;
use uuid::Uuid;

use super::credential_accounts::AccountAttempt;
use super::credentials::{Added, Auth, CredentialView, add, rewrite, view};
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::services::credentials as service;
use crate::services::linked_billing::{self as link, LinkError, Target};
use crate::services::provider_accounts::{Connection, Tokens};
use crate::state::AppState;

const ATTEMPT_LIFETIME: Duration = Duration::minutes(10);
const CALLBACK_URL: &str = "callback_url";

const EXPIRED: &str = "This sign-in expired or was already used; start again.";
const NOT_CALLBACK: &str = "The address isn't this install's linked billing callback; start again.";
const DECLINED: &str =
    "Linking didn't complete; sign in with an account that has the Admin or Billing role.";

fn not_set_up(provider: &Provider) -> ApiError {
    ApiError::bad_request(format!(
        "Linked billing for {} isn't set up on this install; ask an instance admin.",
        provider.name
    ))
}

fn one_link(provider: &Provider) -> ApiError {
    ApiError::Conflict(format!(
        "This organization already has a linked {} key; link it again or unlink it.",
        provider.name
    ))
}

fn already_linked(offer: &LinkedBilling) -> ApiError {
    ApiError::invalid(
        CALLBACK_URL,
        format!(
            "That {} is already linked to another organization on this install.",
            offer.organization
        ),
    )
}

fn app_refused(provider: &Provider) -> ApiError {
    ApiError::bad_request(format!(
        "{} refused the install's OAuth app; ask an instance admin to check it.",
        provider.name
    ))
}

fn no_refresh(provider: &Provider) -> ApiError {
    ApiError::bad_request(format!(
        "{} issued no refresh token; ask an instance admin to turn on Offline access.",
        provider.name
    ))
}

fn unreachable(provider: &Provider) -> ApiError {
    ApiError::Unavailable(format!(
        "{} can't be reached right now; try again shortly.",
        provider.name
    ))
}

fn refused(provider: &Provider) -> ApiError {
    ApiError::invalid(
        CALLBACK_URL,
        format!("{} refused the sign-in; start again.", provider.name),
    )
}

fn failed(provider: &Provider, e: LinkError) -> ApiError {
    tracing::info!(error = %e, "a link at the provider failed");
    match e {
        LinkError::Store(e) => e.into(),
        LinkError::NoApp => not_set_up(provider),
        LinkError::Unreachable(_) | LinkError::TimedOut => unreachable(provider),
        LinkError::Refused { ref code, .. } if code == "user_profile_already_linked" => {
            match provider.linked_billing() {
                Some(offer) => already_linked(offer),
                None => refused(provider),
            }
        }
        e if e.is_setup() => app_refused(provider),
        LinkError::Crypto(e) => ApiError::internal(e),
        _ => refused(provider),
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Pending {
    org_id: Uuid,
    provider: String,
    name: String,
    make_default: bool,
    credential: Option<Uuid>,
    client_id: String,
    user_profile_id: String,
    verifier: String,
}

pub(super) fn offers(provider: &Provider) -> Option<&'static LinkedBilling> {
    provider.linked_billing()
}

/// Begins a link: the organization's user profile at the provider, a pushed authorization
/// request with PKCE, and the attempt that finishes it, bound to this person and organization.
pub(super) async fn begin(
    state: &AppState,
    ctx: &Ctx,
    provider: &'static Provider,
    name: String,
    make_default: bool,
    credential: Option<Uuid>,
) -> ApiResult<AccountAttempt> {
    let org = ctx.org()?;
    let (org_id, org_name) = (org.org.id, org.org.name.clone());
    let user = ctx.user_id()?;
    if credential.is_none()
        && store::list(&state.pg, org_id).await?.iter().any(|c| {
            c.provider == provider.id && c.auth_method == AuthMethod::LinkedBilling.as_str()
        })
    {
        return Err(one_link(provider));
    }
    let app = link::app(state, provider.id)
        .await
        .map_err(|e| failed(provider, e))?;
    let profile = link::profile_of(state, &app, org_id, &org_name)
        .await
        .map_err(|e| failed(provider, e))?;
    let (verifier, challenge) = link::pkce();
    let state_value = kaeso_crypto::random_token();
    let begun = link::push(state, &app, &profile, &state_value, &challenge)
        .await
        .map_err(|e| failed(provider, e))?;
    let pending = Pending {
        org_id,
        provider: provider.id.to_owned(),
        name,
        make_default,
        credential,
        client_id: app.client_id.clone(),
        user_profile_id: profile,
        verifier,
    };
    let expires_at = ctx.now + ATTEMPT_LIFETIME;
    let attempt = tokens::insert(
        &state.pg,
        NewToken {
            kind: TokenKind::ProviderOAuth,
            hash: &secret::hash(&state_value),
            user_id: Some(user.uuid()),
            email: None,
            data: serde_json::json!({ "linked": serde_json::to_value(&pending).map_err(ApiError::internal)? }),
            expires_at,
        },
    )
    .await?;
    Ok(AccountAttempt {
        attempt: attempt.to_string(),
        authorize_url: Some(begun.authorize_url),
        device: None,
        expires_at,
        expires_in: (expires_at - ctx.now).whole_seconds().max(0),
    })
}

/// Begins linking a linked key again: its links at the provider end first, as the provider asks
/// before a profile that is still linked approves again.
pub(super) async fn begin_again(
    state: &AppState,
    ctx: &Ctx,
    provider: &'static Provider,
    before: &CredentialView,
) -> ApiResult<AccountAttempt> {
    let org = ctx.org()?.org.id;
    let row = store::sealed(&state.pg, org, before.id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("credential"))?;
    let saved = service::tokens(&state.keyring, &row).map_err(ApiError::internal)?;
    if let (Ok(app), Some(c)) = (link::app(state, provider.id).await, &saved.connection)
        && app.client_id == saved.client_id
    {
        link::revoke(state, &app, Target::Profile(&c.user_profile_id))
            .await
            .map_err(|e| failed(provider, e))?;
        if links::mark(&state.pg, before.id.uuid(), link::UNLINKED).await? {
            service::forget_org(state, org).await;
        }
    }
    begin(
        state,
        ctx,
        provider,
        before.name.clone(),
        false,
        Some(before.id.uuid()),
    )
    .await
}

/// The parts of the address the provider sent the browser back to.
struct Returned {
    state: String,
    answer: Result<String, String>,
}

fn read_callback(state: &AppState, pasted: &str) -> Option<Returned> {
    let url = Url::parse(pasted.trim()).ok()?;
    let expected = Url::parse(&link::redirect_uri(state)).ok()?;
    if url.origin() != expected.origin() || url.path() != expected.path() {
        return None;
    }
    let (mut st, mut code, mut error) = (None, None, None);
    for (k, v) in url.query_pairs() {
        let slot = match k.as_ref() {
            "state" => &mut st,
            "code" => &mut code,
            "error" => &mut error,
            _ => continue,
        };
        if slot.replace(v.into_owned()).is_some() {
            return None;
        }
    }
    let answer = match (code, error) {
        (_, Some(error)) => Err(error),
        (Some(code), None) if !code.is_empty() => Ok(code),
        _ => Err(String::new()),
    };
    Some(Returned { state: st?, answer })
}

/// Whether a pasted address is a linked billing callback of this install.
pub(super) fn is_callback(state: &AppState, pasted: &str) -> bool {
    read_callback(state, pasted).is_some() || pasted.trim().starts_with(&link::redirect_uri(state))
}

/// Finishes a link from the address the provider sent the browser back to: checks that it
/// answers this person's attempt, which is used up either way, and exchanges the code.
pub(super) async fn finish(
    state: &AppState,
    ctx: &Ctx,
    attempt: &str,
    pasted: &str,
    credential: Option<Uuid>,
) -> ApiResult<CredentialView> {
    let returned = read_callback(state, pasted)
        .ok_or_else(|| ApiError::invalid(CALLBACK_URL, NOT_CALLBACK))?;
    let attempt = Uuid::parse_str(attempt).map_err(|_| ApiError::invalid(CALLBACK_URL, EXPIRED))?;
    let hash = secret::hash(&returned.state);
    let found = tokens::peek(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(|| ApiError::invalid(CALLBACK_URL, EXPIRED))?;
    let pending: Pending = found
        .data
        .get("linked")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(|| ApiError::invalid(CALLBACK_URL, EXPIRED))?;
    let org = ctx.org()?.org.id;
    let user = ctx.user_id()?;
    // The state proves only that someone saw the approval page, so the attempt must also be
    // this person's, in this organization, for this key, before its code is redeemed.
    if found.id != attempt
        || found.user_id != Some(user.uuid())
        || pending.org_id != org
        || pending.credential != credential
    {
        return Err(ApiError::invalid(CALLBACK_URL, EXPIRED));
    }
    tokens::consume(&state.pg, TokenKind::ProviderOAuth, &hash, ctx.now)
        .await?
        .ok_or_else(|| ApiError::invalid(CALLBACK_URL, EXPIRED))?;
    let provider = super::credentials::validate_provider(&pending.provider)?;
    let offer = offers(provider).ok_or_else(|| not_set_up(provider))?;
    let code = match returned.answer {
        Ok(code) => code,
        Err(error) if error == "user_profile_already_linked" => return Err(already_linked(offer)),
        Err(error) if error == "access_denied" => {
            return Err(ApiError::invalid(CALLBACK_URL, DECLINED));
        }
        Err(error) => {
            tracing::info!(error, "the provider sent a link back without a code");
            return Err(refused(provider));
        }
    };
    let app = link::app(state, provider.id)
        .await
        .map_err(|e| failed(provider, e))?;
    if app.client_id != pending.client_id {
        return Err(ApiError::invalid(CALLBACK_URL, EXPIRED));
    }
    let granted = link::exchange(state, &app, &code, &pending.verifier)
        .await
        .map_err(|e| failed(provider, e))?;
    let connection_id = granted.connection_id.clone().unwrap_or_default();
    let Some(refresh_token) = granted.refresh_token.clone() else {
        let _ = link::revoke(state, &app, Target::Connection(&connection_id)).await;
        return Err(no_refresh(provider));
    };
    let now = state.clock.now();
    let tokens = Tokens {
        access_token: granted.access_token.clone(),
        refresh_token,
        id_token: String::new(),
        client_id: app.client_id.clone(),
        scopes: link::SCOPE.split(' ').map(str::to_owned).collect(),
        sub: pending.user_profile_id.clone(),
        earliest_refresh_at: None,
        connection: Some(Connection {
            connection_id: connection_id.clone(),
            user_profile_id: pending.user_profile_id.clone(),
            refresh_expires_at: granted.refresh_expires_at(now),
        }),
    };
    let secret = tokens.sealed_form().map_err(ApiError::internal)?;
    let saved = save(
        state,
        ctx,
        &pending,
        &secret,
        &connection_id,
        granted.expires_at(now),
    )
    .await;
    // A connection Kaeso couldn't keep would bill the organization with nobody able to see or
    // end it from Kaeso.
    if saved.is_err() {
        let _ = link::revoke(state, &app, Target::Connection(&connection_id)).await;
    }
    saved
}

async fn save(
    state: &AppState,
    ctx: &Ctx,
    pending: &Pending,
    secret: &str,
    connection_id: &str,
    expires_at: OffsetDateTime,
) -> ApiResult<CredentialView> {
    let auth = Auth {
        method: AuthMethod::LinkedBilling,
        last4: "",
        expires_at: Some(expires_at),
        account: Some(connection_id),
        project: None,
        location: None,
        scope: None,
        endpoint: None,
    };
    let org = ctx.org()?.org.id;
    match pending.credential {
        None => {
            add(
                state,
                ctx,
                org,
                Added {
                    id: CredentialId::new(),
                    provider: &pending.provider,
                    name: &pending.name,
                    make_default: pending.make_default,
                    secret,
                    auth,
                },
            )
            .await
        }
        Some(id) => {
            let mut tx = state.pg.begin().await?;
            store::lock_provider(&mut tx, org, &pending.provider).await?;
            store::lock_refresh(&mut tx, id).await?;
            let before = view(&mut tx, org, id).await?;
            if before.method() != AuthMethod::LinkedBilling {
                return Err(ApiError::invalid(CALLBACK_URL, EXPIRED));
            }
            rewrite(state, ctx, tx, &before, &before.name, secret, auth).await
        }
    }
}

/// Ends a linked key's connection at the provider before the key is deleted. A provider that
/// can't be reached keeps the key, so no link is left behind; an app that is gone or refused
/// can't end it, and the organization can still unlink at the provider.
pub(super) async fn unlink(state: &AppState, org: Uuid, before: &CredentialView) -> ApiResult<()> {
    let provider = super::credentials::validate_provider(&before.provider)?;
    let Some(row) = store::sealed(&state.pg, org, before.id.uuid()).await? else {
        return Ok(());
    };
    let saved = service::tokens(&state.keyring, &row).map_err(ApiError::internal)?;
    let Some(connection) = saved.connection else {
        return Ok(());
    };
    let app = match link::app(state, provider.id).await {
        Ok(app) if app.client_id == saved.client_id => app,
        _ => {
            tracing::warn!(credential = %before.id, "cannot end a linked key's connection: the install's app changed");
            return Ok(());
        }
    };
    match link::revoke(state, &app, Target::Connection(&connection.connection_id)).await {
        Ok(_) => Ok(()),
        Err(e @ (LinkError::Unreachable(_) | LinkError::TimedOut)) => Err(failed(provider, e)),
        Err(LinkError::Refused { status, .. }) if status >= 500 => Err(unreachable(provider)),
        Err(e) => {
            tracing::warn!(error = %e, credential = %before.id, "the provider refused to end a linked key's connection");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pending_link_keeps_no_more_than_it_needs() {
        let p = Pending {
            org_id: Uuid::nil(),
            provider: "anthropic".into(),
            name: "Claude".into(),
            make_default: false,
            credential: None,
            client_id: "c".into(),
            user_profile_id: "uprof_1".into(),
            verifier: "v".into(),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["user_profile_id"], "uprof_1");
        let back: Pending = serde_json::from_value(v).unwrap();
        assert_eq!(back.client_id, "c");
    }
}
