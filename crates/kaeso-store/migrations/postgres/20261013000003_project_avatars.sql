ALTER TABLE projects ADD COLUMN avatar text REFERENCES avatars (hash);
