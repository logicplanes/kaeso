ALTER TABLE provider_credentials
    ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0);
