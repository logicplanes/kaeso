pub mod access;
pub mod audit;
pub mod audit_log;
pub mod auth;
pub mod avatars;
pub mod cookies;
pub mod error;
pub mod grants;
pub mod instance;
pub mod json;
pub mod keys;
pub mod logs;
pub mod me;
pub mod orgs;
pub mod pagination;
pub mod params;
pub mod projects;
pub mod rights;
pub mod usage;
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
    components(schemas(ErrorEnvelope, error::ErrorBody)),
    modifiers(&Components),
    tags(
        (name = "auth", description = "Sign-in and sessions"),
        (name = "me", description = "The signed-in user's own account"),
        (name = "instance", description = "Instance administration (instance admins only)"),
        (name = "orgs", description = "Organizations"),
        (name = "projects", description = "Projects"),
        (name = "models", description = "Model catalogues and aliases"),
        (name = "logs", description = "Request logs, retention and storage"),
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
                "The console session cookie, set by the sign-in endpoints. State-changing requests must also send an `Origin` header equal to the install's public URL.",
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
                "The caller lacks the right, the org requires 2FA or SSO, or a recent sign-in is needed (`permission_denied`, `recent_sign_in_required`).",
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
                "TooLarge",
                "The request body is larger than the Platform API allows (`request_too_large`).",
            ),
            (
                "RateLimited",
                "Too many attempts; see `Retry-After` (`rate_limited`).",
            ),
            (
                "Unavailable",
                "A dependency is unreachable, for example ClickHouse for logs and usage (`unavailable`).",
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
        .method_not_allowed_fallback(error::method_not_allowed)
        .layer(axum::extract::DefaultBodyLimit::max(limit))
        .layer(axum::middleware::from_fn(move |req, next| {
            security_headers(req, next, hsts)
        }));
    (router, doc)
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

pub fn openapi() -> OpenApiDoc {
    build(&Secure::documentation_only()).1
}

fn build(secure: &Secure) -> (Router<AppState>, OpenApiDoc) {
    let api = OpenApiRouter::new()
        .merge(auth::routes(secure))
        .merge(me::routes(secure))
        .merge(orgs::routes(secure))
        .merge(projects::routes(secure))
        .merge(instance::routes(secure));
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
