use std::fmt;

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::SecretBox;

use crate::CryptoError;

// An uncompressed SEC1 point: 0x04, then x and y of 32 bytes each.
const POINT: usize = 65;
const COORDINATE: usize = 32;

/// A P-256 key that signs JWTs with ES256, kept as PKCS #8 so it can be sealed and stored.
pub struct EcSigner {
    key: EcdsaKeyPair,
}

impl fmt::Debug for EcSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EcSigner([redacted])")
    }
}

/// The public half of a signing key, as a JSON Web Key's `x` and `y`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EcPublicKey {
    pub x: String,
    pub y: String,
}

impl EcSigner {
    /// A new key, and its PKCS #8 form to store.
    pub fn generate() -> Result<(EcSigner, SecretBox<Vec<u8>>), CryptoError> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|_| CryptoError::Sign)?;
        let der = pkcs8.as_ref().to_vec();
        let signer = EcSigner::from_pkcs8(&der)?;
        Ok((signer, SecretBox::new(Box::new(der))))
    }

    pub fn from_pkcs8(der: &[u8]) -> Result<EcSigner, CryptoError> {
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der)
            .map_err(|_| CryptoError::PrivateKey)?;
        Ok(EcSigner { key })
    }

    pub fn public_key(&self) -> Result<EcPublicKey, CryptoError> {
        let point = self.key.public_key().as_ref();
        if point.len() != POINT {
            return Err(CryptoError::Malformed);
        }
        let x = point.get(1..=COORDINATE).ok_or(CryptoError::Malformed)?;
        let y = point
            .get(1 + COORDINATE..POINT)
            .ok_or(CryptoError::Malformed)?;
        Ok(EcPublicKey {
            x: URL_SAFE_NO_PAD.encode(x),
            y: URL_SAFE_NO_PAD.encode(y),
        })
    }

    /// The JWS signature of `message`: `r` and `s` of 32 bytes each (RFC 7518 §3.4).
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let signature = self
            .key
            .sign(&SystemRandom::new(), message)
            .map_err(|_| CryptoError::Sign)?;
        Ok(signature.as_ref().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn signs_es256_that_its_public_key_verifies_after_a_round_trip_through_pkcs8() {
        let (_, pkcs8) = EcSigner::generate().unwrap_or_else(|_| unreachable!());
        let signer = EcSigner::from_pkcs8(pkcs8.expose_secret()).unwrap_or_else(|_| unreachable!());
        let signature = signer
            .sign(b"header.claims")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(signature.len(), 64);
        let public = signer.public_key().unwrap_or_else(|_| unreachable!());
        let mut point = vec![4_u8];
        point.extend(URL_SAFE_NO_PAD.decode(&public.x).unwrap_or_default());
        point.extend(URL_SAFE_NO_PAD.decode(&public.y).unwrap_or_default());
        let key = UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &point);
        assert!(key.verify(b"header.claims", &signature).is_ok());
        assert!(key.verify(b"header.other", &signature).is_err());
    }

    #[test]
    fn refuses_what_is_not_a_p256_key_and_shows_no_key() {
        assert!(matches!(
            EcSigner::from_pkcs8(b"not a key"),
            Err(CryptoError::PrivateKey)
        ));
        let (signer, _) = EcSigner::generate().unwrap_or_else(|_| unreachable!());
        assert_eq!(format!("{signer:?}"), "EcSigner([redacted])");
    }
}
