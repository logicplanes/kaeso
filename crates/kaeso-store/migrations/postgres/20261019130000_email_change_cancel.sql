ALTER TABLE sign_in_tokens DROP CONSTRAINT sign_in_tokens_kind_check;

ALTER TABLE sign_in_tokens ADD CONSTRAINT sign_in_tokens_kind_check CHECK (kind IN (
    'setup', 'magic_link', 'password_reset', 'email_change', 'email_change_cancel',
    'totp_challenge', 'signup', 'oidc_state'
));

UPDATE sign_in_tokens SET used_at = now() WHERE kind = 'email_change' AND used_at IS NULL;
