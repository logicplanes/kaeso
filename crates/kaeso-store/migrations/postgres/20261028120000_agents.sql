ALTER TABLE instance_settings
    ADD COLUMN agents_allowed boolean NOT NULL DEFAULT true,
    ADD COLUMN agents_max text NOT NULL DEFAULT 'configure'
        CHECK (agents_max IN ('none', 'read', 'configure', 'admin'));

ALTER TABLE organizations
    ADD COLUMN agents_allowed boolean NOT NULL DEFAULT true,
    ADD COLUMN agents_max text NOT NULL DEFAULT 'full'
        CHECK (agents_max IN ('observer', 'builder', 'operator', 'full'));

CREATE TABLE agent_clients (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    redirect_uris text[] NOT NULL CHECK (cardinality(redirect_uris) BETWEEN 1 AND 10),
    software_id text,
    software_version text,
    created_at timestamptz NOT NULL
);

CREATE INDEX agent_clients_created ON agent_clients (created_at);

CREATE TABLE agents (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    client_id uuid REFERENCES agent_clients (id) ON DELETE SET NULL,
    name text NOT NULL,
    harness text NOT NULL,
    model text,
    preset text NOT NULL CHECK (preset IN ('observer', 'builder', 'operator', 'full')),
    customized boolean NOT NULL DEFAULT false,
    instance_rights text[] NOT NULL DEFAULT '{}',
    account_rights text[] NOT NULL DEFAULT '{}',
    future_orgs boolean NOT NULL DEFAULT false,
    future_org_rights text[] NOT NULL DEFAULT '{}',
    future_project_rights text[] NOT NULL DEFAULT '{}',
    sign_in_method text NOT NULL,
    second_factor boolean NOT NULL,
    oidc_org_id uuid,
    created_at timestamptz NOT NULL,
    last_used_at timestamptz,
    expires_at timestamptz,
    revoked_at timestamptz
);

CREATE INDEX agents_user ON agents (user_id) WHERE revoked_at IS NULL;

CREATE TABLE agent_orgs (
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    included boolean NOT NULL,
    rights text[] NOT NULL DEFAULT '{}',
    all_projects boolean NOT NULL DEFAULT true,
    project_rights text[] NOT NULL DEFAULT '{}',
    PRIMARY KEY (agent_id, org_id)
);

CREATE INDEX agent_orgs_org ON agent_orgs (org_id) WHERE included;

CREATE TABLE agent_projects (
    agent_id uuid NOT NULL,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    rights text[] NOT NULL DEFAULT '{}',
    PRIMARY KEY (agent_id, project_id),
    FOREIGN KEY (agent_id, org_id) REFERENCES agent_orgs (agent_id, org_id) ON DELETE CASCADE
);

CREATE TABLE agent_requests (
    id uuid PRIMARY KEY,
    token_hash bytea NOT NULL UNIQUE,
    client_id uuid NOT NULL REFERENCES agent_clients (id) ON DELETE CASCADE,
    redirect_uri text NOT NULL,
    state text,
    code_challenge text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at timestamptz
);

CREATE INDEX agent_requests_expiry ON agent_requests (expires_at);

CREATE TABLE agent_tokens (
    id uuid PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('code', 'access', 'refresh')),
    token_hash bytea NOT NULL UNIQUE,
    family uuid NOT NULL,
    redirect_uri text,
    code_challenge text,
    created_at timestamptz NOT NULL,
    expires_at timestamptz,
    used_at timestamptz,
    revoked_at timestamptz,
    CHECK ((kind = 'code') = (code_challenge IS NOT NULL AND redirect_uri IS NOT NULL))
);

CREATE INDEX agent_tokens_agent ON agent_tokens (agent_id, family);
CREATE INDEX agent_tokens_expiry ON agent_tokens (expires_at);

CREATE TABLE agent_activity (
    id uuid PRIMARY KEY,
    agent_id uuid NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    tool text NOT NULL,
    org_id uuid,
    project_id uuid,
    target text,
    status smallint NOT NULL,
    result text NOT NULL,
    ip inet,
    at timestamptz NOT NULL
);

CREATE INDEX agent_activity_agent ON agent_activity (agent_id, id);
CREATE INDEX agent_activity_at ON agent_activity (at);

ALTER TABLE audit_events ADD COLUMN agent_id uuid;
ALTER TABLE audit_events DROP CONSTRAINT audit_events_actor_kind_check;
ALTER TABLE audit_events ADD CONSTRAINT audit_events_actor_kind_check
    CHECK (actor_kind IN ('user', 'org_platform_key', 'api_key', 'system', 'agent'));
ALTER TABLE audit_events ADD CONSTRAINT audit_events_agent
    CHECK ((actor_kind = 'agent') = (agent_id IS NOT NULL));

CREATE INDEX audit_events_agent ON audit_events (agent_id) WHERE agent_id IS NOT NULL;
