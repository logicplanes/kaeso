-- An Azure key can be a Microsoft Entra ID service principal: its tenant, client id and client
-- secret sealed as one JSON secret, the tenant in `account_label`, and the resource's `endpoint`
-- like a resource key. `auth_error` holds `signed_out` once Entra ID refuses its secret. The
-- checks keep every earlier method, the AWS ones included.
ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok', 'google_service_account', 'aws_access_key',
        'aws_role', 'azure_service_principal'));

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_account;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_account CHECK (
    (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL
        AND project IS NULL)
    OR (auth_method = 'google_service_account' AND expires_at IS NULL AND account_label IS NOT NULL
        AND auth_error IS NULL AND project IS NOT NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method IN ('chatgpt', 'grok') AND expires_at IS NOT NULL AND account_label IS NOT NULL
        AND project IS NULL AND location IS NULL AND account_id IS NULL)
    OR (auth_method IN ('aws_access_key', 'aws_role') AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method = 'azure_service_principal' AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NULL
        AND account_id IS NULL AND endpoint IS NOT NULL)
);

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_endpoint_key;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_endpoint_key CHECK (
    endpoint IS NULL
        OR (auth_method IN ('api_key', 'azure_service_principal') AND account_id IS NULL)
);
