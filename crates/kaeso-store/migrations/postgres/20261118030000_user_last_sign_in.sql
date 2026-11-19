-- When a user last signed in, for instance admins: set by every new session from the sign-in it
-- records, and never moved back by a session that replaces another one. NULL for a user who
-- hasn't signed in since this column exists.
ALTER TABLE users ADD COLUMN last_signed_in_at timestamptz;
