import { matchUiLocale, type UiLocale } from "@logicplanes/kaeso-ui/i18n";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import de from "./locales/de.json";
import zhCN from "./locales/zh-CN.json";

export type Catalog = Record<string, string>;
const CATALOGS: Partial<Record<UiLocale, Catalog>> = { de, "zh-CN": zhCN };

let current: UiLocale = "en";

export function localeOf(tag: string | undefined): UiLocale {
  return (tag && matchUiLocale(tag)) || "en";
}

export function setLocale(tag: string | undefined): UiLocale {
  current = localeOf(tag);
  setFormatLocale(current);
  return current;
}

export function t(key: string, english: string, values: Record<string, string | number> = {}): string {
  const template = CATALOGS[current]?.[key] ?? english;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in values ? String(values[name]) : whole));
}
