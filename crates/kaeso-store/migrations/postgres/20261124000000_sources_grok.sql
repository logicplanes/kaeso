ALTER TABLE sources DROP CONSTRAINT sources_kind_check;
ALTER TABLE sources ADD CONSTRAINT sources_kind_check CHECK (kind IN (
    'claude_code', 'codex', 'gemini_cli', 'antigravity', 'grok', 'opentelemetry', 'http', 'litellm', 'openrouter'
));
