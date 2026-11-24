import { useQuery } from "@tanstack/react-query";
import { Text } from "@logicplanes/kaeso-ui";
import { exchangeRatesQuery } from "~/api/queries/exchange-rates";
import { USD, type Money } from "~/format/currency";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";

/** Which rates turned the dollars Kaeso counts into the organization's currency; nothing to say in dollars. */
export function RatesNote({ money, from }: { money: Money; from?: string | undefined }) {
  const rates = useQuery({ ...exchangeRatesQuery, enabled: money.code !== USD });
  if (money.code === USD || !rates.data?.day || !rates.data.first_day) return null;
  const values = { currency: money.code, day: fullDayLabel(rates.data.day), first: fullDayLabel(rates.data.first_day) };
  return (
    <Text as="p" variant="caption" tone="tertiary">
      {from && from < rates.data.first_day
        ? t("kaeso.currency.noteBefore", "In {currency} at each day's ECB rate; before {first} at the rate of {day}.", values)
        : t("kaeso.currency.note", "In {currency} at each day's ECB rate; the latest is of {day}.", values)}
    </Text>
  );
}
