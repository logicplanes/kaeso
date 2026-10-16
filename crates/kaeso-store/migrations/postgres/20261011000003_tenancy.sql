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
    sso_default_preset text CHECK (sso_default_preset IN ('admin', 'billing', 'developer', 'auditor', 'member', 'custom')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    avatar text REFERENCES avatars (hash),
    sso_default_role_id uuid,
    CHECK ((state = 'scheduled_for_deletion') = (deletion_due_at IS NOT NULL)),
    CHECK (content_retention_days <= metadata_retention_days),
    CONSTRAINT organizations_sso_default_role_check
        CHECK ((sso_default_preset = 'custom') = (sso_default_role_id IS NOT NULL))
);

CREATE INDEX organizations_deletion ON organizations (deletion_due_at) WHERE state = 'scheduled_for_deletion';
CREATE INDEX organizations_deletion_due_at ON organizations (deletion_due_at, id)
    WHERE deletion_due_at IS NOT NULL;

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

CREATE TABLE org_roles (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    scope text NOT NULL CHECK (scope IN ('org', 'project')),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 50),
    description text NOT NULL DEFAULT '' CHECK (length(description) <= 200),
    rights text[] NOT NULL DEFAULT '{}',
    project_rights text[] NOT NULL DEFAULT '{}',
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (scope = 'org' OR project_rights = '{}')
);

CREATE UNIQUE INDEX org_roles_name ON org_roles (org_id, scope, lower(name));

ALTER TABLE organizations
    ADD CONSTRAINT organizations_sso_default_role_id_fkey
        FOREIGN KEY (sso_default_role_id) REFERENCES org_roles (id) ON DELETE RESTRICT;

CREATE TABLE org_memberships (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    preset text NOT NULL CHECK (preset IN ('owner', 'admin', 'billing', 'developer', 'auditor', 'member', 'custom')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    role_id uuid REFERENCES org_roles (id) ON DELETE RESTRICT,
    UNIQUE (org_id, user_id),
    CONSTRAINT org_memberships_role_check CHECK ((preset = 'custom') = (role_id IS NOT NULL))
);

CREATE INDEX org_memberships_user ON org_memberships (user_id);
CREATE INDEX org_memberships_owners ON org_memberships (org_id) WHERE preset = 'owner';
CREATE INDEX org_memberships_role ON org_memberships (role_id) WHERE role_id IS NOT NULL;

CREATE TABLE invites (
    id uuid PRIMARY KEY,
    org_id uuid REFERENCES organizations (id) ON DELETE CASCADE,
    email text NOT NULL,
    org_preset text CHECK (org_preset IN ('admin', 'billing', 'developer', 'auditor', 'member', 'owner', 'custom')),
    project_roles jsonb NOT NULL DEFAULT '[]',
    token_hash bytea NOT NULL UNIQUE,
    invited_by uuid REFERENCES users (id) ON DELETE SET NULL,
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    accepted_by uuid REFERENCES users (id) ON DELETE SET NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    org_role_id uuid REFERENCES org_roles (id) ON DELETE SET NULL,
    CHECK ((org_id IS NULL) = (org_preset IS NULL)),
    CONSTRAINT invites_org_role_check CHECK (org_role_id IS NULL OR org_preset = 'custom')
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
    avatar text REFERENCES avatars (hash),
    CHECK (NOT (is_default AND archived_at IS NOT NULL)),
    CONSTRAINT projects_id_org UNIQUE (id, org_id)
);

CREATE UNIQUE INDEX projects_one_default ON projects (org_id) WHERE is_default;
CREATE INDEX projects_org ON projects (org_id, id);

CREATE TABLE project_memberships (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    preset text NOT NULL CHECK (preset IN ('admin', 'developer', 'viewer', 'custom')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    models_restricted boolean NOT NULL DEFAULT false,
    role_id uuid REFERENCES org_roles (id) ON DELETE RESTRICT,
    UNIQUE (project_id, user_id),
    CONSTRAINT project_memberships_role_check CHECK ((preset = 'custom') = (role_id IS NOT NULL))
);

CREATE INDEX project_memberships_user ON project_memberships (user_id);
CREATE INDEX project_memberships_role ON project_memberships (role_id) WHERE role_id IS NOT NULL;

CREATE TABLE service_accounts (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    preset text NOT NULL CHECK (preset IN ('admin', 'developer', 'viewer', 'custom')),
    rights_added text[] NOT NULL DEFAULT '{}',
    rights_removed text[] NOT NULL DEFAULT '{}',
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    archived_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    models_restricted boolean NOT NULL DEFAULT false,
    role_id uuid REFERENCES org_roles (id) ON DELETE SET NULL,
    CONSTRAINT service_accounts_role_check CHECK (role_id IS NULL OR preset = 'custom')
);

CREATE INDEX service_accounts_project ON service_accounts (project_id, id);
CREATE INDEX service_accounts_role ON service_accounts (role_id) WHERE role_id IS NOT NULL;
