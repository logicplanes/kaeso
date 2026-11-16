use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use url::Url;

pub const ALLOW_METHODS: &str = "GET, POST, DELETE";

pub const MAX_AGE_SECONDS: &str = "7200";

pub const EXPOSE_HEADERS: &str = "kaeso-request-id, kaeso-upstream-request-id, kaeso-cost, kaeso-provider, kaeso-model, kaeso-left-out, kaeso-adapted, kaeso-route, kaeso-cache, server-timing, age, retry-after, retry-after-ms";

pub const ALLOWED_HEADERS: &[&str] = &[
    "accept",
    "anthropic-beta",
    "anthropic-dangerous-direct-browser-access",
    "anthropic-version",
    "authorization",
    "cache-control",
    "content-type",
    "idempotency-key",
    "kaeso-log-content",
    "kaeso-tags",
    "kaeso-timeout",
    "openai-beta",
    "openai-organization",
    "openai-project",
    "traceparent",
    "tracestate",
    "user-agent",
    "x-api-key",
    "x-goog-api-client",
    "x-goog-api-key",
    "xi-api-key",
];

pub const STAINLESS_PREFIX: &str = "x-stainless-";

const MAX_STAINLESS_SUFFIX: usize = 48;

const PREFLIGHT_VARY: &str =
    "origin, access-control-request-method, access-control-request-headers";

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OriginError {
    #[error(
        "must not be `*`; list the origin of every web app that may call the model API, such as `https://app.example.com`"
    )]
    Wildcard,
    #[error("lists `{0}`, which is not an http or https origin such as `https://app.example.com`")]
    NotAnOrigin(String),
    #[error(
        "lists `{0}`, which has a path, query or user; write only the scheme, host and port, such as `https://app.example.com`"
    )]
    NotBare(String),
}

pub fn normalise(raw: &str) -> Result<String, OriginError> {
    let raw = raw.trim();
    if raw == "*" {
        return Err(OriginError::Wildcard);
    }
    let not_an_origin = || OriginError::NotAnOrigin(raw.to_owned());
    let url = Url::parse(raw).map_err(|_| not_an_origin())?;
    let host_is_plain = match url.host() {
        Some(url::Host::Domain(d)) => d
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'),
        Some(_) => true,
        None => false,
    };
    if !matches!(url.scheme(), "http" | "https") || !host_is_plain {
        return Err(not_an_origin());
    }
    if url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(OriginError::NotBare(raw.to_owned()));
    }
    Ok(url.origin().ascii_serialization())
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HeaderError {
    #[error(
        "must not be `*`; list the headers browser apps send beyond the SDKs', such as `x-plan`"
    )]
    Wildcard,
    #[error("lists `{0}`, which is not a header name")]
    NotAName(String),
    #[error(
        "lists `{0}`, which a browser app must never set: Kaeso reads cookies and forwarded addresses only from its own proxy"
    )]
    Reserved(String),
}

/// Headers the install lets browser apps on its listed origins send beyond the SDKs' own, such
/// as one an alias route splits on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtraHeaders(BTreeSet<String>);

impl ExtraHeaders {
    pub fn parse(raw: &str) -> Result<ExtraHeaders, HeaderError> {
        raw.split(',')
            .map(|n| n.trim().to_ascii_lowercase())
            .filter(|n| !n.is_empty())
            .map(|name| {
                if name == "*" {
                    return Err(HeaderError::Wildcard);
                }
                if axum::http::HeaderName::from_bytes(name.as_bytes()).is_err() {
                    return Err(HeaderError::NotAName(name));
                }
                if name == "cookie" || name == "forwarded" || name.starts_with("x-forwarded-") {
                    return Err(HeaderError::Reserved(name));
                }
                Ok(name)
            })
            .collect::<Result<_, _>>()
            .map(ExtraHeaders)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains(name)
    }
}

/// What the CORS middleware answers with: the origins that get CORS headers and the headers
/// their preflights may name beyond the fixed list.
#[derive(Clone, Debug, Default)]
pub struct Cors {
    pub origins: AllowedOrigins,
    pub headers: ExtraHeaders,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllowedOrigins(BTreeSet<String>);

impl AllowedOrigins {
    pub fn parse(raw: &str) -> Result<AllowedOrigins, OriginError> {
        raw.split(',')
            .map(str::trim)
            .filter(|o| !o.is_empty())
            .map(normalise)
            .collect::<Result<_, _>>()
            .map(AllowedOrigins)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    pub fn allows(&self, origin: &str) -> bool {
        normalise(origin).is_ok_and(|o| self.0.contains(&o))
    }
}

pub fn in_scope(path: &str) -> bool {
    ["/v1", "/v1beta", "/v2"].iter().any(|p| {
        path == *p
            || path
                .strip_prefix(p)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

pub fn allowed_header(name: &str) -> bool {
    ALLOWED_HEADERS.contains(&name)
        || name.strip_prefix(STAINLESS_PREFIX).is_some_and(|rest| {
            !rest.is_empty()
                && rest.len() <= MAX_STAINLESS_SUFFIX
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

pub fn allow_headers(headers: &HeaderMap, extra: &ExtraHeaders) -> Option<HeaderValue> {
    let names: BTreeSet<String> = headers
        .get_all(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|n| n.trim().to_ascii_lowercase())
        .filter(|n| allowed_header(n) || extra.contains(n))
        .collect();
    if names.is_empty() {
        return None;
    }
    HeaderValue::from_str(&names.into_iter().collect::<Vec<_>>().join(", ")).ok()
}

fn is_preflight(req: &Request) -> bool {
    req.method() == Method::OPTIONS
        && req.headers().contains_key(header::ORIGIN)
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
}

fn listed_origin(origins: &AllowedOrigins, headers: &HeaderMap) -> Option<HeaderValue> {
    headers
        .get(header::ORIGIN)
        .filter(|o| o.to_str().is_ok_and(|o| origins.allows(o)))
        .cloned()
}

fn preflight(origin: Option<HeaderValue>, headers: &HeaderMap, extra: &ExtraHeaders) -> Response {
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = StatusCode::NO_CONTENT;
    let h = resp.headers_mut();
    h.insert(header::VARY, HeaderValue::from_static(PREFLIGHT_VARY));
    let Some(origin) = origin else {
        return resp;
    };
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(ALLOW_METHODS),
    );
    if let Some(names) = allow_headers(headers, extra) {
        h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, names);
    }
    h.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static(MAX_AGE_SECONDS),
    );
    resp
}

pub async fn middleware(State(cors): State<Arc<Cors>>, req: Request, next: Next) -> Response {
    if !in_scope(req.uri().path()) {
        return next.run(req).await;
    }
    let origin = listed_origin(&cors.origins, req.headers());
    if is_preflight(&req) {
        return preflight(origin, req.headers(), &cors.headers);
    }
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.append(header::VARY, HeaderValue::from_static("origin"));
    if let Some(origin) = origin {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        h.insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(EXPOSE_HEADERS),
        );
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_are_normalised_to_scheme_host_and_port() {
        for (raw, expected) in [
            ("https://app.example.com", "https://app.example.com"),
            ("https://app.example.com/", "https://app.example.com"),
            ("  https://App.Example.COM  ", "https://app.example.com"),
            ("HTTPS://app.example.com", "https://app.example.com"),
            ("https://app.example.com:443", "https://app.example.com"),
            ("http://app.example.com:80", "http://app.example.com"),
            (
                "https://app.example.com:8443",
                "https://app.example.com:8443",
            ),
            ("http://localhost:5173", "http://localhost:5173"),
            ("http://127.0.0.1:3000", "http://127.0.0.1:3000"),
            ("http://[::1]:3000", "http://[::1]:3000"),
            ("https://bücher.example", "https://xn--bcher-kva.example"),
        ] {
            assert_eq!(normalise(raw).unwrap(), expected, "{raw}");
        }
    }

    #[test]
    fn values_that_are_no_bare_origin_are_refused() {
        assert_eq!(normalise("*"), Err(OriginError::Wildcard));
        assert_eq!(normalise(" * "), Err(OriginError::Wildcard));
        for raw in [
            "app.example.com",
            "null",
            "ftp://app.example.com",
            "file:///tmp/app",
            "data:text/plain,hi",
            "https://",
            "https://*.example.com",
        ] {
            assert_eq!(
                normalise(raw),
                Err(OriginError::NotAnOrigin(raw.to_owned())),
                "{raw}"
            );
        }
        for raw in [
            "https://app.example.com/app",
            "https://app.example.com//",
            "https://app.example.com/?a=1",
            "https://app.example.com?",
            "https://app.example.com/#top",
            "https://user@app.example.com",
            "https://user:pw@app.example.com",
        ] {
            assert_eq!(
                normalise(raw),
                Err(OriginError::NotBare(raw.to_owned())),
                "{raw}"
            );
        }
    }

    #[test]
    fn the_messages_name_the_value_and_the_fix() {
        assert_eq!(
            OriginError::Wildcard.to_string(),
            "must not be `*`; list the origin of every web app that may call the model API, such as `https://app.example.com`"
        );
        assert_eq!(
            normalise("https://app.example.com/app")
                .unwrap_err()
                .to_string(),
            "lists `https://app.example.com/app`, which has a path, query or user; write only the scheme, host and port, such as `https://app.example.com`"
        );
        assert_eq!(
            normalise("app.example.com").unwrap_err().to_string(),
            "lists `app.example.com`, which is not an http or https origin such as `https://app.example.com`"
        );
    }

    #[test]
    fn a_list_is_comma_separated_and_empty_entries_are_skipped() {
        let list = AllowedOrigins::parse(
            "https://app.example.com, http://localhost:5173/,,https://APP.example.com",
        )
        .unwrap();
        assert_eq!(
            list.iter().collect::<Vec<_>>(),
            ["http://localhost:5173", "https://app.example.com"]
        );
        assert!(!list.is_empty());
        assert!(AllowedOrigins::parse("").unwrap().is_empty());
        assert!(AllowedOrigins::parse(" , ").unwrap().is_empty());
        assert_eq!(
            AllowedOrigins::parse("https://app.example.com,*"),
            Err(OriginError::Wildcard)
        );
        assert_eq!(
            AllowedOrigins::parse("https://app.example.com,https://b.example.com/x"),
            Err(OriginError::NotBare("https://b.example.com/x".into()))
        );
    }

    #[test]
    fn only_listed_origins_match_exactly() {
        let list = AllowedOrigins::parse("https://app.example.com,http://localhost:5173").unwrap();
        for origin in [
            "https://app.example.com",
            "https://app.example.com:443",
            "https://APP.example.com",
            "http://localhost:5173",
        ] {
            assert!(list.allows(origin), "{origin}");
        }
        for origin in [
            "http://app.example.com",
            "https://app.example.com:8443",
            "https://evil.app.example.com",
            "https://app.example.com.evil.example",
            "https://example.com",
            "http://localhost:5174",
            "http://localhost",
            "null",
            "*",
            "",
            "https://app.example.com/path",
        ] {
            assert!(!list.allows(origin), "{origin}");
        }
        assert!(!AllowedOrigins::default().allows("https://app.example.com"));
    }

    #[test]
    fn the_model_api_is_the_only_scope() {
        for path in [
            "/v1",
            "/v1/",
            "/v1/chat/completions",
            "/v1/models/gpt-4o",
            "/v1beta",
            "/v1beta/models/gemini-3.5-flash:generateContent",
            "/v2/voices",
        ] {
            assert!(in_scope(path), "{path}");
        }
        for path in [
            "/",
            "/v10",
            "/v1betax/models",
            "/v1x/models",
            "/v2x/voices",
            "/platform/v1",
            "/platform/v1/me",
            "/healthz",
            "/readyz",
            "/metrics",
        ] {
            assert!(!in_scope(path), "{path}");
        }
    }

    #[test]
    fn sdk_headers_are_allowed_and_others_are_not() {
        for name in [
            "authorization",
            "x-api-key",
            "content-type",
            "accept",
            "user-agent",
            "anthropic-version",
            "anthropic-beta",
            "anthropic-dangerous-direct-browser-access",
            "openai-organization",
            "openai-project",
            "openai-beta",
            "idempotency-key",
            "kaeso-log-content",
            "kaeso-timeout",
            "x-goog-api-key",
            "x-goog-api-client",
            "cache-control",
            "x-stainless-arch",
            "x-stainless-retry-count",
            "x-stainless-helper-method",
            "x-stainless-package-version",
            "x-stainless-runtime-version",
        ] {
            assert!(allowed_header(name), "{name}");
        }
        for name in [
            "cookie",
            "x-forwarded-for",
            "x-kaeso-access",
            "x-stainless-",
            "x-stainless-UPPER",
            "x-stainless-a_b",
            "x-stainless-a b",
            "Authorization",
            "",
        ] {
            assert!(!allowed_header(name), "{name}");
        }
        let long = format!("{STAINLESS_PREFIX}{}", "a".repeat(MAX_STAINLESS_SUFFIX));
        assert!(allowed_header(&long));
        let longer = format!("{STAINLESS_PREFIX}{}", "a".repeat(MAX_STAINLESS_SUFFIX + 1));
        assert!(!allowed_header(&longer));
    }

    fn requested(values: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn requested_headers_are_reflected_only_when_allowed() {
        let h = requested(&[
            "Authorization, Content-Type,x-stainless-os , X-Stainless-Lang",
            "cookie, x-api-key, authorization, Cache-Control",
        ]);
        let none = ExtraHeaders::default();
        assert_eq!(
            allow_headers(&h, &none).unwrap(),
            "authorization, cache-control, content-type, x-api-key, x-stainless-lang, x-stainless-os"
        );
        assert_eq!(
            allow_headers(&requested(&["cookie, x-forwarded-for"]), &none),
            None
        );
        assert_eq!(allow_headers(&requested(&[""]), &none), None);
        assert_eq!(allow_headers(&HeaderMap::new(), &none), None);
        let extra = ExtraHeaders::parse(" X-Plan, x-user-id ,").unwrap();
        assert_eq!(
            allow_headers(&requested(&["x-plan, X-User-Id, x-tenant"]), &extra).unwrap(),
            "x-plan, x-user-id",
            "listed extra headers only"
        );
        assert_eq!(allow_headers(&requested(&["x-plan"]), &none), None);
    }

    #[test]
    fn extra_headers_refuse_a_wildcard_bad_names_and_reserved_ones() {
        assert_eq!(ExtraHeaders::parse("").unwrap(), ExtraHeaders::default());
        assert_eq!(ExtraHeaders::parse("x-plan, *"), Err(HeaderError::Wildcard));
        assert_eq!(
            ExtraHeaders::parse("x plan"),
            Err(HeaderError::NotAName("x plan".into()))
        );
        for reserved in ["Cookie", "forwarded", "X-Forwarded-For", "x-forwarded-host"] {
            assert_eq!(
                ExtraHeaders::parse(reserved),
                Err(HeaderError::Reserved(reserved.to_ascii_lowercase())),
                "{reserved}"
            );
        }
    }
}
