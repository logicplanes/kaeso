ALTER TABLE request_log ADD COLUMN IF NOT EXISTS cost_known Bool DEFAULT true;
