CREATE TABLE sso_identities (
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    issuer text,
    subject text,
    linked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id),
    UNIQUE (org_id, issuer, subject),
    CHECK ((issuer IS NULL) = (subject IS NULL)),
    CHECK ((issuer IS NULL) = (linked_at IS NULL))
);

CREATE INDEX sso_identities_user ON sso_identities (user_id);

INSERT INTO sso_identities (org_id, user_id)
SELECT DISTINCT joined.org_id, created.actor_id
FROM audit_events created
JOIN audit_events joined
    ON joined.request_id = created.request_id
    AND joined.at = created.at
    AND joined.actor_id = created.actor_id
    AND joined.scope = 'org'
    AND joined.action IN ('member.join_by_sso', 'invite.accept')
JOIN org_oidc connection ON connection.org_id = joined.org_id
JOIN org_memberships membership ON membership.org_id = joined.org_id AND membership.user_id = created.actor_id
JOIN users account ON account.id = created.actor_id AND account.state = 'active'
WHERE created.action = 'user.create_by_sso'
    AND created.actor_kind = 'user'
    AND created.request_id IS NOT NULL;
