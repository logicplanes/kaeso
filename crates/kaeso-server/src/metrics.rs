use axum::Router;
use axum::extract::State;
use axum::routing::get;
use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::PrometheusHandle;

pub const REQUESTS: &str = "kaeso_requests_total";
pub const REQUEST_DURATION: &str = "kaeso_request_duration_seconds";
pub const TIME_TO_FIRST_BYTE: &str = "kaeso_time_to_first_byte_seconds";
pub const GATEWAY_OVERHEAD: &str = "kaeso_gateway_overhead_seconds";
pub const REQUEST_UPLOAD: &str = "kaeso_request_upload_seconds";
pub const TOKENS: &str = "kaeso_tokens_total";
pub const COST: &str = "kaeso_cost_microusd_total";
pub const LIMIT_REFUSALS: &str = "kaeso_limit_refusals_total";
pub const UPSTREAM_ERRORS: &str = "kaeso_upstream_errors_total";
pub const ALIAS_FALLBACKS: &str = "kaeso_alias_fallbacks_total";
pub const STREAM_FAILURES: &str = "kaeso_stream_failures_total";
pub const USAGE_ESTIMATES: &str = "kaeso_usage_estimates_total";
pub const REASONING_CONTINUITY: &str = "kaeso_reasoning_continuity_total";
pub const RESPONSE_CACHE_LOOKUPS: &str = "kaeso_response_cache_lookups_total";
pub const RESPONSE_CACHE_WRITES: &str = "kaeso_response_cache_writes_total";
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
pub const UPDATE_AVAILABLE: &str = "kaeso_update_available";
pub const KPMR_FETCHED: &str = "kaeso_kpmr_fetched_timestamp_seconds";
pub const CONNECTIONS_OPEN: &str = "kaeso_connections_open";
pub const CONNECTIONS_MAX: &str = "kaeso_connections_max";
pub const MODEL_REQUESTS_IN_FLIGHT: &str = "kaeso_model_requests_in_flight";
pub const KPMR_LAST_ATTEMPT: &str = "kaeso_kpmr_last_attempt_timestamp_seconds";
pub const KPMR_LAST_ATTEMPT_FAILED: &str = "kaeso_kpmr_last_attempt_failed";

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
    describe_histogram!(
        GATEWAY_OVERHEAD,
        Unit::Seconds,
        "Model API time Kaeso spent on a request before it called the provider, without the request upload, by provider."
    );
    describe_histogram!(
        REQUEST_UPLOAD,
        Unit::Seconds,
        "Model API time the app took to send the request body, by provider."
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
        ALIAS_FALLBACKS,
        "Alias route steps that failed and fell back to the next, by provider and reason."
    );
    describe_counter!(
        STREAM_FAILURES,
        "Streamed replies that failed after their headers were sent, by error kind and provider."
    );
    describe_counter!(
        USAGE_ESTIMATES,
        "Replies whose usage Kaeso estimated, by provider and reason (unreported, cancelled, failed)."
    );
    describe_counter!(
        REASONING_CONTINUITY,
        "Reasoning kept between turns for OpenAI Chat apps, by outcome (kept, restored, missing, unavailable)."
    );
    describe_counter!(
        RESPONSE_CACHE_LOOKUPS,
        "Requests of projects with the response cache on, by result (hit, miss, bypass)."
    );
    describe_counter!(
        RESPONSE_CACHE_WRITES,
        "Replies offered to the response cache, by outcome (stored, too_large, full, failed)."
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
    describe_gauge!(
        CONNECTIONS_OPEN,
        "Connections the public listener serves now; at kaeso_connections_max, new ones wait."
    );
    describe_gauge!(
        CONNECTIONS_MAX,
        "The most connections the public listener serves at once (server.max_connections)."
    );
    describe_gauge!(
        MODEL_REQUESTS_IN_FLIGHT,
        "Model requests being served, streams and realtime sessions included."
    );
    describe_gauge!(
        KPMR_FETCHED,
        Unit::Seconds,
        "When KPMR published the model data in use, or generated the built-in snapshot; it moves only when KPMR publishes new data."
    );
    describe_gauge!(
        KPMR_LAST_ATTEMPT,
        Unit::Seconds,
        "When the gateway last tried to fetch KPMR's data."
    );
    describe_gauge!(
        KPMR_LAST_ATTEMPT_FAILED,
        "1 while the last attempt to fetch KPMR's data failed, 0 after one succeeded."
    );
    describe_gauge!(
        UPDATE_AVAILABLE,
        "1 while a newer release is offered on the install's channel, with it as version; 0 with the running version while the install is up to date. Absent while update checks are off."
    );
    metrics::gauge!(BUILD_INFO, "version" => env!("CARGO_PKG_VERSION")).set(1.0);
}

/// One model request being served, streams included, counted in
/// `kaeso_model_requests_in_flight` until it is dropped.
#[derive(Debug)]
pub struct Serving(());

impl Serving {
    pub fn start() -> Serving {
        metrics::gauge!(MODEL_REQUESTS_IN_FLIGHT).increment(1.0);
        Serving(())
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        metrics::gauge!(MODEL_REQUESTS_IN_FLIGHT).decrement(1.0);
    }
}

/// The time of the model data in use.
pub fn kpmr_fetched(at: time::OffsetDateTime) {
    metrics::gauge!(KPMR_FETCHED).set(at.unix_timestamp() as f64);
}

/// The last attempt to fetch KPMR's data, and whether it failed.
pub fn kpmr_attempt(at: time::OffsetDateTime, failed: bool) {
    metrics::gauge!(KPMR_LAST_ATTEMPT).set(at.unix_timestamp() as f64);
    metrics::gauge!(KPMR_LAST_ATTEMPT_FAILED).set(if failed { 1.0 } else { 0.0 });
}

/// What `kaeso_update_available` last said, so a newer release sets the old one's series to 0
/// instead of leaving two releases at 1.
#[derive(Debug, Default)]
pub struct UpdateAvailable(std::sync::Mutex<Option<String>>);

pub static UPDATE_REPORTED: UpdateAvailable = UpdateAvailable(std::sync::Mutex::new(None));

impl UpdateAvailable {
    /// `newest` is the newest release offered on the install's channel when it is newer than
    /// the running version.
    pub fn report(&self, newest: Option<&str>) {
        let (version, value) = match newest {
            Some(v) => (v, 1.0),
            None => (env!("CARGO_PKG_VERSION"), 0.0),
        };
        let mut last = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(old) = last.as_deref()
            && old != version
        {
            metrics::gauge!(UPDATE_AVAILABLE, "version" => old.to_owned()).set(0.0);
        }
        metrics::gauge!(UPDATE_AVAILABLE, "version" => version.to_owned()).set(value);
        *last = Some(version.to_owned());
    }
}

pub fn router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(handle)
}

async fn render(State(handle): State<PrometheusHandle>) -> String {
    handle.render()
}
