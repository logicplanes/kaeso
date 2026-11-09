ALTER TABLE request_log MODIFY COLUMN IF EXISTS overhead_ms DEFAULT NULL;

ALTER TABLE request_log MODIFY COLUMN IF EXISTS stream_processing_ms DEFAULT NULL;

ALTER TABLE request_log MODIFY COLUMN IF EXISTS request_upload_ms DEFAULT NULL;

ALTER TABLE request_log ADD COLUMN IF NOT EXISTS overhead_us Nullable(UInt32) DEFAULT if(overhead_ms > 0, toUInt32(least(overhead_ms, 4294967) * 1000), NULL);

ALTER TABLE request_log ADD COLUMN IF NOT EXISTS stream_processing_us Nullable(UInt32) DEFAULT if(stream_processing_ms > 0, toUInt32(least(stream_processing_ms, 4294967) * 1000), NULL);

ALTER TABLE request_log ADD COLUMN IF NOT EXISTS request_upload_us Nullable(UInt32) DEFAULT if(request_upload_ms > 0, toUInt32(least(request_upload_ms, 4294967) * 1000), NULL);
