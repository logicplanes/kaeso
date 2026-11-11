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
use super::stats;
use super::tools::{self, Tool};
use crate::auth::agents::{AgentCaller, Refusal};
use crate::auth::keys::{LookupError, bearer};
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
    it. When the user asks for a statistic to look at, such as this week's spend, call \
    `show_usage_stats`: clients that support MCP Apps show it as an interactive view. To analyse \
    usage, go from wide to narrow: `get_orgs_org_usage_span` gives the days with \
    usage; `get_orgs_org_usage` sums them by day, week or month, or by model, provider, key, \
    member, route or project, and its filters take any `key` a grouping returns, so they combine; \
    `get_orgs_org_usage_timeline` reads the request log by minute or hour with the same filters, \
    a `group_by` and latency percentiles; `get_orgs_org_projects_project_logs` lists the single \
    requests behind a spike. The `get_orgs_org_projects_project_usage…` tools do the same for one \
    project. Costs are USD strings, `null` without the right to see costs.";

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
pub(crate) fn origin_allowed(state: &AppState, headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        None => true,
        Some(o) => {
            o == state.config.public_origin()
                || state.config.cors.origins.iter().any(|allowed| allowed == o)
        }
    }
}

pub(crate) fn client_address(
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
    let ip = client_address(&state, peer.map(|c| c.0), &headers);
    let agent = match crate::auth::agents::authenticate(&state, token, ip).await {
        Ok(Ok(a)) => a,
        Ok(Err(refusal @ Refusal::NotAllowed)) => {
            return json_response(
                StatusCode::FORBIDDEN,
                &json!({ "error": { "type": "permission_denied", "message": refusal.message() } }),
            );
        }
        Ok(Err(refusal)) => return challenge(&state, Some("invalid_token"), refusal.message()),
        Err(LookupError::Throttled(t)) => {
            metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "agent", "reason" => "throttled").increment(1);
            let mut r = json_response(
                StatusCode::TOO_MANY_REQUESTS,
                &json!({ "error": { "type": "rate_limited", "message": t.message() } }),
            );
            r.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(t.retry_after));
            return r;
        }
        Err(LookupError::Store(e)) => {
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
        "tools/call" if params.get("name").and_then(Value::as_str) == Some(stats::TOOL) => {
            show_stats(&ctx, id, &params).await
        }
        "tools/call" => call(&ctx, id, &params).await,
        "resources/list" => rpc(id, json!({ "resources": [stats::resource()] })),
        "resources/templates/list" => rpc(id, json!({ "resourceTemplates": [] })),
        "resources/read" => match params.get("uri").and_then(Value::as_str) {
            Some(stats::VIEW) => rpc(id, stats::contents()),
            _ => rpc_error(
                id,
                kaeso_mcp::code::RESOURCE_NOT_FOUND,
                "Resource not found.",
            ),
        },
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
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "listChanged": false },
            },
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
    let mut tools: Vec<Value> = tools::catalogue()
        .all()
        .iter()
        .filter(|t| usable.contains(t.right))
        .map(Tool::listing)
        .collect();
    if stats::usable(&usable) {
        tools.push(stats::listing());
    }
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
    let args = arguments(params);
    report_model(c, &args).await;
    let request = match tools::call(tool, &args) {
        Ok(r) => r,
        Err(e) => {
            return rpc(
                id,
                text_result(format!("Invalid arguments: {e}."), None, true),
            );
        }
    };
    let (status, body) = dispatch(c, tool.method.clone(), request).await;
    let parsed: Option<Value> = serde_json::from_slice(&body).ok();
    let result_kind = result_kind(status, parsed.as_ref());
    record(
        c,
        &tool.name,
        target_of(tool, &args),
        &args,
        status,
        &result_kind,
    )
    .await;
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
    rpc(id, failure(c, &args, &result_kind, parsed))
}

enum Refused {
    Api(StatusCode, Option<Value>),
    Ask(String),
}

async fn platform_get(
    c: &Call<'_>,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, Refused> {
    let mut uri = format!("{}{path}", crate::platform::PREFIX);
    if !query.is_empty() {
        let mut q = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in query {
            q.append_pair(k, v);
        }
        uri.push('?');
        uri.push_str(&q.finish());
    }
    let (status, body) = dispatch(c, Method::GET, tools::Call { uri, body: None }).await;
    let parsed = serde_json::from_slice::<Value>(&body).ok();
    if status.is_success() {
        Ok(parsed.unwrap_or(Value::Null))
    } else {
        Err(Refused::Api(status, parsed))
    }
}

fn rows_of(v: Value) -> Vec<Value> {
    match v {
        Value::Object(mut m) => match m.remove("data") {
            Some(Value::Array(rows)) => rows,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn org_choice(memberships: &[Value]) -> String {
    memberships
        .iter()
        .filter_map(|m| {
            let id = m["org_id"].as_str()?;
            Some(match m["org_name"].as_str() {
                Some(name) => format!("`{id}` ({name})"),
                None => format!("`{id}`"),
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

async fn show_stats(c: &Call<'_>, id: &Value, params: &Map<String, Value>) -> Value {
    let mut args = arguments(params);
    report_model(c, &args).await;
    let ask = match stats::ask(&args) {
        Ok(a) => a,
        Err(e) => {
            return rpc(
                id,
                text_result(format!("Invalid arguments: {e}."), None, true),
            );
        }
    };
    let memberships = platform_get(c, "/me/memberships", &[])
        .await
        .map(rows_of)
        .unwrap_or_default();
    let org = match (&ask.org, memberships.as_slice()) {
        (Some(org), _) => Ok(org.clone()),
        (None, [one]) => one["org_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refused::Ask("Pass `org`, the organization to show.".to_owned())),
        (None, []) => Err(Refused::Ask(
            "Kaeso finds no organization you can open; pass `org`.".to_owned(),
        )),
        (None, many) => Err(Refused::Ask(format!(
            "Pass `org`, one of {}.",
            org_choice(many)
        ))),
    };
    let outcome = match org {
        Ok(org) => {
            args.insert("org".to_owned(), json!(org));
            stats_of(c, &ask, &org, &memberships).await
        }
        Err(refused) => Err(refused),
    };
    let (status, kind, result) = match outcome {
        Ok(result) => (StatusCode::OK, "ok".to_owned(), result),
        Err(Refused::Api(status, parsed)) => {
            let kind = result_kind(status, parsed.as_ref());
            let result = failure(c, &args, &kind, parsed);
            (status, kind, result)
        }
        Err(Refused::Ask(text)) => (
            StatusCode::BAD_REQUEST,
            "invalid_request".to_owned(),
            text_result(text, None, true),
        ),
    };
    let target = ask.project.clone().or_else(|| ask.org.clone());
    record(c, stats::TOOL, target, &args, status, &kind).await;
    rpc(id, result)
}

async fn stats_of(
    c: &Call<'_>,
    ask: &stats::Ask,
    org: &str,
    memberships: &[Value],
) -> Result<Value, Refused> {
    use stats::{Metric, Range};
    let org_name = memberships
        .iter()
        .find(|m| m["org_id"].as_str() == Some(org))
        .and_then(|m| m["org_name"].as_str())
        .unwrap_or(org)
        .to_owned();
    let mut base = format!("/orgs/{}", tools::encode_segment(org));
    let mut console = format!("orgs/{}", tools::encode_segment(org));
    let mut project_scope = Value::Null;
    let mut scope_name = org_name.clone();
    if let Some(project) = &ask.project {
        base.push_str(&format!("/projects/{}", tools::encode_segment(project)));
        console.push_str(&format!("/projects/{}", tools::encode_segment(project)));
        let name = platform_get(c, &base, &[])
            .await?
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(project)
            .to_owned();
        scope_name = format!("{name} ({org_name})");
        project_scope = json!({ "id": project, "name": name });
    }
    console.push_str("/analytics");
    let w = stats::window(ask.range, c.state.clock.now());
    let timeline = format!("{base}/usage/timeline");
    let usage = format!("{base}/usage");
    let span = [
        ("from", stats::rfc3339(w.start)),
        ("to", stats::rfc3339(w.end)),
        ("step", "hour".to_owned()),
    ];
    let days_span = [
        ("from", stats::day(w.first_day)),
        ("to", stats::day(w.last_day)),
    ];
    let group = ask.group_name().to_owned();
    let hourly_groups = ask.range == Range::Day || ask.metric == Metric::Latency;
    // Daily usage outlives the request log, which an install can turn off or keep for fewer
    // days; without it only the latency totals are missing from a 7- or 30-day answer.
    let hours = match platform_get(c, &timeline, &span).await {
        Ok(v) => rows_of(v),
        Err(Refused::Api(status, parsed))
            if !hourly_groups && result_kind(status, parsed.as_ref()) != "insufficient_rights" =>
        {
            Vec::new()
        }
        Err(e) => return Err(e),
    };
    let grouped = if hourly_groups {
        let mut q = span.to_vec();
        q.push(("group_by", group.clone()));
        q.push(("top", stats::TOP.to_string()));
        rows_of(platform_get(c, &timeline, &q).await?)
    } else {
        let mut q = days_span.to_vec();
        q.push(("group_by", group.clone()));
        rows_of(platform_get(c, &usage, &q).await?)
    };
    let days = if ask.range == Range::Day {
        Vec::new()
    } else {
        let mut q = days_span.to_vec();
        q.push(("group_by", "day".to_owned()));
        rows_of(platform_get(c, &usage, &q).await?)
    };
    let mut key_names = std::collections::HashMap::new();
    if ask.group == stats::Group::Key && ask.project.is_some() {
        let keys = platform_get(
            c,
            &format!("{base}/keys"),
            &[
                ("limit", "100".to_owned()),
                ("include_archived", "true".to_owned()),
            ],
        )
        .await
        .map(rows_of)
        .unwrap_or_default();
        for k in keys {
            if let (Some(id), Some(name)) = (
                k.get("id").and_then(Value::as_str),
                k.get("name").and_then(Value::as_str),
            ) {
                key_names.insert(id.to_owned(), name.to_owned());
            }
        }
    }
    let data = stats::Data {
        hours: &hours,
        days: &days,
        grouped: &grouped,
        key_names: &key_names,
    };
    let scope = json!({ "org": { "id": org, "name": org_name }, "project": project_scope });
    let (structured, groups, totals) =
        stats::structured(ask, &w, &data, scope, oauth::endpoint(c.state, &console));
    let text = stats::summary(ask, &scope_name, &totals, &groups);
    Ok(text_result(text, Some(structured), false))
}

fn arguments(params: &Map<String, Value>) -> Map<String, Value> {
    params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

async fn report_model(c: &Call<'_>, args: &Map<String, Value>) {
    if let Some(reported) = args.get(tools::AGENT_MODEL).and_then(Value::as_str) {
        let model = known_model(reported).unwrap_or("unknown");
        if let Err(e) = agents::report(&c.state.pg, c.agent.agent.id, model).await {
            tracing::warn!(error = %e, "an agent's model could not be stored");
        }
    }
}

fn result_kind(status: StatusCode, parsed: Option<&Value>) -> String {
    let error = parsed.and_then(|v| v.get("error"));
    if status.is_success() {
        "ok".to_owned()
    } else if error.and_then(|e| e.get("right")).is_some() {
        "insufficient_rights".to_owned()
    } else {
        error
            .and_then(|e| e["type"].as_str())
            .unwrap_or("error")
            .to_owned()
    }
}

fn failure(
    c: &Call<'_>,
    args: &Map<String, Value>,
    result_kind: &str,
    parsed: Option<Value>,
) -> Value {
    let error = parsed.as_ref().and_then(|v| v.get("error")).cloned();
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
        let url = approve_url(c.state, c.agent, &right, args);
        return text_result(
            format!("insufficient_rights: {message} Ask the user to approve it at {url}"),
            Some(
                json!({ "error": { "type": "insufficient_rights", "right": right, "message": message, "approve_url": url } }),
            ),
            true,
        );
    }
    text_result(
        format!("{result_kind}: {message}"),
        parsed.filter(Value::is_object),
        true,
    )
}

async fn dispatch(c: &Call<'_>, method: Method, call: tools::Call) -> (StatusCode, Bytes) {
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
    tool: &str,
    target: Option<String>,
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
    let stored = agents::insert_activity(
        &c.state.pg,
        &agents::NewActivity {
            id: Uuid::now_v7(),
            agent_id: c.agent.agent.id,
            tool,
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
        tracing::warn!(error = %e, tool, "an agent's tool call could not be recorded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instructions_name_only_tools_that_exist() {
        let tools = tools::catalogue();
        let named: Vec<&str> = INSTRUCTIONS
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|w| w.starts_with("get_"))
            .collect();
        assert!(named.len() >= 5, "{named:?}");
        for name in named {
            match name.strip_suffix('…') {
                Some(prefix) => assert!(
                    tools
                        .all()
                        .iter()
                        .filter(|t| t.name.starts_with(prefix))
                        .count()
                        >= 3,
                    "{name}"
                ),
                None => assert!(tools.get(name).is_some(), "{name}"),
            }
        }
    }
}
