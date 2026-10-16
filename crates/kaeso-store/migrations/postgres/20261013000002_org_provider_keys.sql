ALTER TABLE provider_credentials ADD COLUMN is_default boolean NOT NULL DEFAULT false;

UPDATE provider_credentials SET is_default = true WHERE project_id IS NULL;

ALTER TABLE provider_credentials
    ADD CONSTRAINT provider_credentials_id_org_provider UNIQUE (id, org_id, provider);
ALTER TABLE projects ADD CONSTRAINT projects_id_org UNIQUE (id, org_id);

CREATE TABLE project_provider_keys (
    project_id uuid NOT NULL,
    provider text NOT NULL,
    org_id uuid NOT NULL,
    credential_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, provider),
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    FOREIGN KEY (credential_id, org_id, provider)
        REFERENCES provider_credentials (id, org_id, provider) ON DELETE CASCADE
);

CREATE INDEX project_provider_keys_credential ON project_provider_keys (credential_id);
CREATE INDEX project_provider_keys_org ON project_provider_keys (org_id);

INSERT INTO project_provider_keys (project_id, provider, org_id, credential_id, created_at)
SELECT project_id, provider, org_id, id, created_at FROM provider_credentials WHERE project_id IS NOT NULL;

UPDATE provider_credentials c SET name = left(p.name || ' · ' || c.name, 100)
FROM projects p WHERE p.id = c.project_id;

DROP INDEX provider_credentials_project;
DROP INDEX provider_credentials_org;
ALTER TABLE provider_credentials DROP COLUMN project_id;

CREATE UNIQUE INDEX provider_credentials_default ON provider_credentials (org_id, provider) WHERE is_default;
CREATE INDEX provider_credentials_org ON provider_credentials (org_id, provider, created_at);

UPDATE project_memberships SET
    rights_added = array_remove(array_remove(rights_added, 'provider_credentials.project.view'), 'provider_credentials.project.manage'),
    rights_removed = array_remove(array_remove(rights_removed, 'provider_credentials.project.view'), 'provider_credentials.project.manage')
WHERE rights_added && ARRAY['provider_credentials.project.view', 'provider_credentials.project.manage']
   OR rights_removed && ARRAY['provider_credentials.project.view', 'provider_credentials.project.manage'];

UPDATE service_accounts SET
    rights_added = array_remove(array_remove(rights_added, 'provider_credentials.project.view'), 'provider_credentials.project.manage'),
    rights_removed = array_remove(array_remove(rights_removed, 'provider_credentials.project.view'), 'provider_credentials.project.manage')
WHERE rights_added && ARRAY['provider_credentials.project.view', 'provider_credentials.project.manage']
   OR rights_removed && ARRAY['provider_credentials.project.view', 'provider_credentials.project.manage'];
