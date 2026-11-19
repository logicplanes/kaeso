-- Linked billing is gone: organizations pay providers with their own keys again. Its keys go, as
-- deleting each one would: a provider whose default was a link gets its newest other key as the
-- default, and the models checked with a link are checked again.
DROP TABLE linked_billing_fallbacks;
DROP TABLE linked_billing_profiles;
DROP TABLE linked_billing_apps;
DROP TABLE linked_billing_keys;

CREATE TEMPORARY TABLE removed_links ON COMMIT DROP AS
    SELECT org_id, provider, bool_or(is_default) AS was_default
    FROM provider_credentials WHERE auth_method = 'linked_billing'
    GROUP BY org_id, provider;

DELETE FROM provider_credentials WHERE auth_method = 'linked_billing';

UPDATE provider_credentials c SET is_default = true
FROM (
    SELECT DISTINCT ON (k.org_id, k.provider) k.id
    FROM provider_credentials k
    JOIN removed_links r ON r.org_id = k.org_id AND r.provider = k.provider AND r.was_default
    ORDER BY k.org_id, k.provider, k.created_at DESC, k.id DESC
) newest
WHERE c.id = newest.id;

UPDATE catalogue_models m SET availability = NULL, availability_checked_at = NULL
FROM removed_links r
WHERE m.org_id = r.org_id AND m.provider = r.provider AND m.availability IS NOT NULL;

DELETE FROM notifications_sent WHERE subject LIKE 'linked\_billing\_setup:%';

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_spend_limited;

ALTER TABLE provider_credentials DROP COLUMN spend_limited_until;

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_method_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_method_check
    CHECK (auth_method IN ('api_key', 'chatgpt', 'grok', 'google_service_account', 'aws_access_key',
        'aws_role', 'azure_service_principal'));

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_account;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_account CHECK (
    (auth_method = 'api_key' AND expires_at IS NULL AND account_label IS NULL AND auth_error IS NULL
        AND project IS NULL)
    OR (auth_method = 'google_service_account' AND expires_at IS NULL AND account_label IS NOT NULL
        AND auth_error IS NULL AND project IS NOT NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method IN ('chatgpt', 'grok') AND expires_at IS NOT NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NULL AND account_id IS NULL)
    OR (auth_method IN ('aws_access_key', 'aws_role') AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NOT NULL
        AND account_id IS NULL)
    OR (auth_method = 'azure_service_principal' AND expires_at IS NULL
        AND account_label IS NOT NULL AND project IS NULL AND location IS NULL
        AND account_id IS NULL AND endpoint IS NOT NULL)
);

ALTER TABLE provider_credentials DROP CONSTRAINT provider_credentials_auth_error_check;

ALTER TABLE provider_credentials ADD CONSTRAINT provider_credentials_auth_error_check
    CHECK (auth_error IN ('signed_out', 'no_api_access'));
