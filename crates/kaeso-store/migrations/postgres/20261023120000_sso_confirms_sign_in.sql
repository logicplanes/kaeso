ALTER TABLE org_oidc RENAME COLUMN sends_auth_time TO confirms_sign_in;
UPDATE org_oidc SET confirms_sign_in = NULL WHERE confirms_sign_in;
