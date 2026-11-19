-- A model line such as `claude-opus-latest` is a catalogue model of its own, so aliases, model
-- access and limits name it like any model; `line_target` is the provider's model it calls now.
ALTER TABLE catalogue_models ADD COLUMN line_target text;

-- How a project follows each line it added: the target before the current one and since when
-- the line points where it does, the policy, a move KPMR announced that waits (for approval or
-- for the delay), the target an owner chose to keep, and why the line couldn't move.
CREATE TABLE catalogue_lines (
    catalogue_model_id uuid PRIMARY KEY REFERENCES catalogue_models (id) ON DELETE CASCADE,
    org_id uuid NOT NULL,
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    provider text NOT NULL,
    line text NOT NULL,
    previous text,
    since timestamptz NOT NULL,
    policy text NOT NULL DEFAULT 'follow' CHECK (policy IN ('follow', 'ask', 'delay', 'pin')),
    delay_days integer CHECK (delay_days BETWEEN 1 AND 30),
    pending text,
    pending_since timestamptz,
    kept text,
    held text,
    updated_at timestamptz NOT NULL,
    CHECK ((policy = 'delay') = (delay_days IS NOT NULL)),
    CHECK ((pending IS NULL) = (pending_since IS NULL))
);

CREATE INDEX catalogue_lines_by_line ON catalogue_lines (provider, line);
