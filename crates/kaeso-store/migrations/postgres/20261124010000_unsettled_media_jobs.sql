-- A background job asks providers about started videos and image generations nobody has
-- billed yet, so the ones an app never asks about are billed too. `checked_at` is when Kaeso
-- last asked, so the job takes the ones asked about least recently.
ALTER TABLE videos ADD COLUMN checked_at timestamptz;
ALTER TABLE image_generations ADD COLUMN checked_at timestamptz;

CREATE INDEX videos_unsettled ON videos (checked_at NULLS FIRST, created_at) WHERE accounted_at IS NULL;
CREATE INDEX image_generations_unsettled ON image_generations (checked_at NULLS FIRST, created_at) WHERE accounted_at IS NULL;
