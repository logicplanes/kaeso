ALTER TABLE organizations ADD COLUMN keep_media boolean NOT NULL DEFAULT false;
ALTER TABLE projects ADD COLUMN keep_media boolean NOT NULL DEFAULT false;
