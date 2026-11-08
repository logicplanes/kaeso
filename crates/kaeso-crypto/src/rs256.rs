use std::fmt;

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use zeroize::Zeroizing;

use crate::CryptoError;

const PEM_BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
const PEM_END: &str = "-----END PRIVATE KEY-----";

// aws-lc-rs signs in constant time; the `rsa` crate's private-key operations are open to the
// Marvin timing attack (RUSTSEC-2023-0071), so they stay out of the gateway.
pub struct RsaSigner {
    key: RsaKeyPair,
}

impl fmt::Debug for RsaSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RsaSigner([redacted])")
    }
}

impl RsaSigner {
    pub fn from_pkcs8_pem(pem: &str) -> Result<RsaSigner, CryptoError> {
        let body = pem
            .trim()
            .strip_prefix(PEM_BEGIN)
            .and_then(|rest| rest.strip_suffix(PEM_END))
            .ok_or(CryptoError::PrivateKey)?;
        let base64: Zeroizing<String> =
            Zeroizing::new(body.chars().filter(|c| !c.is_ascii_whitespace()).collect());
        let der = Zeroizing::new(
            STANDARD
                .decode(base64.as_bytes())
                .map_err(|_| CryptoError::PrivateKey)?,
        );
        let key = RsaKeyPair::from_pkcs8(&der).map_err(|_| CryptoError::PrivateKey)?;
        Ok(RsaSigner { key })
    }

    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut signature = vec![0_u8; self.key.public_modulus_len()];
        self.key
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                message,
                &mut signature,
            )
            .map_err(|_| CryptoError::Sign)?;
        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::encoding::AsDer;
    use aws_lc_rs::signature::{KeyPair, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};

    fn pem() -> (String, Vec<u8>) {
        let pair = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048)
            .unwrap_or_else(|_| unreachable!());
        let der = pair.as_der().unwrap_or_else(|_| unreachable!());
        let body = STANDARD.encode(der.as_ref());
        let lines: Vec<&str> = body
            .as_bytes()
            .chunks(64)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect();
        (
            format!("{PEM_BEGIN}\n{}\n{PEM_END}\n", lines.join("\n")),
            pair.public_key().as_ref().to_vec(),
        )
    }

    #[test]
    fn signs_rs256_that_the_public_key_verifies() {
        let (pem, public) = pem();
        let signer = RsaSigner::from_pkcs8_pem(&pem).unwrap_or_else(|_| unreachable!());
        let signature = signer
            .sign(b"header.claims")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(signature.len(), 256);
        let key = UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, &public);
        assert!(key.verify(b"header.claims", &signature).is_ok());
        assert!(key.verify(b"header.other", &signature).is_err());
    }

    #[test]
    fn refuses_what_is_not_a_pkcs8_rsa_key() {
        let (pem, _) = pem();
        for bad in [
            "",
            "not a key",
            &pem.replace("PRIVATE KEY", "RSA PRIVATE KEY"),
            &format!("{PEM_BEGIN}\n!!!!\n{PEM_END}"),
            &format!("{PEM_BEGIN}\nAAAA\n{PEM_END}"),
        ] {
            assert!(matches!(
                RsaSigner::from_pkcs8_pem(bad),
                Err(CryptoError::PrivateKey)
            ));
        }
    }

    #[test]
    fn its_debug_form_shows_no_key() {
        let (pem, _) = pem();
        let signer = RsaSigner::from_pkcs8_pem(&pem).unwrap_or_else(|_| unreachable!());
        assert_eq!(format!("{signer:?}"), "RsaSigner([redacted])");
    }
}
