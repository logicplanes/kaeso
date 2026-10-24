ALTER TABLE api_keys
    ADD COLUMN rotated_from uuid REFERENCES api_keys (id) ON DELETE SET NULL,
    ADD COLUMN replaced_by uuid REFERENCES api_keys (id) ON DELETE SET NULL;

CREATE INDEX api_keys_rotated_from ON api_keys (rotated_from) WHERE rotated_from IS NOT NULL;
CREATE INDEX api_keys_replaced_by ON api_keys (replaced_by) WHERE replaced_by IS NOT NULL;

ALTER TABLE org_platform_keys
    ADD COLUMN rotated_from uuid REFERENCES org_platform_keys (id) ON DELETE SET NULL,
    ADD COLUMN replaced_by uuid REFERENCES org_platform_keys (id) ON DELETE SET NULL;

CREATE INDEX org_platform_keys_rotated_from ON org_platform_keys (rotated_from) WHERE rotated_from IS NOT NULL;
CREATE INDEX org_platform_keys_replaced_by ON org_platform_keys (replaced_by) WHERE replaced_by IS NOT NULL;
