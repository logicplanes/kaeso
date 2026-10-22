use url::Url;

use crate::platform::error::{ApiError, ApiResult};

pub const MAX_LEN: usize = 2048;

const NOT_CONSOLE: &[&str] = &["platform", "v1"];

pub fn target(base: &Url, path: &str) -> Option<Url> {
    let shaped = path.len() <= MAX_LEN
        && path.starts_with('/')
        && !path.starts_with("//")
        && !path
            .chars()
            .any(|c| c == '\\' || c.is_control() || c.is_whitespace());
    if !shaped {
        return None;
    }
    let joined = base.join(path.get(1..)?).ok()?;
    let rest = joined.path().strip_prefix(base.path())?;
    // Dot segments can collapse `/.//host` into `//host`, which a browser reads as another origin.
    if rest.starts_with('/') {
        return None;
    }
    let first = rest.split('/').next().unwrap_or_default();
    let same_site = joined.origin() == base.origin() && joined.username().is_empty();
    (same_site && !NOT_CONSOLE.contains(&first)).then_some(joined)
}

pub fn validate(base: &Url, next: Option<&str>) -> ApiResult<Option<String>> {
    match next {
        None => Ok(None),
        Some(path) if target(base, path).is_some() => Ok(Some(path.to_owned())),
        Some(_) => Err(ApiError::invalid(
            "next",
            "`next` must be a console path on this install, starting with one `/`.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://kaeso.example.com/").unwrap()
    }

    fn to(path: &str) -> Option<String> {
        target(&base(), path).map(|u| u.to_string())
    }

    #[test]
    fn console_paths_stay_on_the_install() {
        assert_eq!(to("/").as_deref(), Some("https://kaeso.example.com/"));
        assert_eq!(
            to("/orgs/org_1/projects/proj_2/logs?period=7d#top").as_deref(),
            Some("https://kaeso.example.com/orgs/org_1/projects/proj_2/logs?period=7d#top")
        );
        assert_eq!(
            to("/settings/../orgs").as_deref(),
            Some("https://kaeso.example.com/orgs")
        );
        assert_eq!(
            to("/@evil.example").as_deref(),
            Some("https://kaeso.example.com/@evil.example")
        );
    }

    #[test]
    fn anything_that_could_leave_the_install_is_refused() {
        for path in [
            "",
            "orgs",
            "//evil.example",
            "//evil.example/orgs",
            "/\\evil.example",
            "/\\/evil.example",
            "https://evil.example",
            "/orgs\r\nLocation: https://evil.example",
            "/orgs\t",
            "/ orgs",
            "/platform/v1/auth/oidc/start",
            "/platform",
            "/v1/models",
            "/../../platform/v1/auth/session",
            "/.//evil.example",
            "/%2e//evil.example",
            "/%2E//evil.example",
            "/a/..//evil.example",
            "/./\\evil.example",
            "/orgs/../..//evil.example",
        ] {
            assert_eq!(to(path), None, "{path:?}");
        }
        assert_eq!(to(&format!("/{}", "a".repeat(MAX_LEN))), None);
    }

    #[test]
    fn a_refused_next_names_the_field() {
        assert_eq!(validate(&base(), None).unwrap(), None);
        assert_eq!(
            validate(&base(), Some("/orgs")).unwrap().as_deref(),
            Some("/orgs")
        );
        let refused = validate(&base(), Some("//evil.example")).unwrap_err();
        assert!(refused.to_string().contains("`next`"), "{refused}");
        assert!(validate(&base(), Some("/a/..//evil.example")).is_err());
    }

    #[test]
    fn an_install_under_a_path_refuses_the_same_forms() {
        let base = Url::parse("https://kaeso.example.com/kaeso/").unwrap();
        assert_eq!(
            target(&base, "/orgs").map(|u| u.to_string()).as_deref(),
            Some("https://kaeso.example.com/kaeso/orgs")
        );
        for path in [
            "/.//evil.example",
            "/a/..//evil.example",
            "/%2e//evil.example",
        ] {
            assert_eq!(target(&base, path), None, "{path:?}");
        }
    }
}
