-- A key for a provider whose keys each belong to a resource with an address of its own, such as an
-- Azure resource's https://{resource}.openai.azure.com, carries that endpoint: `https://` and the
-- host, without a path. Only an API key has one, and never with an account.
ALTER TABLE provider_credentials
    ADD COLUMN endpoint text CHECK (endpoint ~ '^https://[a-z0-9]([a-z0-9.-]*[a-z0-9])?$' AND length(endpoint) <= 2048),
    ADD CONSTRAINT provider_credentials_endpoint_key CHECK (
        endpoint IS NULL OR (auth_method = 'api_key' AND account_id IS NULL)
    );
