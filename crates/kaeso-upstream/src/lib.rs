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
    pub connect: Duration,
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
    #[error("cannot reach the provider: {0}")]
    Connect(String),
    #[error("the provider did not answer in time ({0})")]
    Timeout(&'static str),
    #[error("the connection to the provider failed: {0}")]
    Transfer(String),
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
    pub request: &'a EncodedRequest,
    pub stream: bool,
    pub timeouts: Timeouts,
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
            .map_err(|e| UpstreamError::Connect(e.to_string()))?;
        clients.insert(key, client.clone());
        Ok(client)
    }

    pub async fn send(&self, call: Call<'_>) -> Result<UpstreamResponse, UpstreamError> {
        let client = self.client(call.base_url)?;
        let url = join(call.base_url, &call.request.path);
        let method = reqwest::Method::from_bytes(call.request.method.as_bytes())
            .map_err(|e| UpstreamError::Connect(e.to_string()))?;
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
        let builder = builder.body(call.request.body.clone());
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
                let body = resp.bytes().await.map_err(classify)?;
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

fn join(base: &Url, path: &str) -> String {
    format!("{}{}", base.as_str().trim_end_matches('/'), path)
}

fn classify(e: reqwest::Error) -> UpstreamError {
    if e.is_timeout() {
        UpstreamError::Timeout("connect")
    } else if e.is_connect() {
        UpstreamError::Connect(without_url(&e))
    } else {
        UpstreamError::Transfer(without_url(&e))
    }
}

fn without_url(e: &reqwest::Error) -> String {
    let mut text = e.to_string();
    if let Some(url) = e.url() {
        text = text.replace(url.as_str(), "the provider");
    }
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        text.push_str(": ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    text
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
            sleep: Box::pin(tokio::time::sleep_until(now + idle)),
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
                        let due = self.last + self.idle;
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

    #[test]
    fn model_overrides_replace_the_long_timeouts() {
        let t = Timeouts {
            connect: Duration::from_secs(10),
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
