import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { ExchangeRates } from "../generated/types";

export const exchangeRatesQuery = queryOptions({
  queryKey: ["exchange-rates"],
  queryFn: async (): Promise<ExchangeRates> => data(await api.GET("/platform/v1/me/exchange-rates")),
  staleTime: 10 * 60_000,
});
