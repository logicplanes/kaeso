ALTER TABLE custom_providers
    ADD COLUMN media text[] NOT NULL DEFAULT '{}'
        CHECK (media <@ ARRAY['images', 'speech', 'transcriptions', 'videos']::text[]);
