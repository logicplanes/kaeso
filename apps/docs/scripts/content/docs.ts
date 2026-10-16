import { existsSync, readdirSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { DEFAULT_LOCALE, LANGUAGE_NAMES, LOCALES, localeOfPath, localePrefix, type Locale } from "../../src/i18n/locales.ts";
import { MESSAGES } from "../../src/i18n/messages/index.ts";
import { buildSite, type Site, type SitePage } from "./site.ts";

export interface Docs {
  contentDir: string;
  sites: Record<Locale, Site>;
  problems: string[];
}

const posix = (p: string) => p.split(sep).join("/");

function strayEntries(contentDir: string, repoRoot: string): string[] {
  return readdirSync(contentDir)
    .filter((name) => !name.startsWith(".") && !(LOCALES as readonly string[]).includes(name))
    .map((name) => `${posix(relative(repoRoot, join(contentDir, name)))}: put every page in a language folder (${LOCALES.join(", ")})`);
}

function translationProblems(sites: Record<Locale, Site>, repoRoot: string): string[] {
  const source = sites[DEFAULT_LOCALE];
  const english = new Map(source.pages.map((p) => [p.local, p]));
  const fileIn = (site: Site, local: string) => posix(relative(repoRoot, join(site.contentDir, local)));
  const problems: string[] = [];
  for (const locale of LOCALES.filter((l) => l !== DEFAULT_LOCALE)) {
    const site = sites[locale];
    const translated = new Map(site.pages.map((p) => [p.local, p]));
    for (const [local, page] of english) {
      const other = translated.get(local);
      if (!other) problems.push(`${fileIn(site, local)}: missing; translate ${page.rel}`);
      else if (other.meta.order !== page.meta.order)
        problems.push(`${other.rel}: its order is ${orderOf(other)}, but ${page.rel} has ${orderOf(page)}; a translation keeps the English order`);
    }
    for (const [local, page] of translated) {
      if (!english.has(local)) problems.push(`${page.rel}: has no English page; add ${fileIn(source, local)} or remove it`);
    }
  }
  return problems;
}

function reservedAddresses(site: Site): string[] {
  return site.pages.flatMap((page) => {
    const locale = localeOfPath(page.path);
    return locale === DEFAULT_LOCALE ? [] : [`${page.rel}: its address ${page.path} is where the ${LANGUAGE_NAMES[locale]} pages live; rename it`];
  });
}

const orderOf = (page: SitePage) => (page.meta.order === undefined ? "not set" : String(page.meta.order));

export function buildDocs(contentDir: string, repoRoot: string): Docs {
  const rel = posix(relative(repoRoot, contentDir));
  if (!existsSync(contentDir)) {
    const empty = buildSite(join(contentDir, DEFAULT_LOCALE), repoRoot);
    const sites = Object.fromEntries(LOCALES.map((l) => [l, empty])) as Record<Locale, Site>;
    return { contentDir, sites, problems: [`${rel} does not exist`] };
  }
  const sites = Object.fromEntries(
    LOCALES.map((locale) => {
      const messages = MESSAGES[locale];
      const options = { prefix: localePrefix(locale), labels: { home: messages["nav.introduction"], overview: messages["nav.overview"] } };
      return [locale, buildSite(join(contentDir, locale), repoRoot, options)];
    }),
  ) as Record<Locale, Site>;
  const problems = [
    ...strayEntries(contentDir, repoRoot),
    ...LOCALES.flatMap((l) => sites[l].problems),
    ...reservedAddresses(sites[DEFAULT_LOCALE]),
    ...translationProblems(sites, repoRoot),
  ];
  return { contentDir, sites, problems };
}
