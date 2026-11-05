use sha2::{Digest, Sha256};

pub const MAX_NAME: usize = 64;
pub const MAX_HANDLE: usize = 24;
const SEPARATOR: &str = "__";
const KEPT: usize = 56;
const HASH_CHARS: usize = 7;

pub fn valid_handle(handle: &str) -> bool {
    (1..=MAX_HANDLE).contains(&handle.len())
        && handle
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

// Clients take `[a-zA-Z0-9_-]{1,64}` from every MCP server they connect to. A name that is too
// long or holds other characters is cut to 56 characters and gets `_` and 7 hex characters of the
// SHA-256 of `handle__name` as the server wrote it, so two tools never share a name.
pub fn exposed(handle: &str, tool: &str) -> String {
    let full = format!("{}{SEPARATOR}{tool}", handle.replace('-', "_"));
    if full.len() <= MAX_NAME && full.chars().all(allowed) {
        return full;
    }
    let digest = Sha256::digest(format!("{handle}{SEPARATOR}{tool}").as_bytes());
    let hash: String = digest
        .iter()
        .flat_map(|b| [b >> 4, b & 0xf])
        .take(HASH_CHARS)
        .map(|n| char::from_digit(u32::from(n), 16).unwrap_or('0'))
        .collect();
    let kept: String = full
        .chars()
        .map(|c| if allowed(c) { c } else { '_' })
        .take(KEPT)
        .collect();
    format!("{kept}_{hash}")
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn name_of_len(handle: &str, len: usize) -> String {
        "a".repeat(len - handle.len() - SEPARATOR.len())
    }

    #[test]
    fn short_names_are_handle_and_name() {
        assert_eq!(exposed("x", "create_post"), "x__create_post");
        assert_eq!(exposed("github", "list-repos"), "github__list-repos");
    }

    #[test]
    fn dashes_in_handles_become_underscores() {
        assert_eq!(exposed("my-crm", "find"), "my_crm__find");
    }

    #[test]
    fn a_name_of_64_characters_is_kept() {
        let tool = name_of_len("x", 64);
        let name = exposed("x", &tool);
        assert_eq!(name.len(), 64);
        assert_eq!(name, format!("x__{tool}"));
    }

    #[test]
    fn a_name_of_65_characters_is_cut_and_hashed() {
        let tool = name_of_len("x", 65);
        let name = exposed("x", &tool);
        assert_eq!(name.len(), 64);
        let (kept, hash) = name.split_at(56);
        assert_eq!(kept, &format!("x__{tool}")[..56]);
        let digest = Sha256::digest(format!("x__{tool}").as_bytes());
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hash, format!("_{}", &hex[..7]));
    }

    #[test]
    fn long_names_that_share_a_start_stay_apart() {
        let base = "a".repeat(70);
        assert_ne!(
            exposed("x", &format!("{base}1")),
            exposed("x", &format!("{base}2"))
        );
    }

    #[test]
    fn other_characters_are_replaced_and_hashed() {
        let dotted = exposed("gh", "repos.list");
        let underscored = exposed("gh", "repos_list");
        assert!(dotted.starts_with("gh__repos_list_"));
        assert_eq!(dotted.len(), "gh__repos_list".len() + 8);
        assert_eq!(underscored, "gh__repos_list");
        assert_ne!(dotted, underscored);
        assert!(exposed("gh", "übersicht").starts_with("gh___bersicht_"));
    }

    #[test]
    fn handles_follow_the_rules() {
        assert!(valid_handle("x"));
        assert!(valid_handle("github-2"));
        assert!(valid_handle(&"a".repeat(24)));
        assert!(!valid_handle(""));
        assert!(!valid_handle(&"a".repeat(25)));
        assert!(!valid_handle("GitHub"));
        assert!(!valid_handle("my_crm"));
        assert!(!valid_handle("x y"));
    }

    proptest! {
        #[test]
        fn every_exposed_name_is_valid(handle in "[a-z0-9-]{1,24}", tool in "\\PC{0,200}") {
            let name = exposed(&handle, &tool);
            prop_assert!(!name.is_empty() && name.len() <= MAX_NAME);
            prop_assert!(name.chars().all(allowed));
        }
    }
}
