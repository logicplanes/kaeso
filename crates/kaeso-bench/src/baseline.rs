use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use hyper::Method;
use serde::Serialize;
use serde_json::json;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::client::{Call, Conn, connect, read_all, server_timing};
use crate::harness::MODEL;
use crate::stats::Summary;
use crate::streams::PATH;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Baseline {
    pub(crate) requests: usize,
    pub(crate) concurrency: usize,
    pub(crate) seconds: f64,
    pub(crate) requests_per_second: f64,
    pub(crate) overhead_ms: Summary,
    pub(crate) upstream_ms: Summary,
    pub(crate) latency_ms: Summary,
    pub(crate) failed: usize,
    pub(crate) failures: BTreeMap<String, usize>,
}

impl Baseline {
    pub(crate) fn why(&self) -> String {
        if self.failures.is_empty() {
            return String::new();
        }
        let listed: Vec<String> = self
            .failures
            .iter()
            .map(|(w, n)| format!("{n} × {w}"))
            .collect();
        format!("; {}", listed.join(", "))
    }
}

#[derive(Default)]
struct Sample {
    overhead_ms: Vec<f64>,
    upstream_ms: Vec<f64>,
    latency_ms: Vec<f64>,
    failures: BTreeMap<String, usize>,
}

impl Sample {
    fn failed(&self) -> usize {
        self.failures.values().sum()
    }
}

fn body() -> Bytes {
    Bytes::from(
        json!({
            "model": MODEL,
            "messages": [{ "role": "user", "content": "bench [mock:gen=16]" }],
        })
        .to_string(),
    )
}

pub(crate) enum Until {
    Count(usize),
    Stopped(watch::Receiver<bool>),
}

pub(crate) async fn run(addr: SocketAddr, key: &str, concurrency: usize, until: Until) -> Baseline {
    let claimed = Arc::new(AtomicUsize::new(0));
    let (limit, stopped) = match until {
        Until::Count(n) => (n, None),
        Until::Stopped(rx) => (usize::MAX, Some(rx)),
    };
    let started = Instant::now();
    let mut tasks = JoinSet::new();
    for _ in 0..concurrency.max(1) {
        let claimed = claimed.clone();
        let key = key.to_owned();
        let stopped = stopped.clone();
        tasks.spawn(async move { worker(addr, &key, &claimed, limit, stopped).await });
    }
    let mut all = Sample::default();
    while let Some(done) = tasks.join_next().await {
        if let Ok(s) = done {
            all.overhead_ms.extend(s.overhead_ms);
            all.upstream_ms.extend(s.upstream_ms);
            all.latency_ms.extend(s.latency_ms);
            for (why, n) in s.failures {
                *all.failures.entry(why).or_default() += n;
            }
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    let failed = all.failed();
    let requests = all.latency_ms.len() + failed;
    Baseline {
        requests,
        concurrency,
        seconds,
        requests_per_second: requests as f64 / seconds.max(f64::EPSILON),
        overhead_ms: Summary::of(&all.overhead_ms),
        upstream_ms: Summary::of(&all.upstream_ms),
        latency_ms: Summary::of(&all.latency_ms),
        failed,
        failures: all.failures,
    }
}

async fn worker(
    addr: SocketAddr,
    key: &str,
    claimed: &AtomicUsize,
    limit: usize,
    stopped: Option<watch::Receiver<bool>>,
) -> Sample {
    let mut sample = Sample::default();
    let mut conn: Option<Conn> = None;
    loop {
        if stopped.as_ref().is_some_and(|s| *s.borrow()) {
            return sample;
        }
        if claimed.fetch_add(1, Ordering::SeqCst) >= limit {
            return sample;
        }
        let started = Instant::now();
        let failure = match call(addr, key, &mut conn).await {
            Ok((200, timing, _)) => {
                sample
                    .latency_ms
                    .push(started.elapsed().as_secs_f64() * 1000.0);
                sample.overhead_ms.extend(timing.kaeso_ms);
                sample.upstream_ms.extend(timing.upstream_ms);
                None
            }
            Ok((status, _, body)) => Some(format!("status {status}{}", message(&body))),
            Err(e) => Some(e.to_string()),
        };
        if let Some(why) = failure {
            *sample.failures.entry(why).or_default() += 1;
            conn = None;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

fn message(body: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(serde_json::Value::as_str)
                .map(|m| format!(": {m}"))
        })
        .unwrap_or_default()
}

async fn call(
    addr: SocketAddr,
    key: &str,
    conn: &mut Option<Conn>,
) -> crate::Result<(u16, crate::client::Timing, Bytes)> {
    if conn.is_none() {
        *conn = Some(connect(addr, None).await?);
    }
    let c = conn.as_mut().ok_or("no connection")?;
    let resp = c
        .send(Call {
            method: Method::POST,
            path: PATH,
            bearer: Some(key),
            origin: None,
            cookie: None,
            body: Some(body()),
        })
        .await?;
    let timing = server_timing(resp.headers());
    let (status, _, body) = read_all(resp).await?;
    Ok((status, timing, body))
}
