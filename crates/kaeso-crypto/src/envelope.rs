use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use secrecy::SecretBox;
use zeroize::Zeroize;

use crate::CryptoError;
use crate::keyring::{KeyRing, MasterKey};
use crate::random::random_bytes;

const NONCE_LEN: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
    pub key_id: String,
}

pub fn seal(ring: &KeyRing, plaintext: &[u8], aad: &[u8]) -> Result<Sealed, CryptoError> {
    let mut data_key = random_bytes::<32>();
    let nonce = random_bytes::<NONCE_LEN>();
    let ciphertext = encrypt(&data_key, &nonce, plaintext, aad);
    let wrapped_key = wrap(ring.active(), &data_key);
    data_key.zeroize();
    Ok(Sealed {
        ciphertext: ciphertext?,
        nonce: nonce.to_vec(),
        wrapped_key: wrapped_key?,
        key_id: ring.active().id().to_owned(),
    })
}

pub fn open(
    ring: &KeyRing,
    sealed: &Sealed,
    aad: &[u8],
) -> Result<SecretBox<Vec<u8>>, CryptoError> {
    let master = ring
        .get(&sealed.key_id)
        .ok_or_else(|| CryptoError::UnknownKeyId(sealed.key_id.clone()))?;
    let mut data_key = unwrap(master, &sealed.wrapped_key)?;
    let nonce: [u8; NONCE_LEN] = sealed
        .nonce
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Malformed)?;
    let plaintext = decrypt(&data_key, &nonce, &sealed.ciphertext, aad);
    data_key.zeroize();
    Ok(SecretBox::new(Box::new(plaintext?)))
}

pub fn rewrap(ring: &KeyRing, sealed: &Sealed) -> Result<Option<Sealed>, CryptoError> {
    if sealed.key_id == ring.active().id() {
        return Ok(None);
    }
    let master = ring
        .get(&sealed.key_id)
        .ok_or_else(|| CryptoError::UnknownKeyId(sealed.key_id.clone()))?;
    let mut data_key = unwrap(master, &sealed.wrapped_key)?;
    let wrapped_key = wrap(ring.active(), &data_key);
    data_key.zeroize();
    Ok(Some(Sealed {
        wrapped_key: wrapped_key?,
        key_id: ring.active().id().to_owned(),
        ..sealed.clone()
    }))
}

fn wrap_aad(master: &MasterKey) -> Vec<u8> {
    format!("kaeso-dek:{}", master.id()).into_bytes()
}

fn wrap(master: &MasterKey, data_key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    let nonce = random_bytes::<NONCE_LEN>();
    let ct = encrypt(master.bytes(), &nonce, data_key, &wrap_aad(master))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

fn unwrap(master: &MasterKey, wrapped: &[u8]) -> Result<[u8; 32], CryptoError> {
    let (nonce, ct) = wrapped
        .split_at_checked(NONCE_LEN)
        .ok_or(CryptoError::Malformed)?;
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Malformed)?;
    let mut key = decrypt(master.bytes(), &nonce, ct, &wrap_aad(master))?;
    let out: Result<[u8; 32], _> = key.as_slice().try_into();
    key.zeroize();
    out.map_err(|_| CryptoError::Malformed)
}

fn encrypt(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    msg: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Malformed)?;
    cipher
        .encrypt(&Nonce::from(*nonce), Payload { msg, aad })
        .map_err(|_| CryptoError::Malformed)
}

fn decrypt(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    msg: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Malformed)?;
    cipher
        .decrypt(&Nonce::from(*nonce), Payload { msg, aad })
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use secrecy::{ExposeSecret, SecretString};

    fn ring(entries: &[(&str, u8)]) -> KeyRing {
        let s = entries
            .iter()
            .map(|(id, b)| format!("{id}:{}", STANDARD.encode([*b; 32])))
            .collect::<Vec<_>>()
            .join(",");
        KeyRing::parse(&SecretString::from(s)).unwrap()
    }

    #[test]
    fn round_trip() {
        let r = ring(&[("k1", 1)]);
        let sealed = seal(&r, b"sk-provider-secret", b"cred_1").unwrap();
        assert_eq!(sealed.key_id, "k1");
        assert!(!sealed.ciphertext.windows(6).any(|w| w == b"sk-pro"));
        let plain = open(&r, &sealed, b"cred_1").unwrap();
        assert_eq!(plain.expose_secret().as_slice(), b"sk-provider-secret");
    }

    #[test]
    fn wrong_key_wrong_aad_and_tampering_fail() {
        let sealed = seal(&ring(&[("k1", 1)]), b"secret", b"a").unwrap();
        assert!(matches!(
            open(&ring(&[("k1", 2)]), &sealed, b"a"),
            Err(CryptoError::Decrypt)
        ));
        assert!(matches!(
            open(&ring(&[("k1", 1)]), &sealed, b"b"),
            Err(CryptoError::Decrypt)
        ));
        let mut tampered = sealed.clone();
        if let Some(b) = tampered.ciphertext.first_mut() {
            *b ^= 1;
        }
        assert!(matches!(
            open(&ring(&[("k1", 1)]), &tampered, b"a"),
            Err(CryptoError::Decrypt)
        ));
        let mut tampered = sealed.clone();
        if let Some(b) = tampered.wrapped_key.last_mut() {
            *b ^= 1;
        }
        assert!(matches!(
            open(&ring(&[("k1", 1)]), &tampered, b"a"),
            Err(CryptoError::Decrypt)
        ));
        let mut truncated = sealed;
        truncated.wrapped_key.truncate(4);
        assert!(matches!(
            open(&ring(&[("k1", 1)]), &truncated, b"a"),
            Err(CryptoError::Malformed)
        ));
    }

    #[test]
    fn a_wrapped_key_cannot_be_relabelled_to_another_key_id() {
        let r = ring(&[("k1", 1), ("k2", 1)]);
        let mut sealed = seal(&r, b"secret", b"a").unwrap();
        sealed.key_id = "k2".into();
        assert!(matches!(open(&r, &sealed, b"a"), Err(CryptoError::Decrypt)));
    }

    #[test]
    fn unknown_key_ids_are_named() {
        let sealed = seal(&ring(&[("old", 1)]), b"secret", b"a").unwrap();
        let err = open(&ring(&[("new", 2)]), &sealed, b"a").unwrap_err();
        assert!(err.to_string().contains("`old`"));
    }

    #[test]
    fn rotation_rewraps_without_touching_the_ciphertext() {
        let old = ring(&[("k1", 1)]);
        let sealed = seal(&old, b"secret", b"a").unwrap();
        let rotated = ring(&[("k2", 2), ("k1", 1)]);
        let new = rewrap(&rotated, &sealed).unwrap().unwrap();
        assert_eq!(new.key_id, "k2");
        assert_eq!(new.ciphertext, sealed.ciphertext);
        assert_ne!(new.wrapped_key, sealed.wrapped_key);
        assert!(rewrap(&rotated, &new).unwrap().is_none());
        let only_new = ring(&[("k2", 2)]);
        assert_eq!(
            open(&only_new, &new, b"a")
                .unwrap()
                .expose_secret()
                .as_slice(),
            b"secret"
        );
    }
}
