use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::config::{LogConfig, LogFormat};

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("KAESO_LOG_LEVEL (log.level) is not a valid filter: {0}")]
    Filter(#[from] tracing_subscriber::filter::ParseError),
    #[error("cannot install the logger: {0}")]
    Logger(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("cannot configure metrics: {0}")]
    Buckets(#[source] BuildError),
    #[error("cannot install the metrics recorder: {0}")]
    Recorder(#[source] BuildError),
}

pub fn init_logging(config: &LogConfig) -> Result<(), TelemetryError> {
    let filter = EnvFilter::try_new(&config.level)?;
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
    result.map_err(TelemetryError::Logger)
}

const LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
    120.0, 300.0, 600.0,
];

pub fn init_metrics() -> Result<PrometheusHandle, TelemetryError> {
    let handle = PrometheusBuilder::new()
        .set_buckets(LATENCY_BUCKETS)
        .map_err(TelemetryError::Buckets)?
        .install_recorder()
        .map_err(TelemetryError::Recorder)?;
    crate::metrics::describe();
    Ok(handle)
}
