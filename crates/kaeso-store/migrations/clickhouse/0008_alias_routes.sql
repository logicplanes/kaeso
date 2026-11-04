ALTER TABLE request_log ADD COLUMN IF NOT EXISTS alias String DEFAULT '';
ALTER TABLE request_log ADD COLUMN IF NOT EXISTS fallbacks String DEFAULT '';
