CREATE TABLE instance_settings (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    org_creation text NOT NULL CHECK (org_creation IN ('instance_admins', 'everyone')),
    metadata_retention_days integer NOT NULL CHECK (metadata_retention_days > 0),
    content_retention_days integer NOT NULL CHECK (content_retention_days > 0),
    max_metadata_retention_days integer NOT NULL CHECK (max_metadata_retention_days > 0),
    max_content_retention_days integer NOT NULL CHECK (max_content_retention_days > 0),
    setup_completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (content_retention_days <= metadata_retention_days),
    CHECK (metadata_retention_days <= max_metadata_retention_days),
    CHECK (content_retention_days <= max_content_retention_days)
);

CREATE TABLE users (
    id uuid PRIMARY KEY,
    email text,
    name text,
    password_hash text,
    email_verified_at timestamptz,
    state text NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'deactivated', 'deleted')),
    is_instance_admin boolean NOT NULL DEFAULT false,
    notify_budget_alerts boolean NOT NULL DEFAULT true,
    notify_key_expiry boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CHECK ((state = 'deleted') = (email IS NULL)),
    CHECK ((state = 'deleted') = (deleted_at IS NOT NULL)),
    CHECK (state = 'deleted' OR name IS NOT NULL)
);

CREATE UNIQUE INDEX users_email_key ON users (lower(email)) WHERE email IS NOT NULL;
CREATE INDEX users_instance_admins ON users (id) WHERE is_instance_admin;

CREATE TABLE user_totp (
    user_id uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    secret_ciphertext bytea NOT NULL,
    secret_nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    enabled_at timestamptz,
    last_used_step bigint,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX user_totp_key_id ON user_totp (key_id);

CREATE TABLE user_recovery_codes (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash bytea NOT NULL,
    used_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX user_recovery_codes_user ON user_recovery_codes (user_id) WHERE used_at IS NULL;
