ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok', 'google_service_account'));

ALTER TABLE provider_credentials
    ADD COLUMN project text CHECK (project ~ '^([a-z0-9.-]{1,63}:)?[a-z][a-z0-9-]{4,28}[a-z0-9]$'),
    ADD COLUMN location text CHECK (location ~ '^(global|us|eu|[a-z]+-[a-z]+[0-9]+)$');

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_account;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_account CHECK (
    (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL
        AND project IS NULL)
    OR (auth_method = 'google_service_account' AND expires_at IS NULL AND account_label IS NOT NULL
        AND auth_error IS NULL AND project IS NOT NULL AND location IS NOT NULL)
    OR (auth_method IN ('chatgpt', 'grok') AND expires_at IS NOT NULL AND account_label IS NOT NULL
        AND project IS NULL AND location IS NULL)
);
