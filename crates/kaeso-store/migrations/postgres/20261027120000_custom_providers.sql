CREATE TABLE custom_providers (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND length(provider) <= 40),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    base_url text NOT NULL CHECK (length(base_url) BETWEEN 1 AND 2048 AND base_url ~ '^https?://'),
    auth text NOT NULL CHECK (auth IN ('bearer', 'x_api_key', 'none')),
    generation text NOT NULL CHECK (generation IN ('openai_chat', 'openai_responses', 'anthropic_messages')),
    embeddings text CHECK (embeddings IN ('openai_embeddings')),
    count_tokens text CHECK (count_tokens IN ('openai_responses', 'anthropic_messages') AND count_tokens = generation),
    created_by_kind text NOT NULL CHECK (created_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    created_by_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    CONSTRAINT custom_providers_org_provider UNIQUE (org_id, provider)
);
