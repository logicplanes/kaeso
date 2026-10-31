ALTER TABLE models_dev_snapshot RENAME TO kpmr_snapshot;
ALTER TABLE kpmr_snapshot RENAME CONSTRAINT models_dev_snapshot_pkey TO kpmr_snapshot_pkey;
ALTER TABLE kpmr_snapshot RENAME CONSTRAINT models_dev_snapshot_id_check TO kpmr_snapshot_id_check;
ALTER TABLE kpmr_snapshot ADD COLUMN etag text;

UPDATE agents SET instance_rights = array_remove(instance_rights, 'instance.model_sources.manage')
WHERE 'instance.model_sources.manage' = ANY (instance_rights);

CREATE TEMPORARY TABLE kpmr_registry (provider text, model_id text, values jsonb) ON COMMIT DROP;
INSERT INTO kpmr_registry VALUES
    ('anthropic', 'claude-haiku-4-5', '{"adaptive_thinking": false, "efforts": []}'),
    ('anthropic', 'claude-haiku-4-5-20251001', '{"adaptive_thinking": false, "efforts": []}'),
    ('anthropic', 'claude-sonnet-4-5', '{"adaptive_thinking": false, "efforts": []}'),
    ('anthropic', 'claude-sonnet-4-5-20250929', '{"adaptive_thinking": false, "efforts": []}'),
    ('anthropic', 'claude-opus-4-5', '{"adaptive_thinking": false, "efforts": ["low", "medium", "high"]}'),
    ('anthropic', 'claude-opus-4-5-20251101', '{"adaptive_thinking": false, "efforts": ["low", "medium", "high"]}'),
    ('anthropic', 'claude-opus-4-6', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "max"]}'),
    ('anthropic', 'claude-sonnet-4-6', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "max"]}'),
    ('anthropic', 'claude-opus-4-7', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"]}'),
    ('anthropic', 'claude-opus-4-8', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"]}'),
    ('anthropic', 'claude-opus-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"]}'),
    ('anthropic', 'claude-sonnet-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"]}'),
    ('anthropic', 'claude-fable-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"]}'),
    ('anthropic', 'claude-opus-5-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"], "forced_tool_choice": false, "thinking_binding": true}'),
    ('anthropic', 'claude-sonnet-5-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"], "forced_tool_choice": false, "thinking_binding": true}'),
    ('anthropic', 'claude-haiku-5-5', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"], "thinking_binding": true}'),
    ('anthropic', 'claude-fable-5-1', '{"adaptive_thinking": true, "efforts": ["low", "medium", "high", "xhigh", "max"], "forced_tool_choice": false, "thinking_binding": true}'),
    ('openai', 'text-embedding-3-small', '{"egress_dialect": "openai_embeddings"}'),
    ('openai', 'text-embedding-3-large', '{"egress_dialect": "openai_embeddings"}'),
    ('openai', 'text-embedding-ada-002', '{"egress_dialect": "openai_embeddings"}'),
    ('openai', 'gpt-5.4-mini', '{"egress_dialect": "openai_responses"}'),
    ('openai', 'gpt-6-astra', '{"egress_dialect": "openai_responses"}'),
    ('openai', 'gpt-5.6-terra', '{"egress_dialect": "openai_responses"}'),
    ('mistral', 'mistral-embed', '{"egress_dialect": "openai_embeddings"}'),
    ('cohere', 'command-a-plus-05-2026', '{"efforts": ["none", "high"]}'),
    ('cohere', 'command-a-reasoning-08-2025', '{"efforts": ["none", "high"]}'),
    ('cohere', 'north-mini-code-1-0', '{"efforts": ["none", "high"]}');

CREATE FUNCTION pg_temp.kpmr_price(v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE jsonb_typeof(v) WHEN 'string' THEN v WHEN 'number' THEN to_jsonb(v #>> '{}') ELSE 'null'::jsonb END
$$;

CREATE FUNCTION pg_temp.kpmr_efforts(m jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN jsonb_typeof(m -> 'reasoning_options') = 'array' THEN
        (SELECT o -> 'values' FROM jsonb_array_elements(m -> 'reasoning_options') o WHERE o ->> 'type' = 'effort' LIMIT 1)
    END
$$;

CREATE FUNCTION pg_temp.kpmr_model(provider text, m jsonb, r jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
    SELECT jsonb_build_object(
        'id', m -> 'id',
        'name', m -> 'name',
        'family', m -> 'family',
        'kind', COALESCE(NULLIF(m -> 'type', 'null'::jsonb), '"generation"'),
        'status', COALESCE(NULLIF(m -> 'status', 'null'::jsonb), '"active"'),
        'release_date', m -> 'release_date',
        'modalities', m -> 'modalities',
        'limits', jsonb_build_object('context', m -> 'limit' -> 'context', 'input', m -> 'limit' -> 'input', 'output', m -> 'limit' -> 'output'),
        'pricing', CASE WHEN jsonb_typeof(m -> 'cost') = 'object' THEN jsonb_build_object(
            'unit', 'usd_per_million_tokens',
            'input', pg_temp.kpmr_price(m -> 'cost' -> 'input'),
            'output', pg_temp.kpmr_price(m -> 'cost' -> 'output'),
            'cache_read', pg_temp.kpmr_price(m -> 'cost' -> 'cache_read'),
            'cache_write', pg_temp.kpmr_price(m -> 'cost' -> 'cache_write'),
            'cache_write_1h', CASE WHEN provider = 'anthropic' AND (m -> 'cost' ->> 'input') IS NOT NULL
                THEN to_jsonb(((m -> 'cost' ->> 'input')::numeric * 2)::text) ELSE 'null'::jsonb END
        ) END,
        'capabilities', jsonb_build_object(
            'tool_call', m -> 'tool_call',
            'forced_tool_choice', r -> 'forced_tool_choice',
            'structured_output', m -> 'structured_output',
            'temperature', m -> 'temperature',
            'attachments', m -> 'attachment',
            'reasoning', m -> 'reasoning'
        ),
        'reasoning', CASE WHEN (m ->> 'reasoning') = 'true' OR r ?| ARRAY['efforts', 'adaptive_thinking', 'thinking_binding'] THEN jsonb_build_object(
            'efforts', COALESCE(r -> 'efforts', pg_temp.kpmr_efforts(m)),
            'adaptive_thinking', r -> 'adaptive_thinking',
            'thinking_binding', r -> 'thinking_binding'
        ) END,
        'gateway', jsonb_build_object('egress_dialect', COALESCE(r -> 'egress_dialect',
            CASE WHEN m ->> 'type' = 'embedding' THEN '"openai_embeddings"'::jsonb END)),
        'origin', 'models_dev',
        'corrected', '[]'::jsonb
    )
$$;

UPDATE kpmr_snapshot s SET format = NULL, data = jsonb_build_object(
    'revision', NULL,
    'generated_at', NULL,
    'refresh_seconds', NULL,
    'provider_ids', '[]'::jsonb,
    'providers', COALESCE((
        SELECT jsonb_object_agg(p.key, jsonb_build_object(
            'id', p.value -> 'id',
            'name', p.value -> 'name',
            'models', COALESCE((
                SELECT jsonb_object_agg(m.key, pg_temp.kpmr_model(p.key, m.value, COALESCE(r.values, '{}'::jsonb)))
                FROM jsonb_each(p.value -> 'models') m
                LEFT JOIN kpmr_registry r ON r.provider = p.key AND r.model_id = m.key
            ), '{}'::jsonb)
        ))
        FROM jsonb_each(s.data -> 'providers') p
    ), '{}'::jsonb)
);

UPDATE catalogue_models c SET last_known = pg_temp.kpmr_model(c.provider, c.last_known, COALESCE((
    SELECT r.values FROM kpmr_registry r WHERE r.provider = c.provider AND r.model_id = c.model_id
), '{}'::jsonb))
WHERE jsonb_typeof(c.last_known) = 'object';
