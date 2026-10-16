ALTER TABLE instance_settings
    ADD COLUMN org_deletion boolean NOT NULL DEFAULT true,
    ADD COLUMN org_deletion_grace_days integer NOT NULL DEFAULT 7
        CHECK (org_deletion_grace_days BETWEEN 1 AND 90);

CREATE INDEX organizations_deletion_due_at ON organizations (deletion_due_at, id)
    WHERE deletion_due_at IS NOT NULL;
