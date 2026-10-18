ALTER TABLE users ADD COLUMN seen_version text CHECK (length(seen_version) BETWEEN 5 AND 100);
