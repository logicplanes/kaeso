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
    alert_thresholds smallint[] NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    CHECK (
        (level = 'project' AND num_nonnulls(user_id, service_account_id, api_key_id) = 0)
        OR (level = 'member' AND num_nonnulls(user_id, service_account_id) = 1 AND api_key_id IS NULL)
        OR (level = 'key' AND api_key_id IS NOT NULL AND num_nonnulls(user_id, service_account_id) = 0)
    ),
    CHECK ((kind IN ('request_rate', 'token_rate')) = (period = 'minute')),
    CONSTRAINT limits_alerts_only_for_budgets
        CHECK (kind IN ('token_budget', 'spend_budget') OR cardinality(alert_thresholds) = 0),
    UNIQUE NULLS NOT DISTINCT (project_id, level, user_id, service_account_id, api_key_id, kind, catalogue_model_id, period),
    CONSTRAINT limits_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX limits_project ON limits (project_id);

CREATE TABLE usage_daily (
    day date NOT NULL,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id uuid,
    service_account_id uuid,
    api_key_id uuid,
    catalogue_model_id uuid,
    model_id text NOT NULL,
    provider text NOT NULL CHECK (provider = '' OR provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    requests bigint NOT NULL DEFAULT 0,
    input_tokens bigint NOT NULL DEFAULT 0,
    output_tokens bigint NOT NULL DEFAULT 0,
    cache_read_tokens bigint NOT NULL DEFAULT 0,
    cache_write_tokens bigint NOT NULL DEFAULT 0,
    reasoning_tokens bigint NOT NULL DEFAULT 0,
    cost numeric(30, 9) NOT NULL DEFAULT 0,
    errors bigint NOT NULL DEFAULT 0,
    CONSTRAINT usage_daily_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX usage_daily_row ON usage_daily
    (project_id, day, api_key_id, user_id, service_account_id, provider, model_id) NULLS NOT DISTINCT;
CREATE INDEX usage_daily_org_day ON usage_daily (org_id, day);

CREATE TABLE budget_alerts_sent (
    limit_id uuid NOT NULL REFERENCES limits (id) ON DELETE CASCADE,
    period_start date NOT NULL,
    threshold smallint NOT NULL,
    sent_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (limit_id, period_start, threshold)
);
