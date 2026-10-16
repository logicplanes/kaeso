CREATE TABLE models_dev_snapshot (
    id smallint PRIMARY KEY CHECK (id = 1),
    fetched_at timestamptz NOT NULL,
    source_url text NOT NULL,
    content_hash bytea NOT NULL,
    data jsonb NOT NULL,
    model_count integer NOT NULL,
    last_attempt_at timestamptz,
    last_error text,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE catalogue_models (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    model_id text NOT NULL CHECK (length(model_id) BETWEEN 1 AND 200),
    overrides jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(overrides) = 'object'),
    custom boolean NOT NULL DEFAULT false,
    delisted_at timestamptz,
    last_known jsonb,
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    UNIQUE (project_id, provider, model_id),
    CONSTRAINT catalogue_models_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX catalogue_models_model ON catalogue_models (provider, model_id);

CREATE TABLE aliases (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    catalogue_model_id uuid NOT NULL REFERENCES catalogue_models (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, name),
    CONSTRAINT aliases_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX aliases_target ON aliases (catalogue_model_id);

CREATE TABLE model_access (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    membership_id uuid REFERENCES project_memberships (id) ON DELETE CASCADE,
    service_account_id uuid REFERENCES service_accounts (id) ON DELETE CASCADE,
    api_key_id uuid REFERENCES api_keys (id) ON DELETE CASCADE,
    catalogue_model_id uuid NOT NULL REFERENCES catalogue_models (id) ON DELETE CASCADE,
    CHECK (num_nonnulls(membership_id, service_account_id, api_key_id) = 1)
);

CREATE UNIQUE INDEX model_access_membership ON model_access (membership_id, catalogue_model_id) WHERE membership_id IS NOT NULL;
CREATE UNIQUE INDEX model_access_service_account ON model_access (service_account_id, catalogue_model_id) WHERE service_account_id IS NOT NULL;
CREATE UNIQUE INDEX model_access_key ON model_access (api_key_id, catalogue_model_id) WHERE api_key_id IS NOT NULL;
CREATE INDEX model_access_project ON model_access (project_id);
CREATE INDEX model_access_model ON model_access (catalogue_model_id);
