-- How many times a request that names a model, or an alias step that sets none, tries the model
-- again after a short-lived failure.
ALTER TABLE projects ADD COLUMN retries smallint NOT NULL DEFAULT 0 CHECK (retries BETWEEN 0 AND 3);
