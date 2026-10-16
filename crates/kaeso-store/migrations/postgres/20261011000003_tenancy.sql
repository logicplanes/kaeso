CREATE TABLE organizations (
    id uuid PRIMARY KEY,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    state text NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'scheduled_for_deletion')),
    deletion_due_at timestamptz,
    content_logging boolean NOT NULL DEFAULT true,
    metadata_retention_days integer NOT NULL CHECK (metadata_retention_days > 0),
    content_retention_days integer NOT NULL CHECK (content_retention_days > 0),
    require_2fa boolean NOT NULL DEFAULT false,
    require_sso boolean NOT NULL DEFAULT false,
    max_key_lifetime_days integer CHECK (max_key_lifetime_days > 0),
    sso_default_preset text CHECK (sso_default_preset IN ('admin', 'billing', 'developer', 'member')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((state = 'scheduled_for_deletion') = (deletion_due_at IS NOT NULL)),
    CHECK (content_retention_days <= metadata_retention_days)
);

CREATE INDEX organizations_deletion ON organizations (deletion_due_at) WHERE state = 'scheduled_for_deletion';

CREATE TABLE org_oidc (
    org_id uuid PRIMARY KEY REFERENCES organizations (id) ON DELETE CASCADE,
    issuer text NOT NULL,
    client_id text NOT NULL,
    secret_ciphertext bytea NOT NULL,
    secret_nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX org_oidc_key_id ON org_oidc (key_id);

CREATE TABLE org_oidc_domains (
    domain text PRIMARY KEY CHECK (domain = lower(domain)),
    org_id uuid NOT NULL REFERENCES org_oidc (org_id) ON DELETE CASCADE
);

CREATE INDEX org_oidc_domains_org ON org_oidc_domains (org_id);

ALTER TABLE sessions
    ADD CONSTRAINT sessions_oidc_org_fkey FOREIGN KEY (oidc_org_id) REFERENCES organizations (id) ON DELETE SET NULL;

CREATE TABLE org_memberships (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    preset text NOT NULL CHECK (preset IN ('owner', 'admin', 'billing', 'developer', 'member')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, user_id)
);

CREATE INDEX org_memberships_user ON org_memberships (user_id);
CREATE INDEX org_memberships_owners ON org_memberships (org_id) WHERE preset = 'owner';

CREATE TABLE invites (
    id uuid PRIMARY KEY,
    org_id uuid REFERENCES organizations (id) ON DELETE CASCADE,
    email text NOT NULL,
    org_preset text CHECK (org_preset IN ('admin', 'billing', 'developer', 'member', 'owner')),
    project_roles jsonb NOT NULL DEFAULT '[]',
    token_hash bytea NOT NULL UNIQUE,
    invited_by uuid REFERENCES users (id) ON DELETE SET NULL,
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    accepted_by uuid REFERENCES users (id) ON DELETE SET NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((org_id IS NULL) = (org_preset IS NULL))
);

CREATE INDEX invites_org ON invites (org_id, id);
CREATE INDEX invites_email ON invites (lower(email)) WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TABLE projects (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    is_default boolean NOT NULL DEFAULT false,
    content_logging boolean NOT NULL DEFAULT true,
    max_key_lifetime_days integer CHECK (max_key_lifetime_days > 0),
    archived_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (NOT (is_default AND archived_at IS NOT NULL))
);

CREATE UNIQUE INDEX projects_one_default ON projects (org_id) WHERE is_default;
CREATE INDEX projects_org ON projects (org_id, id);

CREATE TABLE project_memberships (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    preset text NOT NULL CHECK (preset IN ('admin', 'developer', 'viewer')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, user_id)
);

CREATE INDEX project_memberships_user ON project_memberships (user_id);

CREATE TABLE service_accounts (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    preset text NOT NULL CHECK (preset IN ('admin', 'developer', 'viewer')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    archived_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX service_accounts_project ON service_accounts (project_id, id);
