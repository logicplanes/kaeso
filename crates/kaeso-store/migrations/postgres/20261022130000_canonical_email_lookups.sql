CREATE FUNCTION email_key(address text) RETURNS text
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN normalize(lower(normalize(address, NFKC)), NFKC);

CREATE INDEX users_email_canonical ON users (email_key(email)) WHERE email IS NOT NULL;

DROP INDEX invites_email;
CREATE INDEX invites_email ON invites (email_key(email)) WHERE accepted_at IS NULL AND revoked_at IS NULL;
