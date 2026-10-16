ALTER TABLE instance_settings
    ADD COLUMN account_creation text
        CHECK (account_creation IN ('invitation_only', 'allowed_domains', 'everyone')),
    ADD COLUMN allowed_domains text[] NOT NULL DEFAULT '{}';

ALTER TABLE instance_settings DROP CONSTRAINT instance_settings_org_creation_check;
ALTER TABLE instance_settings ADD CONSTRAINT instance_settings_org_creation_check
    CHECK (org_creation IN ('instance_admins', 'allowed_domains', 'everyone'));

ALTER TABLE instance_settings ADD CONSTRAINT instance_settings_allowed_domains_used
    CHECK (
        cardinality(allowed_domains) > 0
        OR (account_creation IS DISTINCT FROM 'allowed_domains' AND org_creation <> 'allowed_domains')
    );
