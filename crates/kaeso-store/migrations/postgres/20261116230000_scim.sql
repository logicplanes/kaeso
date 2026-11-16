-- SCIM 2.0 provisioning (RFC 7644) for an organization with single sign-on: one bearer token per
-- organization, kept as its hash, with the previous token valid for a grace period after a
-- rotation.
CREATE TABLE scim_tokens (
    org_id uuid PRIMARY KEY REFERENCES organizations (id) ON DELETE CASCADE,
    hash bytea NOT NULL UNIQUE,
    last4 text NOT NULL,
    previous_hash bytea UNIQUE,
    previous_until timestamptz,
    created_at timestamptz NOT NULL,
    last_used_at timestamptz,
    last_error text,
    last_error_at timestamptz,
    CHECK ((previous_hash IS NULL) = (previous_until IS NULL))
);

-- The accounts an organization's identity provider manages through SCIM: provisioned while
-- `active`, deprovisioned (no longer a member) otherwise.
CREATE TABLE scim_users (
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    external_id text,
    active boolean NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (org_id, user_id),
    UNIQUE (org_id, external_id)
);

-- The identity provider's groups, kept as it pushes them, for mapping groups to roles.
CREATE TABLE scim_groups (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    display_name text NOT NULL,
    external_id text,
    members uuid[] NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (org_id, display_name)
);
