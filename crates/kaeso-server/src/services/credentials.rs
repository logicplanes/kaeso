use std::sync::atomic::{AtomicI64, Ordering};

use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_core::provider::AuthMethod;
use kaeso_core::providers::{CustomAuth, CustomProvider, ProviderRef};
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

/// What a signed-out service principal key names as its account.
pub const ENTRA_ID: &str = "Microsoft Entra ID";

pub fn aad(id: CredentialId) -> Vec<u8> {
    format!("cred:{id}").into_bytes()
}

pub struct Resolved {
    pub id: CredentialId,
    pub revision: i64,
    pub secret: SecretString,
    pub method: AuthMethod,
    pub account: Option<Account>,
    pub google: Option<std::sync::Arc<crate::services::google_auth::ServiceAccount>>,
    /// A Microsoft Entra ID service principal, for an Azure key that signs in with one.
    pub azure: Option<std::sync::Arc<crate::services::azure_auth::Principal>>,
    /// Why a key that isn't an account can't make requests, such as a service principal whose
    /// secret Entra ID refused.
    pub stopped: Option<Stopped>,
    /// The account, and optionally the gateway, a key of a provider whose API path names the
    /// account calls, such as Cloudflare AI Gateway.
    pub scope: Option<KeyScope>,
    /// The region an API key of a regional provider, such as Amazon Bedrock, calls.
    pub region: Option<String>,
    /// The endpoint of the resource a key of a provider whose keys each belong to one resource
    /// calls, such as an Azure resource's `https://example.openai.azure.com`.
    pub endpoint: Option<String>,
    /// The keys the project draws from for the provider, with their weights; set on the key a
    /// project resolves first, empty on the others.
    pub pool: Vec<kaeso_store::pg::credentials::PoolMember>,
    keyless: bool,
    last_touched: AtomicI64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyScope {
    pub account_id: String,
    pub gateway_id: Option<String>,
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
    pub fn keyless(provider: &CustomProvider) -> Resolved {
        Resolved {
            id: CredentialId::from_uuid(provider.uuid()),
            revision: 0,
            secret: SecretString::from(String::new()),
            method: AuthMethod::ApiKey,
            account: None,
            google: None,
            azure: None,
            stopped: None,
            scope: None,
            region: None,
            endpoint: None,
            pool: Vec::new(),
            keyless: true,
            last_touched: AtomicI64::new(0),
        }
    }

    pub fn is_keyless(&self) -> bool {
        self.keyless
    }

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
    // Answered as the install's problem, like a refused key: the app can't fix it, and the
    // answer names no organization or Platform API path.
    #[error(
        "This project has no provider key for {provider}. Ask an organization admin to add one under Providers."
    )]
    Missing { provider: String },
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
    due_within(method, expires_at, earliest, now, time::Duration::ZERO)
}

// Due `ahead` from now, and renewable now: never before the provider allows it.
fn due_within(
    method: AuthMethod,
    expires_at: OffsetDateTime,
    earliest: Option<OffsetDateTime>,
    now: OffsetDateTime,
    ahead: time::Duration,
) -> bool {
    now + ahead >= expires_at - renew_before(method)
        && (now >= expires_at || earliest.is_none_or(|e| now >= e))
}

// The background job renews an account this long before a request would find it due: more than
// the job's period, so an account in use is renewed before any request waits for it.
pub const RENEW_AHEAD: time::Duration = time::Duration::minutes(2);

// An account used within this long is renewed in the background, so the first request after a
// pause of hours, such as a nightly agent run, doesn't wait for the provider's sign-in service.
pub const RENEW_IF_USED_WITHIN: time::Duration = time::Duration::days(7);

pub fn seal(state: &AppState, id: CredentialId, secret: &str) -> Result<Sealed, CryptoError> {
    kaeso_crypto::seal(&state.keyring, secret.as_bytes(), &aad(id))
}

pub async fn for_provider(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &ProviderRef,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    for_provider_member(state, org, project, provider, None).await
}

/// The project's key for the provider, or with `member` that key of the project's pool.
pub async fn for_provider_member(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &ProviderRef,
    member: Option<Uuid>,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    match provider.custom() {
        Some(c) if c.auth() == CustomAuth::None => Ok(std::sync::Arc::new(Resolved::keyless(c))),
        _ => resolve_member(state, org, project, provider.id(), member)
            .await
            .map_err(|e| match e {
                ResolveError::Missing { .. } => ResolveError::Missing {
                    provider: provider.name().to_owned(),
                },
                other => other,
            }),
    }
}

pub async fn resolve(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &str,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    resolve_member(state, org, project, provider, None).await
}

async fn resolve_member(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &str,
    member: Option<Uuid>,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    let resolved = cached(state, org, project, provider, member).await?;
    if let Some(stopped) = resolved.stopped {
        return Err(stopped.error(ENTRA_ID));
    }
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
            member,
            product,
            stale: resolved.clone(),
        },
    )
    .await?;
    let renewed = cached(state, org, project, provider, member).await?;
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

/// One of an org's keys by its id, whichever projects use it, with an account's access renewed
/// when it is due; `None` when the org has no such key.
pub async fn by_id(
    state: &AppState,
    org: OrgId,
    id: CredentialId,
    provider: &str,
) -> Result<Option<Resolved>, ResolveError> {
    let read = || async {
        match credentials::sealed(&state.pg, org.uuid(), id.uuid()).await? {
            Some(row) => Ok::<_, ResolveError>(Some(open(&state.keyring, org.uuid(), &row)?)),
            None => Ok(None),
        }
    };
    let Some(resolved) = read().await? else {
        return Ok(None);
    };
    if let Some(stopped) = resolved.stopped {
        return Err(stopped.error(ENTRA_ID));
    }
    let Some(account) = resolved.account else {
        return Ok(Some(resolved));
    };
    let product = product_of(provider);
    if let Some(stopped) = account.stopped {
        return Err(stopped.error(product));
    }
    let now = state.clock.now();
    if !due(
        resolved.method,
        account.expires_at,
        account.earliest_refresh_at,
        now,
    ) {
        return Ok(Some(resolved));
    }
    let Some(hosts) = state.accounts.hosts(resolved.method) else {
        return Err(ResolveError::Unrenewed { product });
    };
    let locked = Locked {
        org,
        id,
        method: resolved.method,
        product,
        cached: Some(account),
    };
    renew_locked(state, hosts, &locked, time::Duration::ZERO).await?;
    let renewed = read().await?;
    match renewed.as_ref().and_then(|r| r.account) {
        Some(Account {
            stopped: Some(stopped),
            ..
        }) => Err(stopped.error(product)),
        Some(a) if state.clock.now() >= a.expires_at => Err(ResolveError::Unrenewed { product }),
        _ => Ok(renewed),
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
    member: Option<Uuid>,
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
    let cached_now = cached(state, org, r.project, &r.provider, r.member).await?;
    if current(r.stale.method, cached_now.account, state.clock.now()) {
        return Ok(());
    }
    let account = Locked {
        org,
        id: r.stale.id,
        method: r.stale.method,
        product: r.product,
        cached: cached_now.account,
    };
    renew_locked(state, hosts, &account, time::Duration::ZERO)
        .await
        .map(|_| ())
}

struct Locked {
    org: OrgId,
    id: CredentialId,
    method: AuthMethod,
    product: &'static str,
    cached: Option<Account>,
}

// Renews under the credential's lock with its row read again, so two replicas never spend a
// rotating refresh token twice. Returns whether it renewed.
async fn renew_locked(
    state: &AppState,
    hosts: &crate::services::provider_accounts::Hosts,
    a: &Locked,
    ahead: time::Duration,
) -> Result<bool, ResolveError> {
    let (org, id) = (a.org, a.id.uuid());
    let mut tx = state.pg.begin().await.map_err(StoreError::from)?;
    credentials::lock_refresh(&mut tx, id).await?;
    let Some(row) = credentials::sealed(&mut *tx, org.uuid(), id).await? else {
        return Ok(false);
    };
    let now = state.clock.now();
    let expires_at = row.expires_at.unwrap_or(now);
    let tokens = tokens(&state.keyring, &row)?;
    if row.auth_error.is_some()
        || !due_within(a.method, expires_at, tokens.earliest_refresh_at, now, ahead)
    {
        tx.commit().await.map_err(StoreError::from)?;
        if outdated(a.cached, &row) {
            forget_org(state, org.uuid()).await;
        }
        return Ok(false);
    }
    match crate::services::provider_accounts::refresh(hosts, a.method, &tokens).await {
        Ok(granted) => {
            let renewed_until = granted.expires_at(now);
            let renewed = granted.renew(&tokens);
            let json = renewed.sealed_form().map_err(|_| CryptoError::Malformed)?;
            let sealed = seal(state, a.id, &json)?;
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
            Ok(true)
        }
        Err(e) if e.signs_out() => {
            tracing::warn!(error = %e, credential = %a.id, "the provider ended an account's session; it needs a new sign-in");
            credentials::sign_out(&mut *tx, id, SIGNED_OUT).await?;
            tx.commit().await.map_err(StoreError::from)?;
            forget_org(state, org.uuid()).await;
            crate::services::notifications::system::account_signed_out(state, org.uuid(), id).await;
            Err(ResolveError::SignedOut { product: a.product })
        }
        Err(AccountError::NoApiAccess) => {
            tracing::warn!(credential = %a.id, "the provider refused an account's plan; it has no API access");
            credentials::sign_out(&mut *tx, id, NO_API_ACCESS).await?;
            tx.commit().await.map_err(StoreError::from)?;
            forget_org(state, org.uuid()).await;
            Err(ResolveError::NoApiAccess { product: a.product })
        }
        Err(e) => {
            tracing::warn!(error = %e, credential = %a.id, "cannot renew an account's access token");
            drop(tx);
            Ok(false)
        }
    }
}

async fn cached(
    state: &AppState,
    org: OrgId,
    project: ProjectId,
    provider: &str,
    member: Option<Uuid>,
) -> Result<std::sync::Arc<Resolved>, ResolveError> {
    let st = state.clone();
    let rest = match member {
        Some(id) => format!("{}:{provider}:{id}", project.uuid()),
        None => format!("{}:{provider}", project.uuid()),
    };
    let name = provider.to_owned();
    let found = state
        .caches
        .credentials
        .get_or_refresh(&org.uuid().to_string(), &rest, move || async move {
            let (org, project) = (org.uuid(), project.uuid());
            let row = match member {
                Some(id) => credentials::resolve_member(&st.pg, org, project, &name, id).await?,
                None => credentials::resolve(&st.pg, org, project, &name).await?,
            };
            let Some(row) = row else {
                return Ok::<_, ResolveError>(None);
            };
            let mut resolved = open(&st.keyring, org, &row)?;
            if member.is_none() {
                resolved.pool = credentials::pool(&st.pg, org, project, &name).await?;
            }
            Ok(Some(resolved))
        })
        .await?;
    found.ok_or_else(|| ResolveError::Missing {
        provider: provider.to_owned(),
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

fn open(
    keyring: &kaeso_crypto::KeyRing,
    org: Uuid,
    row: &SealedRow,
) -> Result<Resolved, CryptoError> {
    let method = AuthMethod::parse(&row.auth_method).ok_or(CryptoError::Malformed)?;
    let mut google = None;
    let mut azure = None;
    let (secret, account) = if method.is_account() {
        let tokens = tokens(keyring, row)?;
        let account = Account {
            expires_at: row.expires_at.ok_or(CryptoError::Malformed)?,
            earliest_refresh_at: tokens.earliest_refresh_at,
            stopped: Stopped::read(row.auth_error.as_deref()),
        };
        (tokens.access_token, Some(account))
    } else if method == AuthMethod::GoogleServiceAccount {
        let json = SecretString::from(plain(keyring, row)?);
        let location = row.location.clone().ok_or(CryptoError::Malformed)?;
        let opened = crate::services::google_auth::ServiceAccount::open(
            secrecy::ExposeSecret::expose_secret(&json),
            org,
            location,
        )
        .ok_or(CryptoError::Malformed)?;
        google = Some(std::sync::Arc::new(opened));
        (String::new(), None)
    } else if method == AuthMethod::AzureServicePrincipal {
        let principal = kaeso_core::azure::ServicePrincipal::parse(&plain(keyring, row)?)
            .map_err(|_| CryptoError::Malformed)?;
        azure = Some(std::sync::Arc::new(
            crate::services::azure_auth::Principal { principal, org },
        ));
        (String::new(), None)
    } else {
        (plain(keyring, row)?, None)
    };
    Ok(Resolved {
        id: CredentialId::from_uuid(row.id),
        revision: row.revision,
        secret: SecretString::from(secret),
        method,
        account,
        google,
        azure,
        stopped: Stopped::read(row.auth_error.as_deref())
            .filter(|_| method == AuthMethod::AzureServicePrincipal),
        scope: row.account_id.clone().map(|account_id| KeyScope {
            account_id,
            gateway_id: row.gateway_id.clone(),
        }),
        region: row
            .location
            .clone()
            .filter(|_| method == AuthMethod::ApiKey || method.is_aws()),
        endpoint: row.endpoint.clone(),
        pool: Vec::new(),
        keyless: false,
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

// Renews the accounts used within `RENEW_IF_USED_WITHIN` that a request would find due within
// `RENEW_AHEAD`, so requests find a fresh token instead of waiting for the renewal. Returns a
// summary for the job's log line.
pub async fn renew_ahead(state: &AppState) -> Result<String, StoreError> {
    let now = state.clock.now();
    let longest = renew_before(AuthMethod::ChatGpt).max(renew_before(AuthMethod::Grok));
    let soon = credentials::renewable(
        &state.pg,
        now + longest + RENEW_AHEAD,
        now - RENEW_IF_USED_WITHIN,
    )
    .await?;
    let (mut renewed, mut failed) = (0usize, 0usize);
    for row in soon {
        let Some(method) = AuthMethod::parse(&row.auth_method).filter(|m| m.is_account()) else {
            continue;
        };
        let Some(hosts) = state.accounts.hosts(method) else {
            continue;
        };
        if row.expires_at - renew_before(method) > now + RENEW_AHEAD {
            continue;
        }
        let account = Locked {
            org: OrgId::from_uuid(row.org_id),
            id: CredentialId::from_uuid(row.id),
            method,
            product: product_of(&row.provider),
            cached: None,
        };
        let _flight = state.caches.refreshes.board(&row.id.to_string()).await;
        match renew_locked(state, hosts, &account, RENEW_AHEAD).await {
            Ok(true) => renewed += 1,
            Ok(false) => {}
            Err(e) => {
                failed += 1;
                tracing::warn!(error = %e, credential = %account.id, "cannot renew an account's access token ahead of time");
            }
        }
    }
    Ok(format!("{renewed} accounts renewed ahead, {failed} failed"))
}

pub async fn forget_org(state: &AppState, org: Uuid) {
    state.google_tokens.forget_org(org);
    state.azure_tokens.forget_org(org);
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
            location: None,
            account_id: None,
            gateway_id: None,
            endpoint: None,
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
