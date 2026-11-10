CREATE TABLE image_generations (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL,
    api_key_id uuid REFERENCES api_keys (id) ON DELETE SET NULL,
    user_id uuid,
    service_account_id uuid,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    credential_id uuid NOT NULL,
    catalogue_model_id uuid,
    model text NOT NULL CHECK (length(model) BETWEEN 1 AND 256),
    upstream_model text NOT NULL CHECK (length(upstream_model) BETWEEN 1 AND 256),
    dialect text NOT NULL CHECK (dialect ~ '^[a-z0-9_]+$'),
    upstream_id text NOT NULL CHECK (length(upstream_id) BETWEEN 1 AND 1024),
    request_id uuid NOT NULL,
    params jsonb NOT NULL,
    prices jsonb,
    status text NOT NULL CHECK (status IN ('pending', 'generating', 'completed', 'failed')),
    content_url text,
    content_mime_type text,
    failure_reason text,
    error_message text,
    units jsonb,
    cost numeric,
    created_at timestamptz NOT NULL,
    completed_at timestamptz,
    accounted_at timestamptz,
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX image_generations_listing ON image_generations (org_id, project_id, created_at DESC, id DESC);
CREATE INDEX image_generations_credential ON image_generations (credential_id);
