export const LOCALES = ["en", "de", "zh-CN"] as const;

export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en";

export const LANGUAGE_NAMES: Readonly<Record<Locale, string>> = {
  en: "English",
  de: "Deutsch",
  "zh-CN": "简体中文",
};

export const LOCALE_STORAGE_KEY = "kaeso-docs:locale";

export function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

export function localePrefix(locale: Locale): string {
  return locale === DEFAULT_LOCALE ? "" : `/${locale}`;
}

export function homePath(locale: Locale): string {
  return localePrefix(locale) || "/";
}

export function localeOfPath(pathname: string): Locale {
  const first = pathname.split("/")[1] ?? "";
  return LOCALES.find((locale) => locale !== DEFAULT_LOCALE && locale.toLowerCase() === first.toLowerCase()) ?? DEFAULT_LOCALE;
}

export function pathWithinLocale(pathname: string): string {
  const locale = localeOfPath(pathname);
  const rest = locale === DEFAULT_LOCALE ? pathname : pathname.slice(localePrefix(locale).length);
  const trimmed = rest.replace(/\/+$/, "");
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

export function localizedPath(path: string, locale: Locale): string {
  const within = path === "/" ? "" : path;
  return `${localePrefix(locale)}${within}` || "/";
}

export function switchLocale(pathname: string, locale: Locale): string {
  return localizedPath(pathWithinLocale(pathname), locale);
}
