ALTER TABLE usage_daily
    ADD COLUMN ingress_dialect text NOT NULL DEFAULT '' CHECK (ingress_dialect ~ '^[a-z0-9_]*$'),
    ADD COLUMN egress_dialect text NOT NULL DEFAULT '' CHECK (egress_dialect ~ '^[a-z0-9_]*$');

DROP INDEX usage_daily_row;

CREATE UNIQUE INDEX usage_daily_row ON usage_daily
    (project_id, day, api_key_id, user_id, service_account_id, provider, model_id, ingress_dialect, egress_dialect)
    NULLS NOT DISTINCT;
