-- lower() under the database's collation depends on the C library and locale; pg_c_utf8 doesn't,
-- and its simple lowercase is kaeso_core::email::canonical's full one except for U+0130.
CREATE FUNCTION email_key(address text) RETURNS text
    LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
    RETURN normalize(lower(replace(normalize(address, NFKC), U&'\0130', U&'i\0307') COLLATE pg_c_utf8), NFKC);

CREATE INDEX users_email_canonical ON users (email_key(email)) WHERE email IS NOT NULL;

DROP INDEX invites_email;
CREATE INDEX invites_email ON invites (email_key(email)) WHERE accepted_at IS NULL AND revoked_at IS NULL;
