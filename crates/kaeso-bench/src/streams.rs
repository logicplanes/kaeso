use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::Method;
use kaeso_testkit::mock_provider::MockProvider;
use serde::Serialize;
use serde_json::json;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

use crate::client::{Call, Timing, connect, server_timing};
use crate::harness::MODEL;
use crate::stats::{Summary, difference};

pub(crate) const PATH: &str = "/v1/chat/completions";
// The mock sends a delta per 4 tokens (kaeso-testkit's synth), so `tps` sets the gap.
const TOKENS_PER_CHUNK: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Healthy,
    Cancel { after: usize },
    Slow,
}

#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub(crate) tag: String,
    pub(crate) chunks: u64,
    pub(crate) gap: Duration,
    pub(crate) role: Role,
}

impl Spec {
    fn body(&self) -> Bytes {
        let tps = TOKENS_PER_CHUNK as f64 / self.gap.as_secs_f64();
        let endless = if self.role == Role::Slow {
            " [mock:endless]"
        } else {
            ""
        };
        let text = format!(
            "bench [mock:gen={}] [mock:tps={tps}] [mock:tag={}]{endless}",
            self.chunks * TOKENS_PER_CHUNK,
            self.tag
        );
        Bytes::from(
            json!({
                "model": MODEL,
                "stream": true,
                "messages": [{ "role": "user", "content": text }],
            })
            .to_string(),
        )
    }
}

#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) addr: SocketAddr,
    pub(crate) bearer: String,
    pub(crate) mock: MockProvider,
    pub(crate) opening: Arc<Semaphore>,
    pub(crate) slow_read_every: Option<Duration>,
    pub(crate) slow_recv_buffer: u32,
    pub(crate) slow_give_up: Duration,
}

#[derive(Clone, Debug)]
pub(crate) struct Run {
    pub(crate) role: Role,
    pub(crate) tag: String,
    pub(crate) status: u16,
    pub(crate) timing: Timing,
    pub(crate) ttfb_ms: Option<f64>,
    pub(crate) gaps_ms: Vec<f64>,
    pub(crate) bytes: u64,
    pub(crate) done: bool,
    pub(crate) error_event: Option<String>,
    pub(crate) cancelled_at: Option<Instant>,
    pub(crate) last_read: Option<Instant>,
    pub(crate) ended: bool,
    pub(crate) error: Option<String>,
}

impl Run {
    fn new(spec: &Spec) -> Run {
        Run {
            role: spec.role,
            tag: spec.tag.clone(),
            status: 0,
            timing: Timing::default(),
            ttfb_ms: None,
            gaps_ms: Vec::new(),
            bytes: 0,
            done: false,
            error_event: None,
            cancelled_at: None,
            last_read: None,
            ended: false,
            error: None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Load {
    opened: AtomicUsize,
    finished: AtomicUsize,
}

impl Load {
    fn open(&self) -> usize {
        self.opened
            .load(Ordering::SeqCst)
            .saturating_sub(self.finished.load(Ordering::SeqCst))
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

const MARKER_TAIL: usize = 16;

fn error_message(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data);
    let at = text.find("{\"error\":")?;
    let event = text.get(at..)?.lines().next()?;
    let value: serde_json::Value = serde_json::from_str(event).ok()?;
    Some(
        value
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("an error event")
            .to_owned(),
    )
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

async fn closed_within(mock: &MockProvider, tag: &str, wait: Duration) -> bool {
    let until = Instant::now() + wait;
    loop {
        if mock.closed_at(tag).is_some() {
            return true;
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        tokio::time::sleep(left.min(Duration::from_millis(20))).await;
    }
}

async fn run_one(target: Target, spec: Spec, load: Arc<Load>) -> Run {
    let mut run = Run::new(&spec);
    if let Err(e) = stream(&target, &spec, &load, &mut run).await {
        run.error = Some(e.to_string());
    }
    load.finished.fetch_add(1, Ordering::SeqCst);
    run
}

async fn stream(target: &Target, spec: &Spec, load: &Load, run: &mut Run) -> crate::Result<()> {
    let slow = spec.role == Role::Slow;
    let permit = target.opening.acquire().await?;
    let mut conn = connect(target.addr, slow.then_some(target.slow_recv_buffer)).await?;
    let started = Instant::now();
    let resp = conn
        .send(Call {
            method: Method::POST,
            path: PATH,
            bearer: Some(&target.bearer),
            origin: None,
            cookie: None,
            body: Some(spec.body()),
        })
        .await?;
    drop(permit);
    run.status = resp.status().as_u16();
    run.timing = server_timing(resp.headers());
    let mut body = resp.into_body();
    let give_up = Instant::now() + target.slow_give_up;
    let pause = target.slow_read_every.unwrap_or(target.slow_give_up);
    let mut frames = 0usize;
    let mut seen: Vec<u8> = Vec::new();
    loop {
        if slow && frames > 0 {
            let wait = pause.min(give_up.saturating_duration_since(Instant::now()));
            if closed_within(&target.mock, &spec.tag, wait).await {
                run.ended = true;
                return Ok(());
            }
            if Instant::now() >= give_up {
                return Err("the gateway never ended this stalled stream".into());
            }
        }
        let frame = if slow {
            match tokio::time::timeout(pause, body.frame()).await {
                Ok(frame) => frame,
                Err(_) => continue,
            }
        } else {
            body.frame().await
        };
        let Some(frame) = frame else { return Ok(()) };
        let Ok(data) = frame?.into_data() else {
            continue;
        };
        let now = Instant::now();
        match run.last_read {
            None => {
                run.ttfb_ms = Some(ms(now - started));
                load.opened.fetch_add(1, Ordering::SeqCst);
            }
            Some(last) => run.gaps_ms.push(ms(now - last)),
        }
        run.last_read = Some(now);
        run.bytes += data.len() as u64;
        frames += 1;
        // A frame ends wherever a read did, so a marker may straddle two frames.
        seen.extend_from_slice(&data);
        run.done |= contains(&seen, b"data: [DONE]");
        if run.error_event.is_none() {
            run.error_event = error_message(&data);
        }
        let keep = seen.len().saturating_sub(MARKER_TAIL);
        seen.drain(..keep);
        if let Role::Cancel { after } = spec.role
            && frames >= after
        {
            drop(body);
            drop(conn);
            run.cancelled_at = Some(Instant::now());
            return Ok(());
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub(crate) struct Memory {
    pub(crate) before_bytes: Option<u64>,
    pub(crate) peak_bytes: Option<u64>,
    pub(crate) most_open: usize,
    pub(crate) with_most_open_bytes: Option<u64>,
    pub(crate) per_open_stream_bytes: Option<f64>,
}

pub(crate) async fn run_all(
    target: &Target,
    specs: Vec<Spec>,
    gateway_pid: Option<u32>,
) -> (Vec<Run>, Memory) {
    let load = Arc::new(Load::default());
    let before = gateway_pid.and_then(crate::machine::rss_bytes);
    let (stop, stopped) = watch::channel(false);
    let sampler = tokio::spawn(sample(gateway_pid, load.clone(), stopped));
    let mut tasks = JoinSet::new();
    for spec in specs {
        tasks.spawn(run_one(target.clone(), spec, load.clone()));
    }
    let mut runs = Vec::with_capacity(tasks.len());
    while let Some(done) = tasks.join_next().await {
        if let Ok(run) = done {
            runs.push(run);
        }
    }
    let _ = stop.send(true);
    let (peak, most_open, with_most_open) = sampler.await.unwrap_or_default();
    let per_open_stream = match (before, with_most_open) {
        (Some(b), Some(m)) if most_open > 0 => Some((m as f64 - b as f64) / most_open as f64),
        _ => None,
    };
    (
        runs,
        Memory {
            before_bytes: before,
            peak_bytes: peak,
            most_open,
            with_most_open_bytes: with_most_open,
            per_open_stream_bytes: per_open_stream,
        },
    )
}

async fn sample(
    pid: Option<u32>,
    load: Arc<Load>,
    mut stopped: watch::Receiver<bool>,
) -> (Option<u64>, usize, Option<u64>) {
    let Some(pid) = pid else {
        return (None, 0, None);
    };
    let mut peak: Option<u64> = None;
    let mut most_open = 0usize;
    let mut with_most_open: Option<u64> = None;
    loop {
        let open = load.open();
        if let Some(rss) = crate::machine::rss_bytes(pid) {
            peak = Some(peak.map_or(rss, |p| p.max(rss)));
            if open > most_open {
                most_open = open;
                with_most_open = Some(rss);
            } else if open == most_open && open > 0 {
                with_most_open = Some(with_most_open.map_or(rss, |m| m.max(rss)));
            }
        }
        tokio::select! {
            _ = stopped.changed() => return (peak, most_open, with_most_open),
            () = tokio::time::sleep(Duration::from_millis(200)) => {}
        }
    }
}

pub(crate) fn healthy(n: usize, chunks: u64, gap: Duration, prefix: &str) -> Vec<Spec> {
    (0..n)
        .map(|i| Spec {
            tag: format!("{prefix}-{i}"),
            chunks,
            gap,
            role: Role::Healthy,
        })
        .collect()
}

// Spreads `share` of `n` evenly over the indices, so a run is the same every time.
pub(crate) fn picked(n: usize, share: f64) -> Vec<bool> {
    let count = (n as f64 * share).round() as usize;
    (0..n)
        .map(|i| (i + 1) * count / n.max(1) > i * count / n.max(1))
        .collect()
}

pub(crate) fn cancel_point(i: usize, chunks: u64) -> usize {
    let spread = (i * 7919 % 1000) as f64 / 1000.0;
    ((chunks as f64 * (0.2 + 0.6 * spread)) as usize).max(1)
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub(crate) struct Pair {
    pub(crate) p50: Option<f64>,
    pub(crate) p99: Option<f64>,
}

impl Pair {
    pub(crate) fn added(through: &Summary, direct: &Summary) -> Pair {
        Pair {
            p50: difference(through.p50, direct.p50),
            p99: difference(through.p99, direct.p99),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Profile {
    pub(crate) ttfb_ms: Summary,
    pub(crate) chunk_gap_ms: Summary,
}

impl Profile {
    pub(crate) fn of<'a>(runs: impl Iterator<Item = &'a Run> + Clone) -> Profile {
        let ttfb: Vec<f64> = runs.clone().filter_map(|r| r.ttfb_ms).collect();
        let gaps: Vec<f64> = runs.flat_map(|r| r.gaps_ms.iter().copied()).collect();
        Profile {
            ttfb_ms: Summary::of(&ttfb),
            chunk_gap_ms: Summary::of(&gaps),
        }
    }
}

// What the app waited for its first byte beyond the provider's own time to answer, which
// Kaeso reports in Server-Timing: Kaeso's work before the call and after the answer.
pub(crate) fn added_ttfb<'a>(runs: impl Iterator<Item = &'a Run>) -> Summary {
    let added: Vec<f64> = runs
        .filter_map(|r| Some(r.ttfb_ms? - r.timing.upstream_ms?))
        .collect();
    Summary::of(&added)
}

pub(crate) async fn upstream_left_open(mock: &MockProvider, wait: Duration) -> usize {
    let until = Instant::now() + wait;
    loop {
        let (started, finished, dropped) = mock.stream_counts();
        let open = started.saturating_sub(finished + dropped);
        if open == 0 || Instant::now() > until {
            return open;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_are_spread_evenly_and_rounded() {
        let p = picked(10, 0.3);
        assert_eq!(p.iter().filter(|x| **x).count(), 3);
        assert_eq!(
            p,
            [
                false, false, false, true, false, false, true, false, false, true
            ]
        );
        assert_eq!(picked(20, 0.25).iter().filter(|x| **x).count(), 5);
        assert!(picked(5, 0.0).iter().all(|x| !x));
        assert!(picked(0, 0.5).is_empty());
    }

    #[test]
    fn cancellations_land_between_a_fifth_and_four_fifths_of_a_stream() {
        for i in 0..1000 {
            let at = cancel_point(i, 400);
            assert!((80..=320).contains(&at), "{i}: {at}");
        }
        assert_eq!(cancel_point(0, 1), 1);
    }

    #[test]
    fn a_stream_asks_the_mock_for_its_length_gap_and_tag() {
        let spec = Spec {
            tag: "s-1".into(),
            chunks: 100,
            gap: Duration::from_millis(25),
            role: Role::Healthy,
        };
        let body: serde_json::Value = serde_json::from_slice(&spec.body()).unwrap_or_default();
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"][0]["content"],
            "bench [mock:gen=400] [mock:tps=160] [mock:tag=s-1]"
        );
    }
}
