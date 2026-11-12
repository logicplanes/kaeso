-- A key for a provider whose API path names the account, such as Cloudflare's
-- /client/v4/accounts/{account_id}/ai/v1, carries the account id and, optionally, the gateway its
-- requests go through.
ALTER TABLE provider_credentials
    ADD COLUMN account_id text CHECK (account_id ~ '^[0-9a-f]{32}$'),
    ADD COLUMN gateway_id text CHECK (gateway_id ~ '^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$'),
    ADD CONSTRAINT provider_credentials_gateway_in_account CHECK (gateway_id IS NULL OR account_id IS NOT NULL);

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_account;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_account CHECK (
    (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL
        AND project IS NULL)
    OR (auth_method = 'google_service_account' AND expires_at IS NULL AND account_label IS NOT NULL
        AND auth_error IS NULL AND project IS NOT NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method IN ('chatgpt', 'grok') AND expires_at IS NOT NULL AND account_label IS NOT NULL
        AND project IS NULL AND location IS NULL AND account_id IS NULL)
);
