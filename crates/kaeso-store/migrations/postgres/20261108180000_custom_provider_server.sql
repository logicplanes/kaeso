ALTER TABLE custom_providers
    ADD COLUMN server text CHECK (server ~ '^[a-z0-9]+(_[a-z0-9]+)*$' AND length(server) <= 40);
