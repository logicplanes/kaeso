import { formatNumber, formatTokens } from "@logicplanes/kaeso-ui/format";
import { moneyIn } from "./currency";

export type LimitFigureText = { short: string; exact: string };

type Money = { decimals?: number | undefined; currency?: string | null | undefined };

// Counts go compact, as the console writes tokens everywhere else, so a budget of a hundred million tokens fits beside its meter; money stays exact.
export function limitAmount(kind: string, value: string | number, { currency, ...options }: Money = {}): LimitFigureText {
  if (kind === "spend_budget") {
    const money = moneyIn(currency).format(String(value), options);
    return { short: money, exact: money };
  }
  const count = Number(value);
  return { short: formatTokens(count), exact: formatNumber(count, { maximumFractionDigits: 0 }) };
}

export function limitUse(
  limit: { kind: string; used?: string | null | undefined; amount: string; currency?: string | null | undefined },
  message: (values: { used: string; amount: string }) => string,
  options: { decimals?: number | undefined } = {},
): LimitFigureText {
  const money = { ...options, currency: limit.currency };
  const used = limitAmount(limit.kind, limit.used ?? "0", money);
  const amount = limitAmount(limit.kind, limit.amount, money);
  return { short: message({ used: used.short, amount: amount.short }), exact: message({ used: used.exact, amount: amount.exact }) };
}
