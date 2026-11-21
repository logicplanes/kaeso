-- A finished video or image generation is logged by whichever request first sees it end; it
-- names the request that started it by these, so tags and client attribution find its cost.
ALTER TABLE videos
    ADD COLUMN tags TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN user_agent TEXT,
    ADD COLUMN alias TEXT,
    ADD COLUMN line TEXT;

ALTER TABLE image_generations
    ADD COLUMN tags TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN user_agent TEXT,
    ADD COLUMN alias TEXT,
    ADD COLUMN line TEXT;
