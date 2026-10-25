use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Extension;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_store::pg::agents;
use serde_json::{Map, Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::oauth;
use super::tools::{self, Tool};
use crate::auth::agents::{AgentCaller, Refusal};
use crate::auth::keys::bearer;
use crate::http::client_ip;
use crate::services::agents::{known_model, usable};
use crate::state::AppState;

const LATEST_PROTOCOL: &str = "2025-11-25";
pub const PROTOCOL_VERSIONS: &[&str] = &[LATEST_PROTOCOL, "2025-06-18", "2025-03-26"];
const MAX_RESULT_BYTES: usize = 200 * 1024;
const INSTRUCTIONS: &str = "Kaeso is a self-hosted AI gateway. These tools read and change it \
    for the user who connected you, with the rights they gave you; tools/list shows only what you \
    can use now. Ids look like `org_…` and `proj_…`; list organizations with `get_me_memberships` \
    first. Every tool takes `agent_model`, the model family you are, or `unknown`. A call that fails \
    with `insufficient_rights` names a link where the user can grant the right; ask them to open \
    it.";

#[derive(Clone)]
pub struct Platform(pub Router);

fn resource_metadata(state: &AppState) -> String {
    oauth::endpoint(state, ".well-known/oauth-protected-resource/mcp")
}

fn challenge(state: &AppState, error: Option<&str>, message: &str) -> Response {
    let mut value = format!("Bearer resource_metadata=\"{}\"", resource_metadata(state));
    if let Some(e) = error {
        value.push_str(&format!(", error=\"{e}\""));
    }
    let mut r = (
        StatusCode::UNAUTHORIZED,
        axum::Json(json!({ "error": { "type": "authentication", "message": message } })),
    )
        .into_response();
    if let Ok(v) = HeaderValue::from_str(&value) {
        r.headers_mut().insert(header::WWW_AUTHENTICATE, v);
    }
    r
}

fn rpc(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn json_response(status: StatusCode, v: &Value) -> Response {
    let mut r = (status, axum::Json(v)).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

pub async fn not_allowed() -> Response {
    let mut r = (
        StatusCode::METHOD_NOT_ALLOWED,
        axum::Json(json!({ "error": { "type": "invalid_request", "message": "This MCP server answers POST requests only; it opens no event stream." } })),
    )
        .into_response();
    r.headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST"));
    r
}

// The MCP transport requires servers to check `Origin` against DNS rebinding; clients outside a
// browser send none.
fn origin_allowed(state: &AppState, headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        None => true,
        Some(o) => {
            o == state.config.public_origin()
                || state.config.cors.origins.iter().any(|allowed| allowed == o)
        }
    }
}

fn client_address(
    state: &AppState,
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Option<IpAddr> {
    let forwarded = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join(",");
    client_ip::resolve(
        peer.map(|p| p.ip()),
        &forwarded,
        &state.config.trusted_proxies,
    )
}

pub async fn post(
    State(state): State<AppState>,
    Extension(platform): Extension<Platform>,
    req: axum::extract::Request,
) -> Response {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().copied();
    let (parts, body) = req.into_parts();
    let headers = parts.headers;
    let limit = state.config.body_limits.platform_api.as_usize();
    let Ok(body) = axum::body::to_bytes(body, limit).await else {
        return json_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            &rpc_error(&Value::Null, -32600, "The request is too large."),
        );
    };
    if !origin_allowed(&state, &headers) {
        return json_response(
            StatusCode::FORBIDDEN,
            &json!({ "error": { "type": "permission_denied", "message": "Requests from this origin are refused." } }),
        );
    }
    if let Some(v) = headers
        .get("mcp-protocol-version")
        .and_then(|v| v.to_str().ok())
        && !PROTOCOL_VERSIONS.contains(&v)
    {
        return json_response(
            StatusCode::BAD_REQUEST,
            &rpc_error(&Value::Null, -32600, "Unsupported MCP protocol version."),
        );
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(bearer);
    let Some(token) = token else {
        return challenge(&state, None, "Sign in to Kaeso first.");
    };
    let agent = match crate::auth::agents::authenticate(&state, token).await {
        Ok(Ok(a)) => a,
        Ok(Err(refusal @ Refusal::NotAllowed)) => {
            return json_response(
                StatusCode::FORBIDDEN,
                &json!({ "error": { "type": "permission_denied", "message": refusal.message() } }),
            );
        }
        Ok(Err(refusal)) => return challenge(&state, Some("invalid_token"), refusal.message()),
        Err(e) => {
            tracing::error!(error = %e, "an agent could not be signed in");
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &rpc_error(
                    &Value::Null,
                    -32603,
                    "Kaeso can't reach its database right now.",
                ),
            );
        }
    };
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return json_response(
            StatusCode::BAD_REQUEST,
            &rpc_error(&Value::Null, -32700, "Parse error."),
        );
    };
    let Some(message) = message.as_object() else {
        return json_response(
            StatusCode::BAD_REQUEST,
            &rpc_error(
                &Value::Null,
                -32600,
                "Send one JSON-RPC message per request.",
            ),
        );
    };
    let (Some(id), Some(method)) = (
        message.get("id"),
        message.get("method").and_then(Value::as_str),
    ) else {
        return StatusCode::ACCEPTED.into_response();
    };
    let params = message
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let ip = client_address(&state, peer.map(|c| c.0), &headers);
    let ctx = Call {
        state: &state,
        platform: &platform,
        agent: &agent,
        headers: &headers,
        peer: peer.map(|c| c.0),
        ip,
        token,
    };
    let reply = match method {
        "initialize" => initialize(id, &params),
        "ping" => rpc(id, json!({})),
        "tools/list" => list(&ctx, id).await,
        "tools/call" => call(&ctx, id, &params).await,
        _ => rpc_error(id, -32601, "Method not found."),
    };
    json_response(StatusCode::OK, &reply)
}

fn initialize(id: &Value, params: &Map<String, Value>) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|v| PROTOCOL_VERSIONS.contains(v))
        .unwrap_or(LATEST_PROTOCOL);
    rpc(
        id,
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "kaeso",
                "title": "Kaeso",
                "version": env!("CARGO_PKG_VERSION"),
                "websiteUrl": "https://kaeso.app",
            },
            "instructions": INSTRUCTIONS,
        }),
    )
}

struct Call<'a> {
    state: &'a AppState,
    platform: &'a Platform,
    agent: &'a Arc<AgentCaller>,
    headers: &'a HeaderMap,
    peer: Option<SocketAddr>,
    ip: Option<IpAddr>,
    token: &'a str,
}

async fn list(c: &Call<'_>, id: &Value) -> Value {
    let usable = match usable(c.state, c.agent).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "an agent's tools could not be listed");
            return rpc_error(
                id,
                -32603,
                "Kaeso couldn't list the tools; try again shortly.",
            );
        }
    };
    let tools: Vec<Value> = tools::catalogue()
        .all()
        .iter()
        .filter(|t| usable.contains(t.right))
        .map(Tool::listing)
        .collect();
    rpc(id, json!({ "tools": tools }))
}

fn text_result(text: String, structured: Option<Value>, is_error: bool) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    });
    if let (Some(obj), Some(s)) = (result.as_object_mut(), structured.filter(Value::is_object)) {
        obj.insert("structuredContent".to_owned(), s);
    }
    result
}

fn approve_url(
    state: &AppState,
    agent: &AgentCaller,
    right: &str,
    args: &Map<String, Value>,
) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("add", right);
    for scope in ["org", "project"] {
        if let Some(v) = args.get(scope).and_then(Value::as_str) {
            q.append_pair(scope, v);
        }
    }
    oauth::endpoint(
        state,
        &format!(
            "settings/account/agents/{}?{}",
            kaeso_core::id::AgentId::from_uuid(agent.agent.id),
            q.finish()
        ),
    )
}

fn target_of(tool: &Tool, args: &Map<String, Value>) -> Option<String> {
    tool.path_params
        .iter()
        .rev()
        .find(|p| *p != "org" && *p != "project")
        .and_then(|p| args.get(p))
        .and_then(Value::as_str)
        .map(|s| s.chars().take(200).collect())
}

async fn call(c: &Call<'_>, id: &Value, params: &Map<String, Value>) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "`name` is required.");
    };
    let Some(tool) = tools::catalogue().get(name) else {
        return rpc_error(id, -32602, &format!("Unknown tool: {name}."));
    };
    let args = params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(reported) = args.get(tools::AGENT_MODEL).and_then(Value::as_str) {
        let model = known_model(reported).unwrap_or("unknown");
        if let Err(e) = agents::report(&c.state.pg, c.agent.agent.id, model).await {
            tracing::warn!(error = %e, "an agent's model could not be stored");
        }
    }
    let request = match tools::call(tool, &args) {
        Ok(r) => r,
        Err(e) => {
            return rpc(
                id,
                text_result(format!("Invalid arguments: {e}."), None, true),
            );
        }
    };
    let (status, body) = dispatch(c, tool, request).await;
    let parsed: Option<Value> = serde_json::from_slice(&body).ok();
    let error = parsed.as_ref().and_then(|v| v.get("error")).cloned();
    let result_kind = if status.is_success() {
        "ok".to_owned()
    } else if error.as_ref().and_then(|e| e.get("right")).is_some() {
        "insufficient_rights".to_owned()
    } else {
        error
            .as_ref()
            .and_then(|e| e["type"].as_str())
            .unwrap_or("error")
            .to_owned()
    };
    record(c, tool, &args, status, &result_kind).await;
    if status.is_success() {
        let text = match &parsed {
            Some(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
            None if body.is_empty() => "Done.".to_owned(),
            None => String::from_utf8_lossy(&body).into_owned(),
        };
        let text = if text.len() > MAX_RESULT_BYTES {
            let cut: String = text.chars().take(MAX_RESULT_BYTES).collect();
            format!("{cut}\n… (cut; ask for a smaller page with `limit`)")
        } else {
            text
        };
        return rpc(
            id,
            text_result(
                text,
                parsed.filter(|v| v.to_string().len() <= MAX_RESULT_BYTES),
                false,
            ),
        );
    }
    let message = error
        .as_ref()
        .and_then(|e| e["message"].as_str())
        .unwrap_or("The call failed.")
        .to_owned();
    if result_kind == "insufficient_rights" {
        let right = error
            .as_ref()
            .and_then(|e| e["right"].as_str())
            .unwrap_or_default()
            .to_owned();
        let url = approve_url(c.state, c.agent, &right, &args);
        return rpc(
            id,
            text_result(
                format!("insufficient_rights: {message} Ask the user to approve it at {url}"),
                Some(
                    json!({ "error": { "type": "insufficient_rights", "right": right, "message": message, "approve_url": url } }),
                ),
                true,
            ),
        );
    }
    rpc(
        id,
        text_result(
            format!("{result_kind}: {message}"),
            parsed.filter(Value::is_object),
            true,
        ),
    )
}

async fn dispatch(c: &Call<'_>, tool: &Tool, call: tools::Call) -> (StatusCode, Bytes) {
    let method: Method = tool.method.clone();
    let mut req = Request::builder()
        .method(method)
        .uri(&call.uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", c.token));
    for name in [header::USER_AGENT, header::ACCEPT_LANGUAGE] {
        if let Some(v) = c.headers.get(&name) {
            req = req.header(name, v);
        }
    }
    for v in c.headers.get_all("x-forwarded-for") {
        req = req.header("x-forwarded-for", v);
    }
    let body = match &call.body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let Ok(mut req) = req.body(body) else {
        return (StatusCode::BAD_REQUEST, Bytes::new());
    };
    if let Some(peer) = c.peer {
        req.extensions_mut().insert(ConnectInfo(peer));
    }
    match c.platform.0.clone().oneshot(req).await {
        Ok(res) => {
            let status = res.status();
            let bytes = axum::body::to_bytes(res.into_body(), 32 << 20)
                .await
                .unwrap_or_default();
            (status, bytes)
        }
        Err(never) => match never {},
    }
}

async fn record(
    c: &Call<'_>,
    tool: &Tool,
    args: &Map<String, Value>,
    status: StatusCode,
    result: &str,
) {
    let id_of = |k: &str| args.get(k).and_then(Value::as_str);
    let org = id_of("org")
        .and_then(|o| o.parse::<OrgId>().ok())
        .map(|o| o.uuid());
    let project = id_of("project")
        .and_then(|p| p.parse::<ProjectId>().ok())
        .map(|p| p.uuid());
    let target = target_of(tool, args);
    let stored = agents::insert_activity(
        &c.state.pg,
        &agents::NewActivity {
            id: Uuid::now_v7(),
            agent_id: c.agent.agent.id,
            tool: &tool.name,
            org_id: org,
            project_id: project,
            target: target.as_deref(),
            status: i16::try_from(status.as_u16()).unwrap_or_default(),
            result,
            ip: c.ip,
            at: c.state.clock.now(),
        },
    )
    .await;
    if let Err(e) = stored {
        tracing::warn!(error = %e, tool = %tool.name, "an agent's tool call could not be recorded");
    }
}
