-- The European Central Bank's euro foreign exchange reference rates, one row per currency and
-- working day: how many units of the currency one euro buys. Kaeso stores and calculates spend in
-- US dollars and uses these only at the edges, for prices entered in another currency and for
-- spend shown in one.
CREATE TABLE exchange_rates (
    currency text NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    day date NOT NULL,
    per_eur numeric NOT NULL CHECK (per_eur > 0),
    PRIMARY KEY (currency, day)
);

-- Whether the gateway fetches the rates, and how its last fetch went.
ALTER TABLE instance_settings
    ADD COLUMN exchange_rates boolean NOT NULL DEFAULT true,
    ADD COLUMN exchange_rates_checked_at timestamptz,
    ADD COLUMN exchange_rates_error text;

-- The currency a person reads spend in, next to US dollars; NULL is US dollars only.
ALTER TABLE users ADD COLUMN display_currency text CHECK (display_currency ~ '^[A-Z]{3}$');
