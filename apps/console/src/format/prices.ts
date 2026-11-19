import { formatLocale, formatPrice, formatUSD } from "@logicplanes/kaeso-ui/format";
import { USD, type Money } from "./currency";

/** Dollar prices, as the console writes them in a currency at its latest rate. */
export interface Prices {
  money: Money;
  /** The dollar amount in the currency, or null without a rate. */
  convert(usd: string): string | null;
  /** A price per million tokens. */
  price(usd: string): string;
  /** A price per unit, such as an image or a web search. */
  unit(usd: string): string;
  /** A price in a list: whole amounts without decimals. */
  short(usd: string): string;
}

const keptPrices = new Map<string, Prices>();

/** `perUsd` is how much of the currency a dollar buys; without it a price can't be shown in the currency. */
export function pricesIn(money: Money, perUsd: string | undefined): Prices {
  const key = `${money.code}|${perUsd ?? ""}`;
  let p = keptPrices.get(key);
  if (!p) {
    p = writePrices(money, perUsd);
    keptPrices.set(key, p);
  }
  return p;
}

function writePrices(money: Money, perUsd: string | undefined): Prices {
  if (money.code === USD)
    return {
      money,
      convert: (usd) => usd,
      price: formatPrice,
      unit: (usd) => formatUSD(usd),
      short: (usd) => (Number(usd) % 1 === 0 ? formatUSD(usd, { decimals: 0 }) : formatPrice(usd)),
    };
  const convert = (usd: string) =>
    perUsd && Number.isFinite(Number(usd)) ? (Number(usd) * Number(perUsd)).toFixed(9).replace(/0+$/, "").replace(/\.$/, "") : null;
  const written = (usd: string) => {
    const v = convert(usd);
    return v === null ? "—" : money.cost(v);
  };
  return { money, convert, price: written, unit: written, short: written };
}

export function currencySymbol(code: string): string {
  return new Intl.NumberFormat(formatLocale(), { style: "currency", currency: code, currencyDisplay: "narrowSymbol" })
    .formatToParts(0)
    .filter((p) => p.type === "currency")
    .map((p) => p.value)
    .join("");
}

export function currencyName(code: string): string {
  try {
    return new Intl.DisplayNames([formatLocale()], { type: "currency", fallback: "none" }).of(code) ?? code;
  } catch {
    return code;
  }
}
