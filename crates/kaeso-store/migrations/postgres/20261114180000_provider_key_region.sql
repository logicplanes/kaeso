-- An API key of a provider whose keys each call one region, such as Amazon Bedrock, keeps the
-- region in `location`: AWS region codes such as `eu-central-1` or `us-gov-west-1`.
ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_location_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_location_check
    CHECK (location ~ '^(global|us|eu|[a-z]+-[a-z]+[0-9]+|[a-z]{2}(-[a-z]+)+-[0-9]+)$');
