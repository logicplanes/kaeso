use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::HeaderList;
use kaeso_codecs::sse::{SseItem, SseParser};
use kaeso_mcp::{Kind, Message, RawObject, Version, code, headers as mcp_headers, meta};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use url::Url;

use crate::guard::{self, GuardedResolver, Lookup, Reach};
use crate::{ByteStream, UpstreamError, classify};

const ACCEPT: &str = "application/json, text/event-stream";
const CLIENT_NAME: &str = "kaeso";
const MAX_PAGES: usize = 20;

#[derive(Debug)]
pub struct McpUpstream {
    reach: Reach,
    lookup: Lookup,
    connect_timeout: Duration,
    user_agent: String,
    version: String,
    clients: OnceLock<Clients>,
}

#[derive(Debug)]
struct Clients {
    https: reqwest::Client,
    http: reqwest::Client,
}

#[derive(Clone, Copy, Debug)]
pub struct SecretHeader<'a> {
    pub name: &'a str,
    pub value: &'a SecretString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub first_byte: Duration,
    pub idle: Duration,
    pub max_body: usize,
}

#[derive(Debug)]
pub struct McpCall<'a> {
    pub url: &'a Url,
    pub body: Bytes,
    pub headers: Vec<(&'static str, String)>,
    pub secret: Option<SecretHeader<'a>>,
    pub limits: Limits,
}

pub enum McpBody {
    Json(Bytes),
    Events(ByteStream),
    Empty,
}

pub struct McpAnswer {
    pub status: u16,
    pub headers: HeaderList,
    pub body: McpBody,
}

impl std::fmt::Debug for McpAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpAnswer")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl McpAnswer {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error(transparent)]
    Transport(#[from] UpstreamError),
    #[error("the MCP server asks for sign-in (HTTP {status})")]
    Unauthorized {
        status: u16,
        challenge: Option<String>,
    },
    #[error("the MCP server answered HTTP {0}")]
    Status(u16),
    #[error("the MCP server answered with error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("the MCP server's answer is not valid MCP: {0}")]
    Protocol(&'static str),
    #[error("the MCP server ended its session")]
    SessionGone,
}

impl McpUpstream {
    pub fn new(
        reach: Reach,
        lookup: Lookup,
        connect_timeout: Duration,
        user_agent: &str,
    ) -> McpUpstream {
        McpUpstream {
            reach,
            lookup,
            connect_timeout,
            user_agent: user_agent.to_owned(),
            version: user_agent
                .rsplit('/')
                .next()
                .unwrap_or(user_agent)
                .to_owned(),
            clients: OnceLock::new(),
        }
    }

    // A proxy would resolve the name itself, past the guard; redirects are handed back so that
    // no hop reaches an address the guard hasn't seen.
    fn build(&self, reach: Reach) -> Result<reqwest::Client, UpstreamError> {
        reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(GuardedResolver::new(reach, self.lookup.clone())))
            .connect_timeout(self.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .user_agent(self.user_agent.as_str())
            .build()
            .map_err(|e| UpstreamError::Client(e.without_url()))
    }

    fn clients(&self) -> Result<&Clients, UpstreamError> {
        if let Some(c) = self.clients.get() {
            return Ok(c);
        }
        let built = Clients {
            https: self.build(self.reach.clone())?,
            http: self.build(self.reach.without_public())?,
        };
        Ok(self.clients.get_or_init(|| built))
    }

    pub fn lookup(&self) -> &Lookup {
        &self.lookup
    }

    pub fn reach_for(&self, url: &Url) -> Reach {
        if url.scheme() == "https" {
            self.reach.clone()
        } else {
            self.reach.without_public()
        }
    }

    fn client_for(&self, url: &Url) -> Result<&reqwest::Client, UpstreamError> {
        guard::check_literal(url, &self.reach_for(url)).map_err(UpstreamError::Refused)?;
        let clients = self.clients()?;
        Ok(if url.scheme() == "https" {
            &clients.https
        } else {
            &clients.http
        })
    }

    async fn plain(
        &self,
        builder: reqwest::RequestBuilder,
        limits: Limits,
    ) -> Result<(u16, Bytes), UpstreamError> {
        let fut = async {
            let resp = builder.send().await.map_err(classify)?;
            let status = resp.status().as_u16();
            let body = crate::read_capped(resp, limits.max_body).await?;
            Ok::<_, UpstreamError>((status, body))
        };
        tokio::time::timeout(limits.first_byte, fut)
            .await
            .map_err(|_| UpstreamError::Timeout("request"))?
    }

    // Sign-in discovery and token calls go to addresses an MCP server names, so they take the
    // same guard and never follow a redirect: a 3xx comes back as its status.
    pub async fn get_json(&self, url: &Url, limits: Limits) -> Result<(u16, Bytes), UpstreamError> {
        let builder = self
            .client_for(url)?
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "application/json");
        self.plain(builder, limits).await
    }

    pub async fn get_json_signed(
        &self,
        url: &Url,
        header: &str,
        value: &SecretString,
        limits: Limits,
    ) -> Result<(u16, Bytes), UpstreamError> {
        let mut v = reqwest::header::HeaderValue::from_str(value.expose_secret())
            .map_err(|_| UpstreamError::Method("a sign-in header value"))?;
        v.set_sensitive(true);
        let builder = self
            .client_for(url)?
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "application/json")
            .header(header, v);
        self.plain(builder, limits).await
    }

    pub async fn post_form(
        &self,
        url: &Url,
        form: &[(&str, &str)],
        basic: Option<(&str, &SecretString)>,
        limits: Limits,
    ) -> Result<(u16, Bytes), UpstreamError> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        let mut builder = self
            .client_for(url)?
            .post(url.clone())
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(reqwest::header::ACCEPT, "application/json")
            .body(body);
        if let Some((user, password)) = basic {
            builder = builder.header(reqwest::header::AUTHORIZATION, basic_auth(user, password)?);
        }
        self.plain(builder, limits).await
    }

    pub async fn post_json(
        &self,
        url: &Url,
        body: Bytes,
        limits: Limits,
    ) -> Result<(u16, Bytes), UpstreamError> {
        let builder = self
            .client_for(url)?
            .post(url.clone())
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
        self.plain(builder, limits).await
    }

    pub async fn post(&self, call: McpCall<'_>) -> Result<McpAnswer, UpstreamError> {
        let client = self.client_for(call.url)?;
        let mut builder = client
            .post(call.url.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, ACCEPT);
        for (name, value) in &call.headers {
            builder = builder.header(*name, value.as_str());
        }
        if let Some(secret) = call.secret {
            let name = reqwest::header::HeaderName::from_bytes(secret.name.as_bytes())
                .map_err(|_| UpstreamError::Method("a sign-in header name"))?;
            let mut value = reqwest::header::HeaderValue::from_str(secret.value.expose_secret())
                .map_err(|_| UpstreamError::Method("a sign-in header value"))?;
            value.set_sensitive(true);
            builder = builder.header(name, value);
        }
        let resp = tokio::time::timeout(call.limits.first_byte, builder.body(call.body).send())
            .await
            .map_err(|_| UpstreamError::Timeout("time to first byte"))?
            .map_err(classify)?;
        let status = resp.status().as_u16();
        let headers = crate::headers_of(&resp);
        let events = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.trim_start().starts_with("text/event-stream"));
        let body = if status == 202 || status == 204 {
            McpBody::Empty
        } else if events {
            McpBody::Events(ByteStream::from_stream(
                Box::pin(resp.bytes_stream()),
                call.limits.idle,
            ))
        } else {
            let read = tokio::time::timeout(
                call.limits.first_byte.max(call.limits.idle),
                crate::read_capped(resp, call.limits.max_body),
            )
            .await
            .map_err(|_| UpstreamError::Timeout("request"))??;
            McpBody::Json(read)
        };
        Ok(McpAnswer {
            status,
            headers,
            body,
        })
    }
}

// RFC 6749 §2.3.1: the client id and secret are form-encoded before they are joined and
// encoded for HTTP Basic authentication.
fn basic_auth(
    user: &str,
    password: &SecretString,
) -> Result<reqwest::header::HeaderValue, UpstreamError> {
    use base64::Engine;
    let encode = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
    let joined = format!("{}:{}", encode(user), encode(password.expose_secret()));
    let mut value = reqwest::header::HeaderValue::from_str(&format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(joined)
    ))
    .map_err(|_| UpstreamError::Method("a client secret"))?;
    value.set_sensitive(true);
    Ok(value)
}

// Reads one JSON-RPC message from an answer: the body itself, or from an event stream the first
// response to `id`, skipping the notifications a server may send before it.
pub async fn reply(body: McpBody, id: &str, max_bytes: usize) -> Result<Bytes, McpError> {
    match body {
        McpBody::Json(bytes) => Ok(bytes),
        McpBody::Empty => Err(McpError::Protocol("the answer is empty")),
        McpBody::Events(mut stream) => {
            let mut parser = SseParser::with_limit(max_bytes);
            let mut read = 0usize;
            let mut items = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                read += chunk.len();
                if read > max_bytes {
                    return Err(UpstreamError::TooLarge(max_bytes).into());
                }
                parser
                    .feed_into(&chunk, &mut items)
                    .map_err(|_| McpError::Protocol("the event stream is malformed"))?;
                if let Some(found) = answer_in(&mut items, id) {
                    return Ok(found);
                }
            }
            let mut rest = parser
                .finish()
                .map_err(|_| McpError::Protocol("the event stream is malformed"))?;
            answer_in(&mut rest, id).ok_or(McpError::Protocol("the stream ended before the answer"))
        }
    }
}

fn answer_in(items: &mut Vec<SseItem>, id: &str) -> Option<Bytes> {
    let found = items.iter().find_map(|item| match item {
        SseItem::Event(e) if !e.data.is_empty() => {
            let m = Message::parse(e.data.as_bytes()).ok()?;
            let matches = matches!(m.kind(), Kind::Response | Kind::Error)
                && m.id().map(serde_json::value::RawValue::get) == Some(id);
            matches.then(|| Bytes::copy_from_slice(e.data.as_bytes()))
        }
        _ => None,
    });
    items.clear();
    found
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handshake {
    pub version: Version,
    pub session: Option<String>,
    pub server_info: Option<Value>,
    pub capabilities: Option<Value>,
    pub instructions: Option<String>,
}

pub struct Client<'a> {
    pub upstream: &'a McpUpstream,
    pub url: &'a Url,
    pub secret: Option<SecretHeader<'a>>,
    pub limits: Limits,
}

impl Client<'_> {
    fn client_info(&self) -> Value {
        json!({ "name": CLIENT_NAME, "title": "Kaeso", "version": self.upstream.version })
    }

    fn headers(
        &self,
        hs: Option<&Handshake>,
        method: &str,
        name: Option<&str>,
    ) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let Some(hs) = hs else {
            return out;
        };
        if hs.version.is_stateless() {
            out.push((
                mcp_headers::PROTOCOL_VERSION,
                hs.version.as_str().to_owned(),
            ));
            out.push((mcp_headers::METHOD, method.to_owned()));
            if let Some(name) = name {
                out.push((mcp_headers::NAME, mcp_headers::encode(name).into_owned()));
            }
        } else {
            if hs.version != Version::V2025_03_26 {
                out.push((
                    mcp_headers::PROTOCOL_VERSION,
                    hs.version.as_str().to_owned(),
                ));
            }
            if let Some(session) = &hs.session {
                out.push((mcp_headers::SESSION_ID, session.clone()));
            }
        }
        out
    }

    fn message(
        &self,
        hs: Option<&Handshake>,
        id: &str,
        method: &str,
        params: Option<Value>,
    ) -> Vec<u8> {
        let mut params = params.unwrap_or_else(|| json!({}));
        if let (Some(hs), Some(obj)) = (hs, params.as_object_mut())
            && hs.version.is_stateless()
        {
            let meta = obj.entry(meta::KEY).or_insert_with(|| json!({}));
            if let Some(m) = meta.as_object_mut() {
                m.insert(
                    meta::PROTOCOL_VERSION.to_owned(),
                    json!(hs.version.as_str()),
                );
                m.insert(meta::CLIENT_INFO.to_owned(), self.client_info());
                m.insert(meta::CLIENT_CAPABILITIES.to_owned(), json!({}));
            }
        }
        serde_json::to_vec(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
        )
        .unwrap_or_default()
    }

    async fn send(
        &self,
        hs: Option<&Handshake>,
        method: &str,
        params: Option<Value>,
        id: &str,
    ) -> Result<McpAnswer, McpError> {
        let name = params
            .as_ref()
            .and_then(|p| mcp_headers::name_source(method).and_then(|f| p.get(f)))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut headers = self.headers(hs, method, name.as_deref());
        if hs.is_none() && method != "initialize" {
            headers.push((
                mcp_headers::PROTOCOL_VERSION,
                Version::LATEST.as_str().to_owned(),
            ));
            headers.push((mcp_headers::METHOD, method.to_owned()));
        }
        let body = self.message(hs, id, method, params);
        let answer = self
            .upstream
            .post(McpCall {
                url: self.url,
                body: Bytes::from(body),
                headers,
                secret: self.secret,
                limits: self.limits,
            })
            .await?;
        if answer.status == 401 || answer.status == 403 {
            return Err(McpError::Unauthorized {
                status: answer.status,
                challenge: answer.header("www-authenticate").map(str::to_owned),
            });
        }
        Ok(answer)
    }

    // Asks one question and returns the result as raw JSON, so nothing the server adds is lost.
    pub async fn request(
        &self,
        hs: &Handshake,
        method: &str,
        params: Option<Value>,
        id: &str,
    ) -> Result<Bytes, McpError> {
        let answer = self.send(Some(hs), method, params, id).await?;
        if answer.status == 404 && hs.session.is_some() {
            return Err(McpError::SessionGone);
        }
        let status = answer.status;
        let bytes = reply(answer.body, &format!("\"{id}\""), self.limits.max_body).await?;
        result_of(&bytes, status)
    }

    pub async fn handshake(&self) -> Result<Handshake, McpError> {
        let id = "kaeso-discover";
        let answer = self.send(None, "server/discover", None, id).await?;
        let status = answer.status;
        let session = answer.header(mcp_headers::SESSION_ID).map(str::to_owned);
        let bytes = match answer.body {
            McpBody::Empty => None,
            body => reply(body, &format!("\"{id}\""), self.limits.max_body)
                .await
                .ok(),
        };
        let parsed = bytes.as_deref().and_then(|b| Message::parse(b).ok());
        if let Some(m) = parsed.as_ref() {
            if status == 200 && m.kind() == Kind::Response {
                let result: Value = m
                    .result()
                    .and_then(|r| serde_json::from_str(r.get()).ok())
                    .unwrap_or(Value::Null);
                let supported = versions(at(&result, "supportedVersions"));
                if supported.contains(&Version::V2026_07_28) {
                    return Ok(Handshake {
                        version: Version::V2026_07_28,
                        session: None,
                        server_info: result
                            .pointer("/_meta/io.modelcontextprotocol~1serverInfo")
                            .cloned(),
                        capabilities: result.get("capabilities").cloned(),
                        instructions: result
                            .get("instructions")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    });
                }
                if supported.is_empty() {
                    return Err(McpError::Protocol(
                        "server/discover named no version Kaeso speaks",
                    ));
                }
            } else if let Some(error) = m.error() {
                let error: Value = serde_json::from_str(error.get()).unwrap_or(Value::Null);
                let code = at(&error, "code").as_i64().unwrap_or_default();
                // A modern error means a modern server: only falling back when the server names an
                // older version Kaeso also speaks, never on a header or capability problem.
                if code == code::UNSUPPORTED_PROTOCOL_VERSION {
                    if versions(at(at(&error, "data"), "supported")).is_empty() {
                        return Err(McpError::Protocol(
                            "the server speaks no version Kaeso speaks",
                        ));
                    }
                } else if code == code::HEADER_MISMATCH
                    || code == code::MISSING_REQUIRED_CLIENT_CAPABILITY
                {
                    return Err(McpError::Rpc {
                        code,
                        message: at(&error, "message")
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    });
                }
            }
        }
        drop(session);
        self.initialize().await
    }

    async fn initialize(&self) -> Result<Handshake, McpError> {
        let id = "kaeso-initialize";
        let params = json!({
            "protocolVersion": Version::LATEST_LEGACY.as_str(),
            "capabilities": {},
            "clientInfo": self.client_info(),
        });
        let answer = self.send(None, "initialize", Some(params), id).await?;
        let status = answer.status;
        let session = answer.header(mcp_headers::SESSION_ID).map(str::to_owned);
        let bytes = reply(answer.body, &format!("\"{id}\""), self.limits.max_body).await?;
        let result: Value = serde_json::from_slice(&result_of(&bytes, status)?)
            .map_err(|_| McpError::Protocol("initialize answered no object"))?;
        let version = at(&result, "protocolVersion")
            .as_str()
            .and_then(Version::parse)
            .filter(|v| !v.is_stateless())
            .ok_or(McpError::Protocol(
                "initialize agreed on no version Kaeso speaks",
            ))?;
        let hs = Handshake {
            version,
            session,
            server_info: result.get("serverInfo").cloned(),
            capabilities: result.get("capabilities").cloned(),
            instructions: result
                .get("instructions")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        let notice =
            serde_json::to_vec(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .unwrap_or_default();
        let sent = self
            .upstream
            .post(McpCall {
                url: self.url,
                body: Bytes::from(notice),
                headers: self.headers(Some(&hs), "notifications/initialized", None),
                secret: self.secret,
                limits: self.limits,
            })
            .await;
        if let Err(e) = sent {
            return Err(e.into());
        }
        Ok(hs)
    }

    pub async fn list_tools(&self, hs: &Handshake) -> Result<Vec<Value>, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for page in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|c| json!({ "cursor": c }));
            let result = self
                .request(hs, "tools/list", params, &format!("kaeso-tools-{page}"))
                .await?;
            let result: Value = serde_json::from_slice(&result)
                .map_err(|_| McpError::Protocol("tools/list answered no object"))?;
            let Some(listed) = at(&result, "tools").as_array() else {
                return Err(McpError::Protocol("tools/list answered no tools"));
            };
            tools.extend(listed.iter().cloned());
            match at(&result, "nextCursor").as_str() {
                Some(next) if !next.is_empty() => cursor = Some(next.to_owned()),
                _ => return Ok(tools),
            }
        }
        Err(McpError::Protocol("tools/list kept paging past 20 pages"))
    }
}

static NULL: Value = Value::Null;

fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&NULL)
}

fn versions(list: &Value) -> Vec<Version> {
    list.as_array()
        .map(|l| {
            l.iter()
                .filter_map(Value::as_str)
                .filter_map(Version::parse)
                .collect()
        })
        .unwrap_or_default()
}

fn result_of(bytes: &[u8], status: u16) -> Result<Bytes, McpError> {
    let Ok(m) = Message::parse(bytes) else {
        return Err(if (200..300).contains(&status) {
            McpError::Protocol("the answer is not a JSON-RPC message")
        } else {
            McpError::Status(status)
        });
    };
    if let Some(error) = m.error() {
        let error =
            RawObject::of(error).map_err(|_| McpError::Protocol("the error is not an object"))?;
        return Err(McpError::Rpc {
            code: error
                .get("code")
                .and_then(|c| c.get().parse().ok())
                .unwrap_or_default(),
            message: error.string("message").unwrap_or_default(),
        });
    }
    match m.result() {
        Some(result) => Ok(Bytes::copy_from_slice(result.get().as_bytes())),
        None => Err(McpError::Protocol("the answer has no result")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_and_errors_are_told_apart() {
        let ok = br#"{"jsonrpc":"2.0","id":"a","result":{"tools":[ ]}}"#;
        assert_eq!(
            result_of(ok, 200).unwrap(),
            Bytes::from_static(br#"{"tools":[ ]}"#)
        );
        let failed =
            br#"{"jsonrpc":"2.0","id":"a","error":{"code":-32601,"message":"Method not found"}}"#;
        assert!(matches!(
            result_of(failed, 200),
            Err(McpError::Rpc { code: -32601, ref message }) if message == "Method not found"
        ));
        assert!(matches!(
            result_of(b"<html>", 502),
            Err(McpError::Status(502))
        ));
        assert!(matches!(
            result_of(b"<html>", 200),
            Err(McpError::Protocol(_))
        ));
        assert!(matches!(
            result_of(br#"{"jsonrpc":"2.0","method":"x"}"#, 200),
            Err(McpError::Protocol(_))
        ));
    }

    #[test]
    fn only_versions_kaeso_speaks_are_kept() {
        assert_eq!(
            versions(&json!(["2026-07-28", "2099-01-01", "2025-06-18", 7])),
            [Version::V2026_07_28, Version::V2025_06_18]
        );
        assert!(versions(&json!(null)).is_empty());
    }

    #[tokio::test]
    async fn the_answer_is_found_after_notifications_in_a_stream() {
        let events = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{}}\n\n\
                      data: {\"jsonrpc\":\"2.0\",\"id\":\"other\",\"result\":{}}\n\n\
                      : keep-alive\n\n\
                      data: {\"jsonrpc\":\"2.0\",\"id\":\"q\",\"result\":{\"ok\":true}}\n\n";
        let chunks: Vec<Result<Bytes, reqwest::Error>> = events
            .as_bytes()
            .chunks(7)
            .map(|c| Ok(Bytes::copy_from_slice(c)))
            .collect();
        let stream = ByteStream::from_stream(
            Box::pin(futures::stream::iter(chunks)),
            Duration::from_secs(5),
        );
        let found = reply(McpBody::Events(stream), "\"q\"", 1 << 20)
            .await
            .unwrap();
        assert_eq!(
            found,
            Bytes::from_static(br#"{"jsonrpc":"2.0","id":"q","result":{"ok":true}}"#)
        );
    }

    #[tokio::test]
    async fn a_stream_without_the_answer_or_over_the_limit_fails() {
        let make = |text: &'static str| {
            ByteStream::from_stream(
                Box::pin(futures::stream::iter(vec![Ok(Bytes::from_static(
                    text.as_bytes(),
                ))])),
                Duration::from_secs(5),
            )
        };
        let none = reply(
            McpBody::Events(make("data: {\"jsonrpc\":\"2.0\",\"method\":\"x\"}\n\n")),
            "\"q\"",
            1 << 20,
        )
        .await;
        assert!(matches!(none, Err(McpError::Protocol(_))));
        let big = reply(
            McpBody::Events(make(
                "data: {\"jsonrpc\":\"2.0\",\"id\":\"q\",\"result\":{}}\n\n",
            )),
            "\"q\"",
            10,
        )
        .await;
        assert!(matches!(
            big,
            Err(McpError::Transport(UpstreamError::TooLarge(10)))
        ));
        assert!(matches!(
            reply(McpBody::Empty, "\"q\"", 10).await,
            Err(McpError::Protocol(_))
        ));
    }
}
