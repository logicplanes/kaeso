ALTER TABLE request_log ADD COLUMN IF NOT EXISTS usage_estimated Bool DEFAULT false;
