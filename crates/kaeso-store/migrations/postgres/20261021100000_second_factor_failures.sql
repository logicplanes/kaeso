ALTER TABLE sign_in_tokens ADD COLUMN attempts integer NOT NULL DEFAULT 0;
ALTER TABLE user_totp ADD COLUMN failed_codes integer NOT NULL DEFAULT 0;
