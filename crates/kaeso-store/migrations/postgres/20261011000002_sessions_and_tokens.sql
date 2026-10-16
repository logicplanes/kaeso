CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    token_hash bytea NOT NULL UNIQUE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    method text NOT NULL CHECK (method IN ('password', 'magic_link', 'oidc', 'setup', 'signup')),
    second_factor boolean NOT NULL,
    oidc_org_id uuid,
    signed_in_at timestamptz NOT NULL,
    last_used_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    ip inet,
    user_agent text,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX sessions_user ON sessions (user_id) WHERE revoked_at IS NULL;
CREATE INDEX sessions_expiry ON sessions (expires_at);

CREATE TABLE sign_in_tokens (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN (
        'setup', 'magic_link', 'password_reset', 'email_change', 'totp_challenge', 'signup', 'oidc_state'
    )),
    token_hash bytea NOT NULL UNIQUE,
    user_id uuid REFERENCES users (id) ON DELETE CASCADE,
    email text,
    data jsonb NOT NULL DEFAULT '{}',
    expires_at timestamptz NOT NULL,
    used_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX sign_in_tokens_expiry ON sign_in_tokens (expires_at);
CREATE INDEX sign_in_tokens_user ON sign_in_tokens (user_id, kind) WHERE used_at IS NULL;
