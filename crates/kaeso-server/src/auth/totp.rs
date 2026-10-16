use kaeso_core::id::UserId;
use kaeso_core::secret;
use kaeso_crypto::{Sealed, SecretBox, random_bytes};
use kaeso_store::StoreError;
use kaeso_store::pg::totp;

use crate::state::AppState;

pub const RECOVERY_CODES: usize = 10;

pub fn aad(user: UserId) -> Vec<u8> {
    format!("totp:{user}").into_bytes()
}

#[derive(Debug, thiserror::Error)]
pub enum SecondFactorError {
    #[error("the code is not valid")]
    Invalid,
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    Crypto(#[from] kaeso_crypto::CryptoError),
}

#[derive(Clone, Debug)]
pub enum Factor<'a> {
    Code(&'a str),
    Recovery(&'a str),
}

pub async fn check_code(
    state: &AppState,
    user: UserId,
    code: &str,
    confirm: bool,
) -> Result<(), SecondFactorError> {
    let row = totp::get(&state.pg, user.uuid())
        .await?
        .ok_or(SecondFactorError::Invalid)?;
    if row.enabled_at.is_none() && !confirm {
        return Err(SecondFactorError::Invalid);
    }
    let sealed = Sealed {
        ciphertext: row.secret_ciphertext,
        nonce: row.secret_nonce,
        wrapped_key: row.wrapped_key,
        key_id: row.key_id,
    };
    let secret: SecretBox<Vec<u8>> = kaeso_crypto::open(&state.keyring, &sealed, &aad(user))?;
    let now = u64::try_from(state.clock.now().unix_timestamp()).unwrap_or(0);
    let step = kaeso_crypto::totp::check(&secret, code, now).ok_or(SecondFactorError::Invalid)?;
    let step = i64::try_from(step).map_err(|_| SecondFactorError::Invalid)?;
    let confirm_at = confirm.then(|| state.clock.now());
    if totp::use_step(&state.pg, user.uuid(), step, confirm_at).await? {
        Ok(())
    } else {
        Err(SecondFactorError::Invalid)
    }
}

pub async fn check(
    state: &AppState,
    user: UserId,
    factor: Factor<'_>,
) -> Result<(), SecondFactorError> {
    match factor {
        Factor::Code(code) => check_code(state, user, code, false).await,
        Factor::Recovery(code) => {
            let normalized = normalize_recovery(code);
            let used = totp::use_recovery_code(
                &state.pg,
                user.uuid(),
                &secret::hash(&normalized),
                state.clock.now(),
            )
            .await?;
            if used {
                Ok(())
            } else {
                Err(SecondFactorError::Invalid)
            }
        }
    }
}

pub fn new_recovery_codes() -> (Vec<String>, Vec<[u8; 32]>) {
    const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let codes: Vec<String> = (0..RECOVERY_CODES)
        .map(|_| {
            let bytes = random_bytes::<10>();
            let mut bits: u128 = 0;
            for b in bytes {
                bits = (bits << 8) | u128::from(b);
            }
            let mut code = String::with_capacity(19);
            for (n, i) in (0..16).rev().enumerate() {
                if n > 0 && n % 4 == 0 {
                    code.push('-');
                }
                let index = ((bits >> (i * 5)) & 0x1f) as usize;
                code.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'0')));
            }
            code
        })
        .collect();
    let hashes = codes
        .iter()
        .map(|c| secret::hash(&normalize_recovery(c)))
        .collect();
    (codes, hashes)
}

fn normalize_recovery(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_are_distinct_and_forgiving() {
        let (codes, hashes) = new_recovery_codes();
        assert_eq!(codes.len(), 10);
        assert_eq!(codes[0].len(), 19);
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 10);
        assert_eq!(
            secret::hash(&normalize_recovery(
                &codes[3].to_uppercase().replace('-', " ")
            )),
            hashes[3]
        );
    }
}
