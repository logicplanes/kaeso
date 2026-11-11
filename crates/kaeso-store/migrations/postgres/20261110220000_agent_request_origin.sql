-- A hash of the network an agent sign-in request came from, so the cap on a client's pending
-- requests counts each network apart and one network can't push out everyone else's.
ALTER TABLE agent_requests ADD COLUMN requested_from bytea;
