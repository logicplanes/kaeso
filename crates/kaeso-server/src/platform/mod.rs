pub mod access;
pub mod audit;
pub mod audit_log;
pub mod auth;
pub mod avatars;
pub mod cookies;
pub mod error;
pub mod exchange_rates;
pub mod grants;
pub mod instance;
pub mod json;
pub mod keys;
pub mod logs;
pub mod me;
pub mod orgs;
pub mod pagination;
pub mod params;
pub mod precondition;
pub mod projects;
pub mod providers;
pub mod response_cache;
pub mod rights;
pub mod share;
pub mod stored_responses;
pub mod tool_logs;
pub mod usage;
pub mod validate;
pub mod views;

use axum::Router;
use axum::routing::any;
use utoipa::openapi::response::ResponseBuilder;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{ContentBuilder, OpenApi as OpenApiDoc, Ref};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;

use crate::state::AppState;
use access::Secure;
use error::ErrorEnvelope;

pub const PREFIX: &str = "/platform/v1";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kaeso Platform API",
        description = "Manage a Kaeso install: organizations, projects, members, keys, provider credentials, models, limits, usage and logs. Every operation names the single right it checks. Errors are `{\"error\": {\"type\", \"message\", \"param\"}}`. Lists are paginated with `limit` and `after`.",
        license(name = "MIT", identifier = "MIT"),
    ),
    // The request log filters' formats and endpoints, which only parameters name, so nothing
    // else adds them.
    components(schemas(ErrorEnvelope, error::ErrorBody, RequestFormat, LoggedEndpoint)),
    modifiers(&Components),
    tags(
        (name = "auth", description = "Sign-in and sessions"),
        (name = "me", description = "The signed-in user's own account"),
        (name = "agents", description = "Agents connected through the MCP server, their rights and policies"),
        (name = "instance", description = "Instance administration (instance admins only)"),
        (name = "orgs", description = "Organizations"),
        (name = "projects", description = "Projects"),
        (name = "models", description = "Model catalogues and aliases"),
        (name = "logs", description = "Request logs, retention and storage"),
        (name = "response-cache", description = "The exact-match response cache: who may use it, how long replies are kept, clearing"),
        (name = "stored-responses", description = "Responses kept at providers for `previous_response_id`: who may keep them"),
    )
)]
struct ApiDoc;

struct Components;

impl Modify for Components {
    fn modify(&self, doc: &mut OpenApiDoc) {
        let components = doc.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::platform::cookies::NAME,
                "The console session cookie, set by the sign-in endpoints. Every request other than `GET`, `HEAD` and `OPTIONS` that carries it, and every such request to a public `auth/*` endpoint, must also send `Origin` with the origin of the install's public URL; `orgs/{org}/*` requests with an API key need none.",
            ))),
        );
        components.add_security_scheme(
            "api_key",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "An API key with platform access (`sk-kso-…`, acting as its principal inside its project) or an org platform key (`sk-kso-org-…`).",
                    ))
                    .build(),
            ),
        );
        components.add_security_scheme(
            "agent",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "An agent's access token (`kso_at_…`) from the OAuth sign-in of the MCP server at `/mcp`. The agent acts for its user with the rights both hold, within the instance's and each org's agent policies.",
                    ))
                    .build(),
            ),
        );
        for (name, description) in [
            (
                "InvalidRequest",
                "The request is malformed or a value is invalid (`invalid_request`).",
            ),
            (
                "Unauthenticated",
                "Missing or invalid credentials (`authentication`).",
            ),
            (
                "Forbidden",
                "The caller lacks the right, the org requires 2FA or SSO, a recent sign-in is needed, or a session from the SSO of an organization that doesn't manage the account can't do this (`permission_denied`, `recent_sign_in_required`, `other_sign_in_required`).",
            ),
            (
                "NotFound",
                "The resource does not exist or is not visible to the caller (`not_found`).",
            ),
            (
                "Conflict",
                "The request conflicts with the current state (`conflict`).",
            ),
            (
                "Stale",
                "The resource changed since the `ETag` sent in `If-Match`; reload it and try again (`conflict`).",
            ),
            (
                "TooLarge",
                "The request body is larger than the Platform API allows (`request_too_large`).",
            ),
            (
                "RateLimited",
                "Too many attempts; see `Retry-After` (`rate_limited`).",
            ),
            (
                "Unavailable",
                "A dependency is unreachable, for example ClickHouse for request logs (`unavailable`).",
            ),
            (
                "Internal",
                "An unexpected error; the message names the request id (`internal`).",
            ),
        ] {
            components.responses.insert(
                name.to_owned(),
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name("ErrorEnvelope")))
                            .build(),
                    )
                    .build()
                    .into(),
            );
        }
    }
}

pub fn router(state: &AppState) -> (Router<AppState>, OpenApiDoc) {
    let (router, doc) = build(&Secure::new(state));
    let limit = state.config.body_limits.platform_api.as_usize();
    let hsts = state.config.public_url.scheme() == "https";
    let router = router
        .route(PREFIX, any(error::unknown_endpoint))
        .route(&format!("{PREFIX}/{{*rest}}"), any(error::unknown_endpoint))
        .method_not_allowed_fallback(error::method_not_allowed);
    let router = if state.config.auth.demo_user().is_some() {
        router.layer(axum::middleware::from_fn(read_only))
    } else {
        router
    };
    let router = router
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            notify_after,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(limit))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            time_zone_header,
        ))
        .layer(axum::middleware::from_fn(move |req, next| {
            security_headers(req, next, hsts)
        }))
        .layer(compression())
        .layer(axum::middleware::from_fn(reads_only_compressed));
    (router, doc)
}

// The console's lists, logs and charts are JSON that shrinks to about a tenth with gzip, which is
// most of a page's load time on a slow connection. Only text is compressed: JSON, JSON lines and
// CSV downloads (which still stream). Server-sent events, media, files and ranges go out as they are.
fn compression()
-> tower_http::compression::CompressionLayer<impl tower_http::compression::Predicate> {
    use tower_http::compression::predicate::{Predicate, SizeAbove};
    let text = |_: axum::http::StatusCode,
                _: axum::http::Version,
                headers: &axum::http::HeaderMap,
                _: &axum::http::Extensions| {
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| {
                let t = t.to_ascii_lowercase();
                t.starts_with("application/json")
                    || t.starts_with("application/x-ndjson")
                    || t.starts_with("text/csv")
                    || t.starts_with("text/plain")
            })
    };
    tower_http::compression::CompressionLayer::new()
        .gzip(true)
        .compress_when(SizeAbove::new(32).and(text))
}

// Compression lets someone who sees only the encrypted size of an answer guess a secret in it, a
// token in a webhook URL or a prompt in the request log, by planting guesses beside it (BREACH):
// they make the victim's browser load the page again and again from another site. Such a load
// carries the session cookie (`SameSite=Lax` lets top-level navigations through) but the browser
// marks it as cross-site. So an answer is compressed only for a read the browser marks as coming
// from the console's own origin, or for one without a cookie, such as a platform key's, which no
// browser sends by itself. Answers to changes, which can hold a new key, are never compressed.
async fn reads_only_compressed(
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let read = matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let headers = req.headers();
    let own_origin = headers
        .get("sec-fetch-site")
        .is_some_and(|v| v.as_bytes() == b"same-origin");
    let cookie = headers.contains_key(axum::http::header::COOKIE);
    if !read || (cookie && !own_origin) {
        req.headers_mut()
            .remove(axum::http::header::ACCEPT_ENCODING);
    }
    next.run(req).await
}

// Handlers write audit events inside their transactions; emails about them go out only once the
// request succeeded, so a change that rolled back never notifies anyone.
async fn notify_after(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let (resp, events) = crate::services::notifications::dispatch::collecting(next.run(req)).await;
    if resp.status().is_success() {
        crate::services::notifications::dispatch::spawn(&state, events);
    }
    resp
}

// A read-only demo lets visitors sign in and out and dismiss what's new; every other request
// that could change something is refused before it reaches a handler.
const READ_ONLY_OPEN: [&str; 4] = [
    "/auth/demo",
    "/auth/methods",
    "/auth/logout",
    "/me/whats-new/seen",
];

async fn read_only(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::Method;
    use axum::response::IntoResponse;
    let reads = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    let open = req
        .uri()
        .path()
        .strip_prefix(PREFIX)
        .is_some_and(|path| READ_ONLY_OPEN.contains(&path));
    if reads || open {
        return next.run(req).await;
    }
    error::ApiError::PermissionDenied(auth::demo::READ_ONLY.into()).into_response()
}

pub const TIME_ZONE_HEADER: &str = "kaeso-time-zone";

async fn time_zone_header(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let zone = state
        .caches
        .time_zone
        .try_get_with((), async {
            kaeso_store::pg::instance::time_zone(&state.pg)
                .await
                .map(|z| std::sync::Arc::<str>::from(z.as_str()))
        })
        .await;
    match zone {
        Ok(zone) => {
            if let Ok(value) = axum::http::HeaderValue::from_str(&zone) {
                resp.headers_mut().insert(TIME_ZONE_HEADER, value);
            }
        }
        Err(e) => tracing::debug!(error = %e, "the instance time zone is unknown for now"),
    }
    resp
}

async fn security_headers(
    req: axum::extract::Request,
    next: axum::middleware::Next,
    hsts: bool,
) -> axum::response::Response {
    use axum::http::{HeaderValue, header};
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    h.entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    if hsts {
        h.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    resp
}

fn share_routes(s: &Secure) -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(s.route(access::Access::Public, utoipa_axum::routes!(share::open)))
        .routes(s.route(access::Access::Public, utoipa_axum::routes!(share::preview)))
}

pub fn openapi() -> OpenApiDoc {
    build(&Secure::documentation_only()).1
}

fn build(secure: &Secure) -> (Router<AppState>, OpenApiDoc) {
    let api = OpenApiRouter::new()
        .merge(auth::routes(secure))
        .merge(me::routes(secure))
        .merge(orgs::routes(secure))
        .merge(projects::routes(secure))
        .merge(instance::routes(secure))
        .merge(share_routes(secure));
    let (router, mut doc) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest(PREFIX, api)
        .split_for_parts();
    unique_operation_ids(&mut doc);
    (router, doc)
}

fn unique_operation_ids(doc: &mut OpenApiDoc) {
    for (path, item) in doc.paths.paths.iter_mut() {
        let name = operation_name(path.trim_start_matches(PREFIX));
        for (method, op) in [
            ("get", &mut item.get),
            ("put", &mut item.put),
            ("post", &mut item.post),
            ("patch", &mut item.patch),
            ("delete", &mut item.delete),
        ] {
            if let Some(op) = op {
                op.operation_id = Some(format!("{method}{name}"));
            }
        }
    }
}

fn operation_name(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut upper = true;
    for c in path.chars().filter(|c| !matches!(c, '{' | '}')) {
        if c.is_ascii_alphanumeric() {
            if upper {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
            upper = false;
        } else {
            upper = true;
        }
    }
    out
}

// Schemas built from the code's own lists, so the reference can't fall behind them.

/// A format Kaeso reads from apps or writes to providers, as the request log names it. Its values
/// come from `DialectId::ALL`, the list the filters accept, so the reference can't fall behind.
pub struct RequestFormat;

impl utoipa::PartialSchema for RequestFormat {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "A request format, as the request log names it: the format an app sent or the one Kaeso sent to the provider.",
            ))
            .enum_values(Some(kaeso_kpp::DialectId::ALL.iter().map(|d| d.as_str())))
            .examples(["openai_responses"])
            .into()
    }
}

impl utoipa::ToSchema for RequestFormat {}

/// An endpoint as the request log names it, such as `chat/completions` or
/// `models:generateContent`; its values come from the list the filter accepts.
pub struct LoggedEndpoint;

impl utoipa::PartialSchema for LoggedEndpoint {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "An endpoint, as the request log names it: the path after the API version, or the Gemini method on the model.",
            ))
            .enum_values(Some(crate::services::logs::ENDPOINTS.iter().copied()))
            .examples(["responses"])
            .into()
    }
}

impl utoipa::ToSchema for LoggedEndpoint {}

/// A client app as the request log names it from the user agent, such as `codex`; its values
/// come from the store's table of user-agent prefixes.
pub struct ClientApp;

impl utoipa::PartialSchema for ClientApp {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "A client app, as Kaeso names it from the start of the user agent; `other` for one it doesn't know.",
            ))
            .enum_values(Some(kaeso_store::clickhouse::logs::client_names()))
            .examples(["codex"])
            .into()
    }
}

impl utoipa::ToSchema for ClientApp {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_names_follow_the_path() {
        assert_eq!(
            operation_name("/orgs/{org}/members/{user}"),
            "OrgsOrgMembersUser"
        );
        assert_eq!(operation_name("/me/totp/confirm"), "MeTotpConfirm");
        assert_eq!(
            operation_name("/instance/users/{user}/reset-2fa"),
            "InstanceUsersUserReset2fa"
        );
    }

    #[test]
    fn operation_ids_are_unique() {
        let doc = openapi();
        let mut seen = std::collections::BTreeSet::new();
        for item in doc.paths.paths.values() {
            for op in [&item.get, &item.put, &item.post, &item.patch, &item.delete]
                .into_iter()
                .flatten()
            {
                let id = op.operation_id.clone().unwrap_or_default();
                assert!(seen.insert(id.clone()), "operation id {id} is used twice");
            }
        }
        assert!(seen.len() > 100);
    }
}
