ALTER TABLE limits
    DROP CONSTRAINT limits_kind_check,
    ADD CONSTRAINT limits_kind_check
        CHECK (kind IN ('request_rate', 'token_rate', 'token_budget', 'spend_budget', 'tool_call_rate')),
    DROP CONSTRAINT limits_check1,
    ADD CONSTRAINT limits_rates_per_minute
        CHECK ((kind IN ('request_rate', 'token_rate', 'tool_call_rate')) = (period = 'minute')),
    ADD CONSTRAINT limits_tool_calls_for_every_tool
        CHECK (kind <> 'tool_call_rate' OR catalogue_model_id IS NULL);

-- Kept after a server is deleted, so a project's history still adds up.
CREATE TABLE tool_usage_daily (
    day date NOT NULL,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL,
    server_id uuid NOT NULL,
    calls bigint NOT NULL DEFAULT 0 CHECK (calls >= 0),
    errors bigint NOT NULL DEFAULT 0 CHECK (errors >= 0),
    PRIMARY KEY (project_id, day, server_id),
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX tool_usage_daily_org_day ON tool_usage_daily (org_id, day);
