use std::sync::atomic::{AtomicI64, Ordering};

use kaeso_core::id::{CredentialId, OrgId, ProjectId};
use kaeso_crypto::{CryptoError, Sealed};
use kaeso_store::StoreError;
use kaeso_store::pg::credentials::{self, SealedRow};
use secrecy::SecretString;
use uuid::Uuid;

use crate::state::AppState;

pub fn aad(id: CredentialId) -> Vec<u8> {
    format!("cred:{id}").into_bytes()
}

pub struct Resolved {
    pub id: CredentialId,
    pub secret: SecretString,
    last_touched: AtomicI64,
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
    let Some(resolved) = found else {
        return Err(ResolveError::Missing {
            provider: provider.to_owned(),
            org,
        });
    };
    touch(state, &resolved);
    Ok(resolved)
}

fn open(keyring: &kaeso_crypto::KeyRing, row: &SealedRow) -> Result<Resolved, CryptoError> {
    let id = CredentialId::from_uuid(row.id);
    let sealed = Sealed {
        ciphertext: row.ciphertext.clone(),
        nonce: row.nonce.clone(),
        wrapped_key: row.wrapped_key.clone(),
        key_id: row.key_id.clone(),
    };
    let plain = kaeso_crypto::open(keyring, &sealed, &aad(id))?;
    let text = secrecy::ExposeSecret::expose_secret(&plain);
    let secret = String::from_utf8(text.clone()).map_err(|_| CryptoError::Malformed)?;
    Ok(Resolved {
        id,
        secret: SecretString::from(secret),
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
