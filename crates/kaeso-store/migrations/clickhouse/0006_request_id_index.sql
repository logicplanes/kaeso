ALTER TABLE request_log ADD INDEX IF NOT EXISTS request_log_request_id request_id TYPE bloom_filter GRANULARITY 4;
ALTER TABLE request_content ADD INDEX IF NOT EXISTS request_content_request_id request_id TYPE bloom_filter GRANULARITY 4;
