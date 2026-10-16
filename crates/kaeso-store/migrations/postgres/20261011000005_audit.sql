CREATE TABLE audit_events (
    id uuid PRIMARY KEY,
    scope text NOT NULL CHECK (scope IN ('instance', 'org')),
    org_id uuid,
    actor_kind text NOT NULL CHECK (actor_kind IN ('user', 'org_platform_key', 'api_key', 'system')),
    actor_id uuid,
    action text NOT NULL,
    target_kind text,
    target_id text,
    before jsonb,
    after jsonb,
    ip inet,
    user_agent text,
    request_id uuid,
    at timestamptz NOT NULL,
    CHECK ((scope = 'org') = (org_id IS NOT NULL)),
    CHECK ((actor_kind = 'system') = (actor_id IS NULL))
);

CREATE INDEX audit_events_org ON audit_events (org_id, id) WHERE scope = 'org';
CREATE INDEX audit_events_instance ON audit_events (id) WHERE scope = 'instance';

CREATE FUNCTION audit_events_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' AND current_setting('kaeso.audit_purge', true) = 'on' THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'audit_events is append-only (% refused)', TG_OP;
END;
$$;

CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION audit_events_append_only();

CREATE TRIGGER audit_events_no_truncate
    BEFORE TRUNCATE ON audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit_events_append_only();
