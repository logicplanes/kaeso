ALTER TABLE update_settings
    ADD COLUMN mode text NOT NULL DEFAULT 'manual' CHECK (mode IN ('manual', 'automatic')),
    ADD COLUMN auto_frequency text NOT NULL DEFAULT 'daily' CHECK (auto_frequency IN ('daily', 'weekly')),
    ADD COLUMN auto_weekday smallint NOT NULL DEFAULT 1 CHECK (auto_weekday BETWEEN 1 AND 7),
    ADD COLUMN auto_hour smallint NOT NULL DEFAULT 3 CHECK (auto_hour BETWEEN 0 AND 23),
    ADD COLUMN auto_window_at timestamptz,
    DROP COLUMN apply_enabled,
    DROP COLUMN check_enabled;

ALTER TABLE update_jobs ADD COLUMN automatic boolean NOT NULL DEFAULT false;
