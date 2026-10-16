use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use zeroize::Zeroize;

use crate::random::random_bytes;

pub struct MasterKey {
    id: String,
    key: SecretBox<[u8; 32]>,
}

impl MasterKey {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        self.key.expose_secret()
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterKey")
            .field("id", &self.id)
            .field("key", &"[redacted]")
            .finish()
    }
}

#[derive(Debug)]
pub struct KeyRing {
    active: MasterKey,
    older: Vec<MasterKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyRingError {
    #[error(
        "no master key is configured; generate one with `kaeso master-key generate` and set KAESO_MASTER_KEYS or KAESO_MASTER_KEYS_FILE"
    )]
    Empty,
    #[error("master key entry {position} is not of the form `id:base64key`")]
    Format { position: usize },
    #[error(
        "master key entry {position} has an invalid id; use 1 to 32 letters, digits, `-` or `_`"
    )]
    InvalidId { position: usize },
    #[error("master key `{id}` is not 32 bytes of standard base64")]
    InvalidKey { id: String },
    #[error("master key id `{id}` appears more than once")]
    DuplicateId { id: String },
}

impl KeyRing {
    pub fn parse(value: &SecretString) -> Result<KeyRing, KeyRingError> {
        let mut keys: Vec<MasterKey> = Vec::new();
        for (i, entry) in value
            .expose_secret()
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .enumerate()
        {
            let position = i + 1;
            let (id, encoded) = entry
                .split_once(':')
                .ok_or(KeyRingError::Format { position })?;
            let id = id.trim();
            if id.is_empty()
                || id.len() > 32
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(KeyRingError::InvalidId { position });
            }
            if keys.iter().any(|k| k.id == id) {
                return Err(KeyRingError::DuplicateId { id: id.to_owned() });
            }
            let mut decoded = STANDARD
                .decode(encoded.trim())
                .map_err(|_| KeyRingError::InvalidKey { id: id.to_owned() })?;
            let key: Result<[u8; 32], _> = decoded.as_slice().try_into();
            decoded.zeroize();
            let key = key.map_err(|_| KeyRingError::InvalidKey { id: id.to_owned() })?;
            keys.push(MasterKey {
                id: id.to_owned(),
                key: SecretBox::new(Box::new(key)),
            });
        }
        let mut keys = keys.into_iter();
        let active = keys.next().ok_or(KeyRingError::Empty)?;
        Ok(KeyRing {
            active,
            older: keys.collect(),
        })
    }

    pub fn active(&self) -> &MasterKey {
        &self.active
    }

    pub fn get(&self, id: &str) -> Option<&MasterKey> {
        self.keys().find(|k| k.id == id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.keys().map(|k| k.id.as_str())
    }

    fn keys(&self) -> impl Iterator<Item = &MasterKey> {
        std::iter::once(&self.active).chain(&self.older)
    }
}

pub fn generate_master_key() -> SecretString {
    let id_bytes = random_bytes::<5>();
    let id = format!("k{}", crate::random::hex(&id_bytes));
    let mut key = random_bytes::<32>();
    let entry = format!("{id}:{}", STANDARD.encode(key));
    key.zeroize();
    SecretString::from(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(s: &str) -> Result<KeyRing, KeyRingError> {
        KeyRing::parse(&SecretString::from(s.to_owned()))
    }

    #[test]
    fn the_first_key_is_active() {
        let a = STANDARD.encode([1u8; 32]);
        let b = STANDARD.encode([2u8; 32]);
        let r = ring(&format!(" k2:{a} , k1:{b} ")).unwrap();
        assert_eq!(r.active().id(), "k2");
        assert_eq!(r.ids().collect::<Vec<_>>(), ["k2", "k1"]);
        assert_eq!(r.get("k1").unwrap().bytes(), &[2u8; 32]);
        assert!(r.get("k3").is_none());
    }

    #[test]
    fn bad_rings_are_refused_without_leaking_keys() {
        let key = STANDARD.encode([7u8; 32]);
        assert_eq!(ring("").unwrap_err(), KeyRingError::Empty);
        assert_eq!(ring(" , ").unwrap_err(), KeyRingError::Empty);
        assert_eq!(
            ring(&key).unwrap_err(),
            KeyRingError::Format { position: 1 }
        );
        assert_eq!(
            ring(&format!("bad id:{key}")).unwrap_err(),
            KeyRingError::InvalidId { position: 1 }
        );
        let short = STANDARD.encode([7u8; 16]);
        let err = ring(&format!("k1:{short}")).unwrap_err();
        assert_eq!(err, KeyRingError::InvalidKey { id: "k1".into() });
        assert!(!err.to_string().contains(&short));
        assert_eq!(
            ring(&format!("k1:{key},k1:{key}")).unwrap_err(),
            KeyRingError::DuplicateId { id: "k1".into() }
        );
    }

    #[test]
    fn generated_keys_parse_and_differ() {
        let a = generate_master_key();
        let b = generate_master_key();
        assert_ne!(a.expose_secret(), b.expose_secret());
        let r = KeyRing::parse(&a).unwrap();
        assert!(r.active().id().starts_with('k'));
        assert!(!format!("{r:?}").contains(a.expose_secret().split(':').nth(1).unwrap()));
    }
}
