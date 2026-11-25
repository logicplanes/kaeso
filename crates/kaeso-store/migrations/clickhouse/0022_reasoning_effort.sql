ALTER TABLE request_log ADD COLUMN IF NOT EXISTS reasoning_effort LowCardinality(String) DEFAULT '';
