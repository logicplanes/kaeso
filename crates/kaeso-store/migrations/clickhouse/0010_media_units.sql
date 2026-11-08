ALTER TABLE request_log ADD COLUMN IF NOT EXISTS media_unit LowCardinality(String) DEFAULT '';
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS media_input UInt64 DEFAULT 0;
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS media_output UInt64 DEFAULT 0;
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS media_detail String DEFAULT '';
