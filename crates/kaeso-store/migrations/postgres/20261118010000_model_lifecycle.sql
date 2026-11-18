-- What a project does when a model it uses shuts down: move its aliases and route steps to the
-- replacement, and answer requests that name the retired model with the replacement. Both off
-- unless the project turns them on; a project without a row has both off.
CREATE TABLE project_model_lifecycle (
    project_id uuid PRIMARY KEY REFERENCES projects (id) ON DELETE CASCADE,
    move_on_shutdown boolean NOT NULL DEFAULT false,
    serve_replacement boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- The lifecycle notices a project got for a model, so each goes out once: the announcement,
-- the warnings 30, 7 and 1 days before the shutdown, and the move on the shutdown day.
CREATE TABLE model_lifecycle_notices (
    project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    provider text NOT NULL,
    model_id text NOT NULL,
    stage text NOT NULL CHECK (stage IN ('deprecated', 'shutdown_30', 'shutdown_7', 'shutdown_1', 'moved')),
    shutdown_on date,
    sent_at timestamptz NOT NULL,
    PRIMARY KEY (project_id, provider, model_id, stage)
);
