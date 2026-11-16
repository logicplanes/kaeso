CREATE TABLE guardrails (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    url text NOT NULL CHECK (length(url) BETWEEN 1 AND 2048 AND url ~ '^https?://'),
    secret_ciphertext bytea NOT NULL,
    secret_nonce bytea NOT NULL,
    secret_wrapped_key bytea NOT NULL,
    secret_key_id text NOT NULL,
    timeout_ms integer NOT NULL DEFAULT 2000 CHECK (timeout_ms BETWEEN 100 AND 10000),
    on_error text NOT NULL DEFAULT 'block' CHECK (on_error IN ('block', 'allow')),
    request_phase boolean NOT NULL DEFAULT true,
    reply_phase boolean NOT NULL DEFAULT false,
    streams text NOT NULL DEFAULT 'skip' CHECK (streams IN ('skip', 'hold')),
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    CONSTRAINT guardrails_id_org UNIQUE (id, org_id),
    CONSTRAINT guardrails_phase CHECK (request_phase OR reply_phase)
);

CREATE INDEX guardrails_org ON guardrails (org_id, name);
CREATE INDEX guardrails_secret_key ON guardrails (secret_key_id);

CREATE TABLE project_guardrails (
    project_id uuid NOT NULL,
    org_id uuid NOT NULL,
    guardrail_id uuid NOT NULL,
    position integer NOT NULL CHECK (position >= 0),
    PRIMARY KEY (project_id, guardrail_id),
    UNIQUE (project_id, position),
    FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE,
    FOREIGN KEY (guardrail_id, org_id) REFERENCES guardrails (id, org_id) ON DELETE CASCADE
);

CREATE INDEX project_guardrails_guardrail ON project_guardrails (guardrail_id);
