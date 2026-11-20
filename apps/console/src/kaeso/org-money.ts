import { useQuery } from "@tanstack/react-query";
import { membershipsQuery } from "~/auth/session-queries";
import { moneyIn, USD, type Money } from "~/format/currency";

/** The currency the organization shows money in; US dollars where the person has no membership to read it from. */
export function useOrgCurrency(org: string): string {
  const memberships = useQuery(membershipsQuery);
  return memberships.data?.find((m) => m.org_id === org)?.org_currency ?? USD;
}

export function useOrgMoney(org: string): Money {
  return moneyIn(useOrgCurrency(org));
}
