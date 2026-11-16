-- Responses a provider keeps for an app that asked it to store them (OpenAI's `store`), so a later
-- request may continue one with `previous_response_id`. Each organization allows it, and each
-- project turns it on; both are off until someone decides to keep conversations at a provider.
ALTER TABLE organizations ADD COLUMN stored_responses boolean NOT NULL DEFAULT false;

ALTER TABLE projects ADD COLUMN stored_responses boolean NOT NULL DEFAULT false;

-- One row per stored response: a response id works for every holder of the provider key, and
-- several projects may share a key, so only the project that stored a response may use it. The id
-- is kept as its SHA-256, never in clear.
CREATE TABLE stored_responses (
    id_hash bytea PRIMARY KEY,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL,
    credential_id uuid NOT NULL,
    provider text NOT NULL,
    model text NOT NULL,
    catalogue_model_id uuid NOT NULL,
    request_id uuid NOT NULL,
    total_tokens bigint NOT NULL CHECK (total_tokens >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE
);

CREATE INDEX stored_responses_project ON stored_responses (project_id, expires_at);
