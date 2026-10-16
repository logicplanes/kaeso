CREATE TABLE log_jobs (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('deletion', 'shrink_metadata', 'shrink_content')),
    from_at timestamptz NOT NULL,
    to_at timestamptz NOT NULL,
    requested_by_kind text NOT NULL CHECK (requested_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    requested_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    done_at timestamptz,
    CHECK (from_at < to_at),
    CONSTRAINT log_jobs_project_org
        FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX log_jobs_pending ON log_jobs (created_at) WHERE done_at IS NULL;
CREATE INDEX log_jobs_org ON log_jobs (org_id, created_at);
