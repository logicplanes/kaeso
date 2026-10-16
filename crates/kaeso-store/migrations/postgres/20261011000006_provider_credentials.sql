CREATE TABLE provider_credentials (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9][a-z0-9._-]*$'),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    ciphertext bytea NOT NULL,
    nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    last4 text NOT NULL,
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    last_used_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX provider_credentials_org ON provider_credentials (org_id, provider) WHERE project_id IS NULL;
CREATE UNIQUE INDEX provider_credentials_project ON provider_credentials (project_id, provider) WHERE project_id IS NOT NULL;
CREATE INDEX provider_credentials_key_id ON provider_credentials (key_id);
