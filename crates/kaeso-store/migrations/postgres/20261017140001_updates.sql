CREATE TABLE update_settings (
    id smallint PRIMARY KEY CHECK (id = 1),
    check_enabled boolean NOT NULL,
    channel text NOT NULL CHECK (channel IN ('stable', 'prerelease')),
    apply_enabled boolean NOT NULL,
    etag text,
    last_checked_at timestamptz,
    last_success_at timestamptz,
    last_error text,
    next_check_at timestamptz,
    blocked_until timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE update_releases (
    version text PRIMARY KEY CHECK (length(version) BETWEEN 5 AND 100),
    published_at timestamptz NOT NULL,
    prerelease boolean NOT NULL,
    notes text NOT NULL,
    url text NOT NULL,
    manifest jsonb,
    manifest_error text
);

CREATE TABLE update_jobs (
    id uuid PRIMARY KEY,
    from_version text NOT NULL,
    to_version text NOT NULL,
    state text NOT NULL CHECK (state IN ('running', 'succeeded', 'failed', 'rolled_back', 'rollback_failed', 'needs_attention')),
    step text NOT NULL CHECK (step IN ('verifying', 'downloading', 'switching', 'migrating', 'online')),
    reason text,
    detail text,
    started_by uuid REFERENCES users (id) ON DELETE SET NULL,
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    CHECK ((state = 'running') = (finished_at IS NULL))
);

CREATE UNIQUE INDEX update_jobs_one_running ON update_jobs ((true)) WHERE state = 'running';

CREATE INDEX update_jobs_started_at ON update_jobs (started_at DESC, id DESC);
