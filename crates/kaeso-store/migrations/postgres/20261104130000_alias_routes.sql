ALTER TABLE aliases ADD COLUMN route jsonb;

UPDATE aliases a SET route = jsonb_build_object(
    'type', 'model',
    'model', 'model_' || (
        SELECT string_agg(substr('0123456789abcdefghjkmnpqrstvwxyz', substring(bits.b FROM i * 5 + 1 FOR 5)::bit(5)::int + 1, 1), '' ORDER BY i)
        FROM generate_series(0, 25) AS i,
             LATERAL (SELECT B'00' || ('x' || replace(a.catalogue_model_id::text, '-', ''))::bit(128) AS b) AS bits
    ),
    'fallback', NULL,
    'fall_back_on', jsonb_build_array('unavailable', 'limited', 'unsupported'),
    'timeout_seconds', NULL
);

ALTER TABLE aliases
    ALTER COLUMN route SET NOT NULL,
    ADD CONSTRAINT aliases_route_object CHECK (jsonb_typeof(route) = 'object');

DROP INDEX aliases_target;
ALTER TABLE aliases DROP COLUMN catalogue_model_id;
