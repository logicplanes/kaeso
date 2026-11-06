pub mod auth;
mod call;
pub mod snapshot;

use std::borrow::Cow;
use std::net::SocketAddr;
use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use kaeso_core::id::ProjectId;
use kaeso_mcp::headers::{self as mcp_headers, RequestHeaders};
use kaeso_mcp::{Kind, Message, RpcError, Version, meta};
use serde_json::json;
use serde_json::value::RawValue;

use self::auth::Caller;
use self::snapshot::PortalSnapshot;
use crate::platform::params::Path;
use crate::state::AppState;

// How long a client may keep a list before asking again; the sync job looks at servers on its own
// interval, so a shorter time would only add requests.
const LIST_TTL_MS: u64 = 300_000;
const INSTRUCTIONS: &str = "These are the tools of the MCP servers this Kaeso project uses. Each \
    tool's name starts with its server's handle and two underscores, as in `x__create_post`.";

pub fn router(state: &AppState) -> Router<AppState> {
    let limit = state.config.body_limits.platform_api.as_usize();
    Router::new()
        .route(
            "/mcp/{project}",
            post(handle)
                .get(crate::mcp::server::not_allowed)
                .delete(crate::mcp::server::not_allowed),
        )
        .layer(DefaultBodyLimit::max(limit))
}

fn json_bytes(status: StatusCode, body: Vec<u8>) -> Response {
    let mut r = (status, Body::from(body)).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn rpc_error(status: StatusCode, id: Option<&RawValue>, e: &RpcError) -> Response {
    json_bytes(status, e.response(id))
}

// The result is written as it was kept, with only the request's `id` around it: a tools list
// is never parsed or serialized again.
fn result_bytes(id: Option<&RawValue>, result: &str) -> Vec<u8> {
    let id = id.map_or("null", RawValue::get);
    let mut out = Vec::with_capacity(result.len() + id.len() + 40);
    out.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":");
    out.extend_from_slice(id.as_bytes());
    out.extend_from_slice(b",\"result\":");
    out.extend_from_slice(result.as_bytes());
    out.push(b'}');
    out
}

fn server_info() -> serde_json::Value {
    json!({ "name": "kaeso-portal", "title": "Kaeso", "version": env!("CARGO_PKG_VERSION") })
}

fn capabilities() -> serde_json::Value {
    json!({ "tools": { "listChanged": false } })
}

fn discover() -> String {
    json!({
        "resultType": "complete",
        "supportedVersions": Version::names(),
        "capabilities": capabilities(),
        "instructions": INSTRUCTIONS,
        "ttlMs": LIST_TTL_MS,
        "cacheScope": "private",
        "_meta": { meta::SERVER_INFO: server_info() },
    })
    .to_string()
}

fn initialize(message: &Message<'_>) -> String {
    let asked = message.params().and_then(|p| p.string("protocolVersion"));
    json!({
        "protocolVersion": Version::for_initialize(asked.as_deref()).as_str(),
        "capabilities": capabilities(),
        "serverInfo": server_info(),
        "instructions": INSTRUCTIONS,
    })
    .to_string()
}

fn allowed(snapshot: &PortalSnapshot, caller: &Caller, tool: uuid::Uuid) -> bool {
    snapshot
        .access
        .allows(caller.user, caller.service_account, caller.key, tool)
}

fn list(snapshot: &PortalSnapshot, caller: &Caller, version: Version) -> String {
    let tools: Cow<'_, str> = if snapshot.offered().all(|t| allowed(snapshot, caller, t.id)) {
        Cow::Borrowed(&snapshot.listed)
    } else {
        Cow::Owned(
            snapshot
                .offered()
                .filter(|t| allowed(snapshot, caller, t.id))
                .map(|t| t.listed.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
    };
    let mut out = String::with_capacity(tools.len() + 80);
    out.push_str("{\"tools\":[");
    out.push_str(&tools);
    out.push(']');
    if version.is_stateless() {
        out.push_str(",\"resultType\":\"complete\",\"ttlMs\":");
        out.push_str(&LIST_TTL_MS.to_string());
        out.push_str(",\"cacheScope\":\"private\"");
    }
    out.push('}');
    out
}

fn request_headers(headers: &HeaderMap) -> RequestHeaders<'_> {
    let get = |name: &str| headers.get(name).map(HeaderValue::as_bytes);
    RequestHeaders {
        protocol_version: get(mcp_headers::PROTOCOL_VERSION),
        method: get(mcp_headers::METHOD),
        name: get(mcp_headers::NAME),
    }
}

async fn handle(
    State(state): State<AppState>,
    Path(project): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let started = Instant::now();
    let at = state.clock.now();
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let (parts, body) = req.into_parts();
    let headers = parts.headers;
    let Ok(project) = project.parse::<ProjectId>() else {
        return auth::refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No such MCP portal; check the project id in its address.",
        );
    };
    let limit = state.config.body_limits.platform_api.as_usize();
    let Ok(body) = axum::body::to_bytes(body, limit).await else {
        return rpc_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            None,
            &RpcError::invalid_request("The request is too large."),
        );
    };
    if !crate::mcp::server::origin_allowed(&state, &headers) {
        return auth::refused(
            StatusCode::FORBIDDEN,
            "permission_denied",
            "Requests from this origin are refused.",
        );
    }
    let ip = crate::mcp::server::client_address(&state, peer, &headers);
    let caller = match auth::authenticate(&state, &headers, ip, project.uuid()).await {
        Ok(c) => c,
        Err(refusal) => return refusal,
    };
    let message = match Message::parse(&body) {
        Ok(m) => m,
        Err(e) => return rpc_error(StatusCode::BAD_REQUEST, None, &e),
    };
    if message.kind() != Kind::Request {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = message.id();
    let rh = request_headers(&headers);
    let version = match mcp_headers::version_of(&rh, &message) {
        Ok(v) => v,
        Err(e) => return rpc_error(StatusCode::BAD_REQUEST, id, &e),
    };
    if let Err(e) = mcp_headers::check(version, &rh, &message) {
        return rpc_error(StatusCode::BAD_REQUEST, id, &e);
    }
    let snapshot = match snapshot::get(&state, project.uuid()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "a portal's tools couldn't be loaded");
            return rpc_error(
                StatusCode::SERVICE_UNAVAILABLE,
                id,
                &RpcError::new(
                    kaeso_mcp::code::INTERNAL_ERROR,
                    "Kaeso can't load this project's tools right now; try again shortly.",
                ),
            );
        }
    };
    let ok = |result: &str| json_bytes(StatusCode::OK, result_bytes(id, result));
    match message.method().unwrap_or_default() {
        "server/discover" => ok(&discover()),
        "initialize" => ok(&initialize(&message)),
        "ping" => ok("{}"),
        "tools/list" => ok(&list(&snapshot, &caller, version)),
        "tools/call" => {
            call::call(
                &state,
                call::Request {
                    caller: &caller,
                    snapshot: &snapshot,
                    version,
                    message: &message,
                    body: &body,
                    started,
                    at,
                    request_bytes: body.len(),
                    source: "portal",
                },
            )
            .await
        }
        _ => rpc_error(
            if version.is_stateless() {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            },
            id,
            &RpcError::method_not_found(),
        ),
    }
}

pub enum PlaygroundRefusal {
    NoTool,
    Unavailable,
    Unreadable,
}

// The Playground calls a tool through the same path as the portal, as the signed-in person, so
// the project's tool switches, access, limits and logs apply alike; its answer is the JSON-RPC
// message a client would get.
pub async fn playground_call(
    state: &AppState,
    org: uuid::Uuid,
    project: uuid::Uuid,
    user: uuid::Uuid,
    tool: uuid::Uuid,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, PlaygroundRefusal> {
    let started = Instant::now();
    let at = state.clock.now();
    let snapshot = snapshot::get(state, project).await.map_err(|e| {
        tracing::error!(error = %e, "a portal's tools couldn't be loaded");
        PlaygroundRefusal::Unavailable
    })?;
    let exposed = snapshot
        .tools
        .iter()
        .find(|t| t.id == tool)
        .map(|t| t.exposed.clone())
        .ok_or(PlaygroundRefusal::NoTool)?;
    let body = serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": exposed, "arguments": arguments },
    }))
    .map_err(|_| PlaygroundRefusal::Unreadable)?;
    let message = Message::parse(&body).map_err(|_| PlaygroundRefusal::Unreadable)?;
    let caller = auth::Caller {
        org,
        project,
        key: None,
        grant: None,
        user: Some(user),
        service_account: None,
        client: Some(PLAYGROUND_CLIENT.to_owned()),
    };
    let answer = call::call(
        state,
        call::Request {
            caller: &caller,
            snapshot: &snapshot,
            version: kaeso_mcp::Version::LATEST,
            message: &message,
            body: &body,
            started,
            at,
            request_bytes: body.len(),
            source: "playground",
        },
    )
    .await;
    let stream = answer
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"text/event-stream"));
    let bytes = axum::body::to_bytes(answer.into_body(), usize::MAX)
        .await
        .map_err(|_| PlaygroundRefusal::Unreadable)?;
    let text = String::from_utf8_lossy(&bytes);
    let last = if stream {
        text.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str::<serde_json::Value>(d.trim()).ok())
            .rfind(|m| m.get("id").is_some())
    } else {
        serde_json::from_str(&text).ok()
    };
    last.ok_or(PlaygroundRefusal::Unreadable)
}

const PLAYGROUND_CLIENT: &str = "Kaeso Playground";
