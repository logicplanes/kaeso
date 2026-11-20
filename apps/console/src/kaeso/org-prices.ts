import { useQuery } from "@tanstack/react-query";
import { exchangeRatesQuery } from "~/api/queries/exchange-rates";
import { USD } from "~/format/currency";
import { pricesIn, type Prices } from "~/format/prices";
import { useOrgMoney } from "./org-money";

/** Model prices in the organization's currency, at the latest rate. */
export function useOrgPrices(org: string): Prices {
  const money = useOrgMoney(org);
  const rates = useQuery({ ...exchangeRatesQuery, enabled: money.code !== USD });
  return pricesIn(money, rates.data?.rates.find((r) => r.currency === money.code)?.per_usd);
}
