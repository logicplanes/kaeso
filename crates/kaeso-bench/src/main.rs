mod baseline;
mod client;
mod gateway;
mod harness;
mod machine;
mod report;
mod scenarios;
mod stats;
mod streams;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use crate::harness::{Install, Settings};
use crate::scenarios::{Plan, Scenario};

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;
pub(crate) type Result<T> = std::result::Result<T, Error>;

pub(crate) fn fail(message: &str) -> Error {
    message.into()
}

#[derive(Parser)]
#[command(
    name = "kaeso-bench",
    about = "Benchmark the whole Kaeso gateway against the mock provider",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    options: Options,
}

#[derive(Subcommand)]
enum Command {
    #[command(hide = true, about = "Run the gateway the benchmark measures")]
    Gateway,
}

#[derive(Args, Debug)]
struct Options {
    #[arg(long, value_delimiter = ',', help = "Scenarios to run, all by default")]
    scenarios: Vec<Scenario>,
    #[arg(
        long,
        help = "Small sizes that check correctness only, as CI runs them"
    )]
    smoke: bool,
    #[arg(
        long,
        default_value = "target/bench",
        help = "Where to write report.md, report.json and gateway.log"
    )]
    out: PathBuf,
    #[arg(long, help = "Non-streamed requests in the baseline [20000]")]
    baseline_requests: Option<usize>,
    #[arg(long, help = "Connections the baseline sends them over [64]")]
    baseline_concurrency: Option<usize>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Concurrent stream counts, comma-separated [100,1000,5000]"
    )]
    streams: Vec<usize>,
    #[arg(
        long,
        help = "Milliseconds between two chunks of the mock provider [100]"
    )]
    gap_ms: Option<u64>,
    #[arg(long, help = "Chunks in each stream [100]")]
    chunks: Option<u64>,
    #[arg(long, help = "Streams in the cancellation scenario [1000]")]
    cancel_streams: Option<usize>,
    #[arg(long, help = "Share of them that disconnect mid-stream [0.3]")]
    cancel_share: Option<f64>,
    #[arg(long, help = "Streams in the slow-client scenario [1000]")]
    slow_streams: Option<usize>,
    #[arg(long, help = "Share of them that read slowly [0.1]")]
    slow_share: Option<f64>,
    #[arg(
        long,
        help = "Milliseconds a slow client waits between two reads [it stops after the first]"
    )]
    slow_read_every_ms: Option<u64>,
    #[arg(
        long,
        help = "The gateway's KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT, in milliseconds [5000]"
    )]
    idle_timeout_ms: Option<u64>,
    #[arg(long, help = "Streams in the mixed scenario [1000]")]
    mixed_streams: Option<usize>,
    #[arg(long, help = "Streams the clients open at once [100]")]
    opening: Option<usize>,
}

fn pick<T>(smoke: bool, full: T, small: T) -> T {
    if smoke { small } else { full }
}

impl Options {
    fn plan(self) -> Plan {
        let smoke = self.smoke;
        let idle_timeout_ms = self.idle_timeout_ms.unwrap_or(pick(smoke, 5000, 2000));
        let stream_sizes = if self.streams.is_empty() {
            pick(smoke, vec![100, 1000, 5000], vec![20])
        } else {
            self.streams
        };
        let mixed_streams = self.mixed_streams.unwrap_or(pick(smoke, 1000, 20));
        let cancel_streams = self.cancel_streams.unwrap_or(pick(smoke, 1000, 20));
        let slow_streams = self.slow_streams.unwrap_or(pick(smoke, 1000, 20));
        let biggest = stream_sizes
            .iter()
            .copied()
            .chain([mixed_streams, cancel_streams, slow_streams])
            .max()
            .unwrap_or(0);
        Plan {
            scenarios: if self.scenarios.is_empty() {
                vec![
                    Scenario::Baseline,
                    Scenario::Streams,
                    Scenario::Cancellation,
                    Scenario::SlowClients,
                    Scenario::Mixed,
                ]
            } else {
                self.scenarios
            },
            smoke,
            app_format: "OpenAI Chat Completions",
            provider_format: "OpenAI Responses",
            baseline_requests: self.baseline_requests.unwrap_or(pick(smoke, 20_000, 200)),
            baseline_concurrency: self.baseline_concurrency.unwrap_or(pick(smoke, 64, 8)),
            stream_sizes,
            gap_ms: self.gap_ms.unwrap_or(pick(smoke, 100, 20)),
            chunks_per_stream: self.chunks.unwrap_or(pick(smoke, 100, 30)),
            cancel_streams,
            cancel_share: self.cancel_share.unwrap_or(0.3),
            slow_streams,
            slow_share: self.slow_share.unwrap_or(pick(smoke, 0.1, 0.25)),
            slow_read_every_ms: self.slow_read_every_ms,
            slow_recv_buffer_bytes: 8 * 1024,
            idle_timeout_ms,
            mixed_streams,
            mixed_cancel_share: 0.3,
            mixed_slow_share: pick(smoke, 0.1, 0.25),
            mixed_baseline_concurrency: pick(smoke, 16, 4),
            opening_at_once: self.opening.unwrap_or(100),
            gateway_max_connections: biggest + 1000,
        }
    }
}

fn say(text: &str) {
    let _ = writeln!(std::io::stderr(), "kaeso-bench: {text}");
}

async fn bench(plan: Plan, out: PathBuf) -> Result<bool> {
    std::fs::create_dir_all(&out)?;
    let machine = machine::machine();
    let build = machine::build();
    if build.profile == "debug" && !plan.smoke {
        say("this is a debug build; run with --release for numbers worth comparing");
    }
    say("starting the databases, the mock provider and the gateway");
    let mut install = Install::start(
        &Settings {
            idle_timeout: Duration::from_millis(plan.idle_timeout_ms),
            max_connections: plan.gateway_max_connections,
        },
        &out.join("gateway.log"),
    )
    .await?;
    let results = scenarios::run(&plan, &mut install).await;
    install.stop().await?;
    let report = report::Report {
        kaeso: &build,
        machine: &machine,
        parameters: &plan,
        results: &results,
    };
    let table = report::table(&report);
    std::fs::write(out.join("report.md"), &table)?;
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    writeln!(std::io::stdout().lock(), "{table}")?;
    say(&format!(
        "wrote {} and {}",
        out.join("report.md").display(),
        out.join("report.json").display()
    ));
    Ok(report.passed())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            say(&format!("cannot start the async runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let outcome = match cli.command {
        Some(Command::Gateway) => runtime.block_on(gateway::serve()).map(|()| true),
        None => {
            let out = cli.options.out.clone();
            runtime.block_on(bench(cli.options.plan(), out))
        }
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            say("a check failed; see the table above");
            ExitCode::FAILURE
        }
        Err(e) => {
            say(&e.to_string());
            ExitCode::FAILURE
        }
    }
}
