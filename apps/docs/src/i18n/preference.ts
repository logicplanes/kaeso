import { DEFAULT_LOCALE, LOCALE_STORAGE_KEY, browserLocale, isLocale, localeOfPath, localizedPath, type Locale } from "./locales";

export function storedLocale(): Locale | undefined {
  try {
    const value = localStorage.getItem(LOCALE_STORAGE_KEY);
    return value !== null && isLocale(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

export function rememberLocale(locale: Locale): void {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    return;
  }
}

export function preferredLocale(languages: readonly string[]): Locale {
  return storedLocale() ?? browserLocale(languages);
}

export function preferredPath(pathname: string, preferred: Locale): string | null {
  if (localeOfPath(pathname) !== DEFAULT_LOCALE || preferred === DEFAULT_LOCALE) return null;
  return localizedPath(pathname.replace(/\/+$/, "") || "/", preferred);
}

export function followPreferredLocale(win: Window): void {
  const target = preferredPath(win.location.pathname, preferredLocale(win.navigator.languages));
  if (target) win.history.replaceState(win.history.state, "", target);
}
