#![forbid(unsafe_code)]

pub mod guard;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
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
    #[error("the provider's address is one this install does not allow")]
    Refused(#[source] guard::Refused),
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
    pub auth: Option<AuthStyle>,
    pub guarded: bool,
    pub credential: &'a SecretString,
    pub request: Outgoing,
    pub stream: bool,
    pub timeouts: Timeouts,
    pub max_body: usize,
}

const STREAMS_PER_CONNECTION: usize = 25;
const CONNECTIONS_PER_PROVIDER: usize = 32;
const MAX_IDLE_ORIGINS: usize = 256;

fn secret_header(
    builder: reqwest::RequestBuilder,
    name: &'static str,
    value: String,
) -> reqwest::RequestBuilder {
    match reqwest::header::HeaderValue::from_str(&value) {
        Ok(mut v) => {
            v.set_sensitive(true);
            builder.header(name, v)
        }
        Err(_) => builder.header(name, value),
    }
}

#[derive(Debug)]
pub struct Upstream {
    connect_timeout: Duration,
    origins: Mutex<HashMap<String, Origin>>,
    user_agent: String,
    roots: Vec<reqwest::Certificate>,
    reach: guard::Reach,
    lookup: guard::Lookup,
}

#[derive(Debug)]
struct Origin {
    negotiated: Connection,
    multiplexed: Vec<Connection>,
    speaks_h2: Arc<AtomicBool>,
}

impl Origin {
    fn busy(&self) -> bool {
        std::iter::once(&self.negotiated)
            .chain(&self.multiplexed)
            .any(|c| c.in_flight.load(Ordering::Relaxed) > 0)
    }
}

#[derive(Debug)]
struct Connection {
    client: reqwest::Client,
    in_flight: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct Lease {
    client: reqwest::Client,
    in_flight: Arc<AtomicUsize>,
    speaks_h2: Arc<AtomicBool>,
    multiplexed: bool,
}

impl Lease {
    fn take(c: &Connection, speaks_h2: &Arc<AtomicBool>, multiplexed: bool) -> Lease {
        c.in_flight.fetch_add(1, Ordering::Relaxed);
        Lease {
            client: c.client.clone(),
            in_flight: c.in_flight.clone(),
            speaks_h2: speaks_h2.clone(),
            multiplexed,
        }
    }

    fn answered(&self, version: reqwest::Version) {
        if !self.multiplexed {
            self.speaks_h2
                .store(version == reqwest::Version::HTTP_2, Ordering::Relaxed);
        }
    }

    fn failed(&self, e: &UpstreamError) {
        if self.multiplexed && matches!(e, UpstreamError::Connect(_) | UpstreamError::Transfer(_)) {
            self.speaks_h2.store(false, Ordering::Relaxed);
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Pick {
    Use(usize),
    Open,
}

fn pick(in_flight: impl ExactSizeIterator<Item = usize>) -> Pick {
    let open = in_flight.len();
    match in_flight.enumerate().min_by_key(|&(_, n)| n) {
        Some((i, n)) if n < STREAMS_PER_CONNECTION || open >= CONNECTIONS_PER_PROVIDER => {
            Pick::Use(i)
        }
        _ => Pick::Open,
    }
}

impl Upstream {
    pub fn new(connect_timeout: Duration, user_agent: impl Into<String>) -> Upstream {
        Upstream {
            connect_timeout,
            origins: Mutex::new(HashMap::new()),
            user_agent: user_agent.into(),
            roots: Vec::new(),
            reach: guard::Reach::public_only(),
            lookup: guard::Lookup::default(),
        }
    }

    #[must_use]
    pub fn guarding(mut self, reach: guard::Reach) -> Upstream {
        self.reach = reach;
        self
    }

    #[must_use]
    pub fn resolving_with(mut self, lookup: guard::Lookup) -> Upstream {
        self.lookup = lookup;
        self
    }

    pub fn reach(&self) -> &guard::Reach {
        &self.reach
    }

    pub fn lookup(&self) -> &guard::Lookup {
        &self.lookup
    }

    pub fn reach_for(&self, base: &Url) -> guard::Reach {
        if base.scheme() == "https" {
            self.reach.clone()
        } else {
            self.reach.without_public()
        }
    }

    #[cfg(test)]
    fn trusting(mut self, root: reqwest::Certificate) -> Upstream {
        self.roots.push(root);
        self
    }

    fn connection(
        &self,
        h2_only: bool,
        reach: Option<&guard::Reach>,
    ) -> Result<Connection, UpstreamError> {
        let mut builder = reqwest::Client::builder();
        // A proxy would resolve the name itself, past the guard.
        if let Some(reach) = reach {
            builder = builder
                .no_proxy()
                .dns_resolver(Arc::new(guard::GuardedResolver::new(
                    reach.clone(),
                    self.lookup.clone(),
                )));
        }
        if !self.roots.is_empty() {
            builder = builder.tls_certs_only(self.roots.iter().cloned());
        }
        if h2_only {
            builder = builder.http2_prior_knowledge();
        }
        let client = builder
            .connect_timeout(self.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .user_agent(self.user_agent.clone())
            .build()
            .map_err(|e| UpstreamError::Client(e.without_url()))?;
        Ok(Connection {
            client,
            in_flight: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn lease(&self, base: &Url, guarded: bool) -> Result<Lease, UpstreamError> {
        let reach = guarded.then(|| self.reach_for(base));
        let key = if guarded {
            format!("guarded {base}")
        } else {
            base.as_str().to_owned()
        };
        let mut origins = self.origins.lock().unwrap_or_else(PoisonError::into_inner);
        if origins.len() >= MAX_IDLE_ORIGINS && !origins.contains_key(&key) {
            origins.retain(|_, o| o.busy());
        }
        let origin = match origins.entry(key) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(Origin {
                negotiated: self.connection(false, reach.as_ref())?,
                multiplexed: Vec::new(),
                speaks_h2: Arc::new(AtomicBool::new(false)),
            }),
        };
        if !origin.speaks_h2.load(Ordering::Relaxed) {
            return Ok(Lease::take(&origin.negotiated, &origin.speaks_h2, false));
        }
        let loads = origin
            .multiplexed
            .iter()
            .map(|c| c.in_flight.load(Ordering::Relaxed));
        if let Pick::Use(i) = pick(loads)
            && let Some(c) = origin.multiplexed.get(i)
        {
            return Ok(Lease::take(c, &origin.speaks_h2, true));
        }
        let c = self.connection(true, reach.as_ref())?;
        let lease = Lease::take(&c, &origin.speaks_h2, true);
        origin.multiplexed.push(c);
        Ok(lease)
    }

    pub async fn send(&self, call: Call<'_>) -> Result<UpstreamResponse, UpstreamError> {
        if call.guarded {
            guard::check_literal(call.base_url, &self.reach_for(call.base_url))
                .map_err(UpstreamError::Refused)?;
        }
        let lease = self.lease(call.base_url, call.guarded)?;
        let url = join(call.base_url, &call.request.path);
        let method = reqwest::Method::from_bytes(call.request.method.as_bytes())
            .map_err(|_| UpstreamError::Method(call.request.method))?;
        let mut builder = lease.client.request(method, url);
        for (name, value) in call.request.headers.iter() {
            builder = builder.header(name, value);
        }
        builder = match call.auth {
            Some(AuthStyle::Bearer) => secret_header(
                builder,
                "authorization",
                format!("Bearer {}", call.credential.expose_secret()),
            ),
            Some(AuthStyle::XApiKey) => secret_header(
                builder,
                "x-api-key",
                call.credential.expose_secret().to_owned(),
            ),
            None => builder,
        };
        let builder = builder.body(call.request.body);
        if call.stream {
            let resp = tokio::time::timeout(call.timeouts.first_byte, builder.send())
                .await
                .map_err(|_| UpstreamError::Timeout("time to first byte"))?
                .map_err(classify)
                .inspect_err(|e| lease.failed(e))?;
            lease.answered(resp.version());
            let status = resp.status().as_u16();
            let headers = headers_of(&resp);
            Ok(UpstreamResponse {
                status,
                headers,
                body: ResponseBody::Stream(ByteStream::new(resp, call.timeouts.idle, lease)),
            })
        } else {
            let fut = async {
                let resp = builder
                    .send()
                    .await
                    .map_err(classify)
                    .inspect_err(|e| lease.failed(e))?;
                lease.answered(resp.version());
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
    if guard::refused_in(&e) {
        let host = e
            .url()
            .and_then(Url::host_str)
            .unwrap_or_default()
            .to_owned();
        UpstreamError::Refused(guard::Refused { host })
    } else if e.is_timeout() {
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
    lease: Option<Lease>,
}

impl ByteStream {
    fn new(resp: reqwest::Response, idle: Duration, lease: Lease) -> ByteStream {
        let mut s = ByteStream::from_stream(Box::pin(resp.bytes_stream()), idle);
        s.lease = Some(lease);
        s
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
            lease: None,
        }
    }

    fn finish(&mut self) {
        self.done = true;
        self.lease = None;
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
                self.finish();
                Poll::Ready(Some(Err(classify(e))))
            }
            Poll::Ready(None) => {
                self.finish();
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
                        self.finish();
                        return Poll::Ready(Some(Err(UpstreamError::Timeout("idle stream"))));
                    }
                }
            },
        }
    }
}

#[cfg(test)]
#[path = "tests/load.rs"]
mod load;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_marked_sensitive() {
        let client = reqwest::Client::new();
        for (name, value) in [("authorization", "Bearer sk-x"), ("x-api-key", "sk-ant-x")] {
            let req = secret_header(client.get("http://127.0.0.1:9/"), name, value.into())
                .build()
                .unwrap_or_else(|_| unreachable!());
            let sent = req.headers().get(name).unwrap_or_else(|| unreachable!());
            assert!(sent.is_sensitive());
            assert!(!format!("{:?}", req.headers()).contains("sk-"));
        }
        assert!(
            secret_header(
                client.get("http://127.0.0.1:9/"),
                "x-api-key",
                "bad\nvalue".into()
            )
            .build()
            .is_err()
        );
    }

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
                auth: Some(AuthStyle::Bearer),
                guarded: false,
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
                auth: Some(AuthStyle::XApiKey),
                guarded: false,
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
    fn new_streams_go_to_the_least_busy_connection_until_it_is_full() {
        assert_eq!(pick([].into_iter()), Pick::Open);
        assert_eq!(pick([0].into_iter()), Pick::Use(0));
        assert_eq!(pick([STREAMS_PER_CONNECTION - 1].into_iter()), Pick::Use(0));
        assert_eq!(pick([STREAMS_PER_CONNECTION].into_iter()), Pick::Open);
        assert_eq!(
            pick([STREAMS_PER_CONNECTION, 3, 1, 1].into_iter()),
            Pick::Use(2)
        );
        assert_eq!(
            pick([STREAMS_PER_CONNECTION, STREAMS_PER_CONNECTION + 1].into_iter()),
            Pick::Open
        );
        let mut full = [STREAMS_PER_CONNECTION * 4; CONNECTIONS_PER_PROVIDER];
        assert_eq!(pick(full.iter().copied()), Pick::Use(0));
        full[5] -= 1;
        assert_eq!(pick(full.iter().copied()), Pick::Use(5));
    }

    fn connection_of(upstream: &Upstream, base: &Url) -> (Arc<AtomicUsize>, Vec<Arc<AtomicUsize>>) {
        let origins = upstream.origins.lock().unwrap();
        let origin = &origins[base.as_str()];
        (
            origin.negotiated.in_flight.clone(),
            origin
                .multiplexed
                .iter()
                .map(|c| c.in_flight.clone())
                .collect(),
        )
    }

    #[test]
    fn idle_origins_are_let_go_once_there_are_many_and_busy_ones_are_kept() {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let busy = Url::parse("https://busy.example.com").unwrap();
        let held = upstream.lease(&busy, true).unwrap();
        for i in 1..MAX_IDLE_ORIGINS {
            let idle = Url::parse(&format!("https://probe-{i}.example.com/v1")).unwrap();
            drop(upstream.lease(&idle, true).unwrap());
        }
        assert_eq!(upstream.origins.lock().unwrap().len(), MAX_IDLE_ORIGINS);
        let next = Url::parse("https://next.example.com").unwrap();
        drop(upstream.lease(&next, true).unwrap());
        let origins = upstream.origins.lock().unwrap();
        assert_eq!(origins.len(), 2);
        assert!(origins.contains_key(&format!("guarded {busy}")));
        assert!(origins.contains_key(&format!("guarded {next}")));
        drop(origins);
        drop(held);
    }

    #[test]
    fn an_origin_keeps_one_client_until_it_answers_over_http2() {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let a = Url::parse("https://a.example.com").unwrap();
        let b = Url::parse("http://127.0.0.1:9").unwrap();
        let leases: Vec<_> = (0..STREAMS_PER_CONNECTION * 3)
            .map(|_| upstream.lease(&a, false).unwrap())
            .collect();
        let plain = upstream.lease(&b, false).unwrap();
        let (negotiated, multiplexed) = connection_of(&upstream, &a);
        assert!(multiplexed.is_empty());
        assert_eq!(
            negotiated.load(Ordering::Relaxed),
            STREAMS_PER_CONNECTION * 3
        );
        assert!(
            leases
                .iter()
                .all(|l| Arc::ptr_eq(&l.in_flight, &negotiated))
        );
        assert!(!Arc::ptr_eq(&plain.in_flight, &negotiated));
        plain.answered(reqwest::Version::HTTP_11);
        drop(leases);
        assert_eq!(negotiated.load(Ordering::Relaxed), 0);
        assert!(connection_of(&upstream, &b).1.is_empty());
    }

    #[test]
    fn streams_to_an_http2_origin_spread_over_more_connections_as_they_fill() {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let base = Url::parse("https://api.example.com").unwrap();
        upstream
            .lease(&base, false)
            .unwrap()
            .answered(reqwest::Version::HTTP_2);
        let mut leases: Vec<_> = (0..STREAMS_PER_CONNECTION)
            .map(|_| upstream.lease(&base, false).unwrap())
            .collect();
        let (negotiated, multiplexed) = connection_of(&upstream, &base);
        assert_eq!(negotiated.load(Ordering::Relaxed), 0);
        assert_eq!(multiplexed.len(), 1);
        assert_eq!(
            multiplexed[0].load(Ordering::Relaxed),
            STREAMS_PER_CONNECTION
        );
        leases.push(upstream.lease(&base, false).unwrap());
        leases.push(upstream.lease(&base, false).unwrap());
        let (_, multiplexed) = connection_of(&upstream, &base);
        assert_eq!(multiplexed.len(), 2);
        assert_eq!(multiplexed[1].load(Ordering::Relaxed), 2);
        leases.truncate(10);
        leases.push(upstream.lease(&base, false).unwrap());
        assert_eq!(
            multiplexed
                .iter()
                .map(|c| c.load(Ordering::Relaxed))
                .collect::<Vec<_>>(),
            [10, 1]
        );
        let transport = reqwest::Client::new()
            .get("http://[::1")
            .build()
            .unwrap_err();
        leases[0].failed(&UpstreamError::Transfer(transport));
        let probe = upstream.lease(&base, false).unwrap();
        assert!(Arc::ptr_eq(&probe.in_flight, &negotiated));
        assert_eq!(connection_of(&upstream, &base).1.len(), 2);
        drop(leases);
        drop(probe);
        assert!(multiplexed.iter().all(|c| c.load(Ordering::Relaxed) == 0));
    }

    #[tokio::test]
    async fn a_streamed_reply_holds_its_connection_until_it_ends() {
        use futures::StreamExt;
        let base = serve_once(
            b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n9\r\ndata: x\n\n\r\n0\r\n\r\n".to_vec(),
        )
        .await;
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let credential = SecretString::from("sk-test");
        let resp = upstream
            .send(Call {
                base_url: &base,
                auth: Some(AuthStyle::Bearer),
                guarded: false,
                credential: &credential,
                request: EncodedRequest::post_json("/v1/messages", HeaderList::new(), Vec::new())
                    .into(),
                stream: true,
                timeouts: Timeouts {
                    request: Duration::from_secs(5),
                    first_byte: Duration::from_secs(5),
                    idle: Duration::from_secs(5),
                },
                max_body: 10,
            })
            .await
            .unwrap();
        let (in_flight, _) = connection_of(&upstream, &base);
        assert_eq!(in_flight.load(Ordering::Relaxed), 1);
        let ResponseBody::Stream(mut body) = resp.body else {
            unreachable!()
        };
        assert_eq!(body.next().await.unwrap().unwrap().as_ref(), b"data: x\n\n");
        assert!(body.next().await.is_none());
        assert_eq!(in_flight.load(Ordering::Relaxed), 0);
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

    async fn serve_counting(response: &'static [u8]) -> (Url, Arc<AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                count.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let _ = socket.write_all(response).await;
                let _ = socket.shutdown().await;
            }
        });
        (Url::parse(&format!("http://{addr}")).unwrap(), accepted)
    }

    const OK_CLOSE: &[u8] = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok";

    async fn guarded(upstream: &Upstream, base: &Url) -> Result<UpstreamResponse, UpstreamError> {
        let credential = SecretString::from("sk-test");
        upstream
            .send(Call {
                base_url: base,
                auth: None,
                guarded: true,
                credential: &credential,
                request: EncodedRequest::post_json(
                    "/v1/chat/completions",
                    HeaderList::new(),
                    Vec::new(),
                )
                .into(),
                stream: false,
                timeouts: Timeouts {
                    request: Duration::from_secs(5),
                    first_byte: Duration::from_secs(5),
                    idle: Duration::from_secs(5),
                },
                max_body: 1024,
            })
            .await
    }

    fn loopback() -> guard::Reach {
        guard::Reach::networks(vec![
            "127.0.0.0/8".parse().unwrap(),
            "::1/128".parse().unwrap(),
        ])
    }

    #[tokio::test]
    async fn a_guarded_call_never_reaches_a_loopback_address_the_install_does_not_allow() {
        let (base, accepted) = serve_counting(OK_CLOSE).await;
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test");
        let err = guarded(&upstream, &base).await.unwrap_err();
        assert!(matches!(err, UpstreamError::Refused(_)), "{err:?}");
        let named = Url::parse(&format!("http://localhost:{}", base.port().unwrap())).unwrap();
        let err = guarded(&upstream, &named).await.unwrap_err();
        assert!(matches!(err, UpstreamError::Refused(_)), "{err:?}");
        assert_eq!(accepted.load(Ordering::SeqCst), 0);

        let allowed = Upstream::new(Duration::from_secs(5), "kaeso-test").guarding(loopback());
        assert_eq!(guarded(&allowed, &base).await.unwrap().status, 200);
        assert_eq!(guarded(&allowed, &named).await.unwrap().status, 200);
        assert_eq!(accepted.load(Ordering::SeqCst), 2);
        assert_eq!(
            fetch(&base, 10).await.unwrap().status,
            200,
            "built-in providers are not guarded"
        );
    }

    #[tokio::test]
    async fn a_name_that_moves_to_a_refused_address_is_refused_on_the_next_connection() {
        let (base, accepted) = serve_counting(OK_CLOSE).await;
        let port = base.port().unwrap();
        let flips = Arc::new(AtomicUsize::new(0));
        let seen = flips.clone();
        let lookup = guard::Lookup::new(move |_, p| {
            let ip = if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                "127.0.0.1"
            } else {
                "10.0.0.7"
            };
            let addr = std::net::SocketAddr::new(ip.parse().unwrap(), p);
            Box::pin(async move { Ok(vec![addr]) })
        });
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test")
            .guarding(loopback())
            .resolving_with(lookup);
        let named = Url::parse(&format!("http://vllm.internal.example:{port}")).unwrap();
        assert_eq!(guarded(&upstream, &named).await.unwrap().status, 200);
        let err = guarded(&upstream, &named).await.unwrap_err();
        assert!(matches!(err, UpstreamError::Refused(_)), "{err:?}");
        assert_eq!(flips.load(Ordering::SeqCst), 2);
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_redirect_is_handed_back_and_never_followed() {
        let (target, reached) = serve_counting(OK_CLOSE).await;
        let redirect: &'static [u8] = Box::leak(
            format!(
                "HTTP/1.1 307 Temporary Redirect\r\nlocation: {target}v1/chat/completions\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            )
            .into_bytes()
            .into_boxed_slice(),
        );
        let (base, _) = serve_counting(redirect).await;
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test").guarding(loopback());
        let resp = guarded(&upstream, &base).await.unwrap();
        assert_eq!(resp.status, 307);
        assert_eq!(reached.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn plain_http_to_a_public_address_is_refused_even_with_an_allowlist() {
        let upstream = Upstream::new(Duration::from_secs(5), "kaeso-test").guarding(loopback());
        let err = guarded(&upstream, &Url::parse("http://1.1.1.1").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, UpstreamError::Refused(_)), "{err:?}");
        let metadata = Url::parse("https://169.254.169.254/latest/meta-data").unwrap();
        assert!(matches!(
            guarded(&upstream, &metadata).await,
            Err(UpstreamError::Refused(_))
        ));
    }
}
