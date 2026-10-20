ALTER TABLE org_oidc ADD COLUMN verification_token text;

UPDATE org_oidc SET verification_token = replace(gen_random_uuid()::text, '-', '');

ALTER TABLE org_oidc ALTER COLUMN verification_token SET NOT NULL;

ALTER TABLE org_oidc_domains DROP CONSTRAINT org_oidc_domains_pkey;

ALTER TABLE org_oidc_domains ADD PRIMARY KEY (org_id, domain);

ALTER TABLE org_oidc_domains ADD COLUMN verified_at timestamptz;

CREATE UNIQUE INDEX org_oidc_domains_verified ON org_oidc_domains (domain) WHERE verified_at IS NOT NULL;
