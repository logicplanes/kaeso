-- Read-only links to an organization's usage, a member's stats, a project's budgets or a
-- project's summary, which people without a Kaeso account can open. Only the token's SHA-256 is
-- kept; the token itself is shown once.
CREATE TABLE shared_links (
    id uuid PRIMARY KEY,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('analytics', 'member', 'budget', 'project_card')),
    member_id uuid REFERENCES users (id) ON DELETE CASCADE,
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 100),
    token_hash bytea NOT NULL UNIQUE,
    created_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    revoked_at timestamptz,
    revoked_by uuid REFERENCES users (id) ON DELETE SET NULL,
    -- A live link shows the last `period_days` days whenever it is opened; a snapshot keeps what
    -- `snapshot` holds, the numbers of `from_day` to `to_day` when it was made.
    live boolean NOT NULL,
    period_days integer CHECK (period_days BETWEEN 1 AND 400),
    from_day date,
    to_day date,
    snapshot jsonb,
    metric text NOT NULL DEFAULT 'requests',
    show_costs boolean NOT NULL,
    hide_members boolean NOT NULL,
    hide_keys boolean NOT NULL,
    hide_projects boolean NOT NULL,
    audience text NOT NULL CHECK (audience IN ('anyone', 'members')),
    views bigint NOT NULL DEFAULT 0,
    last_opened_at timestamptz,
    CHECK (kind <> 'member' OR member_id IS NOT NULL),
    CHECK (kind NOT IN ('budget', 'project_card') OR project_id IS NOT NULL),
    CHECK (live = (period_days IS NOT NULL)),
    CHECK (live OR (from_day IS NOT NULL AND to_day IS NOT NULL AND snapshot IS NOT NULL))
);

CREATE INDEX shared_links_org ON shared_links (org_id, created_at DESC);

-- Who an organization's links may reach (`off`, `members` or `anyone`) and their longest expiry
-- in days; NULL lets a link never expire.
ALTER TABLE organizations
    ADD COLUMN shared_links text NOT NULL DEFAULT 'members'
        CHECK (shared_links IN ('off', 'members', 'anyone')),
    ADD COLUMN shared_link_max_days integer DEFAULT 90
        CHECK (shared_link_max_days IN (1, 7, 30, 90));

-- Whether the install allows shared links at all, and whether their pages say "Made with Kaeso".
ALTER TABLE instance_settings
    ADD COLUMN shared_links boolean NOT NULL DEFAULT true,
    ADD COLUMN share_mark boolean NOT NULL DEFAULT true;
