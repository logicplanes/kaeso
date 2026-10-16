CREATE TABLE limits (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    level text NOT NULL CHECK (level IN ('project', 'member', 'key')),
    user_id uuid REFERENCES users (id) ON DELETE CASCADE,
    service_account_id uuid REFERENCES service_accounts (id) ON DELETE CASCADE,
    api_key_id uuid REFERENCES api_keys (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('request_rate', 'token_rate', 'token_budget', 'spend_budget')),
    catalogue_model_id uuid REFERENCES catalogue_models (id) ON DELETE CASCADE,
    period text NOT NULL CHECK (period IN ('minute', 'day', 'month', 'total')),
    amount numeric(30, 9) NOT NULL CHECK (amount > 0),
    hard boolean NOT NULL DEFAULT true,
    alert_thresholds smallint[] NOT NULL DEFAULT '{50,80,100}',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (
        (level = 'project' AND num_nonnulls(user_id, service_account_id, api_key_id) = 0)
        OR (level = 'member' AND num_nonnulls(user_id, service_account_id) = 1 AND api_key_id IS NULL)
        OR (level = 'key' AND api_key_id IS NOT NULL AND num_nonnulls(user_id, service_account_id) = 0)
    ),
    CHECK ((kind IN ('request_rate', 'token_rate')) = (period = 'minute')),
    UNIQUE NULLS NOT DISTINCT (project_id, level, user_id, service_account_id, api_key_id, kind, catalogue_model_id, period)
);

CREATE INDEX limits_project ON limits (project_id);

CREATE TABLE usage_daily (
    day date NOT NULL,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id uuid,
    service_account_id uuid,
    api_key_id uuid NOT NULL,
    catalogue_model_id uuid,
    model_id text NOT NULL,
    provider text NOT NULL,
    requests bigint NOT NULL DEFAULT 0,
    input_tokens bigint NOT NULL DEFAULT 0,
    output_tokens bigint NOT NULL DEFAULT 0,
    cache_read_tokens bigint NOT NULL DEFAULT 0,
    cache_write_tokens bigint NOT NULL DEFAULT 0,
    reasoning_tokens bigint NOT NULL DEFAULT 0,
    cost numeric(30, 9) NOT NULL DEFAULT 0,
    PRIMARY KEY (project_id, day, api_key_id, provider, model_id)
);

CREATE INDEX usage_daily_org_day ON usage_daily (org_id, day);

CREATE TABLE budget_alerts_sent (
    limit_id uuid NOT NULL REFERENCES limits (id) ON DELETE CASCADE,
    period_start date NOT NULL,
    threshold smallint NOT NULL,
    sent_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (limit_id, period_start, threshold)
);
