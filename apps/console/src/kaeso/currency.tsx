import { useQuery } from "@tanstack/react-query";
import { Button, Select, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { ExchangeRates } from "~/api/generated/types";
import { displayCode, exchangeRatesQuery, USD } from "~/api/queries/exchange-rates";
import { meQuery } from "~/api/queries/me";
import { useApiMutation } from "~/auth/mutation";
import { currencyName } from "~/format/currency";
import { t } from "~/i18n";

/** The currency a person reads spend in next to US dollars, while the install has a rate for it. */
export interface DisplayCurrency {
  code: string;
  /** The day of the latest rates. */
  day: string;
  /** The day of the oldest rates; spend before it is converted at the latest. */
  firstDay: string;
}

/** The display currency, and whether it is known yet, so usage isn't fetched once without it and again with it. */
export function useDisplayChoice(): { display: DisplayCurrency | undefined; settled: boolean } {
  const me = useQuery(meQuery);
  const rates = useQuery(exchangeRatesQuery);
  const settled = !me.isPending && !rates.isPending;
  const code = displayCode(me.data?.display_currency, rates.data);
  if (!code || !rates.data?.day || !rates.data.first_day) return { display: undefined, settled };
  return { display: { code, day: rates.data.day, firstDay: rates.data.first_day }, settled };
}

export function useDisplayCurrency(): DisplayCurrency | undefined {
  return useDisplayChoice().display;
}

function onlyDollars(rates: ExchangeRates | undefined): string | undefined {
  if (!rates || rates.rates.some((r) => r.currency !== USD)) return undefined;
  return rates.fetching
    ? t("kaeso.currency.noRates", "No exchange rates yet: Kaeso fetches the ECB's once a working day")
    : t("kaeso.currency.ratesOff", "Exchange rates are off on this install");
}

export function CurrencyPicker({ size = "sm" }: { size?: "sm" | "md" }) {
  const me = useQuery(meQuery);
  const rates = useQuery(exchangeRatesQuery);
  const choose = useApiMutation({
    mutationFn: async (code: string) => data(await api.PATCH("/platform/v1/me", { body: { display_currency: code } })),
    invalidates: [["me"]],
  });
  const value = choose.isPending && choose.variables ? choose.variables : (me.data?.display_currency ?? USD);
  const reason = onlyDollars(rates.data);
  const codes = [USD, ...(rates.data?.rates ?? []).map((r) => r.currency).filter((c) => c !== USD)];
  if (reason)
    return (
      <Button size={size} variant="ghost" disabled disabledReason={reason}>
        {USD}
      </Button>
    );
  return (
    <Select
      aria-label={t("kaeso.currency.label", "Currency")}
      size={size}
      value={value}
      disabled={choose.isPending}
      onValueChange={(code) => choose.mutate(code)}
      options={codes.map((code) => ({
        value: code,
        label: code,
        description: code === USD ? t("kaeso.currency.usdOnly", "US dollars only") : currencyName(code),
      }))}
    />
  );
}

export function RatesNote({ display, from }: { display: DisplayCurrency; from?: string | undefined }) {
  const values = { currency: display.code, day: formatDate(display.day), first: formatDate(display.firstDay) };
  return (
    <Text as="p" variant="caption" tone="tertiary">
      {from && from < display.firstDay
        ? t("kaeso.currency.noteBefore", "≈ {currency} at each day's ECB rate; before {first} at the rate of {day}.", values)
        : t("kaeso.currency.note", "≈ {currency} at each day's ECB rate; the latest is of {day}.", values)}
    </Text>
  );
}
