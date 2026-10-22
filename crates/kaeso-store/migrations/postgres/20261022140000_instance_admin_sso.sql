ALTER TABLE instance_settings
    ADD COLUMN admin_sso_org_id uuid REFERENCES organizations (id) ON DELETE SET NULL;
