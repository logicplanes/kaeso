//! SCIM 2.0 (RFC 7643, RFC 7644) for an organization with single sign-on: its identity provider
//! adds and removes the members whose addresses are at the organization's verified domains.
//!
//! Requests carry the organization's own SCIM token, never a platform key or a session.

mod groups;
mod members;
mod users;

use axum::Router;
use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use kaeso_core::id::OrgId;
use kaeso_store::pg::{oidc, scim as store};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::http::client_ip::ClientIp;
use crate::state::AppState;

pub const BASE: &str = "/platform/v1/orgs/{org}/scim/v2";

const ERROR_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:Error";
const LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
pub(crate) const USER_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
pub(crate) const GROUP_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:Group";

/// The most resources one page lists.
const MAX_COUNT: usize = 200;

/// An error as RFC 7644 §3.12 writes it.
#[derive(Debug)]
pub struct ScimError {
    status: StatusCode,
    scim_type: Option<&'static str>,
    detail: String,
}

impl ScimError {
    pub fn new(
        status: StatusCode,
        scim_type: Option<&'static str>,
        detail: impl Into<String>,
    ) -> Self {
        ScimError {
            status,
            scim_type,
            detail: detail.into(),
        }
    }

    fn bad(scim_type: &'static str, detail: impl Into<String>) -> Self {
        ScimError::new(StatusCode::BAD_REQUEST, Some(scim_type), detail)
    }

    fn not_found(detail: impl Into<String>) -> Self {
        ScimError::new(StatusCode::NOT_FOUND, None, detail)
    }

    fn conflict(scim_type: Option<&'static str>, detail: impl Into<String>) -> Self {
        ScimError::new(StatusCode::CONFLICT, scim_type, detail)
    }
}

impl From<kaeso_store::StoreError> for ScimError {
    fn from(e: kaeso_store::StoreError) -> Self {
        tracing::error!(error = %e, "SCIM request failed in the database");
        ScimError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            None,
            "Kaeso can't reach its database right now. Try again shortly.",
        )
    }
}

impl From<sqlx::Error> for ScimError {
    fn from(e: sqlx::Error) -> Self {
        ScimError::from(kaeso_store::StoreError::from(e))
    }
}

impl From<crate::platform::error::ApiError> for ScimError {
    fn from(e: crate::platform::error::ApiError) -> Self {
        tracing::error!(error = %e, "SCIM request failed");
        ScimError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            "Kaeso couldn't finish this request.",
        )
    }
}

impl IntoResponse for ScimError {
    fn into_response(self) -> Response {
        let mut body = json!({
            "schemas": [ERROR_SCHEMA],
            "status": self.status.as_u16().to_string(),
            "detail": self.detail,
        });
        if let (Some(t), Some(o)) = (self.scim_type, body.as_object_mut()) {
            o.insert("scimType".into(), t.into());
        }
        scim_json(self.status, &body)
    }
}

pub(crate) fn scim_json(status: StatusCode, body: &Value) -> Response {
    let mut resp = Response::new(Body::from(body.to_string()));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/scim+json"),
    );
    resp
}

pub type ScimResult<T> = Result<T, ScimError>;

/// The organization a request may act for, once its token is checked.
#[derive(Clone, Debug)]
pub struct Scope {
    pub org: Uuid,
    pub verified_domains: Vec<String>,
    pub base: String,
}

impl Scope {
    /// Whether SCIM manages this address: one at the organization's verified domains.
    pub fn manages(&self, email: &str) -> bool {
        let domain = kaeso_core::email::domain(email);
        self.verified_domains
            .iter()
            .any(|d| d.eq_ignore_ascii_case(domain))
    }

    pub fn location(&self, kind: &str, id: &str) -> String {
        format!("{}/{kind}/{id}", self.base)
    }
}

fn unauthorized() -> ScimError {
    ScimError::new(
        StatusCode::UNAUTHORIZED,
        None,
        "The bearer token is not this organization's SCIM token.",
    )
}

fn org_in(path: &str) -> Option<OrgId> {
    path.strip_prefix("/platform/v1/orgs/")?
        .split('/')
        .next()?
        .parse()
        .ok()
}

async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    ip: ClientIp,
    path: &str,
) -> ScimResult<Scope> {
    let gate = crate::auth::key_failures::Gate::new(state, ip.0);
    if let Some(g) = &gate {
        g.check()
            .await
            .map_err(|t| ScimError::new(StatusCode::TOO_MANY_REQUESTS, None, t.message()))?;
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(unauthorized)?;
    let hash = kaeso_core::secret::hash(token);
    let found = store::org_of(&state.pg, &hash, state.clock.now()).await?;
    let asked = org_in(path);
    let org = match (found, asked) {
        (Some(org), Some(asked)) if asked.uuid() == org => org,
        _ => {
            if let Some(g) = &gate
                && let Err(t) = g.failed().await
            {
                return Err(ScimError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    None,
                    t.message(),
                ));
            }
            return Err(unauthorized());
        }
    };
    let connection = oidc::get(&state.pg, org).await?.ok_or_else(|| {
        ScimError::new(
            StatusCode::FORBIDDEN,
            None,
            "The organization has no single sign-on connection, so SCIM manages no accounts.",
        )
    })?;
    Ok(Scope {
        org,
        verified_domains: connection.verified_domains,
        base: base_url(state, OrgId::from_uuid(org)),
    })
}

pub fn base_url(state: &AppState, org: OrgId) -> String {
    format!(
        "{}{}",
        state.config.public_url.as_str().trim_end_matches('/'),
        BASE.replace("{org}", &org.to_string())
    )
}

/// What a SCIM request asks, as the handlers read it.
pub struct Asked {
    pub method: axum::http::Method,
    pub path: String,
    pub query: Option<String>,
    pub body: bytes::Bytes,
}

/// Runs one SCIM request: checks the token, answers, and records its use on the token.
async fn handle(
    State(state): State<AppState>,
    ip: ClientIp,
    method: axum::http::Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: bytes::Bytes,
) -> Response {
    let scope = match authorize(&state, &headers, ip, uri.path()).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let asked = Asked {
        method,
        path: uri.path().to_owned(),
        query: uri.query().map(str::to_owned),
        body,
    };
    let answer = route(&state, &scope, &asked).await;
    let error = match &answer {
        Err(e) if e.status != StatusCode::NOT_FOUND => Some(e.detail.clone()),
        _ => None,
    };
    if let Err(e) = store::used(&state.pg, scope.org, state.clock.now(), error.as_deref()).await {
        tracing::warn!(error = %e, "cannot record the SCIM token's use");
    }
    answer.unwrap_or_else(IntoResponse::into_response)
}

async fn route(state: &AppState, scope: &Scope, asked: &Asked) -> ScimResult<Response> {
    let rest = asked
        .path
        .split_once("/scim/v2")
        .map_or("", |(_, r)| r)
        .trim_end_matches('/');
    let mut parts = rest.trim_start_matches('/').splitn(2, '/');
    let resource = parts.next().unwrap_or_default();
    let id = parts.next();
    use axum::http::Method as M;
    match (resource, id, &asked.method) {
        ("ServiceProviderConfig", None, &M::GET) => {
            Ok(scim_json(StatusCode::OK, &service_provider_config()))
        }
        ("ResourceTypes", None, &M::GET) => Ok(scim_json(StatusCode::OK, &resource_types(scope))),
        ("Schemas", None, &M::GET) => Ok(scim_json(StatusCode::OK, &schemas())),
        ("Users", None, &M::GET) => users::list(state, scope, asked).await,
        ("Users", None, &M::POST) => users::create(state, scope, asked).await,
        ("Users", Some(id), &M::GET) => users::get(state, scope, id).await,
        ("Users", Some(id), &M::PUT) => users::replace(state, scope, id, asked).await,
        ("Users", Some(id), &M::PATCH) => users::patch(state, scope, id, asked).await,
        ("Users", Some(id), &M::DELETE) => users::delete(state, scope, id).await,
        ("Groups", None, &M::GET) => groups::list(state, scope, asked).await,
        ("Groups", None, &M::POST) => groups::create(state, scope, asked).await,
        ("Groups", Some(id), &M::GET) => groups::get(state, scope, id).await,
        ("Groups", Some(id), &M::PUT) => groups::replace(state, scope, id, asked).await,
        ("Groups", Some(id), &M::PATCH) => groups::patch(state, scope, id, asked).await,
        ("Groups", Some(id), &M::DELETE) => groups::delete(state, scope, id).await,
        ("ServiceProviderConfig" | "ResourceTypes" | "Schemas" | "Users" | "Groups", _, _) => {
            Err(ScimError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                None,
                format!(
                    "`{}` does not accept `{}` requests.",
                    asked.path, asked.method
                ),
            ))
        }
        _ => Err(ScimError::not_found(format!(
            "No SCIM endpoint at `{}`.",
            asked.path
        ))),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(BASE, get(handle))
        .route(&format!("{BASE}/{{*rest}}"), axum::routing::any(handle))
}

// ----- shared parsing -----

pub(crate) fn body_json(asked: &Asked) -> ScimResult<Value> {
    serde_json::from_slice::<Value>(&asked.body)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| ScimError::bad("invalidSyntax", "The body must be a JSON object."))
}

/// A boolean as identity providers send it: `true`, or the string `"True"` (Entra ID).
pub(crate) fn boolean(v: &Value, attribute: &str) -> ScimResult<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::String(s) if s.eq_ignore_ascii_case("true") => Ok(true),
        Value::String(s) if s.eq_ignore_ascii_case("false") => Ok(false),
        _ => Err(ScimError::bad(
            "invalidValue",
            format!("`{attribute}` must be true or false."),
        )),
    }
}

/// A filter Kaeso answers: `attribute eq "value"` for one of `attributes`.
pub(crate) fn filter(
    query: Option<&str>,
    attributes: &[&'static str],
) -> ScimResult<Option<(&'static str, String)>> {
    let Some(raw) = param(query, "filter") else {
        return Ok(None);
    };
    let invalid = || {
        ScimError::bad(
            "invalidFilter",
            format!(
                "Kaeso filters only by {} with `eq`.",
                attributes
                    .iter()
                    .map(|a| format!("`{a}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
        )
    };
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let at = lower.find(" eq ").ok_or_else(invalid)?;
    let (name, value) = (
        trimmed.get(..at).unwrap_or_default().trim(),
        trimmed.get(at + 4..).unwrap_or_default().trim(),
    );
    let attribute = attributes
        .iter()
        .find(|a| a.eq_ignore_ascii_case(name))
        .ok_or_else(invalid)?;
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(invalid)?
        .replace("\\\"", "\"");
    Ok(Some((attribute, value)))
}

pub(crate) fn param(query: Option<&str>, name: &str) -> Option<String> {
    url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.into_owned())
}

/// A page of resources as a `ListResponse`, from `startIndex` (1-based) and `count`.
pub(crate) fn page(query: Option<&str>, all: Vec<Value>) -> Response {
    let start = param(query, "startIndex")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let count = param(query, "count")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100)
        .min(MAX_COUNT);
    let total = all.len();
    let items: Vec<Value> = all.into_iter().skip(start - 1).take(count).collect();
    scim_json(
        StatusCode::OK,
        &json!({
            "schemas": [LIST_SCHEMA],
            "totalResults": total,
            "startIndex": start,
            "itemsPerPage": items.len(),
            "Resources": items,
        }),
    )
}

/// The operations of a PATCH request (RFC 7644 §3.5.2), each as (op, path, value).
pub(crate) fn operations(asked: &Asked) -> ScimResult<Vec<(String, Option<String>, Value)>> {
    let body = body_json(asked)?;
    let ops = body
        .get("Operations")
        .and_then(Value::as_array)
        .ok_or_else(|| ScimError::bad("invalidSyntax", "A PATCH needs `Operations`."))?;
    ops.iter()
        .map(|op| {
            let kind = op
                .get("op")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
                .filter(|k| matches!(k.as_str(), "add" | "replace" | "remove"))
                .ok_or_else(|| {
                    ScimError::bad(
                        "invalidSyntax",
                        "Each operation's `op` is add, replace or remove.",
                    )
                })?;
            let path = op.get("path").and_then(Value::as_str).map(str::to_owned);
            Ok((kind, path, op.get("value").cloned().unwrap_or(Value::Null)))
        })
        .collect()
}

pub(crate) fn rfc3339(t: time::OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn service_provider_config() -> Value {
    json!({
        "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig"],
        "documentationUri": "https://kaeso.app/docs/api/scim",
        "patch": { "supported": true },
        "bulk": { "supported": false, "maxOperations": 0, "maxPayloadSize": 0 },
        "filter": { "supported": true, "maxResults": MAX_COUNT },
        "changePassword": { "supported": false },
        "sort": { "supported": false },
        "etag": { "supported": false },
        "authenticationSchemes": [{
            "type": "oauthbearertoken",
            "name": "OAuth Bearer Token",
            "description": "The organization's SCIM token, created under Security in the console.",
            "primary": true
        }],
        "meta": { "resourceType": "ServiceProviderConfig" }
    })
}

fn resource_types(scope: &Scope) -> Value {
    let types = [("User", "/Users", USER_SCHEMA), ("Group", "/Groups", GROUP_SCHEMA)]
        .iter()
        .map(|(name, endpoint, schema)| {
            json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ResourceType"],
                "id": name,
                "name": name,
                "endpoint": endpoint,
                "schema": schema,
                "meta": { "resourceType": "ResourceType", "location": format!("{}/ResourceTypes/{name}", scope.base) }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schemas": [LIST_SCHEMA],
        "totalResults": types.len(),
        "startIndex": 1,
        "itemsPerPage": types.len(),
        "Resources": types,
    })
}

fn attribute(name: &str, kind: &str, mutability: &str, uniqueness: &str) -> Value {
    json!({
        "name": name, "type": kind, "multiValued": false, "required": name == "userName" || name == "displayName",
        "caseExact": false, "mutability": mutability, "returned": "default", "uniqueness": uniqueness
    })
}

fn schemas() -> Value {
    let user = json!({
        "id": USER_SCHEMA, "name": "User", "description": "A member the identity provider manages",
        "attributes": [
            attribute("userName", "string", "immutable", "server"),
            attribute("displayName", "string", "readWrite", "none"),
            attribute("active", "boolean", "readWrite", "none"),
            {
                "name": "name", "type": "complex", "multiValued": false, "required": false, "mutability": "readWrite", "returned": "default",
                "subAttributes": [
                    attribute("formatted", "string", "readWrite", "none"),
                    attribute("givenName", "string", "readWrite", "none"),
                    attribute("familyName", "string", "readWrite", "none")
                ]
            },
            {
                "name": "emails", "type": "complex", "multiValued": true, "required": false, "mutability": "readOnly", "returned": "default",
                "subAttributes": [attribute("value", "string", "readOnly", "none"), attribute("primary", "boolean", "readOnly", "none")]
            }
        ],
        "meta": { "resourceType": "Schema" }
    });
    let group = json!({
        "id": GROUP_SCHEMA, "name": "Group", "description": "A group of the identity provider",
        "attributes": [
            attribute("displayName", "string", "readWrite", "server"),
            {
                "name": "members", "type": "complex", "multiValued": true, "required": false, "mutability": "readWrite", "returned": "default",
                "subAttributes": [attribute("value", "string", "immutable", "none")]
            }
        ],
        "meta": { "resourceType": "Schema" }
    });
    json!({
        "schemas": [LIST_SCHEMA],
        "totalResults": 2,
        "startIndex": 1,
        "itemsPerPage": 2,
        "Resources": [user, group],
    })
}
