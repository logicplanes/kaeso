ALTER TABLE catalogue_models
    ADD COLUMN availability text CHECK (availability IN ('available', 'unavailable')),
    ADD COLUMN availability_checked_at timestamptz,
    ADD CONSTRAINT catalogue_models_availability_checked
        CHECK ((availability IS NULL) = (availability_checked_at IS NULL));
