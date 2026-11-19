import { createFormatters, formatLocale, formatPrice, formatUSD } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

export const USD = "USD";

type Options = { significant?: number | undefined; decimals?: number | undefined };

/** Amounts in one currency, written the way the console has always written dollars: the same rounding, in the person's language. */
export interface Money {
  code: string;
  format(amount: string, options?: Options): string;
  /** A period's spend: in cents, and under a cent as such. */
  period(amount: string | null | undefined): string;
  /** A tile's total: in cents, and two significant digits below a cent. */
  tile(amount: string): string;
  /** One request's cost in a column: six places, so the column lines up. */
  request(amount: string): string;
  /** One request's cost on its own: three significant figures, cents at least and six places at most. */
  cost(amount: string): string;
  /** A price per million tokens as it was set: at least two decimals and up to six. */
  price(amount: string): string;
}

const english = createFormatters({ locale: "en" });
const numberFormats = new Map<string, Intl.NumberFormat>();

function numberFormat(code: string, decimals: number): Intl.NumberFormat {
  const key = `${formatLocale()}|${code}|${decimals}`;
  let f = numberFormats.get(key);
  if (!f) {
    f = new Intl.NumberFormat(formatLocale(), {
      style: "currency",
      currency: code,
      currencyDisplay: "narrowSymbol",
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
    numberFormats.set(key, f);
  }
  return f;
}

export function currencyDigits(code: string): number {
  return new Intl.NumberFormat("en", { style: "currency", currency: code }).resolvedOptions().maximumFractionDigits ?? 2;
}

// Rounds as the dollar formatter does, then writes the digits it kept with the currency's own symbol and
// separators; Intl takes the digits as a decimal string, so nothing passes through a float.
function inCurrency(code: string, amount: string, options: Options = {}): string {
  const written = english.formatUSD(amount, options);
  const m = /^([<>]\u00a0)?(−)?\$([\d,]+)(?:\.(\d+))?$/.exec(written);
  if (!m) return written;
  const [, bound = "", minus = "", whole = "0", fraction = ""] = m;
  const digits = `${minus ? "-" : ""}${whole.replace(/,/g, "")}${fraction ? `.${fraction}` : ""}` as Intl.StringNumericLiteral;
  return `${bound}${numberFormat(code, fraction.length).format(digits).replace("-", "−")}`;
}

const isZero = (amount: string) => !/[1-9]/.test(amount);

function costDecimals(amount: string): number {
  const [whole = "0", frac = ""] = amount.replace(/^-/, "").split(".");
  const lead = /^0*$/.test(whole) ? frac.length - frac.replace(/^0+/, "").length : 0;
  const places = Math.min(6, Math.max(2, /^0*$/.test(whole) ? lead + 3 : 2));
  const rounded = Math.abs(Number(amount)).toFixed(places);
  return isZero(rounded) ? places : Math.max(2, rounded.replace(/0+$/, "").split(".")[1]?.length ?? 0);
}

function money(code: string): Money {
  const format = (amount: string, options?: Options) => (code === USD ? formatUSD(amount, options) : inCurrency(code, amount, options));
  // A currency without cents (yen) counts its smallest unit where dollars count cents.
  const cents = code === USD ? 2 : Math.min(2, currencyDigits(code));
  const smallest = cents ? `0.${"1".padStart(cents, "0")}` : "1";
  return {
    code,
    format,
    period(amount) {
      if (!amount || isZero(amount)) return format("0", { decimals: cents });
      const n = Number(amount);
      if (n > 0 && n < Number(smallest) / 2) return t("kaeso.usage.lessThan", "<{amount}", { amount: format(smallest, { decimals: cents }) });
      return format(amount, { decimals: cents });
    },
    tile(v) {
      if (!(Number(v) > 0 && Number(v) < 0.01)) return cents === 2 ? format(v) : format(v, { decimals: cents });
      const rounded = Number(Number(v).toPrecision(2));
      const fixed = rounded.toFixed(1 - Math.floor(Math.log10(rounded))).replace(/0+$/, "");
      return format(fixed, { decimals: fixed.length - "0.".length });
    },
    request(amount) {
      return isZero(amount) ? format("0", { decimals: 0 }) : format(amount, { decimals: 6 });
    },
    cost(amount) {
      return isZero(amount) ? format("0", { decimals: 0 }) : format(amount, { decimals: costDecimals(amount) });
    },
    price(amount) {
      if (code === USD) return formatPrice(amount);
      const frac = /\.(\d*?)0*\s*$/.exec(amount)?.[1] ?? "";
      return format(amount, { decimals: Math.min(6, Math.max(2, frac.length)) });
    },
  };
}

const kept = new Map<string, Money>();

/** The formatters for amounts in `code`. */
export function moneyIn(code: string): Money {
  let m = kept.get(code);
  if (!m) {
    m = money(code);
    kept.set(code, m);
  }
  return m;
}

/** Adds decimal amounts in millionths, so sums never drift. */
export function sumAmounts(values: (string | null | undefined)[]): string {
  const micros = values.reduce((acc, v) => acc + (v ? Math.round(Number(v) * 1e6) : 0), 0);
  return (micros / 1e6).toFixed(6).replace(/0+$/, "").replace(/\.$/, ".00");
}
