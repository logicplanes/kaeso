ALTER TABLE provider_credentials
    ADD COLUMN auth_method text NOT NULL DEFAULT 'api_key' CHECK (auth_method IN ('api_key', 'chatgpt')),
    ADD COLUMN expires_at timestamptz,
    ADD COLUMN account_label text CHECK (length(account_label) BETWEEN 1 AND 320),
    ADD COLUMN auth_error text CHECK (auth_error IN ('signed_out')),
    ADD CONSTRAINT provider_credentials_account CHECK (
        (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL)
        OR (auth_method <> 'api_key' AND expires_at IS NOT NULL AND account_label IS NOT NULL)
    );

ALTER TABLE sign_in_tokens DROP CONSTRAINT sign_in_tokens_kind_check;

ALTER TABLE sign_in_tokens ADD CONSTRAINT sign_in_tokens_kind_check CHECK (kind IN (
    'setup', 'magic_link', 'password_reset', 'email_change', 'email_change_cancel',
    'totp_challenge', 'signup', 'oidc_state', 'provider_oauth'
));

ALTER TABLE instance_settings ADD COLUMN agent_host_id text CHECK (agent_host_id ~ '^urn:uuid:[0-9a-f-]{36}$');
