ALTER TABLE org_oidc
    ADD COLUMN display_name text CHECK (char_length(display_name) BETWEEN 1 AND 50),
    ADD COLUMN auto_redirect boolean NOT NULL DEFAULT false;
