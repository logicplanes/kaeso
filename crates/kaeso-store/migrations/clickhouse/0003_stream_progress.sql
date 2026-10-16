ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_events UInt32 DEFAULT 0;
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_bytes UInt64 DEFAULT 0;
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_first_byte_ms Nullable(UInt32);
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_keepalives UInt32 DEFAULT 0;
