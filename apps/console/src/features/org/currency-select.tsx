import { useQuery } from "@tanstack/react-query";
import { Button, Select } from "@logicplanes/kaeso-ui";
import type { ExchangeRates } from "~/api/generated/types";
import { exchangeRatesQuery } from "~/api/queries/exchange-rates";
import { USD } from "~/format/currency";
import { currencyName } from "~/format/prices";
import { t } from "~/i18n";

/** Why the install can only show US dollars, or nothing while it has other rates. */
function onlyDollars(rates: ExchangeRates | undefined): string | undefined {
  if (!rates || rates.rates.some((r) => r.currency !== USD)) return undefined;
  return rates.fetching
    ? t("kaeso.currency.noRates", "No exchange rates yet: Kaeso fetches the ECB's once a working day")
    : t("kaeso.currency.ratesOff", "Exchange rates are off on this install");
}

export function CurrencySelect({ value, onChange, disabled }: { value: string; onChange: (code: string) => void; disabled?: boolean | undefined }) {
  const rates = useQuery(exchangeRatesQuery);
  const none = onlyDollars(rates.data);
  if (none && value === USD)
    return (
      <Button size="sm" variant="ghost" disabled disabledReason={none}>
        {USD}
      </Button>
    );
  const listed = (rates.data?.rates ?? []).map((r) => r.currency);
  const codes = [...new Set([USD, ...listed, value])];
  return (
    <Select
      aria-label={t("kaeso.currency.label", "Currency")}
      size="sm"
      value={value}
      disabled={!!disabled || !rates.data}
      onValueChange={onChange}
      options={codes.map((code) => ({ value: code, label: code, description: currencyName(code) }))}
    />
  );
}
