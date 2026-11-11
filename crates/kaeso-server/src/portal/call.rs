use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use kaeso_codecs::sse::{SseItem, SseParser};
use kaeso_core::limits::Subject;
use kaeso_mcp::raw::{self, RawObject};
use kaeso_mcp::{Kind, Message, RpcError, Version, meta, params};
use kaeso_store::clickhouse::tool_calls::ToolCallRow;
use kaeso_upstream::UpstreamError;
use kaeso_upstream::mcp::{Client, Handshake, Limits, McpBody, McpError, SecretHeader};
use serde_json::Value;
use serde_json::value::RawValue;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::auth::Caller;
use super::snapshot::{PortalServer, PortalSnapshot, Standing};
use super::{json_bytes, rpc_error};
use crate::services::mcp_connections::{self, ConnectionError, Credential};
use crate::state::AppState;

// JSON-RPC leaves -32000 to -32099 to the server; a rate limit is one of those.
const LIMITED: i64 = -32000;

pub(super) struct Request<'a> {
    pub(super) caller: &'a Caller,
    pub(super) snapshot: &'a PortalSnapshot,
    pub(super) version: Version,
    pub(super) message: &'a Message<'a>,
    pub(super) body: &'a [u8],
    pub(super) started: Instant,
    pub(super) at: OffsetDateTime,
    pub(super) request_bytes: usize,
    pub(super) source: &'static str,
}

#[derive(Clone, Debug)]
struct Logged {
    source: &'static str,
    caller: Caller,
    server: Uuid,
    tool: String,
    upstream: String,
    version: Version,
    client: String,
    started: Instant,
    at: OffsetDateTime,
    request_bytes: usize,
    arguments: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Denied,
    Limited,
    ToolOff,
    ToolChanged,
    UpstreamAuth,
    UpstreamUnreachable,
    UpstreamError,
    Timeout,
    TooLarge,
}

impl Failure {
    fn as_str(self) -> &'static str {
        match self {
            Failure::Denied => "denied",
            Failure::Limited => "limited",
            Failure::ToolOff => "tool_off",
            Failure::ToolChanged => "tool_changed",
            Failure::UpstreamAuth => "upstream_auth",
            Failure::UpstreamUnreachable => "upstream_unreachable",
            Failure::UpstreamError => "upstream_error",
            Failure::Timeout => "timeout",
            Failure::TooLarge => "too_large",
        }
    }

    fn refused(self) -> bool {
        matches!(
            self,
            Failure::Denied | Failure::Limited | Failure::ToolOff | Failure::ToolChanged
        )
    }
}

fn log(
    state: &AppState,
    l: &Logged,
    failure: Option<(Failure, &str)>,
    upstream_ms: Option<u32>,
    response: Option<&[u8]>,
) {
    let ms = |d: Duration| u32::try_from(d.as_millis()).unwrap_or(u32::MAX);
    let row = ToolCallRow {
        id: kaeso_core::id::new_v7(),
        org_id: l.caller.org,
        project_id: l.caller.project,
        server_id: l.server,
        started_at: l.at,
        tool: l.tool.clone(),
        upstream_tool: l.upstream.clone(),
        caller_kind: if l.caller.user.is_some() {
            "user"
        } else {
            "service_account"
        }
        .to_owned(),
        caller_id: l
            .caller
            .user
            .or(l.caller.service_account)
            .unwrap_or_default(),
        key_id: l.caller.key,
        grant_id: l.caller.grant,
        client_name: l.client.clone(),
        protocol_version: l.version.as_str().to_owned(),
        source: l.source.to_owned(),
        status: match failure {
            None => "ok",
            Some((f, _)) if f.refused() => "refused",
            Some(_) => "error",
        }
        .to_owned(),
        error_type: failure
            .map(|(f, _)| f.as_str())
            .unwrap_or_default()
            .to_owned(),
        error_message: failure.map(|(_, m)| m).unwrap_or_default().to_owned(),
        duration_ms: ms(l.started.elapsed()),
        upstream_ms,
        request_bytes: l.request_bytes as u64,
        response_bytes: response.map_or(0, |r| r.len() as u64),
        has_content: false,
        expires_at: l.at,
    };
    let content = l.arguments.clone().map(|arguments| {
        (
            arguments,
            response
                .map(|r| String::from_utf8_lossy(r).into_owned())
                .unwrap_or_default(),
        )
    });
    state.tool_usage.add(
        l.at,
        l.caller.org,
        l.caller.project,
        l.server,
        failure.is_some(),
    );
    let st = state.clone();
    state.in_flight.finish(async move {
        crate::services::logs::record_tool_call(&st, row, content).await;
    });
}

// A tool result that says what went wrong: clients show it to the model and the person, where a
// protocol error would only end the call.
fn error_result(id: Option<&RawValue>, version: Version, message: &str) -> Vec<u8> {
    let mut result = serde_json::json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    });
    if version.is_stateless()
        && let Some(o) = result.as_object_mut()
    {
        o.insert("resultType".into(), Value::String("complete".into()));
    }
    super::result_bytes(id, &result.to_string())
}

fn refuse(id: Option<&RawValue>, message: String) -> Response {
    rpc_error(StatusCode::OK, id, &RpcError::invalid_params(message))
}

pub(super) fn owner(server: &PortalServer, caller: &Caller) -> Option<Uuid> {
    if server.per_user { caller.user } else { None }
}

fn sign_in_hint(server: &PortalServer) -> String {
    if server.per_user {
        format!(
            "Connect your account to {} under Account › Connections, then try again.",
            server.name
        )
    } else {
        format!(
            "An admin connects {} again under MCP servers; then try again.",
            server.name
        )
    }
}

pub(super) fn connection_failure(server: &PortalServer, e: &ConnectionError) -> (Failure, String) {
    match e {
        ConnectionError::NotConnected | ConnectionError::SignedOut | ConnectionError::Unrenewed => {
            (
                Failure::UpstreamAuth,
                format!(
                    "Kaeso has no working sign-in to {}. {}",
                    server.name,
                    sign_in_hint(server)
                ),
            )
        }
        ConnectionError::Store(_) | ConnectionError::Crypto(_) => (
            Failure::UpstreamError,
            "Kaeso can't open its sign-in for this server right now; try again shortly.".to_owned(),
        ),
    }
}

pub(super) fn upstream_failure(
    server: &PortalServer,
    e: &McpError,
    timeout: Duration,
    limit: usize,
) -> (Failure, String) {
    match e {
        McpError::Unauthorized { .. } => (
            Failure::UpstreamAuth,
            format!(
                "{} refused Kaeso's sign-in. {}",
                server.name,
                sign_in_hint(server)
            ),
        ),
        McpError::Transport(UpstreamError::Timeout(_)) => (
            Failure::Timeout,
            format!(
                "{} didn't answer within {} seconds; try again later.",
                server.name,
                timeout.as_secs()
            ),
        ),
        McpError::Transport(UpstreamError::TooLarge(_)) => (
            Failure::TooLarge,
            format!(
                "{}'s answer is larger than the portal passes on ({}); ask for less at once.",
                server.name,
                size(limit)
            ),
        ),
        McpError::Transport(UpstreamError::Refused(_)) => (
            Failure::UpstreamUnreachable,
            format!(
                "This install doesn't let Kaeso reach {}'s address; an admin checks it under MCP servers.",
                server.name
            ),
        ),
        McpError::Transport(_) => (
            Failure::UpstreamUnreachable,
            format!("Kaeso couldn't reach {}; try again later.", server.name),
        ),
        McpError::Status(status) => (
            Failure::UpstreamError,
            format!(
                "{} answered with HTTP {status}; try again later.",
                server.name
            ),
        ),
        McpError::Rpc { .. } | McpError::Protocol(_) | McpError::SessionGone => (
            Failure::UpstreamError,
            format!(
                "{} answered with something that isn't MCP; try again later.",
                server.name
            ),
        ),
    }
}

fn size(bytes: usize) -> String {
    if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", (bytes >> 10).max(1))
    }
}

pub(super) fn handshake_key(server: Uuid, owner: Option<Uuid>) -> String {
    format!(
        "{server}:{}",
        owner.map_or_else(|| "shared".to_owned(), |u| u.to_string())
    )
}

pub(super) async fn handshake(
    state: &AppState,
    client: &Client<'_>,
    key: &str,
) -> Result<Arc<Handshake>, McpError> {
    if let Some(hs) = state.caches.mcp_handshakes.get(key).await {
        return Ok(hs);
    }
    let hs = Arc::new(client.handshake().await?);
    state
        .caches
        .mcp_handshakes
        .insert(key.to_owned(), hs.clone())
        .await;
    Ok(hs)
}

// The client's message goes on as it came, `arguments` and `_meta` byte for byte; only the tool's
// name changes. A server of the newest version is told whom it talks to; an older one is spared
// the newest version's `_meta` claims, which belong to a protocol it doesn't speak.
fn outgoing(
    body: &[u8],
    upstream: &str,
    hs: &Handshake,
    client_info: &Value,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut top = RawObject::parse(body)?;
    let mut params = match top.borrowed("params") {
        Some(p) => RawObject::of(p)?,
        None => RawObject::parse(b"{}")?,
    };
    params.set("name", raw::json_string(upstream));
    if hs.version.is_stateless() {
        let mut m = match params.borrowed(meta::KEY) {
            Some(m) => RawObject::of(m)?,
            None => RawObject::parse(b"{}")?,
        };
        m.set(
            meta::PROTOCOL_VERSION,
            raw::json_string(hs.version.as_str()),
        );
        m.set(
            meta::CLIENT_INFO,
            serde_json::value::to_raw_value(client_info)?,
        );
        if !m.contains(meta::CLIENT_CAPABILITIES) {
            m.set(meta::CLIENT_CAPABILITIES, raw::empty_object());
        }
        params.set(meta::KEY, m.into_raw());
    } else if let Some(m) = params.borrowed(meta::KEY) {
        // An older server takes the version from the handshake and the headers; a newest-version
        // client's claims in `_meta` would contradict them, and some servers (Linear) refuse that.
        let mut m = RawObject::of(m)?;
        for key in [
            meta::PROTOCOL_VERSION,
            meta::CLIENT_INFO,
            meta::CLIENT_CAPABILITIES,
        ] {
            m.remove(key);
        }
        if m.is_empty() {
            params.remove(meta::KEY);
        } else {
            params.set(meta::KEY, m.into_raw());
        }
    }
    top.set("params", params.into_raw());
    Ok(top.to_vec())
}

// A newest-version client expects `resultType`; an older server never sends it.
fn completed(message: &[u8], version: Version) -> Vec<u8> {
    if !version.is_stateless() {
        return message.to_vec();
    }
    let Ok(mut top) = RawObject::parse(message) else {
        return message.to_vec();
    };
    let Some(Ok(mut result)) = top.borrowed("result").map(RawObject::of) else {
        return message.to_vec();
    };
    if result.contains("resultType") {
        return message.to_vec();
    }
    result.set("resultType", raw::json_string("complete"));
    top.set("result", result.into_raw());
    top.to_vec()
}

fn tool_failed(message: &[u8]) -> bool {
    let Ok(m) = Message::parse(message) else {
        return true;
    };
    match m.kind() {
        Kind::Error => true,
        Kind::Response => m
            .result()
            .and_then(|r| RawObject::of(r).ok())
            .and_then(|r| r.get("isError").map(|v| v.get() == "true"))
            .unwrap_or(false),
        _ => true,
    }
}

pub(super) async fn call(state: &AppState, r: Request<'_>) -> Response {
    let id = r.message.id();
    let Some(name) = r.message.name() else {
        return refuse(id, "Send the tool's `name` in `params`.".to_owned());
    };
    let Some(tool) = r
        .snapshot
        .tool(&name)
        .or_else(|| r.snapshot.tool_of_view(&name))
    else {
        return refuse(
            id,
            format!("No tool `{name}` in this project's portal; list the tools again."),
        );
    };
    let Some(server) = r.snapshot.server(tool) else {
        return refuse(
            id,
            format!("No tool `{name}` in this project's portal; list the tools again."),
        );
    };
    let client = r
        .message
        .params()
        .and_then(|p| p.borrowed(meta::KEY))
        .and_then(|m| RawObject::of(m).ok())
        .and_then(|m| meta::client_info(&m))
        .map(|i| i.name)
        .or_else(|| r.caller.client.clone())
        .unwrap_or_default();
    let arguments = r
        .message
        .params()
        .and_then(|p| p.borrowed("arguments"))
        .map(|a| a.get().to_owned());
    let keeps = crate::services::logs::keeps_content(state, r.caller.project).await;
    let logged = Logged {
        source: r.source,
        caller: r.caller.clone(),
        server: server.id,
        tool: tool.exposed.clone(),
        upstream: tool.upstream.clone(),
        version: r.version,
        client,
        started: r.started,
        at: r.at,
        request_bytes: r.request_bytes,
        arguments: if keeps { arguments.clone() } else { None },
    };
    let refusal = match tool.standing {
        Standing::Off => Some((
            Failure::ToolOff,
            format!(
                "The tool `{name}` is off in this project; a project admin turns it on under Tools."
            ),
        )),
        Standing::Held => Some((
            Failure::ToolChanged,
            format!(
                "The tool `{name}` changed and waits for an admin's review; try again once it's approved."
            ),
        )),
        Standing::Offered if !super::allowed(r.snapshot, r.caller, tool.id) => Some((
            Failure::Denied,
            format!("You may not call `{name}`; ask a project admin for access to it."),
        )),
        Standing::Offered => None,
    };
    if let Some((failure, message)) = refusal {
        log(state, &logged, Some((failure, &message)), None, None);
        return refuse(id, message);
    }
    let member = r
        .caller
        .user
        .map(Subject::User)
        .or(r.caller.service_account.map(Subject::ServiceAccount));
    if let Err(limited) = crate::services::limits::admit_tool_call(
        state,
        r.caller.project,
        member,
        r.caller.key,
        r.at,
    )
    .await
    {
        log(
            state,
            &logged,
            Some((Failure::Limited, &limited.message)),
            None,
            None,
        );
        let mut response = rpc_error(
            StatusCode::TOO_MANY_REQUESTS,
            id,
            &RpcError::new(LIMITED, limited.message),
        );
        if let Some(after) = limited.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(after.as_secs()));
        }
        return response;
    }
    let fail = |failure: Failure, message: String, upstream_ms: Option<u32>| {
        log(state, &logged, Some((failure, &message)), upstream_ms, None);
        json_bytes(StatusCode::OK, error_result(id, r.version, &message))
    };
    let owner = owner(server, r.caller);
    let credential = match mcp_connections::cached(state, server.id, owner).await {
        Ok(c) => c,
        Err(e) => {
            let (failure, message) = connection_failure(server, &e);
            return fail(failure, message, None);
        }
    };
    let Ok(url) = Url::parse(&server.url) else {
        return fail(
            Failure::UpstreamUnreachable,
            format!(
                "{}'s address isn't a URL; an admin checks it under MCP servers.",
                server.name
            ),
            None,
        );
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
    let headers = match &arguments {
        Some(a) if !tool.params.is_empty() => serde_json::from_str::<Value>(a)
            .map(|args| params::values(&tool.params, &args))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let key = handshake_key(server.id, owner);
    let upstream_started = Instant::now();
    let sent = tokio::time::timeout(timeout, async {
        let mut retried = false;
        loop {
            let hs = handshake(state, &upstream, &key).await?;
            let bytes = outgoing(r.body, &tool.upstream, &hs, &state.mcp.client_info())
                .map_err(|_| McpError::Protocol("the request couldn't be rewritten"))?;
            match upstream
                .forward(
                    &hs,
                    "tools/call",
                    Some(&tool.upstream),
                    Bytes::from(bytes),
                    headers.clone(),
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
    .unwrap_or(Err(McpError::Transport(UpstreamError::Timeout("call"))));
    let upstream_ms =
        || Some(u32::try_from(upstream_started.elapsed().as_millis()).unwrap_or(u32::MAX));
    let answer = match sent {
        Ok(a) => a,
        Err(e) => {
            if matches!(e, McpError::Unauthorized { .. }) {
                if let Err(err) =
                    mcp_connections::rejected(state, r.caller.org, server.id, owner).await
                {
                    tracing::error!(error = %err, "a refused MCP sign-in couldn't be marked");
                }
                state.caches.mcp_handshakes.invalidate(&key).await;
            }
            let (failure, message) = upstream_failure(server, &e, timeout, limit);
            return fail(failure, message, upstream_ms());
        }
    };
    if answer.status != 200 {
        return fail(
            Failure::UpstreamError,
            format!(
                "{} answered with HTTP {}; try again later.",
                server.name, answer.status
            ),
            upstream_ms(),
        );
    }
    match answer.body {
        McpBody::Json(bytes) => {
            let upstream_took = upstream_started.elapsed();
            let out = completed(&bytes, r.version);
            let failed = tool_failed(&out);
            log(
                state,
                &logged,
                failed.then_some((Failure::UpstreamError, "the tool answered with an error")),
                upstream_ms(),
                Some(&out),
            );
            let mut response = json_bytes(StatusCode::OK, out);
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            let timing = format!(
                "kaeso;dur={:.3}, upstream;dur={:.3}",
                ms(r.started.elapsed().saturating_sub(upstream_took)),
                ms(upstream_took)
            );
            if let Ok(v) = HeaderValue::from_str(&timing) {
                response.headers_mut().insert("server-timing", v);
            }
            response
        }
        McpBody::Empty => fail(
            Failure::UpstreamError,
            format!("{} answered nothing; try again later.", server.name),
            upstream_ms(),
        ),
        McpBody::Events(stream) => stream_through(
            state,
            stream,
            Streamed {
                logged,
                id: id.map_or_else(|| "null".to_owned(), |i| i.get().to_owned()),
                server: server.clone(),
                version: r.version,
                timeout,
                limit,
                upstream_started,
            },
        ),
    }
}

struct Streamed {
    logged: Logged,
    id: String,
    server: PortalServer,
    version: Version,
    timeout: Duration,
    limit: usize,
    upstream_started: Instant,
}

fn event(data: &[u8]) -> Bytes {
    let mut out = Vec::with_capacity(data.len() + 24);
    out.extend_from_slice(b"event: message\ndata: ");
    out.extend_from_slice(data);
    out.extend_from_slice(b"\n\n");
    Bytes::from(out)
}

// Events reach the client as the server sends them; the answer ends the stream, and a stream
// that runs out of time or grows past the limit ends with a tool error instead.
fn stream_through(
    state: &AppState,
    mut upstream: kaeso_upstream::ByteStream,
    s: Streamed,
) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(16);
    let st = state.clone();
    tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + s.timeout;
        let mut parser = SseParser::with_limit(s.limit);
        let mut items = Vec::new();
        let mut read = 0usize;
        let id = RawValue::from_string(s.id.clone()).ok();
        let id = id.as_deref();
        let end = |failure: Failure, message: String| -> (Option<(Failure, String)>, Vec<u8>) {
            (
                Some((failure, message.clone())),
                error_result(id, s.version, &message),
            )
        };
        let (failure, last): (Option<(Failure, String)>, Vec<u8>) = 'read: loop {
            let next = tokio::time::timeout_at(deadline, upstream.next()).await;
            let chunk = match next {
                Err(_) => {
                    break 'read end(
                        Failure::Timeout,
                        format!(
                            "{} didn't answer within {} seconds; try again later.",
                            s.server.name,
                            s.timeout.as_secs()
                        ),
                    );
                }
                Ok(None) => {
                    break 'read end(
                        Failure::UpstreamError,
                        format!("{} ended its answer early; try again later.", s.server.name),
                    );
                }
                Ok(Some(Err(e))) => {
                    let (f, m) =
                        upstream_failure(&s.server, &McpError::Transport(e), s.timeout, s.limit);
                    break 'read end(f, m);
                }
                Ok(Some(Ok(chunk))) => chunk,
            };
            read += chunk.len();
            if read > s.limit {
                let (f, m) = upstream_failure(
                    &s.server,
                    &McpError::Transport(UpstreamError::TooLarge(s.limit)),
                    s.timeout,
                    s.limit,
                );
                break 'read end(f, m);
            }
            if parser.feed_into(&chunk, &mut items).is_err() {
                break 'read end(
                    Failure::UpstreamError,
                    format!(
                        "{} sent a malformed event stream; try again later.",
                        s.server.name
                    ),
                );
            }
            for item in items.drain(..) {
                let SseItem::Event(e) = item else { continue };
                if e.data.is_empty() {
                    continue;
                }
                let Ok(m) = Message::parse(e.data.as_bytes()) else {
                    continue;
                };
                match m.kind() {
                    Kind::Response | Kind::Error
                        if m.id().map(RawValue::get) == Some(s.id.as_str()) =>
                    {
                        let out = completed(e.data.as_bytes(), s.version);
                        let failed = tool_failed(&out);
                        break 'read (
                            failed.then(|| {
                                (
                                    Failure::UpstreamError,
                                    "the tool answered with an error".to_owned(),
                                )
                            }),
                            out,
                        );
                    }
                    Kind::Request => {
                        break 'read end(
                            Failure::UpstreamError,
                            format!(
                                "{} asked a question the portal can't pass on; try the tool in another way.",
                                s.server.name
                            ),
                        );
                    }
                    Kind::Notification if tx.send(event(e.data.as_bytes())).await.is_err() => {
                        return;
                    }
                    _ => {}
                }
            }
        };
        let upstream_ms =
            Some(u32::try_from(s.upstream_started.elapsed().as_millis()).unwrap_or(u32::MAX));
        log(
            &st,
            &s.logged,
            failure.as_ref().map(|(f, m)| (*f, m.as_str())),
            upstream_ms,
            Some(&last),
        );
        let _ = tx.send(event(&last)).await;
    });
    let body = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|b| (Ok::<_, std::io::Error>(b), rx))
    });
    let mut r = (StatusCode::OK, Body::from_stream(body)).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("x-accel-buffering", HeaderValue::from_static("no"));
    r
}
