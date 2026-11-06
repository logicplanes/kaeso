CREATE TABLE notification_settings (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind ~ '^[a-z_]{1,64}$'),
    delivery text NOT NULL CHECK (delivery IN ('now', 'daily', 'off')),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, kind)
);

CREATE TABLE notification_mutes (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, org_id)
);
CREATE INDEX notification_mutes_org ON notification_mutes (org_id);

CREATE TABLE notification_digest (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind ~ '^[a-z_]{1,64}$'),
    org_id uuid REFERENCES organizations (id) ON DELETE CASCADE,
    project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    sent_at timestamptz
);
CREATE INDEX notification_digest_waiting ON notification_digest (user_id, created_at) WHERE sent_at IS NULL;

CREATE TABLE notifications_sent (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind ~ '^[a-z_]{1,64}$'),
    subject text NOT NULL CHECK (length(subject) BETWEEN 1 AND 200),
    sent_on date NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, kind, subject, sent_on)
);
CREATE INDEX notifications_sent_hour ON notifications_sent (user_id, created_at);

INSERT INTO notification_settings (user_id, kind, delivery)
SELECT id, 'budget_alerts', 'off' FROM users WHERE NOT notify_budget_alerts;
INSERT INTO notification_settings (user_id, kind, delivery)
SELECT id, 'key_expiry', 'off' FROM users WHERE NOT notify_key_expiry;

ALTER TABLE users DROP COLUMN notify_budget_alerts, DROP COLUMN notify_key_expiry;
