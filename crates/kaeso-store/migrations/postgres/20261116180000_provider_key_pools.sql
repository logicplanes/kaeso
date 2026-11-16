-- A project may use several keys of one provider, each with a weight from 1 to 100: each caller
-- draws one of them, and a refused or rate-limited key gives way to the next. A project with none
-- assigned uses the provider's default, as before.
ALTER TABLE project_provider_keys DROP CONSTRAINT project_provider_keys_pkey;

ALTER TABLE project_provider_keys ADD PRIMARY KEY (project_id, provider, credential_id);

ALTER TABLE project_provider_keys
    ADD COLUMN weight smallint NOT NULL DEFAULT 1 CHECK (weight BETWEEN 1 AND 100);
