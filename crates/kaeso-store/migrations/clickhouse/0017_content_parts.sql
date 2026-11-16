CREATE TABLE IF NOT EXISTS request_content_part
(
    org_id UUID,
    project_id UUID,
    hash FixedString(16),
    body SimpleAggregateFunction(any, String) CODEC(ZSTD(3)),
    first_at SimpleAggregateFunction(min, DateTime64(3, 'UTC')),
    last_at SimpleAggregateFunction(max, DateTime64(3, 'UTC')),
    expires_at SimpleAggregateFunction(max, DateTime('UTC'))
)
ENGINE = AggregatingMergeTree
ORDER BY (org_id, project_id, hash)
TTL expires_at
SETTINGS min_compress_block_size = 16777216, max_compress_block_size = 33554432;
ALTER TABLE request_content ADD COLUMN IF NOT EXISTS request_parts UInt32 DEFAULT 0;
