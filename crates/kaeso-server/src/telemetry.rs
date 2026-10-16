use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::config::{LogConfig, LogFormat};

pub fn init_logging(config: &LogConfig) -> Result<(), String> {
    let filter = EnvFilter::try_new(&config.level)
        .map_err(|e| format!("KAESO_LOG_LEVEL (log.level) is not a valid filter: {e}"))?;
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::NONE)
        .with_writer(std::io::stdout);
    let result = match config.format {
        LogFormat::Json => builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .try_init(),
        LogFormat::Text => builder.try_init(),
    };
    result.map_err(|e| format!("cannot install the logger: {e}"))
}

const LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
    120.0, 300.0, 600.0,
];

pub fn init_metrics() -> Result<PrometheusHandle, String> {
    let handle = PrometheusBuilder::new()
        .set_buckets(LATENCY_BUCKETS)
        .map_err(|e| format!("cannot configure metrics: {e}"))?
        .install_recorder()
        .map_err(|e| format!("cannot install the metrics recorder: {e}"))?;
    crate::metrics::describe();
    Ok(handle)
}
