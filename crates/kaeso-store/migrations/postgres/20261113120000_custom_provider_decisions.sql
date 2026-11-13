ALTER TABLE custom_providers DROP CONSTRAINT custom_providers_media_check;

-- A missing key reads as NULL, which a CHECK lets through, so the shape test is COALESCEd to false.
ALTER TABLE custom_providers
    ADD CONSTRAINT custom_providers_media_check
        CHECK (media <@ ARRAY['images', 'speech', 'transcriptions', 'videos', 'decisions']::text[]),
    ADD COLUMN decisions jsonb,
    ADD CONSTRAINT custom_providers_decisions_check CHECK (
        (decisions IS NOT NULL) = ('decisions' = ANY (media))
        AND (
            decisions IS NULL
            OR COALESCE(
                jsonb_typeof(decisions) = 'object'
                AND decisions - ARRAY['format', 'path', 'media'] = '{}'::jsonb
                AND jsonb_typeof(decisions -> 'path') = 'string'
                AND length(decisions ->> 'path') <= 200
                AND decisions ->> 'path' ~ '^(/[A-Za-z0-9._~!$&''()*+,;=:@%-]+)+$'
                AND decisions ->> 'path' !~ '/\.\.?(/|$)'
                AND CASE decisions ->> 'format'
                    WHEN 'system_one' THEN
                        jsonb_typeof(decisions -> 'media') = 'string'
                        AND decisions ->> 'media' IN ('assets', 'state', 'root', 'none')
                    WHEN 'openai_decisions' THEN NOT decisions ? 'media'
                    ELSE false
                END,
                false
            )
        )
    );
