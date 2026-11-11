use std::time::Instant;

use axum::http::StatusCode;
use axum::response::Response;
use kaeso_mcp::{Message, RpcError, Version, apps, code};
use kaeso_upstream::UpstreamError;
use kaeso_upstream::mcp::{Client, Limits, McpError, SecretHeader};
use serde_json::{Value, json};
use url::Url;

use super::auth::Caller;
use super::call::{connection_failure, handshake, handshake_key, owner, upstream_failure};
use super::snapshot::{PortalSnapshot, PortalView};
use super::{json_bytes, result_bytes, rpc_error};
use crate::services::mcp_connections::{self, Credential};
use crate::state::AppState;

// A view is the caller's when it may call one of the offered tools that name it; any other URI
// is answered as unknown, so the portal shows nothing of tools it hides.
fn usable<'a>(
    snapshot: &'a PortalSnapshot,
    caller: &Caller,
) -> impl Iterator<Item = &'a PortalView> {
    snapshot.views.iter().filter(move |v| {
        snapshot
            .offered()
            .any(|t| v.tools.contains(&t.id) && super::allowed(snapshot, caller, t.id))
    })
}

pub(super) fn list(snapshot: &PortalSnapshot, caller: &Caller, version: Version) -> String {
    let resources: Vec<Value> = usable(snapshot, caller)
        .map(|v| {
            json!({
                "uri": v.exposed,
                "name": v.exposed.trim_start_matches(apps::SCHEME),
                "mimeType": apps::MIME,
            })
        })
        .collect();
    listing(json!({ "resources": resources }), version)
}

pub(super) fn templates(version: Version) -> String {
    listing(json!({ "resourceTemplates": [] }), version)
}

// The newest version's list results say how long a client may keep them, as tools/list does.
fn listing(mut out: Value, version: Version) -> String {
    if version.is_stateless()
        && let Some(o) = out.as_object_mut()
    {
        o.insert("resultType".into(), json!("complete"));
        o.insert("ttlMs".into(), json!(super::LIST_TTL_MS));
        o.insert("cacheScope".into(), json!("private"));
    }
    out.to_string()
}

fn failed(message: impl Into<std::borrow::Cow<'static, str>>) -> RpcError {
    RpcError::new(code::INTERNAL_ERROR, message)
}

// The server's answer goes on with its `_meta` (the view's CSP, permissions and border) as the
// server sent it; only the URI changes back to the one the portal showed.
fn renamed(result: &[u8], view: &PortalView, version: Version) -> Option<String> {
    let mut result: Value = serde_json::from_slice(result).ok()?;
    for item in result.get_mut("contents")?.as_array_mut()? {
        if item.get("uri").and_then(Value::as_str) == Some(view.upstream.as_str()) {
            item["uri"] = json!(view.exposed);
        }
    }
    if version.is_stateless()
        && let Some(o) = result.as_object_mut()
    {
        o.entry("resultType").or_insert_with(|| json!("complete"));
    }
    Some(result.to_string())
}

pub(super) async fn read(
    state: &AppState,
    caller: &Caller,
    snapshot: &PortalSnapshot,
    version: Version,
    message: &Message<'_>,
) -> Response {
    let id = message.id();
    let error = |e: RpcError| rpc_error(StatusCode::OK, id, &e);
    let uri = message.params().and_then(|p| p.string("uri"));
    let Some(view) = uri
        .as_deref()
        .and_then(|u| usable(snapshot, caller).find(|v| v.exposed == u))
    else {
        return error(RpcError::new(
            code::RESOURCE_NOT_FOUND,
            "No such resource in this project's portal; list the tools again.",
        ));
    };
    let Some(server) = snapshot.servers.get(view.server) else {
        return error(RpcError::new(
            code::RESOURCE_NOT_FOUND,
            "No such resource in this project's portal; list the tools again.",
        ));
    };
    let owner = owner(server, caller);
    let credential = match mcp_connections::cached(state, server.id, owner).await {
        Ok(c) => c,
        Err(e) => return error(failed(connection_failure(server, &e).1)),
    };
    let Ok(url) = Url::parse(&server.url) else {
        return error(failed(format!(
            "{}'s address isn't a URL; an admin checks it under MCP servers.",
            server.name
        )));
    };
    let timeout = state.config.mcp_portal.call_timeout;
    let limit = state.config.mcp_portal.response_limit.as_usize();
    let upstream = Client {
        upstream: &state.mcp,
        url: &url,
        secret: credential.as_ref().map(|c: &Credential| SecretHeader {
            name: &c.header,
            value: &c.value,
        }),
        limits: Limits {
            first_byte: timeout,
            idle: timeout,
            max_body: limit,
        },
    };
    let key = handshake_key(server.id, owner);
    let started = Instant::now();
    let answer = tokio::time::timeout(timeout, async {
        let mut retried = false;
        loop {
            let hs = handshake(state, &upstream, &key).await?;
            match upstream
                .request(
                    &hs,
                    "resources/read",
                    Some(json!({ "uri": view.upstream })),
                    "kaeso-resource",
                )
                .await
            {
                Err(McpError::SessionGone) if !retried => {
                    retried = true;
                    state.caches.mcp_handshakes.invalidate(&key).await;
                }
                other => return other,
            }
        }
    })
    .await
    .unwrap_or(Err(McpError::Transport(UpstreamError::Timeout("resource"))));
    match answer {
        Ok(result) => match renamed(&result, view, version) {
            Some(result) => {
                tracing::debug!(server = %server.id, took_ms = started.elapsed().as_millis(), "a view went through the portal");
                json_bytes(StatusCode::OK, result_bytes(id, &result))
            }
            None => error(failed(format!(
                "{} answered with something that isn't MCP; try again later.",
                server.name
            ))),
        },
        Err(e) => {
            if matches!(e, McpError::Unauthorized { .. }) {
                state.caches.mcp_handshakes.invalidate(&key).await;
            }
            error(failed(upstream_failure(server, &e, timeout, limit).1))
        }
    }
}
