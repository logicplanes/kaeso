use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use http_body_util::StreamBody;
use hyper::body::Frame;
use hyper_util::rt::{TokioExecutor, TokioIo};
use kaeso_codecs::{EncodedRequest, HeaderList};
use kaeso_core::provider::AuthStyle;
use secrecy::SecretString;
use url::Url;

use crate::{Call, ResponseBody, Timeouts, Upstream};

const STREAMS: usize = 400;
const MAX_CONCURRENT_STREAMS: u32 = 100;
const CHUNKS: usize = 20;
const CHUNK_GAP: Duration = Duration::from_millis(25);
const WARM_UP: &str = "/v1/warm-up";

type Chunks = Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, Infallible>> + Send>>;

#[derive(Clone, Copy)]
enum Pace {
    Gap(Duration),
    AllOpen,
}

struct Counters {
    accepted: AtomicUsize,
    served: AtomicUsize,
    open: AtomicUsize,
    peak: AtomicUsize,
    all_open: tokio::sync::watch::Sender<bool>,
}

impl Counters {
    fn new() -> Arc<Counters> {
        Arc::new(Counters {
            accepted: AtomicUsize::new(0),
            served: AtomicUsize::new(0),
            open: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            all_open: tokio::sync::watch::Sender::new(false),
        })
    }
}

struct Open(Arc<Counters>);

impl Open {
    fn new(c: &Arc<Counters>) -> Open {
        let now = c.open.fetch_add(1, Ordering::SeqCst) + 1;
        c.peak.fetch_max(now, Ordering::SeqCst);
        if now >= STREAMS {
            c.all_open.send_replace(true);
        }
        Open(c.clone())
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::SeqCst);
    }
}

fn sse(open: Open, pace: Pace) -> Chunks {
    Box::pin(futures::stream::unfold(
        (0, open),
        move |(i, open)| async move {
            if i == CHUNKS {
                return None;
            }
            match pace {
                Pace::Gap(gap) if i > 0 => tokio::time::sleep(gap).await,
                Pace::AllOpen if i == 1 => {
                    let mut all_open = open.0.all_open.subscribe();
                    let _ = all_open.wait_for(|all| *all).await;
                }
                _ => {}
            }
            let event = format!("event: delta\ndata: {{\"index\":{i},\"text\":\"token\"}}\n\n");
            Some((Ok(Frame::data(Bytes::from(event))), (i + 1, open)))
        },
    ))
}

async fn provider(counters: Arc<Counters>, pace: Pace) -> (Url, reqwest::Certificate) {
    let issued = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()]).unwrap();
    let cert = issued.cert.der().clone();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(issued.signing_key.serialize_der().into());
    let mut tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert.clone()], key)
    .unwrap();
    tls.alpn_protocols = vec![b"h2".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            counters.accepted.fetch_add(1, Ordering::SeqCst);
            let acceptor = acceptor.clone();
            let counters = counters.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let served = AtomicUsize::new(0);
                let service = hyper::service::service_fn(|req: hyper::Request<_>| {
                    if served.fetch_add(1, Ordering::SeqCst) == 0 {
                        counters.served.fetch_add(1, Ordering::SeqCst);
                    }
                    let pace = if req.uri().path() == WARM_UP {
                        Pace::Gap(Duration::ZERO)
                    } else {
                        pace
                    };
                    let body = StreamBody::new(sse(Open::new(&counters), pace));
                    async move {
                        Ok::<_, Infallible>(
                            hyper::Response::builder()
                                .header("content-type", "text/event-stream")
                                .body(body)
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                    .max_concurrent_streams(MAX_CONCURRENT_STREAMS)
                    .serve_connection(TokioIo::new(tls), service)
                    .await;
            });
        }
    });
    let base = Url::parse(&format!("https://{addr}")).unwrap();
    (base, reqwest::Certificate::from_der(&cert).unwrap())
}

struct Run {
    ttfb: Duration,
    done: Duration,
    events: usize,
}

async fn one(upstream: &Upstream, base: &Url, path: &str) -> Run {
    let started = Instant::now();
    let credential = SecretString::from("sk-test");
    let request = EncodedRequest::post_json(
        path,
        HeaderList::new(),
        br#"{"model":"m","stream":true}"#.to_vec(),
    );
    let resp = upstream
        .send(Call {
            base_url: base,
            auth: AuthStyle::Bearer,
            credential: &credential,
            request: request.into(),
            stream: true,
            timeouts: Timeouts {
                request: Duration::from_secs(120),
                first_byte: Duration::from_secs(120),
                idle: Duration::from_secs(120),
            },
            max_body: 1 << 20,
        })
        .await
        .unwrap();
    assert_eq!(resp.status, 200);
    let ResponseBody::Stream(mut body) = resp.body else {
        unreachable!()
    };
    let mut ttfb = None;
    let mut text = Vec::new();
    while let Some(chunk) = body.next().await {
        ttfb.get_or_insert_with(|| started.elapsed());
        text.extend_from_slice(&chunk.unwrap());
    }
    Run {
        ttfb: ttfb.unwrap(),
        done: started.elapsed(),
        events: text.windows(2).filter(|w| w == b"\n\n").count(),
    }
}

struct Outcome {
    wall: Duration,
    runs: Vec<Run>,
    counters: Arc<Counters>,
}

async fn burst(pace: Pace) -> Outcome {
    let counters = Counters::new();
    let (base, root) = provider(counters.clone(), pace).await;
    let upstream = Arc::new(Upstream::new(Duration::from_secs(5), "kaeso-load").trusting(root));
    assert_eq!(one(&upstream, &base, WARM_UP).await.events, CHUNKS);
    let started = Instant::now();
    let calls = (0..STREAMS).map(|_| {
        let upstream = upstream.clone();
        let base = base.clone();
        tokio::spawn(async move { one(&upstream, &base, "/v1/chat/completions").await })
    });
    let runs = tokio::time::timeout(Duration::from_secs(20), futures::future::join_all(calls))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the streams never were all open at once: open {} peak {} connections {}/{}",
                counters.open.load(Ordering::SeqCst),
                counters.peak.load(Ordering::SeqCst),
                counters.served.load(Ordering::SeqCst),
                counters.accepted.load(Ordering::SeqCst),
            )
        })
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(runs.iter().all(|r| r.events == CHUNKS));
    Outcome {
        wall: started.elapsed(),
        runs,
        counters,
    }
}

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    sorted[(sorted.len() * p / 100).min(sorted.len() - 1)]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn more_streams_than_one_connection_allows_are_all_open_at_once() {
    let out = burst(Pace::AllOpen).await;
    let c = &out.counters;
    assert_eq!(c.peak.load(Ordering::SeqCst), STREAMS);
    let needed = STREAMS.div_ceil(MAX_CONCURRENT_STREAMS as usize);
    assert!(c.served.load(Ordering::SeqCst) > needed);
    assert_eq!(
        c.accepted.load(Ordering::SeqCst),
        c.served.load(Ordering::SeqCst)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load test: cargo test -p kaeso-upstream --release -- --ignored --nocapture"]
async fn many_concurrent_streams_to_one_provider() {
    let out = burst(Pace::Gap(CHUNK_GAP)).await;
    let mut ttfb: Vec<_> = out.runs.iter().map(|r| r.ttfb).collect();
    let mut total: Vec<_> = out.runs.iter().map(|r| r.done).collect();
    ttfb.sort();
    total.sort();
    let c = &out.counters;
    let stream_length = CHUNK_GAP * u32::try_from(CHUNKS - 1).unwrap();
    println!(
        "streams={STREAMS} server_max_concurrent_streams={MAX_CONCURRENT_STREAMS} stream_length={stream_length:?}"
    );
    println!(
        "wall={:?} ttfb_p50={:?} ttfb_p99={:?} ttfb_max={:?} total_p50={:?} total_p99={:?}",
        out.wall,
        percentile(&ttfb, 50),
        percentile(&ttfb, 99),
        ttfb[ttfb.len() - 1],
        percentile(&total, 50),
        percentile(&total, 99),
    );
    println!(
        "tcp_connections_opened={} connections_serving={} peak_open_streams={}",
        c.accepted.load(Ordering::SeqCst),
        c.served.load(Ordering::SeqCst),
        c.peak.load(Ordering::SeqCst),
    );
    assert_eq!(c.peak.load(Ordering::SeqCst), STREAMS);
    assert!(percentile(&ttfb, 99) < stream_length);
}
