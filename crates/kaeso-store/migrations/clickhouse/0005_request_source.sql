ALTER TABLE request_log ADD COLUMN IF NOT EXISTS source LowCardinality(String) DEFAULT 'api';
