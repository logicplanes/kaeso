ALTER TABLE update_releases ADD COLUMN art jsonb CHECK (art IS NULL OR (jsonb_typeof(art) = 'object' AND length(art::text) <= 200));
