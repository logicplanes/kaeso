import { formatLocale } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

const formatters = new Map<string, Intl.NumberFormat>();

function formatter(code: string, maximumFractionDigits: number): Intl.NumberFormat {
  const key = `${formatLocale()}|${code}|${maximumFractionDigits}`;
  let f = formatters.get(key);
  if (!f) {
    f = new Intl.NumberFormat(formatLocale(), { style: "currency", currency: code, maximumFractionDigits });
    formatters.set(key, f);
  }
  return f;
}

export function currencyDigits(code: string): number {
  return new Intl.NumberFormat("en", { style: "currency", currency: code })
    .formatToParts(1)
    .filter((p) => p.type === "fraction")
    .reduce((n, p) => n + p.value.length, 0);
}

// Amounts under one unit keep up to four decimals, so a request's cost doesn't read as 0.
export function formatMoney(amount: string | number, code: string): string {
  const n = Number(amount);
  const digits = currencyDigits(code);
  const small = n !== 0 && Math.abs(n) < 1;
  return formatter(code, small ? Math.max(digits, 4) : digits).format(n);
}

export function currencySymbol(code: string): string {
  return new Intl.NumberFormat(formatLocale(), { style: "currency", currency: code, currencyDisplay: "narrowSymbol" })
    .formatToParts(0)
    .filter((p) => p.type === "currency")
    .map((p) => p.value)
    .join("");
}

export function approxMoney(amount: string | number, code: string): string {
  return t("kaeso.currency.approx", "≈ {amount}", { amount: formatMoney(amount, code) });
}

export function currencyName(code: string): string {
  try {
    return new Intl.DisplayNames([formatLocale()], { type: "currency", fallback: "none" }).of(code) ?? code;
  } catch {
    return code;
  }
}
