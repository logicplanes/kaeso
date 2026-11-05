CREATE TABLE mcp_servers (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    handle text NOT NULL CHECK (handle ~ '^[a-z0-9-]{1,24}$'),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    url text NOT NULL CHECK (length(url) BETWEEN 1 AND 2048 AND url ~ '^https?://'),
    auth_method text NOT NULL CHECK (auth_method IN ('none', 'header', 'oauth')),
    credential_mode text NOT NULL DEFAULT 'shared' CHECK (credential_mode IN ('shared', 'per_user')),
    oauth_registration text CHECK (oauth_registration IN ('cimd', 'dcr', 'manual')),
    oauth_issuer text CHECK (length(oauth_issuer) BETWEEN 1 AND 2048),
    oauth_client_id text CHECK (length(oauth_client_id) BETWEEN 1 AND 2048),
    oauth_client_secret_ciphertext bytea,
    oauth_client_secret_nonce bytea,
    oauth_client_secret_wrapped_key bytea,
    oauth_client_secret_key_id text,
    scopes text[] NOT NULL DEFAULT '{}',
    change_policy text NOT NULL DEFAULT 'hold' CHECK (change_policy IN ('hold', 'accept')),
    protocol_version text,
    server_info jsonb CHECK (jsonb_typeof(server_info) = 'object'),
    status text NOT NULL DEFAULT 'unchecked'
        CHECK (status IN ('unchecked', 'ok', 'unreachable', 'signed_out', 'error')),
    last_checked_at timestamptz,
    last_error text CHECK (length(last_error) <= 1000),
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    CONSTRAINT mcp_servers_org_handle UNIQUE (org_id, handle),
    CONSTRAINT mcp_servers_id_org UNIQUE (id, org_id),
    CONSTRAINT mcp_servers_oauth CHECK ((auth_method = 'oauth') = (oauth_registration IS NOT NULL)),
    CONSTRAINT mcp_servers_oauth_client CHECK (oauth_client_id IS NULL OR auth_method = 'oauth'),
    CONSTRAINT mcp_servers_oauth_secret CHECK (
        num_nonnulls(
            oauth_client_secret_ciphertext, oauth_client_secret_nonce,
            oauth_client_secret_wrapped_key, oauth_client_secret_key_id
        ) IN (0, 4)
        AND (oauth_client_secret_ciphertext IS NULL OR oauth_client_id IS NOT NULL)
    ),
    CONSTRAINT mcp_servers_per_user CHECK (credential_mode = 'shared' OR auth_method <> 'none')
);

CREATE INDEX mcp_servers_checked ON mcp_servers (last_checked_at NULLS FIRST);
CREATE INDEX mcp_servers_secret_key ON mcp_servers (oauth_client_secret_key_id)
    WHERE oauth_client_secret_key_id IS NOT NULL;

CREATE TABLE mcp_connections (
    id uuid PRIMARY KEY,
    server_id uuid NOT NULL,
    org_id uuid NOT NULL,
    user_id uuid,
    kind text NOT NULL CHECK (kind IN ('header', 'oauth')),
    issuer text CHECK (length(issuer) BETWEEN 1 AND 2048),
    header_name text CHECK (header_name ~ '^[!#$%&''*+.^_`|~0-9A-Za-z-]{1,100}$'),
    ciphertext bytea NOT NULL,
    nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    last4 text NOT NULL,
    expires_at timestamptz,
    account_label text CHECK (length(account_label) BETWEEN 1 AND 320),
    auth_error text CHECK (auth_error IN ('signed_out')),
    last_used_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    FOREIGN KEY (server_id, org_id) REFERENCES mcp_servers (id, org_id) ON DELETE CASCADE,
    -- A person's own connection goes when they leave the organization.
    FOREIGN KEY (org_id, user_id) REFERENCES org_memberships (org_id, user_id) ON DELETE CASCADE,
    CONSTRAINT mcp_connections_header CHECK ((kind = 'header') = (header_name IS NOT NULL)),
    CONSTRAINT mcp_connections_oauth CHECK ((kind = 'oauth') = (issuer IS NOT NULL))
);

CREATE UNIQUE INDEX mcp_connections_shared ON mcp_connections (server_id) WHERE user_id IS NULL;
CREATE UNIQUE INDEX mcp_connections_person ON mcp_connections (server_id, user_id) WHERE user_id IS NOT NULL;
CREATE INDEX mcp_connections_user ON mcp_connections (user_id) WHERE user_id IS NOT NULL;
CREATE INDEX mcp_connections_key_id ON mcp_connections (key_id);
CREATE INDEX mcp_connections_refresh ON mcp_connections (expires_at)
    WHERE kind = 'oauth' AND auth_error IS NULL;

CREATE TABLE mcp_tools (
    id uuid PRIMARY KEY,
    server_id uuid NOT NULL,
    org_id uuid NOT NULL,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 256),
    title text CHECK (length(title) <= 256),
    description text NOT NULL DEFAULT '',
    input_schema jsonb NOT NULL CHECK (jsonb_typeof(input_schema) = 'object'),
    output_schema jsonb CHECK (jsonb_typeof(output_schema) = 'object'),
    annotations jsonb CHECK (jsonb_typeof(annotations) = 'object'),
    meta jsonb CHECK (jsonb_typeof(meta) = 'object'),
    definition_hash bytea NOT NULL CHECK (length(definition_hash) = 32),
    approved_hash bytea CHECK (length(approved_hash) = 32),
    approved jsonb CHECK (jsonb_typeof(approved) = 'object'),
    state text NOT NULL CHECK (state IN ('active', 'changed', 'removed')),
    first_seen_at timestamptz NOT NULL,
    changed_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (server_id, name),
    CONSTRAINT mcp_tools_id_server UNIQUE (id, server_id),
    FOREIGN KEY (server_id, org_id) REFERENCES mcp_servers (id, org_id) ON DELETE CASCADE,
    CONSTRAINT mcp_tools_approved CHECK ((approved_hash IS NULL) = (approved IS NULL))
);

CREATE INDEX mcp_tools_org ON mcp_tools (org_id);
CREATE INDEX mcp_tools_changed ON mcp_tools (server_id) WHERE state = 'changed';

CREATE TABLE project_mcp_servers (
    project_id uuid NOT NULL,
    org_id uuid NOT NULL,
    server_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, server_id),
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    FOREIGN KEY (server_id, org_id) REFERENCES mcp_servers (id, org_id) ON DELETE CASCADE
);

CREATE INDEX project_mcp_servers_server ON project_mcp_servers (server_id);

CREATE TABLE project_tools (
    project_id uuid NOT NULL,
    server_id uuid NOT NULL,
    tool_id uuid NOT NULL,
    enabled boolean NOT NULL DEFAULT false,
    confirm boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, tool_id),
    FOREIGN KEY (project_id, server_id) REFERENCES project_mcp_servers (project_id, server_id) ON DELETE CASCADE,
    FOREIGN KEY (tool_id, server_id) REFERENCES mcp_tools (id, server_id) ON DELETE CASCADE
);

CREATE INDEX project_tools_tool ON project_tools (tool_id);

CREATE TABLE tool_access (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    membership_id uuid REFERENCES project_memberships (id) ON DELETE CASCADE,
    service_account_id uuid REFERENCES service_accounts (id) ON DELETE CASCADE,
    api_key_id uuid REFERENCES api_keys (id) ON DELETE CASCADE,
    tool_id uuid NOT NULL REFERENCES mcp_tools (id) ON DELETE CASCADE,
    CHECK (num_nonnulls(membership_id, service_account_id, api_key_id) = 1)
);

CREATE UNIQUE INDEX tool_access_membership ON tool_access (membership_id, tool_id) WHERE membership_id IS NOT NULL;
CREATE UNIQUE INDEX tool_access_service_account ON tool_access (service_account_id, tool_id) WHERE service_account_id IS NOT NULL;
CREATE UNIQUE INDEX tool_access_key ON tool_access (api_key_id, tool_id) WHERE api_key_id IS NOT NULL;
CREATE INDEX tool_access_project ON tool_access (project_id);
CREATE INDEX tool_access_tool ON tool_access (tool_id);

ALTER TABLE api_keys
    ADD COLUMN tools boolean NOT NULL DEFAULT false,
    ADD COLUMN tools_restricted boolean NOT NULL DEFAULT false,
    DROP CONSTRAINT api_keys_check1,
    ADD CONSTRAINT api_keys_abilities CHECK (inference OR platform OR tools);

ALTER TABLE project_memberships ADD COLUMN tools_restricted boolean NOT NULL DEFAULT false;
ALTER TABLE service_accounts ADD COLUMN tools_restricted boolean NOT NULL DEFAULT false;

ALTER TABLE agent_clients
    ADD COLUMN metadata_url text UNIQUE CHECK (length(metadata_url) <= 2048 AND metadata_url ~ '^https://'),
    ADD COLUMN metadata_fetched_at timestamptz;

ALTER TABLE agent_requests ADD COLUMN resource text CHECK (length(resource) <= 2048);

CREATE TABLE portal_grants (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL,
    client_id uuid NOT NULL REFERENCES agent_clients (id) ON DELETE CASCADE,
    redirect_uri text NOT NULL CHECK (length(redirect_uri) <= 2048),
    created_at timestamptz NOT NULL,
    last_used_at timestamptz,
    revoked_at timestamptz,
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    -- Leaving the organization ends every portal grant in it.
    FOREIGN KEY (org_id, user_id) REFERENCES org_memberships (org_id, user_id) ON DELETE CASCADE
);

CREATE INDEX portal_grants_project ON portal_grants (project_id, created_at) WHERE revoked_at IS NULL;
CREATE INDEX portal_grants_user ON portal_grants (user_id) WHERE revoked_at IS NULL;

CREATE TABLE portal_tokens (
    id uuid PRIMARY KEY,
    grant_id uuid NOT NULL REFERENCES portal_grants (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('code', 'access', 'refresh')),
    token_hash bytea NOT NULL UNIQUE,
    family uuid NOT NULL,
    resource text NOT NULL CHECK (length(resource) BETWEEN 1 AND 2048),
    redirect_uri text,
    code_challenge text,
    created_at timestamptz NOT NULL,
    expires_at timestamptz,
    used_at timestamptz,
    revoked_at timestamptz,
    CHECK ((kind = 'code') = (code_challenge IS NOT NULL AND redirect_uri IS NOT NULL))
);

CREATE INDEX portal_tokens_grant ON portal_tokens (grant_id, family);
CREATE INDEX portal_tokens_expiry ON portal_tokens (expires_at);

ALTER TABLE sign_in_tokens DROP CONSTRAINT sign_in_tokens_kind_check;

ALTER TABLE sign_in_tokens ADD CONSTRAINT sign_in_tokens_kind_check CHECK (kind IN (
    'setup', 'magic_link', 'password_reset', 'email_change', 'email_change_cancel',
    'totp_challenge', 'signup', 'oidc_state', 'provider_oauth', 'provider_device', 'mcp_oauth'
));
