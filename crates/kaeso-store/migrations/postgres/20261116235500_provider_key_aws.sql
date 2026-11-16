-- An Amazon Bedrock key can sign its requests with AWS Signature Version 4: `aws_access_key` keeps
-- an access key (id, secret and optional session token) sealed as one JSON secret, `aws_role`
-- the role the gateway's own AWS identity assumes, if any. Both name their region in `location`
-- and what they sign in as in `account_label` (the access key id, or the role).
ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok', 'google_service_account', 'aws_access_key', 'aws_role'));

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
);
