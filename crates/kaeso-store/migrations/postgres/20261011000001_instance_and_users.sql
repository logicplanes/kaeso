CREATE TABLE instance_settings (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    org_creation text NOT NULL CHECK (org_creation IN ('instance_admins', 'allowed_domains', 'everyone')),
    metadata_retention_days integer NOT NULL CHECK (metadata_retention_days > 0),
    content_retention_days integer NOT NULL CHECK (content_retention_days > 0),
    max_metadata_retention_days integer NOT NULL CHECK (max_metadata_retention_days > 0),
    max_content_retention_days integer NOT NULL CHECK (max_content_retention_days > 0),
    setup_completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    org_deletion boolean NOT NULL DEFAULT true,
    org_deletion_grace_days integer NOT NULL DEFAULT 7 CHECK (org_deletion_grace_days BETWEEN 1 AND 90),
    account_creation text CHECK (account_creation IN ('invitation_only', 'allowed_domains', 'everyone')),
    allowed_domains text[] NOT NULL DEFAULT '{}',
    CHECK (content_retention_days <= metadata_retention_days),
    CHECK (metadata_retention_days <= max_metadata_retention_days),
    CHECK (content_retention_days <= max_content_retention_days),
    CONSTRAINT instance_settings_allowed_domains_used CHECK (
        cardinality(allowed_domains) > 0
        OR (account_creation IS DISTINCT FROM 'allowed_domains' AND org_creation <> 'allowed_domains')
    )
);

CREATE TABLE avatars (
    hash text PRIMARY KEY,
    content_type text NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/webp')),
    data bytea NOT NULL CHECK (octet_length(data) <= 524288),
    created_at timestamptz NOT NULL DEFAULT now()
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
    avatar text REFERENCES avatars (hash),
    locale text CHECK (locale IN ('en', 'de', 'zh-CN')),
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
