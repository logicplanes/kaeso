use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

pub fn hashes_equal(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

const TOKEN_PREFIXES: &[&str] = &[
    "sk-", "sk_", "org-", "proj_", "proj-", "key-", "xai-", "gsk_", "nvapi-", "AIza", "tgp_v1_",
];

const ACCOUNT_LABELS: &[&str] = &[
    "org",
    "organization",
    "account",
    "user",
    "project",
    "workspace",
];

const ACCOUNT_FIELDS: &[&str] = &[
    "user_id",
    "account_id",
    "org_id",
    "organization_id",
    "project_id",
    "workspace_id",
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

fn labels_account(word: &str) -> bool {
    ACCOUNT_LABELS.iter().any(|l| word.eq_ignore_ascii_case(l))
}

fn joins_label(gap: &str) -> bool {
    !gap.is_empty()
        && gap
            .chars()
            .all(|c| c.is_whitespace() || matches!(c, ':' | '=' | '(' | '"' | '\''))
}

fn id_like(token: &str) -> bool {
    token.len() >= 8 && token.chars().any(|c| c.is_ascii_digit())
}

const JWT_HEADER: &str = "eyJ";

// RFC 7519 §7.2: a compact JWT is three base64url segments, and its header is a JSON object,
// so it starts with `eyJ`: OAuth access, refresh and ID tokens of providers that sign in.
fn jwt(token: &str) -> bool {
    let base64url = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    let mut parts = token.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(claims), Some(signature), None) => {
            header.starts_with(JWT_HEADER)
                && claims.len() >= 4
                && [header, claims, signature].into_iter().all(base64url)
        }
        _ => false,
    }
}

pub fn redact_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous = "";
    while let Some(start) = rest.find(token_char) {
        let (before, from) = rest.split_at(start);
        out.push_str(before);
        let end = from.find(|c: char| !token_char(c)).unwrap_or(from.len());
        let (word, after) = from.split_at(end);
        let token = word.trim_end_matches('.');
        let tail = word.get(token.len()..).unwrap_or_default();
        if let Some(prefix) = redacts(token) {
            out.push_str(prefix);
            out.push_str(REDACTED);
            out.push_str(tail);
        } else if jwt(token) {
            out.push_str(JWT_HEADER);
            out.push_str(REDACTED);
            out.push_str(tail);
        } else if labels_account(previous) && joins_label(before) && id_like(token) {
            out.push_str(REDACTED);
            out.push_str(tail);
        } else {
            out.push_str(word);
        }
        previous = token;
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn redact_json(value: &mut serde_json::Value) {
    use serde_json::Value;
    match value {
        Value::String(s) => *s = redact_tokens(s),
        Value::Array(items) => items.iter_mut().for_each(redact_json),
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                let account = ACCOUNT_FIELDS.iter().any(|f| key.eq_ignore_ascii_case(f));
                if account && (v.is_string() || v.is_number()) {
                    *v = Value::String(REDACTED.to_owned());
                } else {
                    redact_json(v);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
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
        assert_eq!(
            redact_tokens("Invalid API key tgp_v1_AbCdEf123456."),
            "Invalid API key tgp_v1_[redacted]."
        );
    }

    #[test]
    fn signed_tokens_are_redacted_wherever_they_appear() {
        let jwt = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyLTEifQ.c2lnbmF0dXJl-_x";
        assert_eq!(
            redact_tokens(&format!("Bearer {jwt} was refused.")),
            "Bearer eyJ[redacted] was refused."
        );
        assert_eq!(
            redact_tokens(&format!("token={jwt}&next=1")),
            "token=eyJ[redacted]&next=1"
        );
        assert_eq!(
            redact_tokens(&format!("The token was {jwt}.")),
            "The token was eyJ[redacted]."
        );
        assert_eq!(
            redact_tokens("version 1.2.3 of eyJ.a.b"),
            "version 1.2.3 of eyJ.a.b"
        );
        assert_eq!(
            redact_tokens("eyJhbGc.eyJz.c2ln.ZXh0cmE"),
            "eyJhbGc.eyJz.c2ln.ZXh0cmE"
        );
    }

    #[test]
    fn labelled_account_ids_are_redacted_and_plain_words_after_a_label_are_kept() {
        assert_eq!(
            redact_tokens(
                "This request would exceed your organization's rate limit (org: 0d5b7a5e-1c2f-4a59-9a8e-3f1d2c4b5a6e, model: claude-opus-5-5)."
            ),
            "This request would exceed your organization's rate limit (org: [redacted], model: claude-opus-5-5)."
        );
        assert_eq!(
            redact_tokens("No credits left on account 4417092233 or user=u8Xk2mPq9."),
            "No credits left on account [redacted] or user=[redacted]."
        );
        assert_eq!(
            redact_tokens("Your organization has no access to this project. Retry on 2026-09-01."),
            "Your organization has no access to this project. Retry on 2026-09-01."
        );
        assert_eq!(
            redact_tokens("(request_id: 1e3d4dbc-772c-4507-95de-ecd7100b7c3f)"),
            "(request_id: 1e3d4dbc-772c-4507-95de-ecd7100b7c3f)"
        );
    }

    #[test]
    fn every_string_of_a_json_body_is_redacted_and_account_fields_are_hidden() {
        let mut body = serde_json::json!({
            "error": {
                "message": "Rate limit reached for key sk-proj-Ab1xYz789.",
                "code": 429,
                "metadata": {
                    "raw": ["in organization org-AbCdEf123456", true, null],
                    "headers": { "Retry-After": "1" }
                }
            },
            "user_id": "user_2xQ9mLp7Vt4Kd8Rf",
            "Account_ID": 12_345_678,
            "project_id": { "nested": "sk-abcdefgh" }
        });
        redact_json(&mut body);
        assert_eq!(
            body,
            serde_json::json!({
                "error": {
                    "message": "Rate limit reached for key sk-[redacted].",
                    "code": 429,
                    "metadata": {
                        "raw": ["in organization org-[redacted]", true, null],
                        "headers": { "Retry-After": "1" }
                    }
                },
                "user_id": "[redacted]",
                "Account_ID": "[redacted]",
                "project_id": { "nested": "sk-[redacted]" }
            })
        );
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
