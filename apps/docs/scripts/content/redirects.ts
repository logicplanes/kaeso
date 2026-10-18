import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { DEFAULT_LOCALE, LOCALES, localePrefix, type Locale } from "../../src/i18n/locales.ts";
import type { Site } from "./site.ts";

export interface Redirect {
  from: string;
  to: string;
  title: string;
}

interface Redirects {
  byLocale: Record<Locale, Redirect[]>;
  problems: string[];
}

const REDIRECTS_FILE = "redirects.json";

export const redirectsFileOf = (contentDir: string) => join(dirname(resolve(contentDir)), REDIRECTS_FILE);

export const noRedirects = (): Record<Locale, Redirect[]> => ({ en: [], de: [], "zh-CN": [] });

const ADDRESS = /^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/;

function readEntries(file: string, shown: string): { entries: [string, string][]; problems: string[] } {
  if (!existsSync(file)) return { entries: [], problems: [] };
  let data: unknown;
  try {
    data = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return { entries: [], problems: [`${shown}: is not JSON`] };
  }
  if (!data || typeof data !== "object" || Array.isArray(data))
    return { entries: [], problems: [`${shown}: must be an object of old addresses and the pages they moved to`] };
  const entries: [string, string][] = [];
  const problems: string[] = [];
  for (const [from, to] of Object.entries(data)) {
    if (typeof to !== "string") problems.push(`${shown}: ${from} must point to a page as a string, such as "guides/keys.md"`);
    else entries.push([from, to]);
  }
  return { entries, problems };
}

export function buildRedirects(file: string, shown: string, sites: Record<Locale, Site>): Redirects {
  const { entries, problems } = readEntries(file, shown);
  const byLocale = noRedirects();
  for (const [from, target] of entries) {
    if (!ADDRESS.test(from)) {
      problems.push(`${shown}: "${from}" is not an address within a language, such as guides/old-keys`);
      continue;
    }
    const [local = "", anchor] = target.split("#");
    const english = sites[DEFAULT_LOCALE].pages.find((p) => p.local === local);
    if (!english) {
      problems.push(`${shown}: ${from} points to ${target}, which is not an English page`);
      continue;
    }
    const index = anchor === undefined ? -1 : english.lexed.headings.findIndex((h) => h.id === anchor);
    if (anchor !== undefined && index < 0) {
      problems.push(`${shown}: ${from} points to ${target}, but ${english.rel} has no heading with that anchor`);
      continue;
    }
    for (const locale of LOCALES) {
      const site = sites[locale];
      const path = `${localePrefix(locale)}/${from}`;
      const clash = site.pages.find((p) => p.path === path);
      if (clash) {
        problems.push(`${shown}: ${from} is the address of ${clash.rel}; remove the page or the redirect`);
        continue;
      }
      const page = site.pages.find((p) => p.local === local);
      if (!page) continue;
      const id = index < 0 ? undefined : page.lexed.headings[index]?.id;
      byLocale[locale].push({ from: path, to: `${page.path}${id ? `#${id}` : ""}`, title: page.meta.title });
    }
  }
  return { byLocale, problems };
}
