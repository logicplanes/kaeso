use secrecy::{ExposeSecret, SecretBox};
use totp_rs::{Algorithm, Builder, Totp};

use crate::random::random_bytes;

pub const ISSUER: &str = "Kaeso";

#[derive(Debug, thiserror::Error)]
#[error("the TOTP secret is invalid")]
pub struct TotpError;

pub fn new_secret() -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(random_bytes::<20>().to_vec()))
}

fn totp(secret: &SecretBox<Vec<u8>>, account: &str) -> Result<Totp, TotpError> {
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret.expose_secret().clone())
        .with_account_name(account)
        .with_issuer(Some(ISSUER))
        .build()
        .map_err(|_| TotpError)
}

pub fn uri(secret: &SecretBox<Vec<u8>>, account: &str) -> Result<String, TotpError> {
    totp(secret, account)?.to_url().map_err(|_| TotpError)
}

pub fn base32(secret: &SecretBox<Vec<u8>>) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let bytes = secret.expose_secret();
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &b in bytes.iter() {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((buffer >> bits) & 0x1f) as usize;
            out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
        }
    }
    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0x1f) as usize;
        out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
    }
    out
}

pub fn check(secret: &SecretBox<Vec<u8>>, code: &str, unix_seconds: u64) -> Option<u64> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    totp(secret, "check").ok()?.check(&code, unix_seconds)
}

pub fn code_at(secret: &SecretBox<Vec<u8>>, unix_seconds: u64) -> Option<String> {
    totp(secret, "code")
        .ok()
        .map(|t| t.generate(unix_seconds).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc_secret() -> SecretBox<Vec<u8>> {
        SecretBox::new(Box::new(b"12345678901234567890".to_vec()))
    }

    #[test]
    fn rfc_6238_vectors_match_the_last_six_digits() {
        let s = rfc_secret();
        assert_eq!(code_at(&s, 59).unwrap(), "287082");
        assert_eq!(code_at(&s, 1_111_111_109).unwrap(), "081804");
        assert_eq!(code_at(&s, 1_234_567_890).unwrap(), "005924");
    }

    #[test]
    fn one_step_of_skew_is_accepted() {
        let s = rfc_secret();
        let now = 1_234_567_890;
        let code = code_at(&s, now).unwrap();
        assert_eq!(check(&s, &code, now), Some(now / 30));
        assert_eq!(check(&s, &code, now + 30), Some(now / 30));
        assert_eq!(check(&s, &code, now - 30), Some(now / 30));
        assert_eq!(check(&s, &code, now + 61), None);
        assert_eq!(
            check(&s, &format!("{} {}", &code[..3], &code[3..]), now),
            Some(now / 30)
        );
        assert_eq!(check(&s, "12345", now), None);
        assert_eq!(check(&s, "abcdef", now), None);
    }

    #[test]
    fn uris_and_base32_carry_the_secret() {
        let s = rfc_secret();
        assert_eq!(base32(&s), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        let uri = uri(&s, "owner@example.com").unwrap();
        assert!(
            uri.starts_with("otpauth://totp/Kaeso:owner%40example.com?"),
            "{uri}"
        );
        assert!(
            uri.contains("secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            "{uri}"
        );
        assert!(uri.contains("issuer=Kaeso"), "{uri}");
    }
}
