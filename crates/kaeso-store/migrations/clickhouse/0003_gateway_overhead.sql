ALTER TABLE request_log ADD COLUMN IF NOT EXISTS overhead_ms Nullable(UInt32);

ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_processing_ms Nullable(UInt32);
