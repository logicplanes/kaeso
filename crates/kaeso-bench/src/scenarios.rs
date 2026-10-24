use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::ValueEnum;
use serde::Serialize;
use tokio::sync::{Semaphore, watch};

use crate::baseline::{self, Baseline, Until};
use crate::harness::{Install, counter};
use crate::stats::Summary;
use crate::streams::{
    self, Memory, Pair, Profile, Role, Run, Spec, Target, added_ttfb, cancel_point, healthy,
    picked, upstream_left_open,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Scenario {
    Baseline,
    Streams,
    Cancellation,
    SlowClients,
    Mixed,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Plan {
    pub(crate) scenarios: Vec<Scenario>,
    pub(crate) smoke: bool,
    pub(crate) app_format: &'static str,
    pub(crate) provider_format: &'static str,
    pub(crate) baseline_requests: usize,
    pub(crate) baseline_concurrency: usize,
    pub(crate) stream_sizes: Vec<usize>,
    pub(crate) gap_ms: u64,
    pub(crate) chunks_per_stream: u64,
    pub(crate) cancel_streams: usize,
    pub(crate) cancel_share: f64,
    pub(crate) slow_streams: usize,
    pub(crate) slow_share: f64,
    pub(crate) slow_read_every_ms: Option<u64>,
    pub(crate) slow_recv_buffer_bytes: u32,
    pub(crate) idle_timeout_ms: u64,
    pub(crate) mixed_streams: usize,
    pub(crate) mixed_cancel_share: f64,
    pub(crate) mixed_slow_share: f64,
    pub(crate) mixed_baseline_concurrency: usize,
    pub(crate) opening_at_once: usize,
    pub(crate) gateway_max_connections: usize,
}

impl Plan {
    fn gap(&self) -> Duration {
        Duration::from_millis(self.gap_ms)
    }

    fn idle_timeout(&self) -> Duration {
        Duration::from_millis(self.idle_timeout_ms)
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Check {
    pub(crate) scenario: String,
    pub(crate) name: String,
    pub(crate) passed: bool,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct StreamsResult {
    pub(crate) streams: usize,
    pub(crate) overhead_ms: Summary,
    pub(crate) upstream_ms: Summary,
    pub(crate) through_kaeso: Profile,
    pub(crate) mock_alone: Profile,
    pub(crate) added_ttfb_ms: Summary,
    pub(crate) added_chunk_gap_ms: Pair,
    pub(crate) memory: Memory,
    pub(crate) incomplete: usize,
    pub(crate) upstream_left_open: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CancelResult {
    pub(crate) streams: usize,
    pub(crate) cancelled: usize,
    pub(crate) reached_the_mock: usize,
    pub(crate) disconnect_to_upstream_close_ms: Summary,
    pub(crate) healthy_incomplete: usize,
    pub(crate) upstream_left_open: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SlowResult {
    pub(crate) streams: usize,
    pub(crate) stalled: usize,
    pub(crate) ended: usize,
    pub(crate) counted_as_client_stalled: u64,
    pub(crate) idle_timeout_ms: u64,
    pub(crate) ended_after_last_read_ms: Summary,
    pub(crate) memory: Memory,
    pub(crate) growth_bytes: Option<i64>,
    pub(crate) growth_per_stream_bytes: Option<f64>,
    pub(crate) budget_bytes: u64,
    pub(crate) healthy_overhead_ms: Summary,
    pub(crate) healthy: Profile,
    pub(crate) healthy_added_ttfb_ms: Summary,
    pub(crate) healthy_added_chunk_gap_ms: Pair,
    pub(crate) healthy_incomplete: usize,
    pub(crate) upstream_left_open: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MixedResult {
    pub(crate) streams: usize,
    pub(crate) baseline: Baseline,
    pub(crate) cancellation: CancelResult,
    pub(crate) slow_clients: SlowResult,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Results {
    pub(crate) baseline: Option<Baseline>,
    pub(crate) streams: Vec<StreamsResult>,
    pub(crate) cancellation: Option<CancelResult>,
    pub(crate) slow_clients: Option<SlowResult>,
    pub(crate) mixed: Option<MixedResult>,
    pub(crate) checks: Vec<Check>,
}

struct Bench<'a> {
    plan: &'a Plan,
    install: &'a mut Install,
    opening: Arc<Semaphore>,
    alone: HashMap<usize, Profile>,
    results: Results,
}

const LEAK_WAIT: Duration = Duration::from_secs(30);
const CANCEL_WAIT: Duration = Duration::from_secs(10);
const STALLED: &str = "kind=\"client_stalled\",provider=\"openai\"";
const SLOW_TEXT_CHUNKS: u64 = 16;
// A hundred deltas a millisecond fill the sockets and Kaeso's queue in well under a second,
// so a stalled stream ends close to the idle timeout after the app's last read.
const SLOW_GAP: Duration = Duration::from_micros(10);
// A stream costs the gateway well under this. One that kept what a stalled app doesn't read
// would hold the mock's chunk a millisecond until Kaeso gave up on the app, many times it.
const MEMORY_PER_STREAM: u64 = 1024 * 1024;

fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "kaeso-bench: {text}");
}

pub(crate) async fn run(plan: &Plan, install: &mut Install) -> Results {
    let mut bench = Bench {
        plan,
        install,
        opening: Arc::new(Semaphore::new(plan.opening_at_once.max(1))),
        alone: HashMap::new(),
        results: Results::default(),
    };
    for scenario in &plan.scenarios {
        match scenario {
            Scenario::Baseline => {
                bench.fresh("baseline").await;
                bench.baseline().await;
            }
            Scenario::Streams => {
                for &n in &plan.stream_sizes {
                    bench.fresh(&format!("streams-{n}")).await;
                    bench.streams(n).await;
                }
            }
            Scenario::Cancellation => {
                bench.fresh("cancellation").await;
                let r = bench
                    .cancellation(plan.cancel_streams, plan.cancel_share, "cancellation")
                    .await;
                bench.results.cancellation = Some(r);
            }
            Scenario::SlowClients => {
                bench.fresh("slow-clients").await;
                let r = bench
                    .slow_clients(plan.slow_streams, plan.slow_share, "slow-clients")
                    .await;
                bench.results.slow_clients = Some(r);
            }
            Scenario::Mixed => {
                bench.fresh("mixed").await;
                bench.mixed().await;
            }
        }
    }
    bench.results
}

impl Bench<'_> {
    fn target(&self, direct: bool) -> Target {
        let (addr, bearer) = if direct {
            (self.install.mock_addr, "sk-bench-direct".to_owned())
        } else {
            (self.install.gateway.addr, self.install.key.clone())
        };
        Target {
            addr,
            bearer,
            mock: self.install.mock.clone(),
            opening: self.opening.clone(),
            slow_read_every: self.plan.slow_read_every_ms.map(Duration::from_millis),
            slow_recv_buffer: self.plan.slow_recv_buffer_bytes,
            slow_give_up: self.plan.idle_timeout() * 4 + Duration::from_secs(60),
        }
    }

    fn check(&mut self, scenario: &str, name: &str, passed: bool, detail: String) {
        if !passed {
            say(&format!("{scenario}: {name} failed: {detail}"));
        }
        self.results.checks.push(Check {
            scenario: scenario.to_owned(),
            name: name.to_owned(),
            passed,
            detail,
        });
    }

    async fn fresh(&mut self, scenario: &str) {
        if let Err(e) = self.install.restart().await {
            self.check(
                scenario,
                "the gateway restarted cleanly",
                false,
                e.to_string(),
            );
        }
        self.warm_up().await;
    }

    async fn warm_up(&mut self) {
        say("warming up a fresh gateway");
        let gw = self.install.gateway.addr;
        let _ = baseline::run(gw, &self.install.key, 4, Until::Count(50)).await;
        let specs = healthy(8, 8, Duration::from_millis(5), "warm");
        let _ = streams::run_all(&self.target(false), specs, None).await;
        let _ = upstream_left_open(&self.install.mock, LEAK_WAIT).await;
    }

    async fn baseline(&mut self) {
        let p = self.plan;
        say(&format!(
            "baseline: {} requests over {} connections",
            p.baseline_requests, p.baseline_concurrency
        ));
        let b = baseline::run(
            self.install.gateway.addr,
            &self.install.key,
            p.baseline_concurrency,
            Until::Count(p.baseline_requests),
        )
        .await;
        self.check(
            "baseline",
            "every request succeeded",
            b.failed == 0,
            format!("{} of {} failed", b.failed, b.requests),
        );
        self.results.baseline = Some(b);
    }

    async fn mock_alone(&mut self, n: usize) -> Profile {
        if let Some(p) = self.alone.get(&n) {
            return p.clone();
        }
        say(&format!("{n} streams from the mock provider alone"));
        let specs = healthy(n, self.plan.chunks_per_stream, self.plan.gap(), "alone");
        let (runs, _) = streams::run_all(&self.target(true), specs, None).await;
        let profile = Profile::of(runs.iter());
        let _ = upstream_left_open(&self.install.mock, LEAK_WAIT).await;
        self.alone.insert(n, profile.clone());
        profile
    }

    async fn streams(&mut self, n: usize) {
        let alone = self.mock_alone(n).await;
        say(&format!("{n} concurrent streams through Kaeso"));
        let specs = healthy(
            n,
            self.plan.chunks_per_stream,
            self.plan.gap(),
            &format!("s{n}"),
        );
        let (runs, memory) =
            streams::run_all(&self.target(false), specs, Some(self.install.gateway.pid)).await;
        let left = upstream_left_open(&self.install.mock, LEAK_WAIT).await;
        let through = Profile::of(runs.iter());
        let incomplete = incomplete(runs.iter());
        let name = format!("streams-{n}");
        self.check(
            &name,
            "every stream completed",
            incomplete == 0,
            format!(
                "{incomplete} of {n} did not end with [DONE]{}",
                first_error(&runs)
            ),
        );
        self.check(
            &name,
            "no upstream stream was left open",
            left == 0,
            format!("{left} still open"),
        );
        self.results.streams.push(StreamsResult {
            streams: n,
            overhead_ms: Summary::of(&overheads(runs.iter())),
            upstream_ms: Summary::of(&upstreams(runs.iter())),
            added_ttfb_ms: added_ttfb(runs.iter()),
            added_chunk_gap_ms: Pair::added(&through.chunk_gap_ms, &alone.chunk_gap_ms),
            through_kaeso: through,
            mock_alone: alone,
            memory,
            incomplete,
            upstream_left_open: left,
        });
    }

    fn cancel_specs(&self, n: usize, share: f64, prefix: &str) -> Vec<Spec> {
        let chunks = self.plan.chunks_per_stream;
        picked(n, share)
            .into_iter()
            .enumerate()
            .map(|(i, cancel)| Spec {
                tag: format!("{prefix}-{i}"),
                chunks,
                gap: self.plan.gap(),
                role: if cancel {
                    Role::Cancel {
                        after: cancel_point(i, chunks),
                    }
                } else {
                    Role::Healthy
                },
            })
            .collect()
    }

    async fn cancel_result(&mut self, scenario: &str, n: usize, runs: &[Run]) -> CancelResult {
        let cancelled: Vec<&Run> = runs.iter().filter(|r| r.cancelled_at.is_some()).collect();
        let until = Instant::now() + CANCEL_WAIT;
        while Instant::now() < until
            && cancelled
                .iter()
                .any(|r| self.install.mock.closed_at(&r.tag).is_none())
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let latencies: Vec<f64> = cancelled
            .iter()
            .filter_map(|r| {
                let closed = self.install.mock.closed_at(&r.tag)?;
                let left = r.cancelled_at?;
                Some(closed.saturating_duration_since(left).as_secs_f64() * 1000.0)
            })
            .collect();
        let expected = runs
            .iter()
            .filter(|r| matches!(r.role, Role::Cancel { .. }))
            .count();
        let left = upstream_left_open(&self.install.mock, LEAK_WAIT).await;
        let healthy_incomplete = incomplete(runs.iter().filter(|r| r.role == Role::Healthy));
        self.check(
            scenario,
            "every disconnect reached the mock provider",
            cancelled.len() == expected && latencies.len() == expected,
            format!(
                "{} of {expected} disconnected, {} upstream streams closed{}",
                cancelled.len(),
                latencies.len(),
                first_error(runs)
            ),
        );
        self.check(
            scenario,
            "every other stream completed",
            healthy_incomplete == 0,
            format!(
                "{healthy_incomplete} did not end with [DONE]{}",
                first_error(runs)
            ),
        );
        self.check(
            scenario,
            "no upstream stream was left open",
            left == 0,
            format!("{left} still open"),
        );
        CancelResult {
            streams: n,
            cancelled: cancelled.len(),
            reached_the_mock: latencies.len(),
            disconnect_to_upstream_close_ms: Summary::of(&latencies),
            healthy_incomplete,
            upstream_left_open: left,
        }
    }

    async fn cancellation(&mut self, n: usize, share: f64, scenario: &str) -> CancelResult {
        say(&format!("{n} streams, {:.0}% disconnecting", share * 100.0));
        let specs = self.cancel_specs(n, share, scenario);
        let (runs, _) =
            streams::run_all(&self.target(false), specs, Some(self.install.gateway.pid)).await;
        self.cancel_result(scenario, n, &runs).await
    }

    fn slow_spec(&self, tag: String) -> Spec {
        Spec {
            tag,
            chunks: SLOW_TEXT_CHUNKS,
            gap: SLOW_GAP,
            role: Role::Slow,
        }
    }

    fn slow_specs(&self, n: usize, share: f64, prefix: &str) -> Vec<Spec> {
        let chunks = self.plan.chunks_per_stream;
        picked(n, share)
            .into_iter()
            .enumerate()
            .map(|(i, slow)| {
                let tag = format!("{prefix}-{i}");
                if slow {
                    self.slow_spec(tag)
                } else {
                    Spec {
                        tag,
                        chunks,
                        gap: self.plan.gap(),
                        role: Role::Healthy,
                    }
                }
            })
            .collect()
    }

    async fn stalled_count(&self) -> u64 {
        match self.install.gateway.metrics().await {
            Ok(text) => counter(&text, "kaeso_stream_failures_total", STALLED),
            Err(_) => 0,
        }
    }

    async fn slow_result(
        &mut self,
        scenario: &str,
        n: usize,
        runs: &[Run],
        memory: Memory,
        counted_before: u64,
        only_streams: bool,
    ) -> SlowResult {
        let slow: Vec<&Run> = runs.iter().filter(|r| r.role == Role::Slow).collect();
        let ended: Vec<f64> = slow
            .iter()
            .filter(|r| r.ended)
            .filter_map(|r| {
                let closed = self.install.mock.closed_at(&r.tag)?;
                Some(closed.saturating_duration_since(r.last_read?).as_secs_f64() * 1000.0)
            })
            .collect();
        let left = upstream_left_open(&self.install.mock, LEAK_WAIT).await;
        let mut counted = 0;
        let until = Instant::now() + LEAK_WAIT;
        while Instant::now() < until {
            counted = self.stalled_count().await.saturating_sub(counted_before);
            if counted >= slow.len() as u64 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let growth = match (memory.peak_bytes, memory.before_bytes) {
            (Some(p), Some(b)) => Some(p as i64 - b as i64),
            _ => None,
        };
        let budget = n as u64 * MEMORY_PER_STREAM;
        let healthy_runs = runs.iter().filter(|r| r.role == Role::Healthy);
        let healthy = Profile::of(healthy_runs.clone());
        let healthy_n = healthy_runs.clone().count();
        let alone = self.mock_alone(healthy_n.max(1)).await;
        let healthy_incomplete = incomplete(healthy_runs.clone());
        self.check(
            scenario,
            "every stalled stream ended",
            ended.len() == slow.len(),
            format!(
                "{} of {} ended{}",
                ended.len(),
                slow.len(),
                first_error(runs)
            ),
        );
        self.check(
            scenario,
            "Kaeso logged every stalled stream as client_stalled",
            counted == slow.len() as u64,
            format!("{counted} counted for {} stalled streams", slow.len()),
        );
        // Beside non-streamed requests, the gateway's memory also holds their log rows,
        // so the bound is only checked when streams are all it serves.
        if only_streams {
            self.check(
                scenario,
                "memory stayed bounded",
                growth.is_some_and(|g| g < budget as i64),
                format!("grew by {growth:?} bytes, budget {budget}"),
            );
        }
        // Beside disconnects, `cancel_result` has checked these for the same streams.
        if only_streams {
            self.check(
                scenario,
                "every other stream completed",
                healthy_incomplete == 0,
                format!(
                    "{healthy_incomplete} did not end with [DONE]{}",
                    first_error(runs)
                ),
            );
            self.check(
                scenario,
                "no upstream stream was left open",
                left == 0,
                format!("{left} still open"),
            );
        }
        SlowResult {
            streams: n,
            stalled: slow.len(),
            ended: ended.len(),
            counted_as_client_stalled: counted,
            idle_timeout_ms: self.plan.idle_timeout_ms,
            ended_after_last_read_ms: Summary::of(&ended),
            memory,
            growth_bytes: growth,
            growth_per_stream_bytes: growth.map(|g| g as f64 / n.max(1) as f64),
            budget_bytes: budget,
            healthy_overhead_ms: Summary::of(&overheads(
                runs.iter().filter(|r| r.role == Role::Healthy),
            )),
            healthy_added_ttfb_ms: added_ttfb(runs.iter().filter(|r| r.role == Role::Healthy)),
            healthy_added_chunk_gap_ms: Pair::added(&healthy.chunk_gap_ms, &alone.chunk_gap_ms),
            healthy,
            healthy_incomplete,
            upstream_left_open: left,
        }
    }

    async fn slow_clients(&mut self, n: usize, share: f64, scenario: &str) -> SlowResult {
        let before = self.stalled_count().await;
        say(&format!(
            "{n} streams, {:.0}% reading slowly",
            share * 100.0
        ));
        let specs = self.slow_specs(n, share, scenario);
        let (runs, memory) =
            streams::run_all(&self.target(false), specs, Some(self.install.gateway.pid)).await;
        self.slow_result(scenario, n, &runs, memory, before, true)
            .await
    }

    async fn mixed(&mut self) {
        let p = self.plan;
        let n = p.mixed_streams;
        say(&format!(
            "mixed: {n} streams ({:.0}% disconnecting, {:.0}% reading slowly) and non-streamed requests over {} connections",
            p.mixed_cancel_share * 100.0,
            p.mixed_slow_share * 100.0,
            p.mixed_baseline_concurrency
        ));
        let stalled_before = self.stalled_count().await;
        let cancel = picked(n, p.mixed_cancel_share);
        let slow_share = p.mixed_slow_share / (1.0 - p.mixed_cancel_share).max(f64::EPSILON);
        let others = cancel.iter().filter(|c| !**c).count();
        let mut slow = picked(others, slow_share).into_iter();
        let specs: Vec<Spec> = self
            .cancel_specs(n, p.mixed_cancel_share, "mixed")
            .into_iter()
            .map(|spec| {
                if spec.role == Role::Healthy && slow.next() == Some(true) {
                    self.slow_spec(spec.tag)
                } else {
                    spec
                }
            })
            .collect();
        let (stop, stopped) = watch::channel(false);
        let load = baseline::run(
            self.install.gateway.addr,
            &self.install.key,
            p.mixed_baseline_concurrency,
            Until::Stopped(stopped),
        );
        let target = self.target(false);
        let pid = self.install.gateway.pid;
        let streaming = async move {
            let out = streams::run_all(&target, specs, Some(pid)).await;
            let _ = stop.send(true);
            out
        };
        let (b, (runs, memory)) = tokio::join!(load, streaming);
        self.check(
            "mixed",
            "every non-streamed request succeeded",
            b.failed == 0,
            format!("{} of {} failed", b.failed, b.requests),
        );
        let cancellation = self.cancel_result("mixed", n, &runs).await;
        let slow_clients = self
            .slow_result("mixed", n, &runs, memory, stalled_before, false)
            .await;
        self.results.mixed = Some(MixedResult {
            streams: n,
            baseline: b,
            cancellation,
            slow_clients,
        });
    }
}

fn incomplete<'a>(runs: impl Iterator<Item = &'a Run>) -> usize {
    runs.filter(|r| r.status != 200 || !r.done).count()
}

fn overheads<'a>(runs: impl Iterator<Item = &'a Run>) -> Vec<f64> {
    runs.filter_map(|r| r.timing.kaeso_ms).collect()
}

fn upstreams<'a>(runs: impl Iterator<Item = &'a Run>) -> Vec<f64> {
    runs.filter_map(|r| r.timing.upstream_ms).collect()
}

fn first_error(runs: &[Run]) -> String {
    let mut why: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for r in runs {
        let reason = match (&r.error, r.status) {
            (Some(e), _) => e.clone(),
            (None, 200) if r.error_event.is_some() => {
                format!(
                    "error event: {}",
                    r.error_event.as_deref().unwrap_or_default()
                )
            }
            (None, 200) if !r.done && r.role == Role::Healthy => "no [DONE]".to_owned(),
            (None, 200 | 0) => continue,
            (None, status) => format!("status {status}"),
        };
        *why.entry(reason).or_default() += 1;
    }
    if why.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = why.iter().map(|(w, n)| format!("{n} × {w}")).collect();
    format!("; {}", listed.join(", "))
}
