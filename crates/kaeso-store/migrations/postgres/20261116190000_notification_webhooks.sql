-- Webhooks an organization registers for its operational notices, and the outbox of what each
-- of them is sent. The signing secret is sealed with the master keys, as provider keys are.
CREATE TABLE notification_webhooks (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    url text NOT NULL CHECK (length(url) BETWEEN 1 AND 2048 AND url ~ '^https?://'),
    format text NOT NULL CHECK (format IN ('json', 'slack')),
    kinds text[] NOT NULL CHECK (cardinality(kinds) BETWEEN 1 AND 20),
    ciphertext bytea NOT NULL,
    nonce bytea NOT NULL,
    wrapped_key bytea NOT NULL,
    key_id text NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    -- Why Kaeso turned it off itself; null while it is on or after a person turned it off.
    disabled_reason text CHECK (disabled_reason IN ('failures')),
    consecutive_failures integer NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    last_status integer,
    last_error text,
    last_attempt_at timestamptz,
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX notification_webhooks_org ON notification_webhooks (org_id);

CREATE TABLE webhook_deliveries (
    id uuid PRIMARY KEY,
    webhook_id uuid NOT NULL REFERENCES notification_webhooks (id) ON DELETE CASCADE,
    -- The event's id, the same in every try: the receiver's key for telling repeats apart.
    message_id uuid NOT NULL,
    event_type text NOT NULL CHECK (length(event_type) BETWEEN 1 AND 100),
    body text NOT NULL,
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'delivered', 'failed')),
    last_status integer,
    last_error text,
    created_at timestamptz NOT NULL,
    finished_at timestamptz
);

CREATE INDEX webhook_deliveries_due ON webhook_deliveries (next_attempt_at) WHERE status = 'pending';
CREATE INDEX webhook_deliveries_of_webhook ON webhook_deliveries (webhook_id, created_at DESC);
CREATE INDEX webhook_deliveries_created ON webhook_deliveries (created_at);
