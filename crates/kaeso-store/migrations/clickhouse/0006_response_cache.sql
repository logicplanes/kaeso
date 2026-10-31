ALTER TABLE request_log ADD COLUMN IF NOT EXISTS response_cache LowCardinality(String) DEFAULT '';
