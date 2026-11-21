-- A finished video or image generation is logged by a later request, so its entry takes these
-- from the job, not from that request. They are cleared once the job is billed.
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
