-- The expiry warnings sent for an org's platform keys, 7 days and 1 day before, like
-- key_expiry_warnings for API keys: one row per key and warning, so each goes out once.
CREATE TABLE platform_key_expiry_warnings (
    key_id uuid NOT NULL REFERENCES org_platform_keys (id) ON DELETE CASCADE,
    days_before integer NOT NULL CHECK (days_before IN (1, 7)),
    sent_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (key_id, days_before)
);
