-- The part of a limit's use that sources reported reads only their rows; observed usage is a
-- small share of a project's, so the index stays small.
CREATE INDEX usage_daily_observed ON usage_daily (project_id, day) WHERE source_id IS NOT NULL;
