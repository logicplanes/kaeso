use std::collections::BTreeMap;

use kaeso_crypto::{CryptoError, KeyRing, Sealed};
use kaeso_store::StoreError;
use kaeso_store::pg::{credentials, oidc, totp};
use sqlx::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum MasterKeyError {
    #[error(
        "stored secrets are encrypted with master keys that are not configured: {}. Add them to \
         KAESO_MASTER_KEYS (or the file in KAESO_MASTER_KEYS_FILE); without them those secrets cannot \
         be decrypted",
        .0.join(", ")
    )]
    Missing(Vec<String>),
    #[error("cannot re-wrap a stored secret: {0}")]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub async fn usage(pg: &PgPool) -> Result<BTreeMap<String, i64>, StoreError> {
    Ok(credentials::key_id_usage(pg).await?.into_iter().collect())
}

pub async fn check(pg: &PgPool, ring: &KeyRing) -> Result<(), MasterKeyError> {
    let missing: Vec<String> = usage(pg)
        .await?
        .into_keys()
        .filter(|id| ring.get(id).is_none())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(MasterKeyError::Missing(missing))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewrapReport {
    pub rewrapped: u64,
    pub usage: BTreeMap<String, i64>,
}

pub async fn rewrap(pg: &PgPool, ring: &KeyRing) -> Result<RewrapReport, MasterKeyError> {
    check(pg, ring).await?;
    let mut report = RewrapReport::default();
    for row in credentials::all_sealed(pg).await? {
        let sealed = Sealed {
            ciphertext: row.ciphertext,
            nonce: row.nonce,
            wrapped_key: row.wrapped_key,
            key_id: row.key_id,
        };
        if let Some(new) = kaeso_crypto::rewrap(ring, &sealed)? {
            credentials::set_wrapped_key(pg, row.id, &new.wrapped_key, &new.key_id).await?;
            report.rewrapped += 1;
        }
    }
    for (user, t) in totp::all_sealed(pg).await? {
        let sealed = Sealed {
            ciphertext: t.secret_ciphertext,
            nonce: t.secret_nonce,
            wrapped_key: t.wrapped_key,
            key_id: t.key_id,
        };
        if let Some(new) = kaeso_crypto::rewrap(ring, &sealed)? {
            totp::set_wrapped_key(pg, user, &new.wrapped_key, &new.key_id).await?;
            report.rewrapped += 1;
        }
    }
    for c in oidc::all_sealed(pg).await? {
        let sealed = Sealed {
            ciphertext: c.secret_ciphertext,
            nonce: c.secret_nonce,
            wrapped_key: c.wrapped_key,
            key_id: c.key_id,
        };
        if let Some(new) = kaeso_crypto::rewrap(ring, &sealed)? {
            oidc::set_wrapped_key(pg, c.org_id, &new.wrapped_key, &new.key_id).await?;
            report.rewrapped += 1;
        }
    }
    report.usage = usage(pg).await?;
    Ok(report)
}
