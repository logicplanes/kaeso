import { useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { CommandPalette, useTheme, type CommandGroup, type CommandItem } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { SearchEntry } from "~/markdown/ast";
import { matchHeadings, rankPages } from "./search-rank";
import { setSearchOpen, takeTypedQuery, useSearchOpen } from "./search-store";
import { useSwitchLocale } from "./use-switch-locale";

export default function Search({ index }: { index: SearchEntry[] }) {
  const open = useSearchOpen();
  const navigate = useNavigate();
  const { setTheme } = useTheme();
  const t = useText();
  const locale = useSiteLocale();
  const switchLocale = useSwitchLocale();
  const [first, setFirst] = useState(takeTypedQuery);
  const [query, setQuery] = useState(first);
  const [openedWith, setOpenedWith] = useState(open);
  if (openedWith !== open) {
    setOpenedWith(open);
    if (open) {
      setFirst("");
      setQuery("");
    }
  }

  const groups = useMemo<CommandGroup[]>(() => {
    const run = (action: () => void) => () => {
      setSearchOpen(false);
      action();
    };
    const go = (to: string, hash?: string) => run(() => void navigate({ to, ...(hash ? { hash } : {}) }));
    const pages: CommandItem[] = rankPages(index, query).map((e) => ({
      id: e.path,
      label: e.title,
      ...(e.section && e.section !== e.title ? { hint: e.section } : {}),
      keywords: [e.description, e.words],
      onSelect: go(e.path),
    }));
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
      { label: t("search.pages"), items: pages },
      ...(headings.length ? [{ label: t("search.headings"), items: headings }] : []),
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
      onQueryChange={setQuery}
      defaultQuery={first}
      placeholder={t("search.placeholder")}
      emptyText={t("search.empty")}
    />
  );
}
