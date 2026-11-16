-- Roles and project memberships from the identity provider's groups: an organization reads a
-- claim of the ID token and maps its values to roles with ordered rules.
CREATE TABLE sso_group_settings (
    org_id uuid PRIMARY KEY REFERENCES organizations (id) ON DELETE CASCADE,
    claim text NOT NULL DEFAULT 'groups' CHECK (length(claim) BETWEEN 1 AND 100),
    scopes text[] NOT NULL DEFAULT '{}',
    -- `add` never takes anything away; `sync` also removes what a rule gave once it no longer
    -- matches.
    mode text NOT NULL DEFAULT 'add' CHECK (mode IN ('add', 'sync')),
    -- The last sign-in whose ID token carried no such claim, which changed nothing.
    claim_missing_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sso_group_rules (
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    position integer NOT NULL CHECK (position BETWEEN 0 AND 199),
    group_value text NOT NULL CHECK (length(group_value) BETWEEN 1 AND 256),
    org_preset text CHECK (org_preset IN ('admin', 'billing', 'developer', 'auditor', 'member', 'custom')),
    org_role_id uuid REFERENCES org_roles (id) ON DELETE CASCADE,
    -- `[{"project_id": uuid, "preset": text, "role_id": uuid | null}]`
    projects jsonb NOT NULL DEFAULT '[]',
    PRIMARY KEY (org_id, position),
    CHECK ((org_preset = 'custom') = (org_role_id IS NOT NULL))
);

-- Whether a rule gave the role or membership; a change by hand clears it, and only what a rule
-- gave is ever removed by one.
ALTER TABLE org_memberships ADD COLUMN from_sso_group boolean NOT NULL DEFAULT false;
ALTER TABLE project_memberships ADD COLUMN from_sso_group boolean NOT NULL DEFAULT false;

-- The groups the last sign-in through the organization's provider carried, for its admins to
-- find the exact values.
ALTER TABLE sso_identities ADD COLUMN last_groups text[];
