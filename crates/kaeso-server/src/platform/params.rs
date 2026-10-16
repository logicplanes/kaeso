use axum::extract::rejection::{PathRejection, QueryRejection, RawPathParamsRejection};
use axum::extract::{FromRequestParts, path::ErrorKind};
use axum::http::request::Parts;
use kaeso_core::id::PublicId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::error::ApiError;

#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[expect(clippy::disallowed_types, reason = "the wrapper itself")]
        let extracted = axum::extract::Query::<T>::from_request_parts(parts, state).await;
        match extracted {
            Ok(axum::extract::Query(v)) => Ok(Query(v)),
            Err(QueryRejection::FailedToDeserializeQueryString(e)) => {
                Err(query_error(&e.body_text()))
            }
            Err(_) => Err(ApiError::bad_request(QUERY_FORM)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[expect(clippy::disallowed_types, reason = "the wrapper itself")]
        let extracted = axum::extract::Path::<T>::from_request_parts(parts, state).await;
        match extracted {
            Ok(axum::extract::Path(v)) => Ok(Path(v)),
            Err(PathRejection::FailedToDeserializePathParams(e)) => {
                let internal = e.status().is_server_error();
                let text = e.body_text();
                Err(match e.into_kind() {
                    ErrorKind::InvalidUtf8InPathParam { key } => not_utf8(&key),
                    ErrorKind::ParseErrorAtKey { key, .. }
                    | ErrorKind::DeserializeError { key, .. } => ApiError::invalid(
                        &key,
                        format!("`{key}` in the path does not have the expected form."),
                    ),
                    _ if internal => ApiError::internal(text),
                    _ => ApiError::bad_request(PATH_FORM),
                })
            }
            Err(e) => Err(ApiError::internal(e.body_text())),
        }
    }
}

pub fn raw_path_error(rejection: &RawPathParamsRejection) -> ApiError {
    match rejection {
        RawPathParamsRejection::InvalidUtf8InPathParam(e) => {
            let text = e.body_text();
            match text.split('`').nth(1) {
                Some(key) => not_utf8(key),
                None => ApiError::bad_request(PATH_FORM),
            }
        }
        other => ApiError::internal(other.body_text()),
    }
}

const QUERY_FORM: &str = "The query string does not have the form this endpoint expects.";
const PATH_FORM: &str = "The path does not have the form this endpoint expects.";

fn not_utf8(key: &str) -> ApiError {
    ApiError::invalid(key, format!("`{key}` in the path is not valid UTF-8."))
}

fn query_error(text: &str) -> ApiError {
    let text = text
        .strip_prefix("Failed to deserialize query string: ")
        .unwrap_or(text);
    let (path, detail) = match text.split_once(": ") {
        Some((p, d)) if !p.is_empty() && !p.contains(' ') => (Some(p), d),
        _ => (None, text),
    };
    let quoted = |prefix: &str| {
        detail
            .strip_prefix(prefix)
            .and_then(|rest| rest.split('`').next())
    };
    if let Some(name) = quoted("missing field `") {
        return ApiError::invalid(name, format!("`{name}` is required."));
    }
    if let Some(name) = quoted("duplicate field `") {
        return ApiError::invalid(name, format!("`{name}` is given more than once."));
    }
    match path {
        Some(p) => value_error(p, detail),
        None => {
            tracing::debug!(detail, "query string refused");
            ApiError::bad_request(QUERY_FORM)
        }
    }
}

pub fn value_error(param: &str, detail: &str) -> ApiError {
    let message = match detail {
        "provided string was not `true` or `false`" => {
            format!("`{param}` must be `true` or `false`.")
        }
        "invalid digit found in string" | "cannot parse integer from empty string" => {
            format!("`{param}` must be a whole number.")
        }
        "number too large to fit in target type" | "number too small to fit in target type" => {
            format!("`{param}` is out of range.")
        }
        _ => match detail.strip_prefix("expected ") {
            Some(what) if !what.contains(": ") => format!("`{param}` must be {what}."),
            _ if detail == "the id is malformed" => format!("`{param}` is not a valid id."),
            _ => format!("`{param}` has a value of the wrong type or form."),
        },
    };
    ApiError::invalid(param, message)
}

pub fn id_param<T: PublicId + FromStr>(raw: &str, what: &str) -> Result<T, ApiError> {
    raw.parse().map_err(|_| ApiError::not_found(what))
}

pub fn rfc3339<'de, D: Deserializer<'de>>(d: D) -> Result<Option<OffsetDateTime>, D::Error> {
    let Some(s) = Option::<std::borrow::Cow<'de, str>>::deserialize(d)? else {
        return Ok(None);
    };
    OffsetDateTime::parse(&s, &Rfc3339)
        .map(Some)
        .map_err(|_| serde::de::Error::custom("expected a time such as 2026-10-01T00:00:00Z"))
}

#[cfg(test)]
mod tests {

    use axum::http::Request;
    use kaeso_core::id::UserId;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct Filter {
        limit: Option<u32>,
        status: Option<u16>,
        include_archived: Option<bool>,
        user: Option<UserId>,
        #[serde(default, deserialize_with = "rfc3339")]
        from: Option<OffsetDateTime>,
    }

    #[derive(Debug, Deserialize)]
    struct Required {
        provider: String,
    }

    async fn reject<T: DeserializeOwned>(query: &str) -> (String, Option<String>) {
        let (mut parts, ()) = Request::builder()
            .uri(format!("/x?{query}"))
            .body(())
            .unwrap()
            .into_parts();
        let Err(e) = Query::<T>::from_request_parts(&mut parts, &()).await else {
            panic!("{query} was accepted");
        };
        match e {
            ApiError::InvalidRequest { message, param } => (message, param),
            other => panic!("{query}: {other:?}"),
        }
    }

    #[test]
    fn ids_in_the_path_that_do_not_parse_are_not_found() {
        let user = UserId::new();
        assert_eq!(
            id_param::<UserId>(&user.to_string(), "member").unwrap(),
            user
        );
        for raw in ["usr_nope", "proj_01k7a3f8m2xq4v9c5r6t8w0yz1", ""] {
            let e = id_param::<UserId>(raw, "member").unwrap_err();
            assert_eq!(e.status(), axum::http::StatusCode::NOT_FOUND, "{raw}");
            assert_eq!(e.to_string(), "No such member, or you cannot see it.");
        }
    }

    #[tokio::test]
    async fn a_valid_query_is_read_whole() {
        let user = UserId::new();
        let (mut parts, ()) = Request::builder()
            .uri(format!(
                "/x?limit=5&status=404&include_archived=true&user={user}&from=2026-10-01T00:00:00Z&provider=openai"
            ))
            .body(())
            .unwrap()
            .into_parts();
        let Query(filter) = Query::<Filter>::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(filter.limit, Some(5));
        assert_eq!(filter.status, Some(404));
        assert_eq!(filter.include_archived, Some(true));
        assert_eq!(filter.user, Some(user));
        assert!(filter.from.is_some());
        let Query(required) = Query::<Required>::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(required.provider, "openai");
    }

    #[tokio::test]
    async fn query_errors_name_the_parameter() {
        for (query, message, param) in [
            ("limit=abc", "`limit` must be a whole number.", "limit"),
            ("limit=-1", "`limit` must be a whole number.", "limit"),
            ("limit=", "`limit` must be a whole number.", "limit"),
            ("status=70000", "`status` is out of range.", "status"),
            (
                "include_archived=yes",
                "`include_archived` must be `true` or `false`.",
                "include_archived",
            ),
            (
                "user=bob",
                "`user` must be an id starting with `usr_`.",
                "user",
            ),
            ("user=usr_x", "`user` is not a valid id.", "user"),
            (
                "from=yesterday",
                "`from` must be a time such as 2026-10-01T00:00:00Z.",
                "from",
            ),
            (
                "limit=1&limit=2",
                "`limit` is given more than once.",
                "limit",
            ),
        ] {
            let (m, p) = reject::<Filter>(query).await;
            assert_eq!(m, message, "{query}");
            assert_eq!(p.as_deref(), Some(param), "{query}");
        }
        let (m, p) = reject::<Required>("model=x").await;
        assert_eq!(m, "`provider` is required.");
        assert_eq!(p.as_deref(), Some("provider"));
    }

    #[test]
    fn times_are_rfc3339() {
        #[derive(Deserialize)]
        struct T {
            #[serde(default, deserialize_with = "rfc3339")]
            from: Option<OffsetDateTime>,
        }
        let t: T = serde_json::from_str(r#"{"from": "2026-10-01T00:00:00Z"}"#).unwrap();
        assert!(t.from.is_some());
        let t: T = serde_json::from_str("{}").unwrap();
        assert!(t.from.is_none());
    }
}
