use axum::Router;
use axum::extract::State;
use axum::routing::get;
use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::PrometheusHandle;

pub const REQUESTS: &str = "kaeso_requests_total";
pub const REQUEST_DURATION: &str = "kaeso_request_duration_seconds";
pub const TIME_TO_FIRST_BYTE: &str = "kaeso_time_to_first_byte_seconds";
pub const TOKENS: &str = "kaeso_tokens_total";
pub const COST: &str = "kaeso_cost_microusd_total";
pub const LIMIT_REFUSALS: &str = "kaeso_limit_refusals_total";
pub const UPSTREAM_ERRORS: &str = "kaeso_upstream_errors_total";
pub const STREAM_FAILURES: &str = "kaeso_stream_failures_total";
pub const REASONING_CONTINUITY: &str = "kaeso_reasoning_continuity_total";
pub const LOG_BUFFER_BYTES: &str = "kaeso_log_buffer_bytes";
pub const LOG_BUFFER_ROWS: &str = "kaeso_log_buffer_rows";
pub const LOG_ROWS_DROPPED: &str = "kaeso_log_rows_dropped_total";
pub const CLICKHOUSE_WRITE_FAILURES: &str = "kaeso_clickhouse_write_failures_total";
pub const USAGE_ROWS_DROPPED: &str = "kaeso_usage_rows_dropped_total";
pub const VALKEY_FAILURES: &str = "kaeso_valkey_failures_total";
pub const FAIL_OPEN: &str = "kaeso_limits_fail_open_total";
pub const JOB_RUNS: &str = "kaeso_job_runs_total";
pub const AUTH_FAILURES: &str = "kaeso_auth_failures_total";
pub const KEY_FAILURE_BLOCKS: &str = "kaeso_key_failure_blocks_total";
pub const EMAILS: &str = "kaeso_emails_total";
pub const BUILD_INFO: &str = "kaeso_build_info";

pub fn describe() {
    describe_counter!(
        REQUESTS,
        "Requests by surface, route, status, provider and model."
    );
    describe_histogram!(REQUEST_DURATION, Unit::Seconds, "Request duration.");
    describe_histogram!(
        TIME_TO_FIRST_BYTE,
        Unit::Seconds,
        "Model API time to the first byte of the reply."
    );
    describe_counter!(
        TOKENS,
        "Tokens by kind (input, output, cache_read, cache_write, reasoning)."
    );
    describe_counter!(
        COST,
        "Cost in millionths of USD at the prices in force at request time, by provider."
    );
    describe_counter!(
        LIMIT_REFUSALS,
        "Requests refused by a limit, by kind and level."
    );
    describe_counter!(UPSTREAM_ERRORS, "Provider errors by kind.");
    describe_counter!(
        STREAM_FAILURES,
        "Streamed replies that failed after their headers were sent, by error kind and provider."
    );
    describe_counter!(
        REASONING_CONTINUITY,
        "Reasoning kept between turns for OpenAI Chat apps, by outcome (kept, restored, missing, unavailable)."
    );
    describe_gauge!(
        LOG_BUFFER_BYTES,
        Unit::Bytes,
        "Bytes waiting in the request log buffer."
    );
    describe_gauge!(LOG_BUFFER_ROWS, "Rows waiting in the request log buffer.");
    describe_counter!(
        LOG_ROWS_DROPPED,
        "Log rows dropped because the buffer was full."
    );
    describe_counter!(CLICKHOUSE_WRITE_FAILURES, "Failed ClickHouse writes.");
    describe_counter!(
        USAGE_ROWS_DROPPED,
        "Usage rows PostgreSQL refused for good, which are dropped."
    );
    describe_counter!(VALKEY_FAILURES, "Valkey failures by operation.");
    describe_counter!(
        FAIL_OPEN,
        "Requests let through because Valkey was unreachable."
    );
    describe_counter!(JOB_RUNS, "Background job runs by job and outcome.");
    describe_counter!(AUTH_FAILURES, "Sign-in failures by method and reason.");
    describe_counter!(
        KEY_FAILURE_BLOCKS,
        "Client addresses blocked from API key lookups after too many failed ones, once per instance and block."
    );
    describe_counter!(EMAILS, "Emails by kind and outcome.");
    describe_gauge!(BUILD_INFO, "Build information.");
    metrics::gauge!(BUILD_INFO, "version" => env!("CARGO_PKG_VERSION")).set(1.0);
}

pub fn router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(handle)
}

async fn render(State(handle): State<PrometheusHandle>) -> String {
    handle.render()
}
