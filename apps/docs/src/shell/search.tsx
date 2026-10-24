import { useMemo } from "react";
import { useNavigate } from "@tanstack/react-router";
import { CommandPalette, useTheme, type CommandGroup, type CommandItem } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { SearchEntry } from "~/markdown/ast";
import { matchHeadings, splitPages } from "./search-rank";
import { setSearchOpen, setSearchQuery, useSearchOpen, useSearchQuery } from "./search-store";
import { useSwitchLocale } from "./use-switch-locale";

export default function Search({ index }: { index: SearchEntry[] }) {
  const open = useSearchOpen();
  const navigate = useNavigate();
  const { setTheme } = useTheme();
  const t = useText();
  const locale = useSiteLocale();
  const switchLocale = useSwitchLocale();
  const query = useSearchQuery();

  const groups = useMemo<CommandGroup[]>(() => {
    const run = (action: () => void) => () => {
      setSearchOpen(false);
      action();
    };
    const go = (to: string, hash?: string) => run(() => void navigate({ to, ...(hash ? { hash } : {}) }));
    const page = (e: SearchEntry): CommandItem => ({
      id: e.path,
      label: e.title,
      ...(e.section && e.section !== e.title ? { hint: e.section } : {}),
      keywords: [e.description, e.words],
      onSelect: go(e.path),
    });
    const { named, mentioned } = splitPages(index, query);
    const headings: CommandItem[] = matchHeadings(index, query).map((h) => ({
      id: `${h.entry.path}#${h.id}`,
      label: h.label,
      hint: h.entry.title,
      keywords: [],
      onSelect: go(h.entry.path, h.id),
    }));
    const themes: CommandItem[] = [
      { id: "theme:system", label: t("theme.system"), onSelect: run(() => setTheme("system")) },
      { id: "theme:light", label: t("theme.light"), onSelect: run(() => setTheme("light")) },
      { id: "theme:dark", label: t("theme.dark"), onSelect: run(() => setTheme("dark")) },
    ];
    const languages: CommandItem[] = LOCALES.filter((l) => l !== locale).map((l) => ({
      id: `language:${l}`,
      label: LANGUAGE_NAMES[l],
      keywords: [l],
      onSelect: run(() => switchLocale(l)),
    }));
    return [
      { label: t("search.pages"), items: named.map(page) },
      ...(headings.length ? [{ label: t("search.headings"), items: headings }] : []),
      ...(mentioned.length ? [{ label: t("search.mentioned"), items: mentioned.map(page) }] : []),
      { label: t("search.theme"), items: themes },
      { label: t("search.language"), items: languages },
    ];
  }, [index, query, navigate, setTheme, t, locale, switchLocale]);

  return (
    <CommandPalette
      open={open}
      onOpenChange={setSearchOpen}
      groups={groups}
      hotkey={false}
      query={query}
      onQueryChange={setSearchQuery}
      placeholder={t("search.placeholder")}
      emptyText={t("search.empty")}
    />
  );
}
