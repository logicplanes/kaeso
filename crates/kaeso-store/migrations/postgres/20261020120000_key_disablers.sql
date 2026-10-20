ALTER TABLE api_keys
    ADD COLUMN disabled_by_kind text CHECK (disabled_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    ADD COLUMN disabled_by_id uuid;

ALTER TABLE org_platform_keys
    ADD COLUMN disabled_by_kind text CHECK (disabled_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    ADD COLUMN disabled_by_id uuid;
