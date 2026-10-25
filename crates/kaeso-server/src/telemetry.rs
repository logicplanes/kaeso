use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder, PrometheusHandle};
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

// Kaeso's own work before a provider call takes from about 0.1 ms to tens of milliseconds,
// below the first latency bucket.
const OVERHEAD_BUCKETS: &[f64] = &[
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 1.0,
];

fn builder() -> Result<PrometheusBuilder, TelemetryError> {
    PrometheusBuilder::new()
        .set_buckets(LATENCY_BUCKETS)
        .and_then(|b| {
            b.set_buckets_for_metric(
                Matcher::Full(crate::metrics::GATEWAY_OVERHEAD.to_owned()),
                OVERHEAD_BUCKETS,
            )
        })
        .map_err(TelemetryError::Buckets)
}

pub fn init_metrics() -> Result<PrometheusHandle, TelemetryError> {
    let handle = builder()?
        .install_recorder()
        .map_err(TelemetryError::Recorder)?;
    crate::metrics::describe();
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_overhead_is_counted_in_sub_millisecond_buckets() {
        let recorder = builder().unwrap().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            metrics::histogram!(crate::metrics::GATEWAY_OVERHEAD, "provider" => "openai")
                .record(0.0003);
            metrics::histogram!(crate::metrics::TIME_TO_FIRST_BYTE, "provider" => "openai")
                .record(0.0003);
        });
        let text = handle.render();
        assert!(
            text.contains(
                "kaeso_gateway_overhead_seconds_bucket{provider=\"openai\",le=\"0.00025\"} 0"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "kaeso_gateway_overhead_seconds_bucket{provider=\"openai\",le=\"0.0005\"} 1"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "kaeso_time_to_first_byte_seconds_bucket{provider=\"openai\",le=\"0.001\"} 1"
            ),
            "{text}"
        );
        assert!(
            !text.contains("time_to_first_byte_seconds_bucket{provider=\"openai\",le=\"0.0005\"}"),
            "{text}"
        );
    }
}
