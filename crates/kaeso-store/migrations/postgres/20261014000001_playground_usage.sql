ALTER TABLE usage_daily DROP CONSTRAINT usage_daily_pkey;
ALTER TABLE usage_daily ALTER COLUMN api_key_id DROP NOT NULL;
CREATE UNIQUE INDEX usage_daily_row ON usage_daily
    (project_id, day, api_key_id, user_id, service_account_id, provider, model_id) NULLS NOT DISTINCT;
