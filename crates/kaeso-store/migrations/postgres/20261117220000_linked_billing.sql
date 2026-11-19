-- Linked billing: a provider key that is a link to the customer's own organization at the
-- provider. The install signs its client assertions with one P-256 key of its own, sealed like
-- provider keys; each provider that offers linked billing has at most one OAuth app per install.
CREATE TABLE linked_billing_keys (
    kid text PRIMARY KEY CHECK (kid ~ '^[A-Za-z0-9_-]{1,64}$'),
    ciphertext bytea NOT NULL,
    nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE linked_billing_apps (
    provider text PRIMARY KEY CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    client_id text NOT NULL CHECK (length(client_id) BETWEEN 1 AND 200),
    audience text NOT NULL CHECK (length(audience) BETWEEN 1 AND 500),
    organization_id text NOT NULL CHECK (length(organization_id) BETWEEN 1 AND 100),
    federation_rule_id text NOT NULL CHECK (length(federation_rule_id) BETWEEN 1 AND 100),
    workspace_id text CHECK (length(workspace_id) BETWEEN 1 AND 100),
    service_account_id text CHECK (length(service_account_id) BETWEEN 1 AND 100),
    send_billing_policy boolean NOT NULL DEFAULT false,
    check_profile_id text CHECK (length(check_profile_id) BETWEEN 1 AND 100),
    checked_at timestamptz,
    check_problem text CHECK (check_problem ~ '^[a-z_]{1,64}$'),
    updated_by uuid REFERENCES users (id) ON DELETE SET NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Anthropic bills a user profile in the install's own organization; each Kaeso organization
-- gets one per app, reused by every link it makes.
CREATE TABLE linked_billing_profiles (
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    client_id text NOT NULL,
    user_profile_id text NOT NULL CHECK (length(user_profile_id) BETWEEN 1 AND 100),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, provider, client_id)
);

-- A project whose linked key can't pay (its spend limit is reached or the link is gone) can fall
-- back to another key of the same provider.
CREATE TABLE linked_billing_fallbacks (
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

CREATE INDEX linked_billing_fallbacks_credential ON linked_billing_fallbacks (credential_id);

-- `auth_error` of a linked key: `unlinked` once the link is gone, `needs_setup` while the install's
-- OAuth app is missing or changed. `spend_limited_until` is set by a spend-limit refusal and
-- cleared by the next request the linked organization pays for.
ALTER TABLE provider_credentials ADD COLUMN spend_limited_until timestamptz;

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok', 'google_service_account', 'aws_access_key',
        'aws_role', 'azure_service_principal', 'linked_billing'));

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_account;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_account CHECK (
    (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL
        AND project IS NULL)
    OR (auth_method = 'google_service_account' AND expires_at IS NULL AND account_label IS NOT NULL
        AND auth_error IS NULL AND project IS NOT NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method IN ('chatgpt', 'grok', 'linked_billing') AND expires_at IS NOT NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NULL AND account_id IS NULL)
    OR (auth_method IN ('aws_access_key', 'aws_role') AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method = 'azure_service_principal' AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NULL
        AND account_id IS NULL AND endpoint IS NOT NULL)
);

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_error_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_error_check
    CHECK (auth_error IN ('signed_out', 'no_api_access')
        OR (auth_method = 'linked_billing' AND auth_error IN ('unlinked', 'needs_setup', 'on_hold')));

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_spend_limited
    CHECK (spend_limited_until IS NULL OR auth_method = 'linked_billing');

-- Which provider key paid for a day's requests, so usage can be read by who was billed.
ALTER TABLE usage_daily ADD COLUMN credential_id uuid;

DROP INDEX usage_daily_row;

CREATE UNIQUE INDEX usage_daily_row ON usage_daily
    (project_id, day, api_key_id, user_id, service_account_id, provider, model_id, ingress_dialect,
        egress_dialect, credential_id)
    NULLS NOT DISTINCT;
