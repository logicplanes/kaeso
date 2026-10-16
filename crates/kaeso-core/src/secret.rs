use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

pub fn hashes_equal(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable_and_compared_in_constant_time() {
        let a = hash("token");
        assert_eq!(a, hash("token"));
        assert!(hashes_equal(&a, &hash("token")));
        assert!(!hashes_equal(&a, &hash("other")));
        assert!(!hashes_equal(&a, &a[..31]));
    }
}
