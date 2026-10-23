use std::sync::atomic::{AtomicI64, Ordering};

use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_core::provider::AuthMethod;
use kaeso_crypto::{CryptoError, Sealed};
use kaeso_store::StoreError;
use kaeso_store::pg::credentials::{self, KeyInUse, SealedRow};
use secrecy::SecretString;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::services::provider_accounts::{AccountError, Tokens, renew_before};
use crate::state::AppState;

pub const SIGNED_OUT: &str = "signed_out";
pub const NO_API_ACCESS: &str = "no_api_access";

pub fn aad(id: CredentialId) -> Vec<u8> {
    format!("cred:{id}").into_bytes()
}

pub struct Resolved {
    pub id: CredentialId,
    pub revision: i64,
    pub secret: SecretString,
    pub method: AuthMethod,
    pub account: Option<Account>,
    last_touched: AtomicI64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Account {
    pub expires_at: OffsetDateTime,
    pub earliest_refresh_at: Option<OffsetDateTime>,
    pub stopped: Option<Stopped>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    SignedOut,
    NoApiAccess,
}

impl Stopped {
    fn read(auth_error: Option<&str>) -> Option<Stopped> {
        auth_error.map(|e| match e {
            NO_API_ACCESS => Stopped::NoApiAccess,
            _ => Stopped::SignedOut,
        })
    }

    fn error(self, product: &'static str) -> ResolveError {
        match self {
            Stopped::SignedOut => ResolveError::SignedOut { product },
            Stopped::NoApiAccess => ResolveError::NoApiAccess { product },
        }
    }
}

impl Resolved {
    pub fn key(&self) -> KeyInUse {
        KeyInUse {
            id: self.id.uuid(),
            revision: self.revision,
        }
    }
}

impl std::fmt::Debug for Resolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(
        "No credential for the provider `{provider}` is set up for this project. An org admin adds a key for it \
         with POST /platform/v1/orgs/{org}/credentials, makes one of the org's keys the default, or assigns one \
         to this project."
    )]
    Missing { provider: String, org: OrgId },
    #[error("the stored credential cannot be decrypted: {0}")]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the {product} account of the provider key is signed out")]
    SignedOut { product: &'static str },
    #[error("the {product} account of the provider key has no API access on its plan")]
    NoApiAccess { product: &'static str },
    #[error("the {product} account's access token expired and cannot be renewed right now")]
    Unrenewed { product: &'static str },
}

// Renew near expiry, so a request never starts with a token that lapses before the
// provider reads it, and never earlier than the provider allows (SIWC "Refreshing tokens").
fn due(
    method: AuthMethod,
    expires_at: OffsetDateTime,
    earliest: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> bool {
    now >= expires_at - renew_before(method)
        && (now >= expires_at || earliest.is_none_or(|e| now >= e))
}

pub fn seal(state: &AppState, id: CredentialId, secret: &str) -> Result<Sealed, CryptoError> {
    kaeso_crypto::seal(&state.keyring, secret.as_bytes(), &aad(id))
}

pub async fn resolve(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &str,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    let resolved = cached(state, org, project, provider).await?;
    let Some(account) = resolved.account else {
        touch(state, &resolved);
        return Ok(resolved);
    };
    let product = product_of(provider);
    if let Some(stopped) = account.stopped {
        return Err(stopped.error(product));
    }
    if !due(
        resolved.method,
        account.expires_at,
        account.earliest_refresh_at,
        state.clock.now(),
    ) {
        touch(state, &resolved);
        return Ok(resolved);
    }
    renew(
        state,
        Renewal {
            org,
            project,
            provider: provider.to_owned(),
            product,
            stale: resolved.clone(),
        },
    )
    .await?;
    let renewed = cached(state, org, project, provider).await?;
    match renewed.account {
        Some(Account {
            stopped: Some(stopped),
            ..
        }) => Err(stopped.error(product)),
        Some(a) if state.clock.now() >= a.expires_at => Err(ResolveError::Unrenewed { product }),
        _ => {
            touch(state, &renewed);
            Ok(renewed)
        }
    }
}

fn product_of(provider: &str) -> &'static str {
    kaeso_core::provider::provider(provider)
        .and_then(|p| p.account)
        .map_or("provider", |a| a.product)
}

struct Renewal {
    org: OrgId,
    project: ProjectId,
    provider: String,
    product: &'static str,
    stale: std::sync::Arc<Resolved>,
}

// OpenAI rotates the refresh token as it answers, so the answer must be stored even when
// the request that asked for it is gone: a lost rotation signs the account out at the next
// renewal. The work runs as its own task, which shutdown waits for, and the request awaits it.
async fn renew(state: &AppState, renewal: Renewal) -> Result<(), ResolveError> {
    let (done, outcome) = tokio::sync::oneshot::channel();
    let st = state.clone();
    state.in_flight.finish(async move {
        let _ = done.send(renewed(&st, renewal).await);
    });
    outcome.await.unwrap_or(Ok(()))
}

fn outdated(cached: Option<Account>, row: &SealedRow) -> bool {
    cached.is_none_or(|a| {
        row.expires_at != Some(a.expires_at) || row.auth_error.is_some() != a.stopped.is_some()
    })
}

fn current(method: AuthMethod, account: Option<Account>, now: OffsetDateTime) -> bool {
    account.is_some_and(|a| {
        a.stopped.is_some() || !due(method, a.expires_at, a.earliest_refresh_at, now)
    })
}

// One renewal per credential at a time: the flight keeps this instance's requests from
// queueing on the database, and the advisory lock with the re-read row keeps other
// replicas from spending a rotating refresh token twice, which signs the account out.
async fn renewed(state: &AppState, r: Renewal) -> Result<(), ResolveError> {
    let Some(hosts) = state.accounts.hosts(r.stale.method) else {
        return Ok(());
    };
    let org = r.org;
    let id = r.stale.id.uuid();
    let _flight = state.caches.refreshes.board(&id.to_string()).await;
    let cached_now = cached(state, org, r.project, &r.provider).await?;
    if current(r.stale.method, cached_now.account, state.clock.now()) {
        return Ok(());
    }
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    credentials::lock_refresh(&mut tx, id).await?;
    let Some(row) = credentials::sealed(&mut *tx, org.uuid(), id).await? else {
        return Ok(());
    };
    let now = state.clock.now();
    let expires_at = row.expires_at.unwrap_or(now);
    let tokens = tokens(&state.keyring, &row)?;
    if row.auth_error.is_some() || !due(r.stale.method, expires_at, tokens.earliest_refresh_at, now)
    {
        tx.commit().await.map_err(StoreError::from)?;
        if outdated(cached_now.account, &row) {
            forget_org(state, org.uuid()).await;
        }
        return Ok(());
    }
    match crate::services::provider_accounts::refresh(hosts, r.stale.method, &tokens).await {
        Ok(granted) => {
            let renewed_until = granted.expires_at(now);
            let renewed = granted.renew(&tokens);
            let json = renewed.sealed_form().map_err(|_| CryptoError::Malformed)?;
            let sealed = seal(state, r.stale.id, &json)?;
            credentials::refreshed(
                &mut *tx,
                id,
                &credentials::Sealed {
                    ciphertext: &sealed.ciphertext,
                    nonce: &sealed.nonce,
                    wrapped_key: &sealed.wrapped_key,
                    key_id: &sealed.key_id,
                },
                renewed_until,
            )
            .await?;
            tx.commit().await.map_err(StoreError::from)?;
            forget_org(state, org.uuid()).await;
            Ok(())
        }
        Err(e) if e.signs_out() => {
            tracing::warn!(error = %e, credential = %r.stale.id, "the provider ended an account's session; it needs a new sign-in");
            credentials::sign_out(&mut *tx, id, SIGNED_OUT).await?;
            tx.commit().await.map_err(StoreError::from)?;
            forget_org(state, org.uuid()).await;
            Err(ResolveError::SignedOut { product: r.product })
        }
        Err(AccountError::NoApiAccess) => {
            tracing::warn!(credential = %r.stale.id, "the provider refused an account's plan; it has no API access");
            credentials::sign_out(&mut *tx, id, NO_API_ACCESS).await?;
            tx.commit().await.map_err(StoreError::from)?;
            forget_org(state, org.uuid()).await;
            Err(ResolveError::NoApiAccess { product: r.product })
        }
        Err(e) => {
            tracing::warn!(error = %e, credential = %r.stale.id, "cannot renew an account's access token");
            drop(tx);
            Ok(())
        }
    }
}

async fn cached(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &str,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    let st = state.clone();
    let rest = format!("{}:{provider}", project.uuid());
    let found = state
        .caches
        .credentials
        .get_or_load(&org.uuid().to_string(), &rest, || async move {
            let Some(row) =
                credentials::resolve(&st.pg, org.uuid(), project.uuid(), provider).await?
            else {
                return Ok::<_, ResolveError>(None);
            };
            Ok(Some(open(&st.keyring, &row)?))
        })
        .await?;
    found.ok_or_else(|| ResolveError::Missing {
        provider: provider.to_owned(),
        org,
    })
}

fn plain(keyring: &kaeso_crypto::KeyRing, row: &SealedRow) -> Result<String, CryptoError> {
    let id = CredentialId::from_uuid(row.id);
    let sealed = Sealed {
        ciphertext: row.ciphertext.clone(),
        nonce: row.nonce.clone(),
        wrapped_key: row.wrapped_key.clone(),
        key_id: row.key_id.clone(),
    };
    let plain = kaeso_crypto::open(keyring, &sealed, &aad(id))?;
    let text = secrecy::ExposeSecret::expose_secret(&plain);
    String::from_utf8(text.clone()).map_err(|_| CryptoError::Malformed)
}

pub fn tokens(keyring: &kaeso_crypto::KeyRing, row: &SealedRow) -> Result<Tokens, CryptoError> {
    Tokens::parse(&plain(keyring, row)?).ok_or(CryptoError::Malformed)
}

fn open(keyring: &kaeso_crypto::KeyRing, row: &SealedRow) -> Result<Resolved, CryptoError> {
    let method = AuthMethod::parse(&row.auth_method).ok_or(CryptoError::Malformed)?;
    let (secret, account) = if method.is_account() {
        let tokens = tokens(keyring, row)?;
        let account = Account {
            expires_at: row.expires_at.ok_or(CryptoError::Malformed)?,
            earliest_refresh_at: tokens.earliest_refresh_at,
            stopped: Stopped::read(row.auth_error.as_deref()),
        };
        (tokens.access_token, Some(account))
    } else {
        (plain(keyring, row)?, None)
    };
    Ok(Resolved {
        id: CredentialId::from_uuid(row.id),
        revision: row.revision,
        secret: SecretString::from(secret),
        method,
        account,
        last_touched: AtomicI64::new(0),
    })
}

fn touch(state: &AppState, resolved: &Resolved) {
    let now = state.clock.now();
    let at = now.unix_timestamp();
    let last = resolved.last_touched.load(Ordering::Relaxed);
    if at - last < 60
        || resolved
            .last_touched
            .compare_exchange(last, at, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    let pg = state.pg.clone();
    let id = resolved.id.uuid();
    tokio::spawn(async move {
        if let Err(e) = credentials::touch(&pg, id, now).await {
            tracing::warn!(error = %e, "cannot record the credential's last use");
        }
    });
}

pub async fn forget_org(state: &AppState, org: Uuid) {
    state
        .caches
        .credentials
        .invalidate_group(&org.to_string())
        .await;
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn row(expires_at: Option<OffsetDateTime>, auth_error: Option<&str>) -> SealedRow {
        SealedRow {
            id: Uuid::nil(),
            ciphertext: Vec::new(),
            nonce: Vec::new(),
            wrapped_key: Vec::new(),
            key_id: "k1".into(),
            revision: 1,
            auth_method: "chatgpt".into(),
            expires_at,
            auth_error: auth_error.map(str::to_owned),
        }
    }

    #[test]
    fn only_a_row_that_changed_since_it_was_cached_is_forgotten() {
        let at = datetime!(2026-10-23 13:00 UTC);
        let cached = Some(Account {
            expires_at: at,
            earliest_refresh_at: None,
            stopped: None,
        });
        assert!(!outdated(cached, &row(Some(at), None)));
        assert!(outdated(
            cached,
            &row(Some(at + time::Duration::hours(1)), None)
        ));
        assert!(outdated(cached, &row(Some(at), Some("signed_out"))));
        assert!(outdated(None, &row(Some(at), None)));
    }

    #[test]
    fn a_renewal_another_request_finished_is_not_repeated() {
        let at = datetime!(2026-10-23 13:00 UTC);
        let account = |expires_at, signed_out: bool| {
            Some(Account {
                expires_at,
                earliest_refresh_at: None,
                stopped: signed_out.then_some(Stopped::SignedOut),
            })
        };
        let current = |account, at| current(AuthMethod::ChatGpt, account, at);
        assert!(current(account(at + time::Duration::hours(1), false), at));
        assert!(
            current(account(at, true), at),
            "a signed-out account needs a sign-in, not a renewal"
        );
        assert!(!current(account(at, false), at));
        assert!(!current(None, at));
    }

    #[test]
    fn an_account_renews_in_its_last_minutes_and_not_before_the_provider_allows() {
        let expires = datetime!(2026-10-23 13:00 UTC);
        let due = |expires, earliest, at| due(AuthMethod::ChatGpt, expires, earliest, at);
        let now = |at| due(expires, None, at);
        assert!(!now(datetime!(2026-10-23 12:54:59 UTC)));
        assert!(now(datetime!(2026-10-23 12:55 UTC)));
        assert!(now(datetime!(2026-10-23 14:00 UTC)));
        let later = Some(datetime!(2026-10-23 12:58 UTC));
        assert!(!due(expires, later, datetime!(2026-10-23 12:57 UTC)));
        assert!(due(expires, later, datetime!(2026-10-23 12:58 UTC)));
        let after_expiry = Some(datetime!(2026-10-23 15:00 UTC));
        assert!(
            due(expires, after_expiry, datetime!(2026-10-23 13:00 UTC)),
            "an expired token is renewed whatever the provider asked"
        );
        let grok = |at| super::due(AuthMethod::Grok, expires, None, at);
        assert!(!grok(datetime!(2026-10-23 12:57:59 UTC)));
        assert!(grok(datetime!(2026-10-23 12:58 UTC)));
    }
}
