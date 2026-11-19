import { queryOptions, type QueryClient } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ExchangeRates } from "../generated/types";
import { meQuery } from "./me";

export const USD = "USD";

export const exchangeRatesQuery = queryOptions({
  queryKey: ["exchange-rates"],
  queryFn: async (): Promise<ExchangeRates> => data(await api.GET("/platform/v1/me/exchange-rates")),
  staleTime: 10 * 60_000,
});

export function displayCode(code: string | undefined, rates: ExchangeRates | undefined): string | undefined {
  if (!code || code === USD || !rates?.day || !rates.first_day || !rates.rates.some((r) => r.currency === code)) return undefined;
  return code;
}

/** The display currency from what the client already holds, so a prefetch asks for what the page will. */
export function cachedDisplayCode(client: QueryClient): string | undefined {
  return displayCode(client.getQueryData(meQuery.queryKey)?.display_currency, client.getQueryData(exchangeRatesQuery.queryKey));
}
