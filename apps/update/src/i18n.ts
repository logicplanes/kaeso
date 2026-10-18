import { formatMessage, preferredUiLocale, type MessageValues, type UiLocale, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import de from "./locales/de";
import zhCN from "./locales/zh-CN";

export type Locale = UiLocale;

export const CATALOGS: Record<Locale, UiMessages> = { en: {}, de, "zh-CN": zhCN };

let current: Locale = "en";

export function setLocale(locale: Locale): void {
  current = locale;
}

export function locale(): Locale {
  return current;
}

export function localeOf(languages: readonly string[]): Locale {
  return preferredUiLocale(languages);
}

export function t(key: string, english: string, values?: MessageValues): string {
  return formatMessage(CATALOGS[current][key] ?? english, values, current);
}
