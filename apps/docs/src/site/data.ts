import { loadUiMessages, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import type { Locale } from "~/i18n/locales";
import type { NavGroup, PageContent, PageMeta, SearchEntry } from "~/markdown/ast";

export interface LocaleSite {
  editBase: string;
  nav: NavGroup[];
  pages: Record<string, PageMeta>;
  load: Record<string, () => Promise<{ default: PageContent }>>;
}

export interface LocaleData {
  locale: Locale;
  site: LocaleSite;
  ui: UiMessages;
}

const SITES: Record<Locale, () => Promise<LocaleSite>> = {
  en: () => import("virtual:kaeso-docs/site/en"),
  de: () => import("virtual:kaeso-docs/site/de"),
  "zh-CN": () => import("virtual:kaeso-docs/site/zh-CN"),
};

const SEARCH: Record<Locale, () => Promise<{ default: SearchEntry[] }>> = {
  en: () => import("virtual:kaeso-docs/search/en"),
  de: () => import("virtual:kaeso-docs/search/de"),
  "zh-CN": () => import("virtual:kaeso-docs/search/zh-CN"),
};

function cached<T>(load: (locale: Locale) => Promise<T>): (locale: Locale) => Promise<T> {
  const cache = new Map<Locale, Promise<T>>();
  return (locale) => {
    const known = cache.get(locale);
    if (known) return known;
    const loading = load(locale);
    cache.set(locale, loading);
    loading.catch(() => cache.delete(locale));
    return loading;
  };
}

export const loadLocaleData = cached(async (locale): Promise<LocaleData> => {
  const [site, ui] = await Promise.all([SITES[locale](), loadUiMessages(locale)]);
  return { locale, site, ui };
});

export const loadSearchIndex = cached(async (locale) => (await SEARCH[locale]()).default);
