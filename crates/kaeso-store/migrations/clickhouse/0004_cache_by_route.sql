ALTER TABLE request_log ADD COLUMN IF NOT EXISTS cache_breakpoints Nullable(UInt16);

ALTER TABLE usage_daily
    ADD COLUMN IF NOT EXISTS ingress_dialect LowCardinality(String),
    ADD COLUMN IF NOT EXISTS egress_dialect LowCardinality(String),
    MODIFY ORDER BY (org_id, project_id, day, principal_kind, principal_id, key_id, provider, model, ingress_dialect, egress_dialect);

ALTER TABLE usage_daily_mv MODIFY QUERY
SELECT
    org_id,
    project_id,
    toDate(started_at) AS day,
    principal_kind,
    principal_id,
    key_id,
    provider,
    resolved_model AS model,
    ingress_dialect,
    egress_dialect,
    count() AS requests,
    countIf(status >= 400) AS errors,
    sum(input_tokens) AS input_tokens,
    sum(output_tokens) AS output_tokens,
    sum(cache_read_tokens) AS cache_read_tokens,
    sum(cache_write_tokens) AS cache_write_tokens,
    sum(reasoning_tokens) AS reasoning_tokens,
    sum(cost) AS cost
FROM request_log
GROUP BY org_id, project_id, day, principal_kind, principal_id, key_id, provider, model, ingress_dialect, egress_dialect;
