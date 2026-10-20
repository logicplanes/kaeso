#![forbid(unsafe_code)]

mod envelope;
mod keyring;
mod mac;
pub mod password;
mod random;
pub mod totp;

pub use envelope::{Sealed, open, rewrap, seal};
pub use keyring::{KeyRing, KeyRingError, MasterKey, generate_master_key};
pub use mac::MacKey;
pub use random::{random_bytes, random_token};
pub use secrecy::{ExposeSecret, SecretBox, SecretString};

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("the master key `{0}` is not configured; add it to KAESO_MASTER_KEYS")]
    UnknownKeyId(String),
    #[error("the stored value could not be decrypted: wrong master key or tampered data")]
    Decrypt,
    #[error("the stored value is malformed")]
    Malformed,
    #[error("the password could not be hashed")]
    Hash,
}
