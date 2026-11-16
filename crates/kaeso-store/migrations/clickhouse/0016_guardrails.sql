ALTER TABLE request_log
    ADD COLUMN IF NOT EXISTS guardrails String DEFAULT '',
    ADD COLUMN IF NOT EXISTS guardrail_actions Array(LowCardinality(String)) DEFAULT [];
