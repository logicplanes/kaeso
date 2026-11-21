import { useEffect, useMemo, useState } from "react";
import { navigate } from "astro:transitions/client";
import { CommandPalette, icons, type CommandGroup, type IconName } from "@logicplanes/kaeso-ui";
import type { NavSection } from "~/lib/nav";
import searchJson from "~/generated/search.json";
import { matchReason, searchKeywords, type MatchLabels, type SearchIndex } from "~/lib/search";
import { docsText, useDocsLang } from "~/lib/locale";

const index: SearchIndex = searchJson;

export function DocsSearch({ sections, open, onOpenChange }: { sections: NavSection[]; open: boolean; onOpenChange: (o: boolean) => void }) {
  const { lang } = useDocsLang();
  const [query, setQuery] = useState("");
  useEffect(() => {
    if (!open) setQuery("");
  }, [open]);
  const t = useMemo(() => (english: string) => docsText(lang, english), [lang]);
  const labels = useMemo<MatchLabels>(
    () => ({ part: t("Part"), prop: t("Prop"), description: t("Description"), example: t("Example"), colon: lang === "zh-CN" ? "：" : ": " }),
    [t, lang],
  );
  const keywords = useMemo(() => new Map(Object.entries(index).map(([slug, entry]) => [slug, searchKeywords(entry, t)])), [t]);
  const groups = useMemo<CommandGroup[]>(() => {
    const go = (href: string) => () => {
      onOpenChange(false);
      void navigate(href);
    };
    return sections.map((s) => ({
      label: docsText(lang, s.title),
      items: s.items.map((it) => {
        const slug = it.href.split("/").pop() ?? "";
        const overview = s.title === "Overview";
        const label = overview ? docsText(lang, it.title) : it.title;
        const entry = overview ? undefined : index[slug];
        const reason = entry && query.trim() ? matchReason(label, entry, query, t, labels) : null;
        return {
          id: it.href,
          label,
          icon: it.icon ? icons[it.icon as IconName] : icons.code,
          hint: reason ?? (overview ? undefined : docsText(lang, s.title)),
          keywords: keywords.get(slug) ?? [],
          onSelect: go(it.href),
        };
      }) as CommandGroup["items"],
    }));
  }, [sections, onOpenChange, lang, query, t, labels, keywords]);
  return (
    <CommandPalette
      open={open}
      onOpenChange={onOpenChange}
      groups={groups}
      query={query}
      onQueryChange={setQuery}
      placeholder={docsText(lang, "Search pages, components, props and examples")}
      emptyText={docsText(lang, "Nothing matches. Try a component or a prop name.")}
    />
  );
}
