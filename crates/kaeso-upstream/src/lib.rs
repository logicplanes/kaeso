#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures::Stream;
use kaeso_codecs::{EncodedRequest, HeaderList};
use kaeso_core::provider::AuthStyle;
use secrecy::{ExposeSecret, SecretString};
use std::future::Future as _;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    pub request: Duration,
    pub first_byte: Duration,
    pub idle: Duration,
}

impl Timeouts {
    #[must_use]
    pub fn with_model_override(mut self, seconds: Option<u32>) -> Timeouts {
        if let Some(s) = seconds {
            let d = Duration::from_secs(u64::from(s));
            self.request = d;
            self.first_byte = d;
        }
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("cannot reach the provider")]
    Connect(#[source] reqwest::Error),
    #[error("the provider did not answer in time ({0})")]
    Timeout(&'static str),
    #[error("the connection to the provider failed")]
    Transfer(#[source] reqwest::Error),
    #[error("the provider's response is larger than {0} bytes")]
    TooLarge(usize),
    #[error("cannot set up the HTTP client for providers")]
    Client(#[source] reqwest::Error),
    #[error("`{0}` is not an HTTP method")]
    Method(&'static str),
}

#[derive(Debug)]
pub struct Outgoing {
    pub method: &'static str,
    pub path: String,
    pub headers: HeaderList,
    pub body: Bytes,
}

impl From<EncodedRequest> for Outgoing {
    fn from(e: EncodedRequest) -> Outgoing {
        Outgoing {
            method: e.method,
            path: e.path,
            headers: e.headers,
            body: Bytes::from(e.body),
        }
    }
}

pub struct UpstreamResponse {
    pub status: u16,
    pub headers: HeaderList,
    pub body: ResponseBody,
}

impl std::fmt::Debug for UpstreamResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpstreamResponse")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

pub enum ResponseBody {
    Full(Bytes),
    Stream(ByteStream),
}

#[derive(Debug)]
pub struct Call<'a> {
    pub base_url: &'a Url,
    pub auth: AuthStyle,
    pub credential: &'a SecretString,
    pub request: Outgoing,
    pub stream: bool,
    pub timeouts: Timeouts,
    pub max_body: usize,
}

#[derive(Debug)]
pub struct Upstream {
    connect_timeout: Duration,
    clients: Mutex<HashMap<String, reqwest::Client>>,
    user_agent: String,
}

impl Upstream {
    pub fn new(connect_timeout: Duration, user_agent: impl Into<String>) -> Upstream {
        Upstream {
            connect_timeout,
            clients: Mutex::new(HashMap::new()),
            user_agent: user_agent.into(),
        }
    }

    fn client(&self, base: &Url) -> Result<reqwest::Client, UpstreamError> {
        let key = base.as_str().to_owned();
        let mut clients = self.clients.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(c) = clients.get(&key) {
            return Ok(c.clone());
        }
        let client = reqwest::Client::builder()
            .connect_timeout(self.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .user_agent(self.user_agent.clone())
            .build()
            .map_err(|e| UpstreamError::Client(e.without_url()))?;
        clients.insert(key, client.clone());
        Ok(client)
    }

    pub async fn send(&self, call: Call<'_>) -> Result<UpstreamResponse, UpstreamError> {
        let client = self.client(call.base_url)?;
        let url = join(call.base_url, &call.request.path);
        let method = reqwest::Method::from_bytes(call.request.method.as_bytes())
            .map_err(|_| UpstreamError::Method(call.request.method))?;
        let mut builder = client.request(method, url);
        for (name, value) in call.request.headers.iter() {
            builder = builder.header(name, value);
        }
        builder = match call.auth {
            AuthStyle::Bearer => builder.header(
                "authorization",
                format!("Bearer {}", call.credential.expose_secret()),
            ),
            AuthStyle::XApiKey => builder.header("x-api-key", call.credential.expose_secret()),
        };
        let builder = builder.body(call.request.body);
        if call.stream {
            let resp = tokio::time::timeout(call.timeouts.first_byte, builder.send())
                .await
                .map_err(|_| UpstreamError::Timeout("time to first byte"))?
                .map_err(classify)?;
            let status = resp.status().as_u16();
            let headers = headers_of(&resp);
            Ok(UpstreamResponse {
                status,
                headers,
                body: ResponseBody::Stream(ByteStream::new(resp, call.timeouts.idle)),
            })
        } else {
            let fut = async {
                let resp = builder.send().await.map_err(classify)?;
                let status = resp.status().as_u16();
                let headers = headers_of(&resp);
                let body = read_capped(resp, call.max_body).await?;
                Ok::<_, UpstreamError>((status, headers, body))
            };
            let (status, headers, body) = tokio::time::timeout(call.timeouts.request, fut)
                .await
                .map_err(|_| UpstreamError::Timeout("request"))??;
            Ok(UpstreamResponse {
                status,
                headers,
                body: ResponseBody::Full(body),
            })
        }
    }
}

async fn read_capped(mut resp: reqwest::Response, max: usize) -> Result<Bytes, UpstreamError> {
    if resp
        .content_length()
        .is_some_and(|len| usize::try_from(len).map_or(true, |len| len > max))
    {
        return Err(UpstreamError::TooLarge(max));
    }
    let mut body = bytes::BytesMut::new();
    while let Some(chunk) = resp.chunk().await.map_err(classify)? {
        if body.len() + chunk.len() > max {
            return Err(UpstreamError::TooLarge(max));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

fn join(base: &Url, path: &str) -> String {
    format!("{}{}", base.as_str().trim_end_matches('/'), path)
}

fn classify(e: reqwest::Error) -> UpstreamError {
    if e.is_timeout() {
        UpstreamError::Timeout("connect")
    } else if e.is_connect() {
        UpstreamError::Connect(e.without_url())
    } else {
        UpstreamError::Transfer(e.without_url())
    }
}

fn later(from: tokio::time::Instant, by: Duration) -> tokio::time::Instant {
    from.checked_add(by).unwrap_or_else(|| far_future(from))
}

fn far_future(from: tokio::time::Instant) -> tokio::time::Instant {
    from.checked_add(Duration::from_secs(86_400 * 365 * 30))
        .unwrap_or(from)
}

fn headers_of(resp: &reqwest::Response) -> HeaderList {
    resp.headers()
        .iter()
        .filter_map(|(n, v)| {
            v.to_str()
                .ok()
                .map(|v| (n.as_str().to_owned(), v.to_owned()))
        })
        .collect()
}

pub struct ByteStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    idle: Duration,
    last: tokio::time::Instant,
    sleep: Pin<Box<tokio::time::Sleep>>,
    done: bool,
}

impl ByteStream {
    fn new(resp: reqwest::Response, idle: Duration) -> ByteStream {
        ByteStream::from_stream(Box::pin(resp.bytes_stream()), idle)
    }

    fn from_stream(
        inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
        idle: Duration,
    ) -> ByteStream {
        let now = tokio::time::Instant::now();
        ByteStream {
            inner,
            idle,
            last: now,
            sleep: Box::pin(tokio::time::sleep_until(later(now, idle))),
            done: false,
        }
    }
}

impl Stream for ByteStream {
    type Item = Result<Bytes, UpstreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.last = tokio::time::Instant::now();
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.done = true;
                Poll::Ready(Some(Err(classify(e))))
            }
            Poll::Ready(None) => {
                self.done = true;
                Poll::Ready(None)
            }
            Poll::Pending => loop {
                match self.sleep.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        let due = later(self.last, self.idle);
                        if due > tokio::time::Instant::now() {
                            self.sleep.as_mut().reset(due);
                            continue;
                        }
                        self.done = true;
                        return Poll::Ready(Some(Err(UpstreamError::Timeout("idle stream"))));
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_without_double_slashes() {
        let base = Url::parse("https://api.example.com/").unwrap_or_else(|_| unreachable!());
        assert_eq!(
            join(&base, "/v1/messages"),
            "https://api.example.com/v1/messages"
        );
        let base = Url::parse("http://127.0.0.1:9/proxy").unwrap_or_else(|_| unreachable!());
        assert_eq!(
            join(&base, "/v1/responses"),
            "http://127.0.0.1:9/proxy/v1/responses"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_idle_timeout_counts_from_the_last_chunk() {
        use futures::StreamExt;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
        let inner = futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|b| (Ok::<_, reqwest::Error>(b), rx))
        });
        let mut s = ByteStream::from_stream(Box::pin(inner), Duration::from_secs(10));
        for _ in 0..5 {
            tokio::time::advance(Duration::from_secs(6)).await;
            assert!(tx.send(Bytes::from_static(b"data: x\n\n")).is_ok());
            assert!(matches!(s.next().await, Some(Ok(_))));
        }
        let quiet = tokio::time::Instant::now();
        assert!(matches!(
            s.next().await,
            Some(Err(UpstreamError::Timeout("idle stream")))
        ));
        assert_eq!(quiet.elapsed(), Duration::from_secs(10));
        assert!(s.next().await.is_none());
        drop(tx);
    }

    async fn serve_once(response: Vec<u8>) -> Url {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let _ = socket.write_all(&response).await;
            let _ = socket.shutdown().await;
        });
        Url::parse(&format!("http://{addr}")).unwrap()
    }

    async fn fetch(base: &Url, max_body: usize) -> Result<UpstreamResponse, UpstreamError> {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let request = EncodedRequest::post_json("/v1/messages", HeaderList::new(), Vec::new());
        let credential = SecretString::from("sk-test");
        upstream
            .send(Call {
                base_url: base,
                auth: AuthStyle::Bearer,
                credential: &credential,
                request: request.into(),
                stream: false,
                timeouts: Timeouts {
                    request: Duration::from_secs(5),
                    first_byte: Duration::from_secs(5),
                    idle: Duration::from_secs(5),
                },
                max_body,
            })
            .await
    }

    #[tokio::test]
    async fn non_streaming_bodies_stop_at_the_limit() {
        let declared =
            b"HTTP/1.1 200 OK\r\ncontent-length: 11\r\nconnection: close\r\n\r\nhello world"
                .to_vec();
        let base = serve_once(declared.clone()).await;
        assert!(matches!(
            fetch(&base, 10).await,
            Err(UpstreamError::TooLarge(10))
        ));
        let base = serve_once(declared).await;
        let ok = fetch(&base, 11).await.unwrap();
        assert!(matches!(ok.body, ResponseBody::Full(b) if b.as_ref() == b"hello world"));
        let chunked = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n6\r\nhello \r\n5\r\nworld\r\n0\r\n\r\n".to_vec();
        let base = serve_once(chunked).await;
        assert!(matches!(
            fetch(&base, 8).await,
            Err(UpstreamError::TooLarge(8))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn an_idle_timeout_beyond_the_clock_never_fires() {
        use futures::StreamExt;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
        let inner = futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|b| (Ok::<_, reqwest::Error>(b), rx))
        });
        let mut s = ByteStream::from_stream(Box::pin(inner), Duration::MAX);
        tokio::time::advance(Duration::from_secs(86_400 * 365)).await;
        assert!(tx.send(Bytes::from_static(b"data: x\n\n")).is_ok());
        assert!(matches!(s.next().await, Some(Ok(_))));
        drop(tx);
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn transport_errors_keep_their_cause_and_hide_the_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!(
            "http://{}/secret-path",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        drop(listener);
        let err = fetch(&base, 10).await.unwrap_err();
        assert!(matches!(err, UpstreamError::Connect(_)), "{err:?}");
        assert_eq!(err.to_string(), "cannot reach the provider");
        let cause = std::error::Error::source(&err).unwrap();
        assert!(!cause.to_string().contains("secret-path"), "{cause}");
        assert!(std::error::Error::source(cause).is_some(), "{cause:?}");
    }

    #[tokio::test]
    async fn a_bad_method_is_refused_before_anything_is_sent() {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let base = Url::parse("http://127.0.0.1:9").unwrap();
        let credential = SecretString::from("sk-test");
        let timeouts = Timeouts {
            request: Duration::from_secs(5),
            first_byte: Duration::from_secs(5),
            idle: Duration::from_secs(5),
        };
        let err = upstream
            .send(Call {
                base_url: &base,
                auth: AuthStyle::XApiKey,
                credential: &credential,
                request: Outgoing {
                    method: "NOT A METHOD",
                    path: "/v1/messages".into(),
                    headers: HeaderList::new(),
                    body: Bytes::new(),
                },
                stream: false,
                timeouts,
                max_body: 10,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, UpstreamError::Method("NOT A METHOD")),
            "{err:?}"
        );
    }

    #[test]
    fn model_overrides_replace_the_long_timeouts() {
        let t = Timeouts {
            request: Duration::from_secs(600),
            first_byte: Duration::from_secs(60),
            idle: Duration::from_secs(300),
        };
        assert_eq!(t.with_model_override(None), t);
        let o = t.with_model_override(Some(30));
        assert_eq!(
            (o.request, o.first_byte, o.idle),
            (
                Duration::from_secs(30),
                Duration::from_secs(30),
                Duration::from_secs(300)
            )
        );
    }
}
