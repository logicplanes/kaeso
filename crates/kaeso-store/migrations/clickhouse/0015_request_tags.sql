ALTER TABLE request_log ADD COLUMN IF NOT EXISTS tags Array(LowCardinality(String)) DEFAULT [];
