use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretString};

use crate::random::random_bytes;

pub const MIN_LEN: usize = 12;
pub const MAX_LEN: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PasswordPolicyError {
    #[error("Passwords need at least {MIN_LEN} characters.")]
    TooShort,
    #[error("Passwords can have at most {MAX_LEN} characters.")]
    TooLong,
}

pub fn check_policy(password: &SecretString) -> Result<(), PasswordPolicyError> {
    let n = password.expose_secret().chars().count();
    if n < MIN_LEN {
        return Err(PasswordPolicyError::TooShort);
    }
    if n > MAX_LEN {
        return Err(PasswordPolicyError::TooLong);
    }
    Ok(())
}

fn hasher() -> Result<Argon2<'static>, argon2::Error> {
    Ok(Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19_456, 2, 1, None)?,
    ))
}

pub fn hash(password: &SecretString) -> Result<String, crate::CryptoError> {
    let salt = random_bytes::<16>();
    let hasher = hasher().map_err(|_| crate::CryptoError::Malformed)?;
    hasher
        .hash_password_with_salt(password.expose_secret().as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| crate::CryptoError::Malformed)
}

pub fn verify(password: &SecretString, stored: &str) -> bool {
    hasher()
        .map(|h| {
            h.verify_password(password.expose_secret().as_bytes(), stored)
                .is_ok()
        })
        .unwrap_or(false)
}

const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$NGIhWbBZJ2YTZa/vULakKg$8hhjT6esDrn1bzI9B/5/pDX2KFY7ib24EIuasXkoyEk";

pub fn dummy_hash() -> &'static str {
    DUMMY_HASH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(p: &str) -> SecretString {
        SecretString::from(p.to_owned())
    }

    #[test]
    fn hashes_verify_and_differ() {
        let pw = s("correct horse battery staple");
        let a = hash(&pw).unwrap();
        let b = hash(&pw).unwrap();
        assert_ne!(a, b, "salted");
        assert!(a.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify(&pw, &a));
        assert!(!verify(&s("correct horse battery stapler"), &a));
        assert!(!verify(&pw, "not a hash"));
        assert!(dummy_hash().starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(!verify(&pw, dummy_hash()));
    }

    #[test]
    fn length_rules_count_characters() {
        assert_eq!(
            check_policy(&s("short")),
            Err(PasswordPolicyError::TooShort)
        );
        assert!(check_policy(&s("twelve chars")).is_ok());
        assert!(check_policy(&s("äöüäöüäöüäöü")).is_ok());
        assert_eq!(
            check_policy(&s(&"x".repeat(129))),
            Err(PasswordPolicyError::TooLong)
        );
    }
}
