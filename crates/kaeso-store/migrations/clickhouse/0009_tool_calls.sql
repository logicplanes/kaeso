CREATE TABLE IF NOT EXISTS tool_calls
(
    id UUID,
    org_id UUID,
    project_id UUID,
    server_id UUID,
    started_at DateTime64(3, 'UTC'),
    tool String,
    upstream_tool String,
    caller_kind LowCardinality(String),
    caller_id UUID,
    key_id Nullable(UUID),
    grant_id Nullable(UUID),
    client_name String,
    protocol_version LowCardinality(String),
    source LowCardinality(String) DEFAULT 'portal',
    status LowCardinality(String),
    error_type LowCardinality(String),
    error_message String,
    duration_ms UInt32,
    upstream_ms Nullable(UInt32),
    request_bytes UInt64,
    response_bytes UInt64,
    has_content Bool,
    expires_at DateTime('UTC'),
    INDEX tool_calls_id id TYPE bloom_filter GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(started_at)
ORDER BY (org_id, project_id, started_at, id)
TTL expires_at
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS tool_call_content
(
    id UUID,
    org_id UUID,
    project_id UUID,
    started_at DateTime64(3, 'UTC'),
    arguments String CODEC(ZSTD(3)),
    result String CODEC(ZSTD(3)),
    expires_at DateTime('UTC'),
    INDEX tool_call_content_id id TYPE bloom_filter GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(started_at)
ORDER BY (org_id, project_id, started_at, id)
TTL expires_at;
