use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

pub fn hashes_equal(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

const TOKEN_PREFIXES: &[&str] = &[
    "sk-", "sk_", "org-", "proj_", "proj-", "key-", "xai-", "gsk_", "nvapi-", "AIza",
];

const REDACTED: &str = "[redacted]";

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '*' | '.')
}

fn redacts(word: &str) -> Option<&'static str> {
    TOKEN_PREFIXES
        .iter()
        .copied()
        .find(|p| word.len() >= p.len() + 4 && word.starts_with(p))
}

pub fn redact_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(token_char) {
        let (before, from) = rest.split_at(start);
        out.push_str(before);
        let end = from.find(|c: char| !token_char(c)).unwrap_or(from.len());
        let (word, after) = from.split_at(end);
        let token = word.trim_end_matches('.');
        match redacts(token) {
            Some(prefix) => {
                out.push_str(prefix);
                out.push_str(REDACTED);
                out.push_str(word.get(token.len()..).unwrap_or_default());
            }
            None => out.push_str(word),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_like_tokens_are_redacted_and_the_rest_is_kept() {
        assert_eq!(
            redact_tokens(
                "Incorrect API key provided: sk-proj-****************abcd. You can find your API key at https://platform.example.com."
            ),
            "Incorrect API key provided: sk-[redacted]. You can find your API key at https://platform.example.com."
        );
        assert_eq!(
            redact_tokens("Organization org-AbCdEf123456 has no access to project proj_9z8y7x6w."),
            "Organization org-[redacted] has no access to project proj_[redacted]."
        );
        assert_eq!(
            redact_tokens("invalid x-api-key (sk-ant-api03-xyz...) for key-1234abcd"),
            "invalid x-api-key (sk-[redacted]...) for key-[redacted]"
        );
        assert_eq!(redact_tokens("ask-me sk- org-ab"), "ask-me sk- org-ab");
        assert_eq!(
            redact_tokens("Ünïcode sk-abcdefgh!"),
            "Ünïcode sk-[redacted]!"
        );
        assert_eq!(redact_tokens(""), "");
    }

    #[test]
    fn hashes_are_stable_and_compared_in_constant_time() {
        let a = hash("token");
        assert_eq!(a, hash("token"));
        assert!(hashes_equal(&a, &hash("token")));
        assert!(!hashes_equal(&a, &hash("other")));
        assert!(!hashes_equal(&a, &a[..31]));
    }
}
