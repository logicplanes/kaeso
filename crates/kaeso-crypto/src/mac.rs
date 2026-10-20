use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretBox};
use sha2::Sha256;
use zeroize::Zeroize;

use crate::KeyRing;

type HmacSha256 = Hmac<Sha256>;

pub struct MacKey(SecretBox<[u8; 32]>);

impl fmt::Debug for MacKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MacKey([redacted])")
    }
}

impl MacKey {
    pub const TAG_LEN: usize = 32;

    pub fn derive(ring: &KeyRing, purpose: &str) -> MacKey {
        let mut key = keyed(ring.active().bytes());
        key.update(b"kaeso:");
        key.update(purpose.as_bytes());
        let derived: [u8; 32] = key.finalize().into_bytes().into();
        MacKey(SecretBox::new(Box::new(derived)))
    }

    pub fn tag(&self, parts: &[&[u8]]) -> [u8; MacKey::TAG_LEN] {
        self.of(parts).finalize().into_bytes().into()
    }

    pub fn verify(&self, parts: &[&[u8]], tag: &[u8]) -> bool {
        self.of(parts).verify_slice(tag).is_ok()
    }

    fn of(&self, parts: &[&[u8]]) -> HmacSha256 {
        let mut mac = keyed(self.0.expose_secret());
        for part in parts {
            let len = u64::try_from(part.len()).unwrap_or(u64::MAX);
            mac.update(&len.to_be_bytes());
            mac.update(part);
        }
        mac
    }
}

fn keyed(key: &[u8; 32]) -> HmacSha256 {
    let mut block = [0_u8; 64];
    for (b, k) in block.iter_mut().zip(key) {
        *b = *k;
    }
    let mac = HmacSha256::new(&block.into());
    block.zeroize();
    mac
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> KeyRing {
        KeyRing::parse(&crate::generate_master_key()).unwrap()
    }

    #[test]
    fn tags_bind_the_key_the_purpose_and_every_part() {
        let a = ring();
        let mac = MacKey::derive(&a, "cache");
        let tag = mac.tag(&[b"key", b"value"]);
        assert!(mac.verify(&[b"key", b"value"], &tag));
        assert!(!mac.verify(&[b"key", b"other"], &tag));
        assert!(!mac.verify(&[b"keyv", b"alue"], &tag));
        assert!(!mac.verify(&[b"key", b"value"], &tag[..31]));
        assert!(!MacKey::derive(&a, "other").verify(&[b"key", b"value"], &tag));
        let b = ring();
        assert!(!MacKey::derive(&b, "cache").verify(&[b"key", b"value"], &tag));
        assert_eq!(format!("{mac:?}"), "MacKey([redacted])");
    }
}
