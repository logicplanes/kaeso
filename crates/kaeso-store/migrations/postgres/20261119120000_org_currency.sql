-- The currency an organization reads and enters money in. Kaeso still stores and costs spend in
-- US dollars; spend is converted at each day's ECB rate when it is read.
ALTER TABLE organizations
    ADD COLUMN currency text NOT NULL DEFAULT 'USD' CHECK (currency ~ '^[A-Z]{3}$');

-- The currency is the organization's now, not each person's.
ALTER TABLE users DROP COLUMN display_currency;

-- A spend budget keeps its amount in the currency it was set in and is checked at the latest rate.
ALTER TABLE limits ADD COLUMN currency text CHECK (currency ~ '^[A-Z]{3}$');
UPDATE limits SET currency = 'USD' WHERE kind = 'spend_budget';
ALTER TABLE limits ADD CONSTRAINT limits_currency_spend_only
    CHECK ((kind = 'spend_budget') = (currency IS NOT NULL));
