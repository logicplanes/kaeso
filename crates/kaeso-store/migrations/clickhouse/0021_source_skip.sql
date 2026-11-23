ALTER TABLE request_log ADD INDEX IF NOT EXISTS request_log_source_id source_id TYPE bloom_filter GRANULARITY 4;
ALTER TABLE tool_calls ADD INDEX IF NOT EXISTS tool_calls_source_id source_id TYPE bloom_filter GRANULARITY 4;
