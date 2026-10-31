ALTER TABLE organizations
    ADD COLUMN response_cache boolean NOT NULL DEFAULT true;

ALTER TABLE projects
    ADD COLUMN response_cache boolean NOT NULL DEFAULT false,
    ADD COLUMN response_cache_ttl_seconds integer NOT NULL DEFAULT 3600
        CHECK (response_cache_ttl_seconds BETWEEN 60 AND 604800),
    ADD COLUMN response_cache_generation bigint NOT NULL DEFAULT 0;
