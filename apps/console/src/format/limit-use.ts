import { formatNumber, formatTokens, formatUSD } from "@logicplanes/kaeso-ui/format";

export type LimitFigureText = { short: string; exact: string };

type Usd = { decimals?: number | undefined };

// Counts go compact, as the console writes tokens everywhere else, so a budget of a hundred million tokens fits beside its meter; money stays exact.
export function limitAmount(kind: string, value: string | number, usd: Usd = {}): LimitFigureText {
  if (kind === "spend_budget") {
    const money = formatUSD(String(value), usd);
    return { short: money, exact: money };
  }
  const count = Number(value);
  return { short: formatTokens(count), exact: formatNumber(count, { maximumFractionDigits: 0 }) };
}

export function limitUse(
  limit: { kind: string; used?: string | null | undefined; amount: string },
  message: (values: { used: string; amount: string }) => string,
  usd: Usd = {},
): LimitFigureText {
  const used = limitAmount(limit.kind, limit.used ?? "0", usd);
  const amount = limitAmount(limit.kind, limit.amount, usd);
  return { short: message({ used: used.short, amount: amount.short }), exact: message({ used: used.exact, amount: amount.exact }) };
}
