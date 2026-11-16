-- OpenAI's Batch and Files APIs for a project: each file and batch has a Kaeso id of its own and
-- maps to the provider's, with the key that made it and the provider key it lives under.
CREATE TABLE batch_files (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL,
    api_key_id uuid,
    user_id uuid,
    service_account_id uuid,
    credential_id uuid NOT NULL,
    provider text NOT NULL,
    upstream_id text NOT NULL,
    -- `batch` for an input file the app uploaded, `batch_output` for a batch's output or errors.
    purpose text NOT NULL CHECK (purpose IN ('batch', 'batch_output')),
    filename text NOT NULL,
    bytes bigint NOT NULL CHECK (bytes >= 0),
    lines integer NOT NULL CHECK (lines >= 0),
    -- What every line of an input file asks for: one endpoint and one model, as OpenAI requires.
    endpoint text,
    catalogue_model_id uuid,
    model text,
    upstream_model text,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    UNIQUE (provider, upstream_id)
);

CREATE INDEX batch_files_project ON batch_files (project_id, created_at DESC, id DESC);

CREATE TABLE batches (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL,
    api_key_id uuid,
    user_id uuid,
    service_account_id uuid,
    credential_id uuid NOT NULL,
    provider text NOT NULL,
    upstream_id text NOT NULL,
    input_file_id uuid NOT NULL,
    output_file_id uuid,
    error_file_id uuid,
    endpoint text NOT NULL,
    catalogue_model_id uuid NOT NULL,
    model text NOT NULL,
    upstream_model text NOT NULL,
    -- The model's prices at the batch price, read when the batch was created.
    prices jsonb,
    metadata jsonb,
    -- The provider's last answer about the batch, as Kaeso decoded it.
    status text NOT NULL,
    view jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- When its lines' usage was recorded, once it ended.
    accounted_at timestamptz,
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    UNIQUE (provider, upstream_id)
);

CREATE INDEX batches_project ON batches (project_id, created_at DESC, id DESC);
CREATE INDEX batches_open ON batches (created_at) WHERE accounted_at IS NULL;
