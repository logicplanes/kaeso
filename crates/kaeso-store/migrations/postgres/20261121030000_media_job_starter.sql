-- A video says whether a member started it in the Playground instead of Kaeso guessing it from a
-- missing key, and a video or image generation keeps the id of the key that started it, so its
-- finished entry is logged with that key even once the key is gone.
ALTER TABLE videos ADD COLUMN playground BOOLEAN NOT NULL DEFAULT false;
UPDATE videos SET playground = true WHERE api_key_id IS NULL AND user_id IS NOT NULL;
ALTER TABLE videos ALTER COLUMN playground DROP DEFAULT;

ALTER TABLE videos DROP CONSTRAINT videos_api_key_id_fkey;
ALTER TABLE image_generations DROP CONSTRAINT image_generations_api_key_id_fkey;
