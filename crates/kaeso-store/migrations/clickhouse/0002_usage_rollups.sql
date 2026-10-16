CREATE TABLE IF NOT EXISTS usage_daily
(
    org_id UUID,
    project_id UUID,
    day Date,
    principal_kind LowCardinality(String),
    principal_id UUID,
    key_id UUID,
    provider LowCardinality(String),
    model String,
    requests UInt64,
    errors UInt64,
    input_tokens UInt64,
    output_tokens UInt64,
    cache_read_tokens UInt64,
    cache_write_tokens UInt64,
    reasoning_tokens UInt64,
    cost Decimal(38, 12)
)
ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(day)
ORDER BY (org_id, project_id, day, principal_kind, principal_id, key_id, provider, model);

CREATE MATERIALIZED VIEW IF NOT EXISTS usage_daily_mv TO usage_daily AS
SELECT
    org_id,
    project_id,
    toDate(started_at) AS day,
    principal_kind,
    principal_id,
    key_id,
    provider,
    resolved_model AS model,
    count() AS requests,
    countIf(status >= 400) AS errors,
    sum(input_tokens) AS input_tokens,
    sum(output_tokens) AS output_tokens,
    sum(cache_read_tokens) AS cache_read_tokens,
    sum(cache_write_tokens) AS cache_write_tokens,
    sum(reasoning_tokens) AS reasoning_tokens,
    sum(cost) AS cost
FROM request_log
GROUP BY org_id, project_id, day, principal_kind, principal_id, key_id, provider, model;
