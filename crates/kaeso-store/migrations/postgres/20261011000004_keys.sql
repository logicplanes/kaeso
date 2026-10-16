CREATE TABLE api_keys (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id uuid REFERENCES users (id) ON DELETE CASCADE,
    service_account_id uuid REFERENCES service_accounts (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    secret_hash bytea NOT NULL UNIQUE,
    secret_prefix text NOT NULL,
    secret_last4 text NOT NULL,
    inference boolean NOT NULL,
    platform boolean NOT NULL,
    expires_at timestamptz,
    disabled_at timestamptz,
    disabled_reason text CHECK (disabled_reason IN ('manual', 'user_deactivated')),
    archived_at timestamptz,
    last_used_at timestamptz,
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    models_restricted boolean NOT NULL DEFAULT false,
    CHECK ((user_id IS NULL) <> (service_account_id IS NULL)),
    CHECK (inference OR platform),
    CHECK ((disabled_at IS NULL) = (disabled_reason IS NULL)),
    CONSTRAINT api_keys_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX api_keys_project ON api_keys (project_id, id);
CREATE INDEX api_keys_org ON api_keys (org_id);
CREATE INDEX api_keys_user ON api_keys (user_id) WHERE user_id IS NOT NULL;
CREATE INDEX api_keys_service_account ON api_keys (service_account_id) WHERE service_account_id IS NOT NULL;
CREATE INDEX api_keys_expiry ON api_keys (expires_at) WHERE expires_at IS NOT NULL AND archived_at IS NULL;

CREATE TABLE org_platform_keys (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    secret_hash bytea NOT NULL UNIQUE,
    secret_prefix text NOT NULL,
    secret_last4 text NOT NULL,
    expires_at timestamptz,
    disabled_at timestamptz,
    archived_at timestamptz,
    last_used_at timestamptz,
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX org_platform_keys_org ON org_platform_keys (org_id, id);

CREATE TABLE key_expiry_warnings (
    key_id uuid NOT NULL REFERENCES api_keys (id) ON DELETE CASCADE,
    days_before integer NOT NULL CHECK (days_before IN (1, 7)),
    sent_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (key_id, days_before)
);
