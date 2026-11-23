-- Sources report requests and tool calls that never pass through the gateway, such as a coding
-- agent calling its provider directly. Kaeso only observes them: it counts and logs what a
-- source sends and can never refuse the calls themselves.
CREATE TABLE sources (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN (
        'claude_code', 'codex', 'gemini_cli', 'antigravity', 'opentelemetry', 'http', 'litellm', 'openrouter'
    )),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    secret_hash bytea NOT NULL UNIQUE,
    secret_prefix text NOT NULL,
    secret_last4 text NOT NULL,
    user_id uuid REFERENCES users (id) ON DELETE SET NULL,
    service_account_id uuid REFERENCES service_accounts (id) ON DELETE SET NULL,
    match_members boolean NOT NULL DEFAULT true,
    content boolean NOT NULL DEFAULT false,
    disabled_at timestamptz,
    archived_at timestamptz,
    first_event_at timestamptz,
    last_event_at timestamptz,
    last_error_at timestamptz,
    last_error text CHECK (length(last_error) <= 300),
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (num_nonnulls(user_id, service_account_id) <= 1),
    CHECK ((last_error IS NULL) = (last_error_at IS NULL)),
    CONSTRAINT sources_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX sources_project ON sources (project_id, id);
CREATE INDEX sources_org ON sources (org_id, id);

CREATE TABLE source_activity (
    source_id uuid NOT NULL REFERENCES sources (id) ON DELETE CASCADE,
    day date NOT NULL,
    batches bigint NOT NULL DEFAULT 0,
    requests bigint NOT NULL DEFAULT 0,
    tool_calls bigint NOT NULL DEFAULT 0,
    duplicates bigint NOT NULL DEFAULT 0,
    refused bigint NOT NULL DEFAULT 0,
    PRIMARY KEY (source_id, day)
);

ALTER TABLE usage_daily ADD COLUMN source_id uuid;

DROP INDEX usage_daily_row;

CREATE UNIQUE INDEX usage_daily_row ON usage_daily
    (project_id, day, api_key_id, user_id, service_account_id, provider, model_id, ingress_dialect,
        egress_dialect, credential_id, source_id)
    NULLS NOT DISTINCT;
