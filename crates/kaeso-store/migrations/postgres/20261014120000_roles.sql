CREATE TABLE org_roles (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    scope text NOT NULL CHECK (scope IN ('org', 'project')),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 50),
    description text NOT NULL DEFAULT '' CHECK (length(description) <= 200),
    rights text[] NOT NULL DEFAULT '{}',
    project_rights text[] NOT NULL DEFAULT '{}',
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (scope = 'org' OR project_rights = '{}')
);

CREATE UNIQUE INDEX org_roles_name ON org_roles (org_id, scope, lower(name));

ALTER TABLE org_memberships DROP CONSTRAINT org_memberships_preset_check;
ALTER TABLE org_memberships
    ADD CONSTRAINT org_memberships_preset_check
        CHECK (preset IN ('owner', 'admin', 'billing', 'developer', 'auditor', 'member', 'custom')),
    ADD COLUMN role_id uuid REFERENCES org_roles (id) ON DELETE RESTRICT,
    ADD CONSTRAINT org_memberships_role_check CHECK ((preset = 'custom') = (role_id IS NOT NULL));
CREATE INDEX org_memberships_role ON org_memberships (role_id) WHERE role_id IS NOT NULL;

ALTER TABLE project_memberships DROP CONSTRAINT project_memberships_preset_check;
ALTER TABLE project_memberships
    ADD CONSTRAINT project_memberships_preset_check
        CHECK (preset IN ('admin', 'developer', 'viewer', 'custom')),
    ADD COLUMN role_id uuid REFERENCES org_roles (id) ON DELETE RESTRICT,
    ADD CONSTRAINT project_memberships_role_check CHECK ((preset = 'custom') = (role_id IS NOT NULL));
CREATE INDEX project_memberships_role ON project_memberships (role_id) WHERE role_id IS NOT NULL;

ALTER TABLE service_accounts DROP CONSTRAINT service_accounts_preset_check;
ALTER TABLE service_accounts
    ADD CONSTRAINT service_accounts_preset_check
        CHECK (preset IN ('admin', 'developer', 'viewer', 'custom')),
    ADD COLUMN role_id uuid REFERENCES org_roles (id) ON DELETE SET NULL,
    ADD CONSTRAINT service_accounts_role_check CHECK (role_id IS NULL OR preset = 'custom');
CREATE INDEX service_accounts_role ON service_accounts (role_id) WHERE role_id IS NOT NULL;

ALTER TABLE invites DROP CONSTRAINT invites_org_preset_check;
ALTER TABLE invites
    ADD CONSTRAINT invites_org_preset_check
        CHECK (org_preset IN ('admin', 'billing', 'developer', 'auditor', 'member', 'owner', 'custom')),
    ADD COLUMN org_role_id uuid REFERENCES org_roles (id) ON DELETE SET NULL,
    ADD CONSTRAINT invites_org_role_check CHECK (org_role_id IS NULL OR org_preset = 'custom');

ALTER TABLE organizations DROP CONSTRAINT organizations_sso_default_preset_check;
ALTER TABLE organizations
    ADD CONSTRAINT organizations_sso_default_preset_check
        CHECK (sso_default_preset IN ('admin', 'billing', 'developer', 'auditor', 'member', 'custom')),
    ADD COLUMN sso_default_role_id uuid REFERENCES org_roles (id) ON DELETE RESTRICT,
    ADD CONSTRAINT organizations_sso_default_role_check
        CHECK ((sso_default_preset = 'custom') = (sso_default_role_id IS NOT NULL));

UPDATE org_memberships
SET rights_added = array_append(rights_added, 'members.2fa.reset')
WHERE 'security.2fa.require' = ANY (rights_added)
  AND NOT 'members.2fa.reset' = ANY (rights_added);
UPDATE org_memberships
SET rights_removed = array_append(rights_removed, 'members.2fa.reset')
WHERE preset = 'admin'
  AND 'security.2fa.require' = ANY (rights_removed)
  AND NOT 'members.2fa.reset' = ANY (rights_removed);

ALTER TABLE audit_events ADD COLUMN project_id uuid;
CREATE INDEX audit_events_project ON audit_events (org_id, project_id, id) WHERE project_id IS NOT NULL;
