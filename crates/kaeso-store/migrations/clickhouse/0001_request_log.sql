CREATE TABLE request_log
(
    request_id UUID,
    org_id UUID,
    project_id UUID,
    key_id UUID,
    principal_kind LowCardinality(String),
    principal_id UUID,
    started_at DateTime64(3, 'UTC'),
    finished_at DateTime64(3, 'UTC'),
    duration_ms UInt32,
    ttfb_ms Nullable(UInt32),
    endpoint LowCardinality(String),
    ingress_dialect LowCardinality(String),
    egress_dialect LowCardinality(String),
    provider LowCardinality(String),
    requested_model String,
    resolved_model String,
    stream Bool,
    status UInt16,
    error_kind LowCardinality(String),
    error_message String,
    input_tokens UInt64,
    output_tokens UInt64,
    cache_read_tokens UInt64,
    cache_write_tokens UInt64,
    reasoning_tokens UInt64,
    cost Decimal(38, 12),
    user_agent String,
    client_ip String,
    upstream_request_id String,
    has_content Bool,
    logical_bytes UInt64,
    expires_at DateTime('UTC')
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(started_at)
ORDER BY (org_id, project_id, started_at, request_id)
TTL expires_at
SETTINGS index_granularity = 8192;

CREATE TABLE request_content
(
    request_id UUID,
    org_id UUID,
    project_id UUID,
    started_at DateTime64(3, 'UTC'),
    request_body String CODEC(ZSTD(3)),
    reply String CODEC(ZSTD(3)),
    expires_at DateTime('UTC')
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(started_at)
ORDER BY (org_id, project_id, started_at, request_id)
TTL expires_at;
