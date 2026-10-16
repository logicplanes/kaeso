use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderName, HeaderValue, header};

use super::error::{ApiError, ApiResult};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tag {
    Any,
    Strong(String),
    Weak,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IfMatch(Option<Vec<Tag>>);

fn malformed() -> ApiError {
    ApiError::invalid(
        "If-Match",
        "`If-Match` must be an entity tag from `ETag`, such as \"3\".",
    )
}

fn parse(text: &str) -> ApiResult<Vec<Tag>> {
    let mut tags = Vec::new();
    for item in text.split(',').map(str::trim).filter(|i| !i.is_empty()) {
        let (weak, quoted) = match item.strip_prefix("W/") {
            Some(rest) => (true, rest),
            None => (false, item),
        };
        if item == "*" {
            tags.push(Tag::Any);
            continue;
        }
        let opaque = quoted
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .filter(|t| !t.contains('"'))
            .ok_or_else(malformed)?;
        tags.push(if weak {
            Tag::Weak
        } else {
            Tag::Strong(opaque.to_owned())
        });
    }
    if tags.is_empty() {
        return Err(malformed());
    }
    Ok(tags)
}

impl<S: Send + Sync> FromRequestParts<S> for IfMatch {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let mut found: Option<Vec<Tag>> = None;
        for value in parts.headers.get_all(header::IF_MATCH) {
            let text = value.to_str().map_err(|_| malformed())?;
            found.get_or_insert_with(Vec::new).extend(parse(text)?);
        }
        Ok(IfMatch(found))
    }
}

impl IfMatch {
    pub fn check(&self, revision: i64, what: &str) -> ApiResult<()> {
        let Some(tags) = &self.0 else {
            return Ok(());
        };
        let current = revision.to_string();
        let matched = tags.iter().any(|t| match t {
            Tag::Any => true,
            Tag::Strong(tag) => *tag == current,
            Tag::Weak => false,
        });
        if matched {
            Ok(())
        } else {
            Err(ApiError::Stale(format!(
                "This {what} was changed since you loaded it; reload it and try again."
            )))
        }
    }
}

pub fn etag(revision: i64) -> [(HeaderName, HeaderValue); 1] {
    let value = HeaderValue::from_str(&format!("\"{revision}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("\"0\""));
    [(header::ETAG, value)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(header: Option<&str>) -> ApiResult<IfMatch> {
        let mut req = axum::http::Request::builder();
        if let Some(h) = header {
            req = req.header(header::IF_MATCH, h);
        }
        let (mut parts, ()) = req.body(()).unwrap().into_parts();
        futures::executor::block_on(IfMatch::from_request_parts(&mut parts, &()))
    }

    #[test]
    fn without_the_header_every_revision_is_changed() {
        assert!(read(None).unwrap().check(7, "limit").is_ok());
    }

    #[test]
    fn a_strong_tag_matches_only_its_revision() {
        let m = read(Some("\"6\", \"7\"")).unwrap();
        assert!(m.check(7, "limit").is_ok());
        let err = m.check(8, "limit").unwrap_err();
        assert_eq!(err.status(), axum::http::StatusCode::PRECONDITION_FAILED);
        assert_eq!(
            err.to_string(),
            "This limit was changed since you loaded it; reload it and try again."
        );
        assert!(read(Some("*")).unwrap().check(1, "limit").is_ok());
        assert!(read(Some("W/\"7\"")).unwrap().check(7, "limit").is_err());
    }

    #[test]
    fn malformed_tags_are_refused() {
        for bad in ["7", "\"7", "\"a\"b\"", ","] {
            let err = read(Some(bad)).unwrap_err();
            assert_eq!(err.status(), axum::http::StatusCode::BAD_REQUEST, "{bad}");
        }
    }

    #[test]
    fn the_tag_names_the_revision() {
        assert_eq!(etag(3)[0].1, "\"3\"");
    }
}
