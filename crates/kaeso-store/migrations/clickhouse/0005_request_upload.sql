ALTER TABLE request_log ADD COLUMN IF NOT EXISTS request_upload_ms Nullable(UInt32);
