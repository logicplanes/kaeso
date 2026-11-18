//! The states of linked provider keys: renewing a connection's tokens, a spend limit the linked
//! organization reached, a link that ended, and the install's app going missing.

use kaeso_core::id::{CredentialId, OrgId};
use kaeso_core::linked_billing::Refusal;
use kaeso_store::StoreError;
use kaeso_store::pg::credentials as creds;
use kaeso_store::pg::linked_billing as store;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::services::credentials::{self, ResolveError};
use crate::services::linked_billing::{self as link, ON_HOLD, Refreshed, UNLINKED};
use crate::services::provider_accounts::Connection;
use crate::state::AppState;

/// A refusal that comes right after a link is about a connection Anthropic hasn't finished
/// setting up yet, not one that ended
/// (https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections#causes-of-user-profile-not-billable).
const SETTLING: time::Duration = time::Duration::seconds(10);

/// Renews a linked key's tokens under its lock, if they are due within `ahead`. Returns whether
/// it renewed.
pub async fn renew(
    state: &AppState,
    org: OrgId,
    id: CredentialId,
    product: &'static str,
    ahead: time::Duration,
) -> Result<bool, ResolveError> {
    let (org_id, key) = (org.uuid(), id.uuid());
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    creds::lock_refresh(&mut tx, key).await?;
    let Some(row) = creds::sealed(&mut *tx, org_id, key).await? else {
        return Ok(false);
    };
    let now = state.clock.now();
    let expires_at = row.expires_at.unwrap_or(now);
    if row.auth_error.is_some()
        || now + ahead
            < expires_at
                - crate::services::provider_accounts::renew_before(
                    kaeso_core::provider::AuthMethod::LinkedBilling,
                )
    {
        tx.commit().await.map_err(StoreError::from)?;
        return Ok(false);
    }
    let tokens = credentials::tokens(&state.keyring, &row)?;
    let provider = match creds::get(&mut *tx, org_id, key).await? {
        Some(r) => r.provider,
        None => return Ok(false),
    };
    let app = match link::app(state, &provider).await {
        Ok(app) if app.client_id == tokens.client_id => app,
        Ok(_) | Err(link::LinkError::NoApp) => {
            drop(tx);
            needs_setup(state, &provider).await;
            return Err(ResolveError::NeedsSetup { product });
        }
        Err(e) => {
            tracing::warn!(error = %e, credential = %id, "cannot read the install's linked billing app");
            return Ok(false);
        }
    };
    let ended = tokens
        .connection
        .as_ref()
        .and_then(|c| c.refresh_expires_at)
        .is_some_and(|at| now >= at);
    if ended || tokens.refresh_token.is_empty() {
        drop(tx);
        unlinked(state, org_id, key).await;
        return Err(ResolveError::Unlinked { product });
    }
    // The hold is stored before the refresh token leaves Kaeso, on a connection of its own, so
    // a crash between sending it and storing its successor leaves the key held, never sending a
    // refresh token that may have been used: a late repeat ends the connection.
    store::mark(&state.pg, key, ON_HOLD).await?;
    let outcome = link::refresh(state, &app, &tokens.refresh_token).await;
    match outcome {
        Refreshed::Renewed(granted) => {
            let renewed_until = granted.expires_at(now);
            let fresh = crate::services::provider_accounts::Tokens {
                access_token: granted.access_token.clone(),
                refresh_token: granted.refresh_token.clone().unwrap_or_default(),
                connection: tokens.connection.clone().map(|c| Connection {
                    refresh_expires_at: granted.refresh_expires_at(now).or(c.refresh_expires_at),
                    ..c
                }),
                ..tokens
            };
            let json = fresh
                .sealed_form()
                .map_err(|_| kaeso_crypto::CryptoError::Malformed)?;
            let sealed = credentials::seal(state, id, &json)?;
            store::renewed(
                &mut *tx,
                key,
                &creds::Sealed {
                    ciphertext: &sealed.ciphertext,
                    nonce: &sealed.nonce,
                    wrapped_key: &sealed.wrapped_key,
                    key_id: &sealed.key_id,
                },
                renewed_until,
            )
            .await?;
            tx.commit().await.map_err(StoreError::from)?;
            credentials::forget_org(state, org_id).await;
            Ok(true)
        }
        Refreshed::Later(why) => {
            tracing::warn!(credential = %id, why, "cannot renew a linked key's access token now; its refresh token is kept");
            store::release(&state.pg, key).await?;
            Ok(false)
        }
        Refreshed::Setup(why) => {
            tracing::warn!(credential = %id, why, "the provider refused the install's linked billing app");
            store::release(&state.pg, key).await?;
            drop(tx);
            needs_setup(state, &provider).await;
            Err(ResolveError::NeedsSetup { product })
        }
        Refreshed::Ended(why) => {
            tracing::warn!(credential = %id, why, "a linked key's connection ended; it needs a new link");
            drop(tx);
            unlinked(state, org_id, key).await;
            Err(ResolveError::Unlinked { product })
        }
    }
}

async fn mark(state: &AppState, org: Uuid, key: Uuid, how: &str) -> bool {
    match store::mark(&state.pg, key, how).await {
        Ok(changed) => {
            credentials::forget_org(state, org).await;
            changed
        }
        Err(e) => {
            tracing::warn!(error = %e, "cannot record a linked key's state");
            false
        }
    }
}

/// The key's link ended: it shows **Unlinked**, and the people who manage provider keys hear
/// about it once.
pub async fn unlinked(state: &AppState, org: Uuid, key: Uuid) {
    if mark(state, org, key, UNLINKED).await {
        crate::services::notifications::system::linked_unlinked(state, org, key).await;
    }
}

/// The install's app is missing, replaced or refused: every linked key of the provider waits
/// for it, and the instance's admins hear about it once.
pub async fn needs_setup(state: &AppState, provider: &str) {
    match store::needs_setup(&state.pg, provider).await {
        Ok(marked) => {
            for k in store::linked_keys(&state.pg, provider)
                .await
                .unwrap_or_default()
            {
                state
                    .caches
                    .credentials
                    .invalidate_group(&k.org_id.to_string())
                    .await;
            }
            if marked > 0 {
                crate::services::notifications::system::linked_needs_setup(state, provider).await;
            }
        }
        Err(e) => tracing::warn!(error = %e, "cannot mark linked keys as needing setup"),
    }
}

/// The install's app is back with the client the keys were linked to.
pub async fn setup_done(state: &AppState, provider: &str) -> Result<u64, StoreError> {
    let back = store::setup_done(&state.pg, provider).await?;
    if back > 0 {
        for k in store::linked_keys(&state.pg, provider).await? {
            state
                .caches
                .credentials
                .invalidate_group(&k.org_id.to_string())
                .await;
        }
    }
    Ok(back)
}

/// What a provider's refusal of a linked key's request means for the key; called once the
/// request has failed, off the request's path.
pub async fn refused(
    state: &AppState,
    org: Uuid,
    key: Uuid,
    refusal: Refusal,
    now: OffsetDateTime,
) {
    match refusal {
        Refusal::SpendLimit => {
            let until = kaeso_core::linked_billing::spend_limit_resets(now);
            match store::spend_limited(&state.pg, key, until).await {
                Ok(true) => {
                    credentials::forget_org(state, org).await;
                    crate::services::notifications::system::linked_spend_limit(
                        state, org, key, until,
                    )
                    .await;
                }
                Ok(false) => {}
                Err(e) => tracing::warn!(error = %e, "cannot record a linked key's spend limit"),
            }
        }
        Refusal::BillingUnavailable | Refusal::NeedsSetup => {}
        Refusal::Unlinked => {
            let settled = match creds::get(&state.pg, org, key).await {
                Ok(Some(row)) => now - row.updated_at >= SETTLING,
                _ => true,
            };
            if settled {
                unlinked(state, org, key).await;
            }
        }
    }
}

/// A request the linked organization paid for went through: a spend limit it had reached is
/// lifted.
pub async fn paid(state: &AppState, org: Uuid, key: Uuid) {
    match store::spend_lifted(&state.pg, key).await {
        Ok(true) => credentials::forget_org(state, org).await,
        Ok(false) => {}
        Err(e) => tracing::warn!(error = %e, "cannot lift a linked key's spend limit"),
    }
}
