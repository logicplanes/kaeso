ALTER TABLE request_log ADD COLUMN IF NOT EXISTS cost_basis LowCardinality(String) DEFAULT '';
