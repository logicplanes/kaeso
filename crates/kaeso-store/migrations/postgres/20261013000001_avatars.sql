CREATE TABLE avatars (
    hash text PRIMARY KEY,
    content_type text NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/webp')),
    data bytea NOT NULL CHECK (octet_length(data) <= 524288),
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE users ADD COLUMN avatar text REFERENCES avatars (hash);
ALTER TABLE organizations ADD COLUMN avatar text REFERENCES avatars (hash);
