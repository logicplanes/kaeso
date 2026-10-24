use std::fmt::Write;

use serde::Serialize;

use crate::machine::{Build, Machine};
use crate::scenarios::{CancelResult, Plan, Results, SlowResult};
use crate::stats::Summary;
use crate::streams::Pair;

#[derive(Serialize)]
pub(crate) struct Report<'a> {
    pub(crate) kaeso: &'a Build,
    pub(crate) machine: &'a Machine,
    pub(crate) parameters: &'a Plan,
    pub(crate) results: &'a Results,
}

impl Report<'_> {
    pub(crate) fn passed(&self) -> bool {
        self.results.checks.iter().all(|c| c.passed)
    }
}

fn ms(v: Option<f64>) -> String {
    match v {
        None => "n/a".to_owned(),
        Some(v) if v.abs() >= 100.0 => format!("{v:.0} ms"),
        Some(v) if v.abs() >= 10.0 => format!("{v:.1} ms"),
        Some(v) => format!("{v:.2} ms"),
    }
}

fn bytes(v: Option<f64>) -> String {
    match v {
        None => "n/a".to_owned(),
        Some(v) if v.abs() >= 1024.0 * 1024.0 * 1024.0 => {
            format!("{:.1} GiB", v / 1024.0 / 1024.0 / 1024.0)
        }
        Some(v) if v.abs() >= 1024.0 * 1024.0 => format!("{:.1} MiB", v / 1024.0 / 1024.0),
        Some(v) => format!("{:.1} KiB", v / 1024.0),
    }
}

fn int(v: Option<u64>) -> Option<f64> {
    v.map(|v| v as f64)
}

fn pair(p: &Pair) -> String {
    format!("{} / {}", ms(p.p50), ms(p.p99))
}

fn percentiles(s: &Summary) -> String {
    format!("{} / {}", ms(s.p50), ms(s.p99))
}

fn row(out: &mut String, cells: &[String]) {
    let _ = writeln!(out, "| {} |", cells.join(" | "));
}

fn header(out: &mut String, cells: &[&str]) {
    let _ = writeln!(out, "| {} |", cells.join(" | "));
    let _ = writeln!(out, "|{}", " --- |".repeat(cells.len()));
}

pub(crate) fn table(r: &Report<'_>) -> String {
    let mut out = String::new();
    let p = r.parameters;
    let m = r.machine;
    let _ = writeln!(out, "## Kaeso gateway benchmark\n");
    header(&mut out, &["", ""]);
    row(
        &mut out,
        &[
            "Kaeso".into(),
            format!(
                "{}, commit {}, {} build",
                r.kaeso.version, r.kaeso.commit, r.kaeso.profile
            ),
        ],
    );
    row(
        &mut out,
        &[
            "Machine".into(),
            format!(
                "{}, {} cores, {} memory, {} {}",
                m.cpu,
                m.cores,
                bytes(int(m.memory_bytes)),
                m.os,
                m.arch
            ),
        ],
    );
    row(
        &mut out,
        &[
            "Formats".into(),
            format!("{} app, {} provider", p.app_format, p.provider_format),
        ],
    );
    row(
        &mut out,
        &[
            "Streams".into(),
            format!(
                "{} chunks each, one every {} ms; stalled streams end after {} ms",
                p.chunks_per_stream, p.gap_ms, p.idle_timeout_ms
            ),
        ],
    );
    row(
        &mut out,
        &[
            "Clients".into(),
            format!(
                "open at most {} streams at once; slow clients have a {} receive buffer and {}",
                p.opening_at_once,
                bytes(Some(f64::from(p.slow_recv_buffer_bytes))),
                match p.slow_read_every_ms {
                    Some(every) => format!("read a chunk every {every} ms"),
                    None => "stop reading after their first chunk".to_owned(),
                }
            ),
        ],
    );
    if let Some(b) = &r.results.baseline {
        let _ = writeln!(out, "\n### Baseline: non-streamed requests\n");
        header(
            &mut out,
            &[
                "Requests",
                "Connections",
                "Requests/s",
                "Overhead p50 / p99 / p99.9",
                "Latency p50 / p99",
                "Failed",
            ],
        );
        row(
            &mut out,
            &[
                b.requests.to_string(),
                b.concurrency.to_string(),
                format!("{:.0}", b.requests_per_second),
                format!(
                    "{} / {} / {}",
                    ms(b.overhead_ms.p50),
                    ms(b.overhead_ms.p99),
                    ms(b.overhead_ms.p999)
                ),
                percentiles(&b.latency_ms),
                b.failed.to_string(),
            ],
        );
    }
    if !r.results.streams.is_empty() {
        let _ = writeln!(out, "\n### Concurrent streams\n");
        header(
            &mut out,
            &[
                "Streams",
                "Overhead p50 / p99",
                "Added time to first byte p50 / p99",
                "Added chunk gap p50 / p99",
                "Gateway memory before",
                "With most open",
                "Per open stream",
                "Left open",
            ],
        );
        for s in &r.results.streams {
            row(
                &mut out,
                &[
                    s.streams.to_string(),
                    percentiles(&s.overhead_ms),
                    percentiles(&s.added_ttfb_ms),
                    pair(&s.added_chunk_gap_ms),
                    bytes(int(s.memory.before_bytes)),
                    format!(
                        "{} ({} open)",
                        bytes(int(s.memory.with_most_open_bytes)),
                        s.memory.most_open
                    ),
                    bytes(s.memory.per_open_stream_bytes),
                    s.upstream_left_open.to_string(),
                ],
            );
        }
    }
    if let Some(c) = &r.results.cancellation {
        let _ = writeln!(out, "\n### Cancellation\n");
        cancel_table(&mut out, c);
    }
    if let Some(s) = &r.results.slow_clients {
        let _ = writeln!(out, "\n### Slow clients\n");
        slow_table(&mut out, s);
    }
    if let Some(mx) = &r.results.mixed {
        let b = &mx.baseline;
        let _ = writeln!(
            out,
            "\n### Mixed: {} streams with non-streamed requests beside them\n",
            mx.streams
        );
        let _ = writeln!(
            out,
            "Non-streamed: {} requests over {} connections, {:.0} requests/s, overhead p50 / p99 / p99.9 {} / {} / {}, {} failed.\n",
            b.requests,
            b.concurrency,
            b.requests_per_second,
            ms(b.overhead_ms.p50),
            ms(b.overhead_ms.p99),
            ms(b.overhead_ms.p999),
            b.failed
        );
        cancel_table(&mut out, &mx.cancellation);
        let _ = writeln!(out);
        slow_table(&mut out, &mx.slow_clients);
    }
    let _ = writeln!(out, "\n### Checks\n");
    header(&mut out, &["Scenario", "Check", "Result"]);
    for c in &r.results.checks {
        row(
            &mut out,
            &[
                c.scenario.clone(),
                c.name.clone(),
                if c.passed {
                    "passed".to_owned()
                } else {
                    format!("failed: {}", c.detail)
                },
            ],
        );
    }
    out
}

fn cancel_table(out: &mut String, c: &CancelResult) {
    header(
        out,
        &[
            "Streams",
            "Disconnected",
            "Reached the mock",
            "Disconnect to upstream closed p50 / p99 / max",
            "Left open",
        ],
    );
    let d = &c.disconnect_to_upstream_close_ms;
    row(
        out,
        &[
            c.streams.to_string(),
            c.cancelled.to_string(),
            c.reached_the_mock.to_string(),
            format!("{} / {} / {}", ms(d.p50), ms(d.p99), ms(d.max)),
            c.upstream_left_open.to_string(),
        ],
    );
}

fn slow_table(out: &mut String, s: &SlowResult) {
    header(
        out,
        &[
            "Streams",
            "Stalled",
            "Ended",
            "Logged as client_stalled",
            "Ended after the last read p50 / p99",
            "Gateway memory before / peak",
            "Growth per stream",
            "Healthy: added time to first byte p50 / p99",
            "Healthy: added chunk gap p50 / p99",
            "Left open",
        ],
    );
    row(
        out,
        &[
            s.streams.to_string(),
            s.stalled.to_string(),
            s.ended.to_string(),
            s.counted_as_client_stalled.to_string(),
            percentiles(&s.ended_after_last_read_ms),
            format!(
                "{} / {}",
                bytes(int(s.memory.before_bytes)),
                bytes(int(s.memory.peak_bytes))
            ),
            bytes(s.growth_per_stream_bytes),
            percentiles(&s.healthy_added_ttfb_ms),
            pair(&s.healthy_added_chunk_gap_ms),
            s.upstream_left_open.to_string(),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_sizes_read_at_a_glance() {
        assert_eq!(ms(Some(0.4123)), "0.41 ms");
        assert_eq!(ms(Some(12.34)), "12.3 ms");
        assert_eq!(ms(Some(1234.4)), "1234 ms");
        assert_eq!(ms(Some(-0.5)), "-0.50 ms");
        assert_eq!(ms(None), "n/a");
        assert_eq!(bytes(Some(2048.0)), "2.0 KiB");
        assert_eq!(bytes(Some(3.5 * 1024.0 * 1024.0)), "3.5 MiB");
        assert_eq!(bytes(Some(24.0 * 1024.0 * 1024.0 * 1024.0)), "24.0 GiB");
    }
}
