-- The sign-in that approved a grant, so an organization's two-factor and SSO requirements hold
-- for its portal tools as they do for the person's sessions and agents. Grants approved before
-- this was kept count as a password sign-in without a second factor.
ALTER TABLE portal_grants
    ADD COLUMN sign_in_method text NOT NULL DEFAULT 'password',
    ADD COLUMN second_factor boolean NOT NULL DEFAULT false,
    ADD COLUMN oidc_org_id uuid;
