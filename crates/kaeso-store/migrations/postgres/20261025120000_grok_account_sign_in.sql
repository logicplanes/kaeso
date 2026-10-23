ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok'));

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_error_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_error_check
    CHECK (auth_error IN ('signed_out', 'no_api_access'));

ALTER TABLE sign_in_tokens DROP CONSTRAINT sign_in_tokens_kind_check;

ALTER TABLE sign_in_tokens ADD CONSTRAINT sign_in_tokens_kind_check CHECK (kind IN (
    'setup', 'magic_link', 'password_reset', 'email_change', 'email_change_cancel',
    'totp_challenge', 'signup', 'oidc_state', 'provider_oauth', 'provider_device'
));
