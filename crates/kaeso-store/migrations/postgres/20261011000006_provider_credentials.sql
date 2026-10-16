CREATE TABLE provider_credentials (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
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
    updated_at timestamptz NOT NULL DEFAULT now(),
    is_default boolean NOT NULL DEFAULT false,
    CONSTRAINT provider_credentials_id_org_provider UNIQUE (id, org_id, provider)
);

CREATE UNIQUE INDEX provider_credentials_default ON provider_credentials (org_id, provider) WHERE is_default;
CREATE INDEX provider_credentials_org ON provider_credentials (org_id, provider, created_at);
CREATE INDEX provider_credentials_key_id ON provider_credentials (key_id);

CREATE TABLE project_provider_keys (
    project_id uuid NOT NULL,
    provider text NOT NULL,
    org_id uuid NOT NULL,
    credential_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, provider),
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    FOREIGN KEY (credential_id, org_id, provider)
        REFERENCES provider_credentials (id, org_id, provider) ON DELETE CASCADE
);

CREATE INDEX project_provider_keys_credential ON project_provider_keys (credential_id);
CREATE INDEX project_provider_keys_org ON project_provider_keys (org_id);
