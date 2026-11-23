ALTER TABLE mcp_servers ADD COLUMN auto_approve_tool_changes boolean NOT NULL DEFAULT true;
ALTER TABLE mcp_servers DROP COLUMN change_policy;

CREATE TABLE mcp_tool_changes (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL,
    server_id uuid NOT NULL,
    tool_id uuid NOT NULL,
    from_hash bytea NOT NULL CHECK (length(from_hash) = 32),
    to_hash bytea NOT NULL CHECK (length(to_hash) = 32),
    detected_at timestamptz NOT NULL,
    outcome text CHECK (outcome IN ('auto_approved', 'approved', 'kept_off', 'superseded', 'restored')),
    decided_by_kind text CHECK (decided_by_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    decided_by_id uuid,
    decided_at timestamptz,
    FOREIGN KEY (server_id, org_id) REFERENCES mcp_servers (id, org_id) ON DELETE CASCADE,
    FOREIGN KEY (tool_id, server_id) REFERENCES mcp_tools (id, server_id) ON DELETE CASCADE,
    CONSTRAINT mcp_tool_changes_decided CHECK ((outcome IS NULL) = (decided_at IS NULL))
);

CREATE INDEX mcp_tool_changes_server ON mcp_tool_changes (server_id, detected_at DESC, id DESC);
CREATE INDEX mcp_tool_changes_open ON mcp_tool_changes (tool_id)
    WHERE outcome IS NULL OR outcome = 'kept_off';

-- Changes held before this migration get a history entry, so the next check, which approves
-- them now that every server auto-approves, closes one.
INSERT INTO mcp_tool_changes (id, org_id, server_id, tool_id, from_hash, to_hash, detected_at)
SELECT gen_random_uuid(), org_id, server_id, id, approved_hash, definition_hash, COALESCE(changed_at, updated_at)
FROM mcp_tools
WHERE state = 'changed' AND approved_hash IS NOT NULL;
